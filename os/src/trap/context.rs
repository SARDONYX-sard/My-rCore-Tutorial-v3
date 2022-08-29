use riscv::register::sstatus::{self, set_spp, Sstatus, SPP};

/// Physical resources that need to be preserved when traps occur
///
/// - In the case of `scause`/`stval`,
///
/// which is always used or stored elsewhere when Trap is first processed,
/// there is no risk of being modified and causing undesired effects.
///
/// - In the case of `sstatus`/`sepc`,
///
/// which are in danger of being modified and causing adverse effects,
/// they have a meaning throughout Trap processing
/// (and if `sret` is used, it is also used at the end of Trap's control flow),
/// and there are indeed cases where their value can be overridden by Trap nesting.
///
///  Therefore, they must also be saved and restored before `sret`.
#[repr(C)]
pub struct TrapContext {
    /// - x0 ~ x31: General-purpose registers
    pub x: [usize; 32],
    /// - sstatus: [Supervisor Status Register](https://five-embeddev.com/riscv-isa-manual/latest/supervisor.html#sstatus)
    pub sstatus: Sstatus,
    /// - sepc: [Supervisor Exception Program Counter](https://five-embeddev.com/riscv-isa-manual/latest/supervisor.html#supervisor-exception-program-counter-sepc)
    pub sepc: usize,
}

impl TrapContext {
    /// set stack pointer to x_2 reg (sp)
    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }

    /// init app context
    pub fn app_init_context(entry: usize, sp: usize) -> Self {
        unsafe {
            set_spp(SPP::User); //previous privilege mode: user mode
        }
        let sstatus = sstatus::read();
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
        };
        cx.set_sp(sp);
        cx
    }
}
