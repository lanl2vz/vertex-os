KRUST_TARGET := x86_64-unknown-none
HOST_TARGET ?= $(shell $(RUSTC) -vV | sed -n 's/^host: //p')
REQUIRED_RUST := 1.95.0
REQUIRED_CARGO := 1.95.0
REQUIRED_RUSTFMT := 1.9.0-stable
REQUIRED_QEMU := 11.0.0
REQUIRED_LIMINE := 12.3.0
REQUIRED_XORRISO := 1.5.8.pl01
BUILD_DIR := build
ISO_ROOT := $(BUILD_DIR)/iso-root
ISO_IMAGE := $(BUILD_DIR)/krust.iso
BLOCK_IMAGE := $(BUILD_DIR)/krust-block.img
BOOT_MANIFEST := $(BUILD_DIR)/hello-generation.krustboot
FALLBACK_BOOT_MANIFEST := $(BUILD_DIR)/fallback-generation.krustboot
BAD_GENERATION_BOOT_MANIFEST := $(BUILD_DIR)/bad-generation.krustboot
KERNEL_ELF := target/$(KRUST_TARGET)/debug/krust
INIT_TARGET_DIR := user/init/target/$(KRUST_TARGET)/debug
SERIAL_DRIVER_TARGET_DIR := user/serial-driver/target/$(KRUST_TARGET)/debug
CONSOLE_DRIVER_TARGET_DIR := user/console-driver/target/$(KRUST_TARGET)/debug
CONSOLE_SHELL_TARGET_DIR := user/console-shell/target/$(KRUST_TARGET)/debug
LOGD_TARGET_DIR := user/logd/target/$(KRUST_TARGET)/debug
ECHO_TARGET_DIR := user/echo/target/$(KRUST_TARGET)/debug
NETSTACK_TARGET_DIR := user/netstack/target/$(KRUST_TARGET)/debug
BLOCK_DRIVER_TARGET_DIR := user/block-driver/target/$(KRUST_TARGET)/debug
VERTEX_STORE_TARGET_DIR := user/vertex-store/target/$(KRUST_TARGET)/debug
VERTEX_STATE_TARGET_DIR := user/vertex-state/target/$(KRUST_TARGET)/debug
VERTEX_INSPECT_TARGET_DIR := user/vertex-inspect/target/$(KRUST_TARGET)/debug
MODEL_READER_TARGET_DIR := user/model-reader/target/$(KRUST_TARGET)/debug
COUNTER_TARGET_DIR := user/counter/target/$(KRUST_TARGET)/debug
STATE_READER_TARGET_DIR := user/state-reader/target/$(KRUST_TARGET)/debug
TIMER_TARGET_DIR := user/timer/target/$(KRUST_TARGET)/debug
FLAKY_TARGET_DIR := user/flaky/target/$(KRUST_TARGET)/debug
HOG_TARGET_DIR := user/hog/target/$(KRUST_TARGET)/debug
FAULTY_TARGET_DIR := user/faulty/target/$(KRUST_TARGET)/debug
VERTEX_INIT_ELF := $(INIT_TARGET_DIR)/vertex-init
SERIAL_DRIVER_ELF := $(SERIAL_DRIVER_TARGET_DIR)/serial-driver
CONSOLE_DRIVER_ELF := $(CONSOLE_DRIVER_TARGET_DIR)/console-driver
CONSOLE_SHELL_ELF := $(CONSOLE_SHELL_TARGET_DIR)/console-shell
LOGD_ELF := $(LOGD_TARGET_DIR)/logd
ECHO_ELF := $(ECHO_TARGET_DIR)/echo
NETSTACK_ELF := $(NETSTACK_TARGET_DIR)/netstack
BLOCK_DRIVER_ELF := $(BLOCK_DRIVER_TARGET_DIR)/block-driver
VERTEX_STORE_ELF := $(VERTEX_STORE_TARGET_DIR)/vertex-store
VERTEX_STATE_ELF := $(VERTEX_STATE_TARGET_DIR)/vertex-state
VERTEX_INSPECT_ELF := $(VERTEX_INSPECT_TARGET_DIR)/vertex-inspect
MODEL_READER_ELF := $(MODEL_READER_TARGET_DIR)/model-reader
COUNTER_ELF := $(COUNTER_TARGET_DIR)/counter
STATE_READER_ELF := $(STATE_READER_TARGET_DIR)/state-reader
TIMER_ELF := $(TIMER_TARGET_DIR)/timer
FLAKY_ELF := $(FLAKY_TARGET_DIR)/flaky
HOG_ELF := $(HOG_TARGET_DIR)/hog
FAULTY_ELF := $(FAULTY_TARGET_DIR)/faulty
VERTEX_MANIFEST ?= ../../examples/hello-generation.vertex.json
FALLBACK_MANIFEST ?=
BAD_GENERATION_MANIFEST ?=
KRUSTBOOT_CORRUPT ?=
VERTEX_DISK_CORRUPT ?=

