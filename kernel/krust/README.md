# Krust Kernel

Krust M0 is the first native Vertex OS kernel milestone.

The target is intentionally small:

```text
QEMU boots a Limine ISO
Limine loads krust.elf
Krust enters 64-bit Rust code
Krust writes "Krust Kernel booted" to COM1 serial
Krust halts forever
```

No memory manager, heap, interrupts, userspace, manifest parsing, Vertex IR
integration, filesystem, network, or device drivers are part of M0.

## Prerequisites

- Rust stable with the `x86_64-unknown-none` target.
- `qemu-system-x86_64`.
- `limine` v12 or newer.
- `xorriso`.
- `LIMINE_DIR` pointing at Limine boot assets containing:
  - `limine-bios.sys`
  - `limine-bios-cd.bin`
  - `limine-uefi-cd.bin`
  - `BOOTX64.EFI`

Limine installs these files under its configured `${PREFIX}/share` directory.
Typical package-manager locations are `/usr/share/limine`,
`/usr/local/share/limine`, or a Homebrew prefix path.

## Build

```sh
make build
```

This builds `target/x86_64-unknown-none/debug/krust`.

## Build ISO

```sh
LIMINE_DIR=/path/to/limine/assets make iso
```

This creates `build/krust.iso`.

## Run

```sh
LIMINE_DIR=/path/to/limine/assets make run
```

Expected terminal output:

```text
Krust Kernel booted
```

QEMU runs with `-display none`, so all kernel output is written through the
serial console. Interrupt QEMU with `Ctrl-C`.

## Smoke Test

```sh
LIMINE_DIR=/path/to/limine/assets make smoke
```

The smoke test boots QEMU headlessly, captures serial output to
`build/serial.log`, and passes when it sees:

```text
Krust Kernel booted
```

## Machine Notes

Linux x86_64 can add KVM later with QEMU flags such as `-enable-kvm -cpu host`.
That is not enabled by default so the M0 command stays portable.

macOS Apple Silicon can run `qemu-system-x86_64` by emulation. It is slower than
native AArch64 virtualization, but it is enough for the M0 serial milestone.
