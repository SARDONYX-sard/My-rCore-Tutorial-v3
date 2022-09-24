# Easy to forget abbreviations for RISC-V

## Abbreviation

- hart: hardware thread
- sext: [Sign extension](https://en.wikipedia.org/wiki/Sign_extension)

See more: [RV32I Base Integer Instruction Set, Version 2.1](https://five-embeddev.com/riscv-isa-manual/latest/rv32.html#integer-computational-instructions)

- rd: destination register
- rs: source register
- imm: immediate value

## Instruction

- auipc: Add Upper Immediate to PC

	U-type, RV32I and RV64I.

	Adds a 20-bit sign-extension (12-bit left shift) immediate to pc and writes the result to x[rd].
	sext is actually an abbreviation for sign-extension,
	which means to extend the immediate number to 32 bits,
	sign-extension if it is a signed number,
	or unsigned-extension if it is an unsigned number
	e.g.

	```as
	# x[rd] = pc + sext(immediate[31:12] << 12)
	auipc rd, immediate
	```

- sd/ld: <https://msyksphinz-self.github.io/riscv-isadoc/html/rv64i.html#sd>

## sfence.vma is a Barrier

For a RISC-V CPU implementation with a fast table, we can think of sfence.vma as clearing the fast table.
In fact it is defined in the privileged level specification as a much richer memory barrier,
specifically: sfence.vma enables all address translations that occur after it to see all write operations that precede it.
The specific transactions to be done by this instruction vary on different hardware configurations.
This instruction can also be finely configured to reduce synchronization overhead, as described in the RISC-V privileged level specification.
