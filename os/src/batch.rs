//! batch subsystem

use crate::println;
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use core::arch::asm;
use lazy_static::*;

const USER_STACK_SIZE: usize = 4096 * 2; // 8KiB
const KERNEL_STACK_SIZE: usize = 4096 * 2; // 8KiB
const MAX_APP_NUM: usize = 16;
const APP_BASE_ADDRESS: usize = 0x80400000;
const APP_SIZE_LIMIT: usize = 0x20000;

#[repr(align(4096))]
struct KernelStack {
    data: [u8; KERNEL_STACK_SIZE],
}
#[repr(align(4096))]
struct UserStack {
    data: [u8; USER_STACK_SIZE],
}

impl KernelStack {
    /// get stack base pointer
    fn get_sp(&self) -> usize {
        // Returns the end address of the wrapped array
        // because the stack grows downward in RISC-V
        //
        // We need base stack pointer.
        // So We add sp + STACK_SIZE
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }

    ///
    /// # Returns
    /// Top of the kernel stack after the Trap context is pushed to it.
    pub fn push_context(&self, cx: TrapContext) -> &'static mut TrapContext {
        let cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
        unsafe {
            *cx_ptr = cx;
        }
        unsafe { cx_ptr.as_mut().unwrap() }
    }
}

impl UserStack {
    /// get stack base pointer
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}

/// .bss segment
static KERNEL_STACK: KernelStack = KernelStack {
    data: [0; KERNEL_STACK_SIZE],
};

/// .bss segment
static USER_STACK: UserStack = UserStack {
    data: [0; USER_STACK_SIZE],
};

struct AppManager {
    num_app: usize,
    current_app: usize,
    app_start: [usize; MAX_APP_NUM + 1],
}

impl AppManager {
    pub fn print_app_info(&self) {
        println!("[kernel] num_app = {}", self.num_app);
        for i in 0..self.num_app {
            println!(
                "[kernel] app_{} [{:#x}, {:#x})",
                i,
                self.app_start[i],
                self.app_start[i + 1]
            );
        }
    }

    unsafe fn load_app(&self, app_id: usize) {
        if app_id >= self.num_app {
            println!("All applications completed!");

            #[cfg(feature = "board_qemu")]
            use crate::board::QEMUExit;
            #[cfg(feature = "board_qemu")]
            crate::board::QEMU_EXIT_HANDLE.exit_success();
        }
        println!("[kernel] Loading app_{}", app_id);

        //  -------------------------------------------------------------
        // |  -------------------                   -------------------  |
        // | |        CPU        |                 |        CPU        | |
        // | |  ---------------  |  ------------   |  ---------------  | |
        // | | | Control Logic | | | Floating-  |  | | Control Logic | | |
        // | |  ---------------  | | Point Unit |  |  ---------------  | |
        // | |  ---------------  |  ------------   | ----------------  | |
        // | | |   Registers   | |                 | |   Registers   | | |
        // | |  ---------------  |                 | ----------------  | |
        // | |  ---------------  |                 |  ---------------  | |
        // | | |      ALU      | |                 | |      ALU      | | |
        // | |  ---------------  |                 |  ---------------  | |
        // |  -------------------                   -------------------  |
        // |  -------------------   ------------    -------------------  |
        // | | Level-1 | Level-1 | |   Memory   |  | Level-1 | Level-1 | |
        // | |  inst.  |  Data   | | Management |  |  inst.  |  Data   | |
        // | |  cache  |  Cache  | |    Unit    |  |  cache  |  Cache  | |
        // |  -------------------   -----------     -------------------  |
        // |  -------------------   -----------    -------------------   |
        // | |    Level-2 Cache  | |    TLB    |  |    Level-2 Cache  |  |
        // |  -------------------   -----------    -------------------   |
        // |  ---------------------------------------------------------  |
        // | |                     Level-3 Cache                      |  |
        // |  --------------------------------------------------------   |
        // |  --------------------------------------------------------   |
        // | |                     Bus Interface                      |  |
        // |  --------------------------------------------------------   |
        //  -------------------------------------------------------------
        //
        //   ALU: Arithmetic-Logic Unit
        //   TLB: Translation Look aside Buffer
        // inst.: instruction
        //
        // The cache of the CPU's physical memory is divided into a data cache (d-cache)
        // and an instruction cache (i-cache),
        // which are used by the CPU to access memory and used when fetching a instruction.
        //
        // When fetching, for a given instruction address,
        // the CPU first accesses the i-cache to see if it is in one of the cached cache lines, and if so,
        // it retrieves the instruction directly from the cache rather than accessing memory via the bus.
        //
        // Normally, the CPU assumes that the program code segment is unchanged,
        // so the i-cache is a read-only cache.
        //
        // Since the OS changes the memory area that the CPU fetches,
        // the i-cache will contain contents that are inconsistent with what is in memory.
        //
        // Therefore, the OS must manually empty the i-cache using the fence.i instruction
        // and invalidate all of its contents so that the CPU can correctly access memory data and code.

        // clear icache(instruction cache)
        asm!("fence.i");
        // clear app area
        core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, APP_SIZE_LIMIT).fill(0);
        let app_src = core::slice::from_raw_parts(
            self.app_start[app_id] as *const u8,
            self.app_start[app_id + 1] - self.app_start[app_id],
        );
        let app_dst = core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, app_src.len());
        app_dst.copy_from_slice(app_src);
    }

    pub fn get_current_app(&self) -> usize {
        self.current_app
    }

    /// increment self.current_app field.
    pub fn move_to_next_app(&mut self) {
        self.current_app += 1;
    }
}

lazy_static! {
    static ref APP_MANAGER: UPSafeCell<AppManager> = unsafe {
        UPSafeCell::new({
            extern "C" {
                fn _num_app();
            }
            let num_app_ptr = _num_app as usize as *const usize;
            let num_app = num_app_ptr.read_volatile();
            let mut app_start: [usize; MAX_APP_NUM + 1] = [0; MAX_APP_NUM + 1];
            let app_start_raw: &[usize] =
                core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1);
            app_start[..=num_app].copy_from_slice(app_start_raw);
            AppManager {
                num_app,
                current_app: 0,
                app_start,
            }
        })
    };
}

/// init batch subsystem
pub fn init() {
    print_app_info();
}

/// print apps info
pub fn print_app_info() {
    APP_MANAGER.exclusive_access().print_app_info();
}

/// run next app
pub fn run_next_app() -> ! {
    let mut app_manager = APP_MANAGER.exclusive_access();
    let current_app = app_manager.get_current_app();
    unsafe {
        app_manager.load_app(current_app);
    }
    app_manager.move_to_next_app();
    drop(app_manager);
    // before this we have to drop local variables related to resources manually
    // and release the resources
    extern "C" {
        /// Restore the following to transition from Supervisor mode to User mode
        /// - General-purpose registers
        /// - Control and status registers
        /// - Stack pointer = user stack
        /// - sscratch = kernel stack
        ///
        /// This function is defined in trap::trap.S
        ///
        /// # Parameters
        /// - `cx_addr`: The top of kernel stack
        fn __restore(cx_addr: usize);
    }
    unsafe {
        __restore(KERNEL_STACK.push_context(TrapContext::app_init_context(
            APP_BASE_ADDRESS,
            USER_STACK.get_sp(),
        )) as *const _ as usize);
    }
    panic!("Unreachable in batch::run_current_app!");
}
