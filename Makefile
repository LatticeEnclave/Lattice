
project ?= $(shell pwd)
platform ?= qemu-virt
platform_path := $(project)/platform/$(platform)
build_path := $(project)/build/$(platform)
qemu ?= qemu-system-riscv64
mem ?= 4096M
smp ?= 4

qemu_flags := -machine virt -nographic
qemu_flags += -bios $(build_path)/fw_payload.bin
qemu_flags += -m $(mem) -serial mon:stdio
qemu_flags += -smp $(smp)
qemu_flags += -drive file=$(rootfs),format=raw,if=virtio

ifdef gdb
	qemu_flags += -S -gdb tcp::$(gdb)
endif

ifdef share
	qemu_flags += -virtfs local,path=$(share),mount_tag=share0,security_model=passthrough,id=shared0
endif

qemu_flags += -netdev user,id=net0, -device virtio-net-pci,netdev=net0

ifdef QEMU_ARGS
	qemu_flags += QEMU_ARGS
endif

$(build_path):
	mkdir -p $(build_path)

debug: $(build_path)
	make -f $(platform_path)/firmware.mk build_path=$(build_path) platform_path=$(platform_path) profile=debug firmware

release: $(build_path)
	make -f $(platform_path)/firmware.mk build_path=$(build_path) platform_path=$(platform_path) profile=release firmware

clean:
	make -f $(platform_path)/firmware.mk build_path=$(build_path) platform_path=$(platform_path) clean

qemu:
	$(qemu) $(qemu_flags)
