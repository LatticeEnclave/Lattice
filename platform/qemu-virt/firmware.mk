opensbi_path := $(build_path)/opensbi
opensbi_commit := ef4520b1c63fc2770b10d952a800f9734f861b0a
uboot_path := $(build_path)/u-boot
uboot := $(build_path)/u-boot/u-boot.bin
sm_elf := $(platform_path)/../../target/riscv64imac-unknown-none-elf/$(profile)/qemu-virt
sm_bin := $(build_path)/sm.bin

CROSS_COMPILE ?= riscv64-unknown-linux-gnu-
FW_TEXT_START ?= 0x80000000
SM_TEXT_START ?= 0x8001e000
PMP_COUNT ?= 16

cargo_cfg := 
sm_flags := SM_TEXT_START=$(SM_TEXT_START) PMP_COUNT=$(PMP_COUNT) FW_TEXT_START=$(FW_TEXT_START)

ifeq ($(profile),release)
    cargo_cfg += --release
else
	sm_flags += LOG=debug
endif



firmware: sm opensbi

clean:
	rm -rf $(build_path)
	cargo clean

sm:
	cd $(platform_path) && $(sm_flags)  cargo build $(cargo_cfg)
	$(CROSS_COMPILE)objcopy $(sm_elf) --strip-all -O binary $(sm_bin)

$(uboot):
	cd $(uboot_path) && CROSS_COMPILE=$(CROSS_COMPILE) make qemu-riscv64_smode_defconfig
	cd $(uboot_path) && CROSS_COMPILE=$(CROSS_COMPILE) make

$(opensbi_path):
	git clone https://github.com/riscv-software-src/opensbi $(opensbi_path)
	cd $(opensbi_path) && git checkout $(opensbi_commit)
	cd $(opensbi_path) && git apply $(platform_path)/qemu.patch

opensbi: $(opensbi_path) $(uboot)
	@rm -r $(opensbi_path)/build/platform/generic/firmware/
	@cd $(opensbi_path) && make PLATFORM=generic CROSS_COMPILE=$(CROSS_COMPILE) \
	FW_PAYLOAD_PATH=$(uboot) SM_TEXT_START=$(SM_TEXT_START) \
	FW_TEXT_START=$(FW_TEXT_START) SM_PATH=$(sm_bin)
	@echo "GEN opensbi"
	@cp $(opensbi_path)/build/platform/generic/firmware/fw_*.bin $(build_path)