QEMU ?= qemu-system-x86_64
QEMU_EXTRA ?= -object rng-random,filename=/dev/urandom,id=vertexrng -device virtio-rng-pci,rng=vertexrng,disable-modern=on -netdev user,id=vertexnet -device virtio-net-pci,netdev=vertexnet,mac=52:54:00:12:34:56,disable-modern=on
QEMU_MACHINE ?=
QEMU_BLOCK ?= -drive if=none,id=vertexblk,file=$(BLOCK_IMAGE),format=raw -device virtio-blk-pci,drive=vertexblk,disable-modern=on,queue-size=8
LIMINE ?= limine
XORRISO ?= xorriso
CARGO ?= cargo
RUSTC ?= rustc
RUSTFMT ?= rustfmt
VERTEXCTL ?= $(CARGO) run --locked --quiet --manifest-path ../../crates/vertexctl/Cargo.toml --target $(HOST_TARGET) --
LIMINE_DIR ?= $(firstword $(wildcard /opt/homebrew/share/limine /usr/local/share/limine /usr/share/limine))

.PHONY: build kernel user boot-manifest block-image iso run smoke doctor release-gate clean check-tools check-tool-versions check-cargo-locks check-rust-target check-limine-dir

build: kernel user

kernel:
	$(CARGO) build --locked --target $(KRUST_TARGET)

user:
	cd user/init && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/serial-driver && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/console-driver && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/console-shell && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/logd && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/echo && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/netstack && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/block-driver && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/vertex-store && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/vertex-state && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/vertex-inspect && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/model-reader && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/counter && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/state-reader && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/timer && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/flaky && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/hog && $(CARGO) build --locked --target $(KRUST_TARGET)
	cd user/faulty && $(CARGO) build --locked --target $(KRUST_TARGET)

boot-manifest: user
	mkdir -p $(BUILD_DIR)
	$(VERTEXCTL) compile-boot-manifest $(VERTEX_MANIFEST) $(BOOT_MANIFEST)
	@if [ -n "$(KRUSTBOOT_CORRUPT)" ]; then \
		$(VERTEXCTL) corrupt-boot-manifest $(KRUSTBOOT_CORRUPT) $(BOOT_MANIFEST) $(BOOT_MANIFEST); \
	fi
	@if [ -n "$(FALLBACK_MANIFEST)" ]; then \
		$(VERTEXCTL) compile-boot-manifest $(FALLBACK_MANIFEST) $(FALLBACK_BOOT_MANIFEST); \
	else \
		cp $(BOOT_MANIFEST) $(FALLBACK_BOOT_MANIFEST); \
	fi
	@if [ -n "$(BAD_GENERATION_MANIFEST)" ]; then \
		$(VERTEXCTL) compile-boot-manifest $(BAD_GENERATION_MANIFEST) $(BAD_GENERATION_BOOT_MANIFEST); \
	else \
		cp $(BOOT_MANIFEST) $(BAD_GENERATION_BOOT_MANIFEST); \
	fi

block-image: user
	mkdir -p $(BUILD_DIR)
	$(VERTEXCTL) create-vertex-disk $(BLOCK_IMAGE) $(VERTEX_MANIFEST) $(FALLBACK_MANIFEST) $(BAD_GENERATION_MANIFEST)
	@if [ -n "$(VERTEX_DISK_CORRUPT)" ]; then \
		$(VERTEXCTL) corrupt-vertex-disk $(VERTEX_DISK_CORRUPT) $(BLOCK_IMAGE) $(BLOCK_IMAGE); \
	fi

iso: build boot-manifest block-image check-tools check-limine-dir
	rm -rf $(ISO_ROOT) $(ISO_IMAGE)
	mkdir -p $(ISO_ROOT)/boot $(ISO_ROOT)/EFI/BOOT
	cp $(KERNEL_ELF) $(ISO_ROOT)/boot/krust.elf
	cp $(BOOT_MANIFEST) $(ISO_ROOT)/boot/hello-generation.krustboot
	cp $(FALLBACK_BOOT_MANIFEST) $(ISO_ROOT)/boot/fallback-generation.krustboot
	cp $(BAD_GENERATION_BOOT_MANIFEST) $(ISO_ROOT)/boot/bad-generation.krustboot
	cp $(BLOCK_IMAGE) $(ISO_ROOT)/boot/krust-block.img
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
	$(QEMU) $(QEMU_MACHINE) $(QEMU_EXTRA) $(QEMU_BLOCK) -m 256M -serial stdio -monitor none -display none -no-reboot -no-shutdown -cdrom $(ISO_IMAGE)

smoke: iso
	@QEMU="$(QEMU)" \
		QEMU_MACHINE="$(QEMU_MACHINE)" \
		QEMU_EXTRA="$(QEMU_EXTRA)" \
		QEMU_BLOCK="$(QEMU_BLOCK)" \
		ISO_IMAGE="$(CURDIR)/$(ISO_IMAGE)" \
		BLOCK_IMAGE="$(CURDIR)/$(BLOCK_IMAGE)" \
		SERIAL_LOG="$(CURDIR)/$(BUILD_DIR)/serial.log" \
		../../scripts/krust-smoke.sh --no-build

doctor: check-tools check-tool-versions check-cargo-locks check-rust-target check-limine-dir
	@echo "rustc: $$(command -v $(RUSTC)) ($$($(RUSTC) --version))"
	@echo "cargo: $$(command -v $(CARGO)) ($$($(CARGO) --version))"
	@echo "rustfmt: $$(command -v $(RUSTFMT)) ($$($(RUSTFMT) --version))"
	@echo "qemu: $$(command -v $(QEMU))"
	@echo "limine: $$(command -v $(LIMINE)) ($$($(LIMINE) --version | sed -n '1p'))"
	@echo "xorriso: $$(command -v $(XORRISO)) ($$($(XORRISO) --version 2>/dev/null | sed -n '1p'))"
	@echo "LIMINE_DIR: $(LIMINE_DIR)"
	@echo "Krust M14-M65 toolchain is ready."

release-gate:
	../../scripts/krust-release-gate.sh

clean:
	rm -rf $(BUILD_DIR) target user/init/target user/serial-driver/target user/console-driver/target user/console-shell/target user/logd/target user/echo/target user/netstack/target user/block-driver/target user/vertex-store/target user/vertex-state/target user/vertex-inspect/target user/model-reader/target user/counter/target user/state-reader/target user/timer/target user/flaky/target user/hog/target user/faulty/target

check-tools:
	@command -v $(RUSTC) >/dev/null 2>&1 || { echo "error: rustc not found. Install Rust $(REQUIRED_RUST), or run rustup from kernel/krust so rust-toolchain.toml installs it."; exit 1; }
	@command -v $(CARGO) >/dev/null 2>&1 || { echo "error: cargo not found. Install Rust $(REQUIRED_RUST) with Cargo."; exit 1; }
	@command -v $(RUSTFMT) >/dev/null 2>&1 || { echo "error: rustfmt not found. Install Rust component rustfmt for $(REQUIRED_RUST)."; exit 1; }
	@command -v $(QEMU) >/dev/null 2>&1 || { echo "error: qemu-system-x86_64 not found. Install QEMU and/or set QEMU=/path/to/qemu-system-x86_64."; exit 1; }
	@command -v $(LIMINE) >/dev/null 2>&1 || { echo "error: limine not found. Install Limine $(REQUIRED_LIMINE) and/or set LIMINE=/path/to/limine."; exit 1; }
	@command -v $(XORRISO) >/dev/null 2>&1 || { echo "error: xorriso not found. Install xorriso and/or set XORRISO=/path/to/xorriso."; exit 1; }

check-tool-versions:
	@rust_version="$$($(RUSTC) --version | sed -n 's/^rustc \([^ ]*\).*/\1/p')"; \
	if [ "$$rust_version" != "$(REQUIRED_RUST)" ]; then \
		echo "error: rustc $$rust_version found, expected $(REQUIRED_RUST). Run rustup toolchain install $(REQUIRED_RUST) --profile minimal --component rustfmt --target $(KRUST_TARGET)."; \
		exit 1; \
	fi
	@cargo_version="$$($(CARGO) --version | sed -n 's/^cargo \([^ ]*\).*/\1/p')"; \
	if [ "$$cargo_version" != "$(REQUIRED_CARGO)" ]; then \
		echo "error: cargo $$cargo_version found, expected $(REQUIRED_CARGO). Use the Cargo bundled with Rust $(REQUIRED_RUST)."; \
		exit 1; \
	fi
	@rustfmt_version="$$($(RUSTFMT) --version | sed -n 's/^rustfmt \([^ ]*\).*/\1/p')"; \
	if [ "$$rustfmt_version" != "$(REQUIRED_RUSTFMT)" ]; then \
		echo "error: rustfmt $$rustfmt_version found, expected $(REQUIRED_RUSTFMT). Install the rustfmt component for Rust $(REQUIRED_RUST)."; \
		exit 1; \
	fi
	@qemu_version="$$($(QEMU) --version | sed -n '1s/^QEMU emulator version \([^ ]*\).*/\1/p')"; \
	if [ "$$qemu_version" != "$(REQUIRED_QEMU)" ]; then \
		echo "error: QEMU $$qemu_version found, expected $(REQUIRED_QEMU). Install QEMU $(REQUIRED_QEMU) or set QEMU=/path/to/qemu-system-x86_64."; \
		exit 1; \
	fi
	@limine_version="$$($(LIMINE) --version | sed -n '1s/^Limine \([^ ]*\).*/\1/p')"; \
	if [ "$$limine_version" != "$(REQUIRED_LIMINE)" ]; then \
		echo "error: Limine $$limine_version found, expected $(REQUIRED_LIMINE). Install Limine $(REQUIRED_LIMINE) or set LIMINE=/path/to/limine."; \
		exit 1; \
	fi
	@xorriso_version="$$($(XORRISO) --version 2>/dev/null | sed -n '1s/^GNU xorriso \([^ ]*\).*/\1/p')"; \
	if [ "$$xorriso_version" != "$(REQUIRED_XORRISO)" ]; then \
		echo "error: xorriso $$xorriso_version found, expected $(REQUIRED_XORRISO). Install xorriso $(REQUIRED_XORRISO) or set XORRISO=/path/to/xorriso."; \
		exit 1; \
	fi

check-cargo-locks:
	@test -f ../../Cargo.lock || { echo "error: repository Cargo.lock missing. Run cargo generate-lockfile at the repository root with Rust $(REQUIRED_RUST)."; exit 1; }
	@test -f Cargo.lock || { echo "error: kernel/krust/Cargo.lock missing. Run cargo generate-lockfile with Rust $(REQUIRED_RUST)."; exit 1; }
	@for manifest in user/*/Cargo.toml; do \
		lock="$${manifest%Cargo.toml}Cargo.lock"; \
		test -f "$$lock" || { echo "error: $$lock missing. Run cargo generate-lockfile in $${manifest%/Cargo.toml}."; exit 1; }; \
	done

check-rust-target:
	@ls "$$($(RUSTC) --print sysroot)/lib/rustlib/$(KRUST_TARGET)/lib"/libcore-*.rlib >/dev/null 2>&1 || { echo "error: Rust target $(KRUST_TARGET) is not installed for Rust $(REQUIRED_RUST). Run: rustup target add $(KRUST_TARGET) --toolchain $(REQUIRED_RUST)"; exit 1; }

check-limine-dir:
	@test -n "$(LIMINE_DIR)" || { echo "error: LIMINE_DIR is required. Set it to the directory containing Limine boot files. Common paths: /opt/homebrew/share/limine, /usr/local/share/limine, /usr/share/limine."; exit 1; }
	@test -f "$(LIMINE_DIR)/limine-bios.sys" || { echo "error: LIMINE_DIR missing limine-bios.sys"; exit 1; }
	@test -f "$(LIMINE_DIR)/limine-bios-cd.bin" || { echo "error: LIMINE_DIR missing limine-bios-cd.bin"; exit 1; }
	@test -f "$(LIMINE_DIR)/limine-uefi-cd.bin" || { echo "error: LIMINE_DIR missing limine-uefi-cd.bin"; exit 1; }
	@test -f "$(LIMINE_DIR)/BOOTX64.EFI" || { echo "error: LIMINE_DIR missing BOOTX64.EFI"; exit 1; }
