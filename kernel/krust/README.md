# Krust Kernel

Krust M2 is the first native Vertex manifest module milestone.

The target is intentionally small:

```text
QEMU boots a Limine ISO
Limine loads krust.elf
Krust enters 64-bit Rust code
Krust writes "Krust Kernel booted" to COM1 serial
Krust reads the Limine memory map response
Krust prints every memory map entry to serial
Limine loads `hello-generation.vertex.json` as a boot module
Krust finds the manifest module and prints its generation ID
Krust halts forever
```

No memory manager, heap, interrupts, userspace, full manifest parsing, Vertex IR
integration, filesystem, network, or device drivers are part of M2.

## Prerequisites

- Rust stable with the `x86_64-unknown-none` target.
- `qemu-system-x86_64`.
- `limine` v12 or newer.
- `xorriso`.
- Limine boot assets containing:
  - `limine-bios.sys`
  - `limine-bios-cd.bin`
  - `limine-uefi-cd.bin`
  - `BOOTX64.EFI`

Limine installs these files under its configured `${PREFIX}/share` directory.
Typical package-manager locations are `/usr/share/limine`,
`/usr/local/share/limine`, or a Homebrew prefix path.

The Makefile auto-detects these Limine asset directories:

```text
/opt/homebrew/share/limine
/usr/local/share/limine
/usr/share/limine
```

If Limine is installed somewhere else, pass it explicitly:

```sh
LIMINE_DIR=/path/to/limine/assets make smoke
```

On macOS with Homebrew:

```sh
brew install limine xorriso qemu
make doctor
```

On Linux, install the same tools through the host distribution package manager
and run:

```sh
make doctor
```

`make doctor` prints the resolved QEMU, Limine, xorriso, and Limine asset paths.

## Build

```sh
make build
```

This builds `target/x86_64-unknown-none/debug/krust`.

## Build ISO

```sh
make iso
```

This creates `build/krust.iso`.

## Run

```sh
make run
```

Expected terminal output:

```text
Krust Kernel booted
Limine base revision supported
Limine memory map entries: ...
Vertex manifest generation: gen:hello-0001
```

QEMU runs with `-display none`, so all kernel output is written through the
serial console. Interrupt QEMU with `Ctrl-C`.

## Smoke Test

```sh
make smoke
```

The smoke test boots QEMU headlessly, captures serial output to
`build/serial.log`, and passes when it sees:

```text
Krust Kernel booted
Limine memory map entries:
Vertex manifest generation: gen:hello-0001
```

## Machine Notes

Linux x86_64 can add KVM later with QEMU flags such as `-enable-kvm -cpu host`.
That is not enabled by default so the M2 command stays portable:

```sh
QEMU_EXTRA="-enable-kvm -cpu host" make smoke
```

macOS Apple Silicon can run `qemu-system-x86_64` by emulation. It is slower than
native AArch64 virtualization, but it is enough for the M2 serial milestone.
