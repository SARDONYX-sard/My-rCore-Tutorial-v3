/* This is virtual address */
/* Permission:
      u: can access user
      r: readable
      w: writable
      x: executable
*/
OUTPUT_ARCH(riscv)
ENTRY(_start)

BASE_ADDRESS = 0x10000;

SECTIONS
{
    . = BASE_ADDRESS;
    /* Permission: ur-x */
    .text : {
        *(.text.entry)
        *(.text .text.*)
    }
    /* The pages are aligned because the previous and next logical segments are accessed in different ways,
      and since this restriction can only be set on a page-by-page basis,
      the only way to do this is to align the next logical segment with the start of the next page. */
    . = ALIGN(4K);
    /* Permission: ur-- */
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }
    . = ALIGN(4K);
    /* Permission: urw- */
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    /* Alignment between pages is unnecessary because .data and .bss logical segments
      have the same access restrictions (read and write). */
    /* Permission: urw- */
    .bss : {
        start_bss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        end_bss = .;
    }
    /DISCARD/ : {
        *(.eh_frame)
        *(.debug*)
    }
}
