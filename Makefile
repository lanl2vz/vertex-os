VERTEX_TARGET ?= krust
KRUST_MAKE := $(MAKE) --no-print-directory -C kernel/krust

.DEFAULT_GOAL := help

.PHONY: help check-target build iso boot-image run boot run-gui boot-gui run-window smoke doctor release-gate clean

help:
	@printf '%s\n' \
		'Vertex OS commands:' \
		'  make run-gui       Boot Vertex OS in a QEMU window' \
		'  make run           Boot Vertex OS headlessly with serial in this terminal' \
		'  make iso           Build the bootable Vertex OS ISO for the selected target' \
		'  make smoke         Run the headless QEMU smoke test' \
		'  make doctor        Check the native target toolchain' \
		'  make release-gate  Run the release gate' \
		'  make clean         Remove target boot/build artifacts' \
		'' \
		'Options:' \
		'  VERTEX_TARGET=krust  Native target to boot; krust is the supported target today'

check-target:
	@test "$(VERTEX_TARGET)" = "krust" || { \
		echo "error: unsupported VERTEX_TARGET=$(VERTEX_TARGET). Supported target: krust"; \
		exit 2; \
	}

build: check-target
	@$(KRUST_MAKE) build

iso boot-image: check-target
	@$(KRUST_MAKE) iso

run boot: check-target
	@echo "Booting Vertex OS on target=$(VERTEX_TARGET) headless..."
	@$(KRUST_MAKE) run

run-gui boot-gui run-window: check-target
	@echo "Booting Vertex OS on target=$(VERTEX_TARGET) in a QEMU window..."
	@$(KRUST_MAKE) run-gui

smoke: check-target
	@$(KRUST_MAKE) smoke

doctor: check-target
	@$(KRUST_MAKE) doctor

release-gate: check-target
	@scripts/krust-release-gate.sh

clean: check-target
	@$(KRUST_MAKE) clean
