# Krust Kernel

Krust M6 is the first native userspace milestone.

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
Krust builds a physical frame allocator from usable memory map entries
Krust allocates, frees, and reuses 4 KiB physical frames
Krust walks the active x86_64 page tables through Limine's HHDM
Krust maps a small fixed kernel-heap virtual range
Krust writes and reads through the mapped virtual pages
Krust creates fixed kernel objects and boot capabilities
Krust prints the boot capability table
Limine loads `krust-user-hello.elf` as a boot module
Krust loads the static user ELF into a fresh low-half address space
Krust enters ring 3 at the user ELF entry point
Userspace calls `sys_write_serial`
Krust prints the userspace message and halts in the syscall handler
```

No dynamic heap allocator, scheduler, IPC, full manifest parsing, Vertex IR
integration, filesystem, network, or device drivers are part of M6.

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

This builds `target/x86_64-unknown-none/debug/krust` and
`user/hello/target/x86_64-unknown-none/debug/krust-user-hello`.

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
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
Krust userspace ELF loaded: entry=...
Entering Krust userspace
Userspace sys_write_serial: Krust userspace says hello
Userspace syscall demo ok
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
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
Krust userspace says hello
Userspace syscall demo ok
```

## Machine Notes

Linux x86_64 can add KVM later with QEMU flags such as `-enable-kvm -cpu host`.
That is not enabled by default so the M6 command stays portable:

```sh
QEMU_EXTRA="-enable-kvm -cpu host" make smoke
```

macOS Apple Silicon can run `qemu-system-x86_64` by emulation. It is slower than
native AArch64 virtualization, but it is enough for the M6 serial milestone.
