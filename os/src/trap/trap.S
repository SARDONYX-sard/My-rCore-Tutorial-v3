.altmacro # To use `.rept`

# The instructions that read and write CSRs in RISC-V are a class of instructions
# that can perform multiple reads and writes without interruption.
# Such an instruction that completes multiple operations
# without interruption is called an atomic instruction.

.macro SAVE_GP n
    # ```
    # sd: rs2, offset(rs1)
    #   Memory[rs1 + offset] = rs2
    # ```
    sd x\n, \n*8(sp)
.endm

    .section .text.trampoline
    .globl __alltraps
    .globl __restore
    # align directive: align <number>
    #
    # #Examples
    #  .align 1 # 0x0 byte
    #  .align 2 # 0x4 byte
    #  .align 3 # 0x8 byte
    .align 2
__alltraps:
    # - Now
    #   - pointer:
    #     -      sp -> user-stack
    #     - sscratch -> *TrapContext in the application address space(actually the next higher page).
    #   - Permission: S Privilege Level
    #
    # sscratch: Value of sp before entering Trap, pointing to the user stack.
    #
    # ```
    # csrrw: rd, csr, rs1
    #   rd = csr
    #   csr = rs1
    # ```
    # Therefore, this is swap sp sscratch
    # TrapContext: 37 * 8byte
    csrrw sp, sscratch, sp
    # now sp->*TrapContext in user space, sscratch->user stack
    # save other general purpose registers
    sd x1, 1*8(sp)
    # skip sp(x2), we will save it later
    sd x3, 3*8(sp)
    # skip tp(x4), application does not use it
    # save x5~x31
    .set n, 5
    .rept 27
        # store range(start: sp+8n, end: sp+8(n+1))
        SAVE_GP %n
        .set n, n+1
    .endr
    # we can use t0/t1/t2 freely, because they have been saved in TrapContext
    # - t0 is x5
    # - t1 is x6
    # - t2 is x7
    #
    # ```
    # csrr(pseudoinstruction CSRRS rd, csr, x0): r1, csr
    #   r1 = csr
    # ```
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)
    # read user stack from sscratch and save it in TrapContext
    csrr t2, sscratch
    sd t2, 2*8(sp)
    # load kernel_satp(token) into t0
    ld t0, 34*8(sp)
    # load trap_handler into t1
    ld t1, 36*8(sp)
    # move to kernel_sp
    ld sp, 35*8(sp)
    # switch to kernel space
    csrw satp, t0
    # Root page table has been chaged. We need to clear TLB.
    # So we use the sfence.vma instruction to clear the cache.
    sfence.vma
    # jump to trap_handler
    jr t1


.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm

__restore:
    # a0: *TrapContext in user space(Constant); a1: user space token
    # switch to user space
    csrw satp, a1
    sfence.vma
    csrw sscratch, a0
    mv sp, a0
    # now sp points to TrapContext in user space, start restoring based on it
    # restore sstatus/sepc
    #
    # ```
    # load: rd,offset(rs1)
    #   rd = Memory[rs1 + offset]
    # ```
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    # restore general-purpuse registers except sp/tp
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    # back to user stack
    ld sp, 2*8(sp)
    sret
