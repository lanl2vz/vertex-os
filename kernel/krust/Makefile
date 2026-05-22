KRUST_TARGET := x86_64-unknown-none
HOST_TARGET ?= $(shell rustc -vV | sed -n 's/^host: //p')
BUILD_DIR := build
ISO_ROOT := $(BUILD_DIR)/iso-root
ISO_IMAGE := $(BUILD_DIR)/krust.iso
BOOT_MANIFEST := $(BUILD_DIR)/hello-generation.krustboot
KERNEL_ELF := target/$(KRUST_TARGET)/debug/krust
IPC_USER_TARGET_DIR := user/ipc/target/$(KRUST_TARGET)/debug
IPC_SENDER_ELF := $(IPC_USER_TARGET_DIR)/krust-ipc-sender
IPC_RECEIVER_ELF := $(IPC_USER_TARGET_DIR)/krust-ipc-receiver
VERTEX_MANIFEST ?= ../../examples/hello-generation.vertex.json

QEMU ?= qemu-system-x86_64
QEMU_EXTRA ?=
LIMINE ?= limine
XORRISO ?= xorriso
VERTEXCTL ?= cargo run --quiet --manifest-path ../../crates/vertexctl/Cargo.toml --target $(HOST_TARGET) --
LIMINE_DIR ?= $(firstword $(wildcard /opt/homebrew/share/limine /usr/local/share/limine /usr/share/limine))

.PHONY: build kernel user boot-manifest iso run smoke doctor clean check-tools check-limine-dir

build: kernel user

kernel:
	cargo build --target $(KRUST_TARGET)

user:
	cd user/ipc && cargo build --target $(KRUST_TARGET)

boot-manifest:
	mkdir -p $(BUILD_DIR)
	$(VERTEXCTL) compile-boot-manifest $(VERTEX_MANIFEST) $(BOOT_MANIFEST)

iso: build boot-manifest check-tools check-limine-dir
	rm -rf $(ISO_ROOT) $(ISO_IMAGE)
	mkdir -p $(ISO_ROOT)/boot $(ISO_ROOT)/EFI/BOOT
	cp $(KERNEL_ELF) $(ISO_ROOT)/boot/krust.elf
	cp $(IPC_SENDER_ELF) $(ISO_ROOT)/boot/krust-ipc-sender.elf
	cp $(IPC_RECEIVER_ELF) $(ISO_ROOT)/boot/krust-ipc-receiver.elf
	cp $(BOOT_MANIFEST) $(ISO_ROOT)/boot/hello-generation.krustboot
	cp limine.conf $(ISO_ROOT)/boot/limine.conf
	cp $(LIMINE_DIR)/limine-bios.sys $(ISO_ROOT)/boot/limine-bios.sys
	cp $(LIMINE_DIR)/limine-bios-cd.bin $(ISO_ROOT)/boot/limine-bios-cd.bin
	cp $(LIMINE_DIR)/limine-uefi-cd.bin $(ISO_ROOT)/boot/limine-uefi-cd.bin
	cp $(LIMINE_DIR)/BOOTX64.EFI $(ISO_ROOT)/EFI/BOOT/BOOTX64.EFI
	$(XORRISO) -as mkisofs \
		-R \
		-r \
		-J \
		-b boot/limine-bios-cd.bin \
		-no-emul-boot \
		-boot-load-size 4 \
		-boot-info-table \
		-hfsplus \
		-apm-block-size 2048 \
		--efi-boot boot/limine-uefi-cd.bin \
		-efi-boot-part \
		--efi-boot-image \
		--protective-msdos-label \
		$(ISO_ROOT) \
		-o $(ISO_IMAGE)
	$(LIMINE) bios-install $(ISO_IMAGE)

run: iso
	$(QEMU) $(QEMU_EXTRA) -m 256M -serial stdio -monitor none -display none -no-reboot -no-shutdown -cdrom $(ISO_IMAGE)

