.altmacro

# The Rust/C compiler automatically generates code at the beginning of
# the function to preserve the caller's registers s0~s11.
#
# However, since __switch is a special function written in assembly code that is not processed by the Rust/C compiler,
# it is necessary to manually write assembly code in __switch to preserve s0~s11.
#
# Other registers belonging to the caller do not need to be saved
# because they are saved by automatically generated compiler code in the call function written in high-level language,
# and some registers are temporary registers that do not need to be saved and restored.

.macro SAVE_SN n
    # ```
    # sd: rs2, offset(rs1)
    #   Memory[rs1 + offset] = rs2
    # ```
    sd s\n, (\n+2)*8(a0)
.endm
.macro LOAD_SN n
    ld s\n, (\n+2)*8(a1)
.endm
    .section .text
    .globl __switch
__switch:
    # stage [1]
    # __switch(
    #     current_task_cx_ptr: *mut TaskContext,
    #     next_task_cx_ptr: *const TaskContext
    # )
    # stage [2]
    # save kernel stack of current task
    sd sp, 8(a0)
    # save ra & s0~s11 of current execution
    sd ra, 0(a0)
    .set n, 0
    .rept 12
        SAVE_SN %n
        .set n, n + 1
    .endr
    # stage [3]
    # restore ra & s0~s11 of next execution
    ld ra, 0(a1)
    .set n, 0
    .rept 12
        LOAD_SN %n
        .set n, n + 1
    .endr
    # restore kernel stack of next task
    ld sp, 8(a1)
    # stage [4]
    ret
