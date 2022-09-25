# Qemu memo

## Memory mapped I/O(MMIO)

Memory-mapped I/O (MMIO) means that peripheral device registers are accessible via specific physical
memory addresses, distributed over one or more physical address intervals where the device registers
of each peripheral device do not intersect, the physical address spaces occupied by the device
registers of different peripherals do not intersect and that these peripheral physical address
intervals do not intersect with the RAM's physical memory.
The physical address spacing of these peripherals does not intersect the physical memory spacing of RAM.

The Qemu for RISC-V 64 platform source code shows that the MMIO physical address range for the VirtIO
peripheral bus is 4KiB, starting at 0x10001000.
In order for the kernel to access the VirtIO peripheral bus, a specific memory area must be mapped to
the kernel address space in advance.
