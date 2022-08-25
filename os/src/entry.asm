# .text.entry is just a symbol for a segment,
# and is just a name for placement in linker.ld
.section .text.entry
.globl _start
_start:
  li x1, 100
