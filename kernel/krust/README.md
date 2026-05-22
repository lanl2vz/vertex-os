# Krust Kernel

Krust M13 is the first native multi-service activation milestone.

The target is intentionally small:

```text
QEMU boots a Limine ISO
Limine loads krust.elf
Krust enters 64-bit Rust code
Krust writes "Krust Kernel booted" to COM1 serial
Krust reads the Limine memory map response
Krust prints every memory map entry to serial
`vertexctl compile-boot-manifest` compiles `hello-generation.vertex.json`'s `krustBoot` section to `hello-generation.krustboot`
Limine loads `hello-generation.krustboot` as a boot module
Krust parses the fixed-format KrustBoot manifest and prints its generation ID
Krust prints the KrustBoot boot modules, processes, endpoints, and grants
Krust builds a physical frame allocator from usable memory map entries
Krust allocates, frees, and reuses 4 KiB physical frames
Krust walks the active x86_64 page tables through Limine's HHDM
Krust maps a small fixed kernel-heap virtual range
Krust writes and reads through the mapped virtual pages
Krust creates fixed kernel objects and boot capabilities
Krust prints the boot capability table
Limine loads `vertex-init.elf`, `logd.elf`, and `echo.elf` as boot modules
Krust loads each declared process into a fresh low-half address space
Krust creates a runtime process table and endpoint table from the KrustBoot manifest
Krust allocates runtime process IDs and states from KrustBoot process records
Krust grants vertex-init cap[0] read rights to the manifest module
Krust grants vertex-init cap[1] send rights to the serial-log endpoint
Krust grants vertex-init cap[2] process-control authority
Krust installs a minimal IDT for #UD, #GP, and #PF
Krust enters ring 3 at the initial process entry point
Krust tracks Declared, Ready, Running, BlockedOnEndpoint, and Exited process states
Krust validates syscall user buffers by walking user page tables
vertex-init reads the compact manifest through cap[0]
vertex-init logs through cap[1]
vertex-init starts declared services through cap[2]
echo sends one message to logd through an explicit IPC capability
logd receives the message and denial tests reject missing authority
Krust halts after `Native service activation ok`
```

No dynamic heap allocator, timer/APIC setup, preemption, user page-fault
recovery, full interrupt handling, full JSON parsing in the kernel, full service
ordering, filesystem, network, or device drivers are part of M13. The kernel
consumes a compact KrustBoot manifest compiled by hosted `vertexctl`; full graph
interpretation remains a userspace responsibility. M13 proves that native
`vertex-init` can use process-control authority to start declared services and
that service IPC authority is still explicit and process-local.

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

This builds `target/x86_64-unknown-none/debug/krust`,
`user/init/target/x86_64-unknown-none/debug/vertex-init`,
`user/logd/target/x86_64-unknown-none/debug/logd`, and
`user/echo/target/x86_64-unknown-none/debug/echo`.

## Build ISO

```sh
make iso
```

This runs `vertexctl compile-boot-manifest`, writes
`build/hello-generation.krustboot`, and creates `build/krust.iso`.

## Run

```sh
make run
```

Expected terminal output:

```text
Krust Kernel booted
Limine base revision supported
Limine memory map entries: ...
KrustBoot manifest generation: gen:hello-0001
KrustBoot boot modules: 3
KrustBoot processes: 3
KrustBoot endpoints: 2
KrustBoot grants: 5
  grant[0] process=vertex-init cap[1] endpoint=serial-log rights=send
  grant[1] process=logd cap[0] endpoint=log-sink rights=receive
  grant[3] process=echo cap[0] endpoint=log-sink rights=send
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
Process table entries: 3
Endpoint table entries: 2
endpoint[0] id=1 name=serial-log
endpoint[1] id=2 name=log-sink
process[0] id=1 name=vertex-init state=running
process[1] id=2 name=logd state=declared
process[2] id=3 name=echo state=declared
proc=vertex-init cap[0] boot-module=krustboot-manifest rights=read
proc=vertex-init cap[1] endpoint=serial-log rights=send
proc=vertex-init cap[2] process-control=process-control rights=control
proc=logd cap[0] endpoint=log-sink rights=receive
proc=echo cap[0] endpoint=log-sink rights=send
GDT initialized
IDT initialized: #UD #GP #PF
Syscall path initialized
Entering userspace process: vertex-init
vertex-init started
Boot module read accepted: proc=vertex-init module=krustboot-manifest bytes=...
vertex-init received cap[0]=manifest-read
vertex-init received cap[1]=serial-log
vertex-init received cap[2]=process-control
vertex-init manifest generation: gen:hello-0001
vertex-init boot modules: 3
vertex-init processes: 3
vertex-init endpoints: 2
vertex-init grants: 5
vertex-init starting service: logd
Krust process start accepted: proc=vertex-init target=logd
vertex-init starting service: echo
Krust process start accepted: proc=vertex-init target=echo
echo sent message to logd
logd received: hello from echo
negative test: echo receive rejected: bad capability
negative test: logd process-start rejected: bad capability
Native service activation ok
```

QEMU runs with `-display none`, so all kernel output is written through the
serial console. Interrupt QEMU with `Ctrl-C`.

## Smoke Test

```sh
make smoke
```

The smoke test boots QEMU headlessly, captures serial output to
`build/serial.log`, and passes when it sees the M13 boot transcript. The same
check is available from the repository root:

```sh
scripts/krust-smoke.sh
```

The expected transcript includes:

```text
Krust Kernel booted
Limine memory map entries:
KrustBoot manifest generation: gen:hello-0001
KrustBoot boot modules: 3
KrustBoot processes: 3
KrustBoot endpoints: 2
KrustBoot grants: 5
grant[0] process=vertex-init cap[1] endpoint=serial-log rights=send
grant[1] process=logd cap[0] endpoint=log-sink rights=receive
grant[3] process=echo cap[0] endpoint=log-sink rights=send
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
IDT initialized: #UD #GP #PF
Process table entries: 3
Endpoint table entries: 2
endpoint[0] id=1 name=serial-log
endpoint[1] id=2 name=log-sink
process[0] id=1 name=vertex-init state=running
process[1] id=2 name=logd state=declared
process[2] id=3 name=echo state=declared
proc=vertex-init cap[0] boot-module=krustboot-manifest rights=read
proc=vertex-init cap[1] endpoint=serial-log rights=send
proc=vertex-init cap[2] process-control=process-control rights=control
proc=logd cap[0] endpoint=log-sink rights=receive
proc=echo cap[0] endpoint=log-sink rights=send
vertex-init started
Boot module read accepted: proc=vertex-init module=krustboot-manifest bytes=
vertex-init manifest generation: gen:hello-0001
vertex-init starting service: logd
Krust process start accepted: proc=vertex-init target=logd
vertex-init starting service: echo
Krust process start accepted: proc=vertex-init target=echo
echo sent message to logd
logd received: hello from echo
negative test: echo receive rejected: bad capability
negative test: logd process-start rejected: bad capability
Native service activation ok
```

## Machine Notes

Linux x86_64 can add KVM later with QEMU flags such as `-enable-kvm -cpu host`.
That is not enabled by default so the M13 command stays portable:

```sh
QEMU_EXTRA="-enable-kvm -cpu host" make smoke
```

macOS Apple Silicon can run `qemu-system-x86_64` by emulation. It is slower than
native AArch64 virtualization, but it is enough for the M13 serial milestone.
