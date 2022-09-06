//! Loading user applications into memory

/// Get the total number of applications.
pub fn get_num_app() -> usize {
    extern "C" {
        /// The following array data
        /// - Top of array: total number of applications
        /// - Remaining: Address array of data segments for each application
        ///
        /// # Remarks
        ///
        /// - This function is defined `link_app.S`
        /// - `os/build.rs` create `link_app.S`
        ///
        /// # Sample of generated code
        /// - `.quad` define 64 bit numeric
        ///
        /// ```assembly
        /// _num_app:
        ///    .quad 5
        ///    .quad app_0_start
        ///    .quad app_1_start
        ///    .quad app_2_start
        ///    .quad app_3_start
        ///    .quad app_4_start
        ///    .quad app_4_end

        ///    .section .data
        ///    .global app_0_start
        ///    .global app_0_end
        ///    .global app_0_end
        /// ```
        fn _num_app();
    }
    unsafe { (_num_app as usize as *const usize).read_volatile() }
}

/// Based on the application number passed as an argument,
/// obtain the ELF format executable data of the corresponding application.
pub fn get_app_data(app_id: usize) -> &'static [u8] {
    extern "C" {
        /// Get the number of applications linked to the kernel.
        fn _num_app();
    }
    let num_app_ptr = _num_app as usize as *const usize;
    let num_app = get_num_app();
    let app_start = unsafe { core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1) };
    assert!(app_id < num_app);
    unsafe {
        core::slice::from_raw_parts(
            app_start[app_id] as *const u8,
            app_start[app_id + 1] - app_start[app_id],
        )
    }
}
