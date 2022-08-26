# .text.entry is just a symbol for a segment,
# and is just a name for placement in linker.ld
.section .text.entry
.globl _start
_start:
  la sp, boot_stack_top # load address dist, src_symbol: sp <- boot_stack_top
  call rust_main        # This function is defined in main.rs

  .section .bss.stack
  .globl boot_stack     # become global symbol. base stack
boot_stack:
  .space 4096 * 16      # 64KiB
  .globl boot_stack_top # become global symbol to label
boot_stack_top:
