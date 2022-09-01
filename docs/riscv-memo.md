# Easy to forget abbreviations for RISC-V

- hart: hardware thread
- sext: signed extension
- rd: destination register
- rs: source register
- imm: immediate value

## instruction

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

## What does mean `sign expansion`?

Sign expansion is the process of expanding data by filling in bits to keep the value the same when converting signed data to data with a larger bit length.
For sign extension, the bits are filled with the same value as the sign bit so that they are the same size.
In the specific example of handling an 8-bit byte value as a 16-bit word value, sign extension of 8 bits of -2 (111111111110 in 2's complement representation) results in 16 bits of -2 (1111111111111110). Note that since there are cases where it is not convenient to perform sign expansion, processors often have instructions to perform sign expansion and instructions to simply fill in the value with zeros without performing it.

See more: [RV32I Base Integer Instruction Set, Version 2.1](https://five-embeddev.com/riscv-isa-manual/latest/rv32.html#integer-computational-instructions)