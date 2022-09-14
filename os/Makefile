# Building
TARGET := riscv64gc-unknown-none-elf
MODE := release
KERNEL_ELF := target/$(TARGET)/$(MODE)/os
KERNEL_BIN := $(KERNEL_ELF).bin
DISASM_TMP := target/$(TARGET)/$(MODE)/asm

# BOARD
BOARD ?= qemu
SBI ?= rustsbi
BOOTLOADER := ../bootloader/$(SBI)-$(BOARD).bin
QEMU := qemu-system-riscv64

# Building mode argument
ifeq ($(MODE), release)
	MODE_ARG := --release
endif

# KERNEL ENTRY
ifeq ($(BOARD), qemu)
	KERNEL_ENTRY_PA := 0x80200000
endif

# Binutils
OBJDUMP := rust-objdump --arch-name=riscv64
OBJCOPY := rust-objcopy --binary-architecture=riscv64

# Disassembly
DISASM ?= -x

# Run usertests or usershell
TEST ?=

build: env $(KERNEL_BIN)

build-ci: $(KERNEL_BIN)

env:
	(rustup target list | grep "riscv64gc-unknown-none-elf (installed)") || rustup target add $(TARGET)
	cargo install cargo-binutils
	rustup component add rust-src
	rustup component add llvm-tools-preview

$(KERNEL_BIN): kernel
	@$(OBJCOPY) $(KERNEL_ELF) --strip-all -O binary $@

kernel:
	@cd ../user && make build TEST=$(TEST)
	@echo Platform: $(BOARD)
	@cargo build $(MODE_ARG) --features "board_$(BOARD)"


clean:
	@cargo clean

disasm: kernel
	@$(OBJDUMP) $(DISASM) $(KERNEL_ELF) | less

disasm-vim: kernel
	@$(OBJDUMP) $(DISASM) $(KERNEL_ELF) > $(DISASM_TMP)
	@nvim $(DISASM_TMP)
	@rm $(DISASM_TMP)

# https://github.com/rust-embedded/cargo-binutils#objdump
objdump:
	@cargo objdump $(MODE_ARG) --features "board_$(BOARD)" \
		-- \
		--disassemble \
		--no-show-raw-insn \
		> $(DISASM_TMP)
	@nvim $(DISASM_TMP)
	@rm $(DISASM_TMP)

# -m <memory size>: Phisical memory size (default: 128MiB)
# -machine <name>: Virtual machine pc name
# -nographic: CLI mode
define QEMU_OPTIONS
	-machine virt \
	-nographic \
	-bios $(BOOTLOADER) \
	-device loader,file=$(KERNEL_BIN),addr=$(KERNEL_ENTRY_PA)
endef

run: build
	$(QEMU) $(QEMU_OPTIONS)

run-ci: kernel
	$(QEMU) $(QEMU_OPTIONS)

# ---------------  debug  -----------------
# -s: Listens for Qemu on local TCP port 1234 and waits for a connection from a GDB client
# -S: Receives a request from GDB and then starts Qemu execution.
GDB_SERVER_OPTIONS := -s -S

gdb := gdb-multiarch
define GDB_CLIENT_OPTIONS
	-ex 'file $(KERNEL_ELF)' \
	-ex 'set arch riscv:rv64' \
	-ex 'target remote localhost:1234'
endef

debug: build
	@tmux new-session -d \
		"$(QEMU) $(QEMU_OPTIONS) $(GDB_SERVER_OPTIONS)" && \
		tmux split-window -h "$(gdb) $(GDB_CLIENT_OPTIONS)" && \
		tmux -2 attach-session -d

gdb-server:
	@printf "\033[0;34mOpen another terminal, start the GDB client and connect to Qemu.\033[0;0m\n"
	@echo "Please run:"
	@echo "         make gdb-client"
	@echo ""
	@printf "\033[0;33mquit shortcut key: <ctrl+a>+c, next press 'q' in qemu.\033[0;0m\n"
	@$(QEMU) $(QEMU_OPTIONS) $(GDB_SERVER_OPTIONS)

gdb-client:
	@$(gdb) $(GDB_CLIENT_OPTIONS)

os-diff: build
	stat $(KERNEL_ELF)
	stat $(KERNEL_BIN)

.PHONY: build env kernel clean disasm disasm-vim run-inner gdb-server gdb-client debug os-diff
