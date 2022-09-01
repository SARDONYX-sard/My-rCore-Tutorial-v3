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

    .section .text
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
    # sscratch: Value of sp before entering Trap, pointing to the user stack.
    #
    # ```
    # csrrw: rd, csr, rs1
    #   rd = csr
    #   csr = rs1
    # ```
    # Therefore, this is swap sp sscratch
    # TrapContext: 34 * 8byte
    csrrw sp, sscratch, sp
    # now sp->kernel stack, sscratch->user stack
    # allocate a TrapContext on kernel stack
    addi sp, sp, -34*8
    # save general-purpose registers
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
    # we can use t0/t1/t2 freely, because they were saved on kernel stack
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
    # read user stack from sscratch and save it on the kernel stack
    csrr t2, sscratch
    sd t2, 2*8(sp)
    # set input argument of trap_handler(cx: &mut TrapContext).
    # - a0 is x10
    mv a0, sp # now sp->kernel stack
    call trap_handler # This function is defined in os/trap/mod.rs


.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm

__restore:
    # case1: start running app by __restore
    # case2: back to U after handling trap
    mv sp, a0 # To execute App error

    # To properly recover the three temporary registers used,
    # the CSRs must be recovered first, followed by the general-purpose registers.

    # now sp->kernel stack(after allocated), sscratch->user stack
    # restore sstatus/sepc
    #
    # ```
    # load: rd,offset(rs1)
    #   rd = Memory[rs1 + offset]
    # ```
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    ld t2, 2*8(sp) # t2 = head of `user stack`
    csrw sstatus, t0
    csrw sepc, t1
    csrw sscratch, t2 # sscratch = head of `user stack`
    # restore general-purpuse registers except sp/tp
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    # release TrapContext on kernel stack
    addi sp, sp, 34*8
    # ! Swap user-stack & kenel-stack
    # sp->user stack, sscratch->kernel stack
    csrrw sp, sscratch, sp
    sret
