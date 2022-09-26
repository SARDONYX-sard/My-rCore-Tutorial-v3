TARGET := riscv64gc-unknown-none-elf
MODE := release
APP_DIR := src/bin
TARGET_DIR := target/$(TARGET)/$(MODE)
APPS := $(wildcard $(APP_DIR)/*.rs)
ELFS := $(patsubst $(APP_DIR)/%.rs, $(TARGET_DIR)/%, $(APPS))
BINS := $(patsubst $(APP_DIR)/%.rs, $(TARGET_DIR)/%.bin, $(APPS))

OBJDUMP := rust-objdump --arch-name=riscv64
OBJCOPY := rust-objcopy --binary-architecture=riscv64
CP := cp

TEST ?=

elf: $(APPS)
	@[ -f $(TARGET_DIR)/initproc ] && rm -f $(TARGET_DIR)/initproc || echo ""
	@[ -f $(TARGET_DIR)/initproc.bin ] && rm -f $(TARGET_DIR)/initproc.bin || echo ""
	@[ -f $(TARGET_DIR)/initproc.d ] && rm -f $(TARGET_DIR)/initproc.d || echo ""

	@cargo build --release
ifeq ($(TEST), 1)
	@$(CP) $(TARGET_DIR)/usertests $(TARGET_DIR)/initproc
endif

binary: elf
	@$(foreach elf, $(ELFS), $(OBJCOPY) $(elf) --strip-all -O binary $(patsubst $(TARGET_DIR)/%, $(TARGET_DIR)/%.bin, $(elf));)

build: binary

EMU:=	qemu-riscv64
test: build
	@$(foreach elf, $(ELFS), $(EMU) $(elf))

# ---------------  debug  -----------------
# -g <port_number>: Listens for Qemu on local TCP port <port_number>
#                   and waits for a connection from a GDB client
gdb-server: build
	@echo "Open another terminal, start the GDB client and connect to Qemu."
	@echo "Please run: make gdb-client"
	@$(EMU) -g 1234 target/riscv64gc-unknown-none-elf/release/00hello_world

gdb-client:
	@gdb-multiarch \
		-ex 'file target/riscv64gc-unknown-none-elf/release/00hello_world' \
		-ex 'set arch riscv:rv64' \
		-ex 'target remote localhost:1234'

.PHONY: elf binary build test gdb-server gdb-client