smoke: iso
	@rm -f $(BUILD_DIR)/serial.log
	@$(QEMU) $(QEMU_EXTRA) -m 256M -serial file:$(BUILD_DIR)/serial.log -monitor none -display none -no-reboot -no-shutdown -cdrom $(ISO_IMAGE) & \
	pid=$$!; \
	for _ in 1 2 3 4 5 6 7 8; do \
		if grep -q "Krust Kernel booted" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "Limine memory map entries:" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "KrustBoot manifest generation: gen:hello-0001" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "KrustBoot processes: 2" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "KrustBoot endpoints: 1" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "KrustBoot grants: 2" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "grant\\[0\\] process=ipc-sender cap\\[0\\] endpoint=demo-ipc rights=send" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "grant\\[1\\] process=ipc-receiver cap\\[0\\] endpoint=demo-ipc rights=receive" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "Physical allocator demo ok" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "Virtual memory demo ok" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "Capability table demo ok" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "IDT initialized: #UD #GP #PF" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "Process table entries: 2" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "Endpoint table entries: 1" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "endpoint\\[0\\] id=1 name=demo-ipc" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "process\\[0\\] id=1 name=ipc-sender state=ready" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "process\\[1\\] id=2 name=ipc-receiver state=running" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "proc=ipc-sender cap\\[0\\] endpoint=1 rights=send" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "proc=ipc-receiver cap\\[0\\] endpoint=1 rights=receive" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "IPC receive blocked: proc=ipc-receiver endpoint=1" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "Scheduler switch: from=ipc-receiver to=ipc-sender" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "Scheduler switch: from=ipc-sender to=ipc-receiver" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "IPC wake receiver: proc=ipc-receiver endpoint=1" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "Bad pointer test: SYS_WRITE_SERIAL returned STATUS_BAD_BUFFER" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "Bad pointer test: SYS_IPC_SEND returned STATUS_BAD_BUFFER" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "Bad pointer test: SYS_IPC_RECV returned STATUS_BAD_BUFFER" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "IPC send accepted: endpoint=1 bytes=14" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "IPC receive delivered: endpoint=1 bytes=14" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "IPC negative test: ipc-sender receive rejected: bad capability" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "IPC negative test: ipc-receiver send rejected: bad capability" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "Krust IPC ping" $(BUILD_DIR)/serial.log 2>/dev/null && grep -q "IPC demo ok" $(BUILD_DIR)/serial.log 2>/dev/null; then \
				kill $$pid >/dev/null 2>&1 || true; \
				wait $$pid >/dev/null 2>&1 || true; \
				echo "smoke ok: Krust Kernel booted, parsed KrustBoot manifest, installed IDT handlers, rejected bad user buffers, enforced process capability tables, blocked IPC receive, woke receiver, and ran userspace IPC"; \
				exit 0; \
			fi; \
		sleep 1; \
	done; \
	kill $$pid >/dev/null 2>&1 || true; \
	wait $$pid >/dev/null 2>&1 || true; \
		echo "smoke failed: serial output did not contain Krust Kernel booted, Limine memory map entries, KrustBoot manifest records, Physical allocator demo ok, Virtual memory demo ok, Capability table demo ok, IDT initialization, runtime process states, process/endpoint tables, scheduler switches, blocked receive/wake, bad pointer rejections, positive IPC, negative capability rejections, Krust IPC ping, and IPC demo ok"; \
	test ! -f $(BUILD_DIR)/serial.log || cat $(BUILD_DIR)/serial.log; \
	exit 1

doctor: check-tools check-limine-dir
	@echo "qemu: $$(command -v $(QEMU))"
	@echo "limine: $$(command -v $(LIMINE))"
	@echo "xorriso: $$(command -v $(XORRISO))"
	@echo "LIMINE_DIR: $(LIMINE_DIR)"
	@echo "Krust M11 toolchain is ready."

clean:
	rm -rf $(BUILD_DIR) target

check-tools:
	@command -v $(QEMU) >/dev/null 2>&1 || { echo "error: qemu-system-x86_64 not found. Install QEMU and/or set QEMU=/path/to/qemu-system-x86_64."; exit 1; }
	@command -v $(LIMINE) >/dev/null 2>&1 || { echo "error: limine not found. Install Limine v12+ and/or set LIMINE=/path/to/limine."; exit 1; }
	@command -v $(XORRISO) >/dev/null 2>&1 || { echo "error: xorriso not found. Install xorriso and/or set XORRISO=/path/to/xorriso."; exit 1; }

check-limine-dir:
	@test -n "$(LIMINE_DIR)" || { echo "error: LIMINE_DIR is required. Set it to the directory containing Limine boot files. Common paths: /opt/homebrew/share/limine, /usr/local/share/limine, /usr/share/limine."; exit 1; }
	@test -f "$(LIMINE_DIR)/limine-bios.sys" || { echo "error: LIMINE_DIR missing limine-bios.sys"; exit 1; }
	@test -f "$(LIMINE_DIR)/limine-bios-cd.bin" || { echo "error: LIMINE_DIR missing limine-bios-cd.bin"; exit 1; }
	@test -f "$(LIMINE_DIR)/limine-uefi-cd.bin" || { echo "error: LIMINE_DIR missing limine-uefi-cd.bin"; exit 1; }
	@test -f "$(LIMINE_DIR)/BOOTX64.EFI" || { echo "error: LIMINE_DIR missing BOOTX64.EFI"; exit 1; }
