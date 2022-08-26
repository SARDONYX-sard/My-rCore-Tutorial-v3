# -m <memory size>: Phisical memory size (default: 128MiB)
# -machine <name>: Virtual machine pc name
# -nographic: CLI mode
define QEMU_OPTIONS
	-machine virt \
	-nographic \
	-bios ../bootloader/rustsbi-qemu.bin \
	-device loader,file=target/riscv64gc-unknown-none-elf/release/os.bin,addr=0x80200000
endef

qemu: build
	qemu-system-riscv64 $(QEMU_OPTIONS)

# ---------------  debug  -----------------
# -s: Listens for Qemu on local TCP port 1234 and waits for a connection from a GDB client
# -S: Receives a request from GDB and then starts Qemu execution.
gdb-server: build
	@echo "Open another terminal, start the GDB client and connect to Qemu."
	@echo "Please run: make gdb-client"
	qemu-system-riscv64 $(QEMU_OPTIONS) -s -S

gdb-client:
	# riscv64-unknown-elf-gdb \
	gdb-multiarch \
		-ex 'file target/riscv64gc-unknown-none-elf/release/os' \
		-ex 'set arch riscv:rv64' \
		-ex 'target remote localhost:1234'

# ---------------  build  -----------------
release:
	cargo build --release

build: release
	rust-objcopy \
		--strip-all target/riscv64gc-unknown-none-elf/release/os \
		-O binary target/riscv64gc-unknown-none-elf/release/os.bin

os-diff: build
	stat target/riscv64gc-unknown-none-elf/release/os
	stat target/riscv64gc-unknown-none-elf/release/os.bin
