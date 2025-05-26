
CROSS_COMPILE ?= riscv64-unknown-linux-gnu-

profile := release
CARGO_CFG := 

ifdef debug
	profile := debug
else
	CARGO_CFG += --release
	profile := release
endif

build:
	cargo build $(CARGO_CFG)
	$(CROSS_COMPILE)objcopy ../target/riscv64imac-unknown-none-elf/$(profile)/runtime \
	--strip-all -O binary ../target/riscv64imac-unknown-none-elf/$(profile)/runtime.bin