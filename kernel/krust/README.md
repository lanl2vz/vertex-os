# Krust Kernel

Krust M11 is the first tiny cooperative-scheduler milestone.

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
Limine loads `krust-ipc-sender.elf` and `krust-ipc-receiver.elf` as boot modules
Krust loads each static user ELF into a fresh low-half address space
Krust creates a runtime process table and endpoint table from the KrustBoot manifest
Krust allocates runtime process IDs and states from KrustBoot process records
Krust grants ipc-sender cap[0] send rights to demo-ipc from the KrustBoot manifest
Krust grants ipc-receiver cap[0] receive rights to demo-ipc from the KrustBoot manifest
Krust installs a minimal IDT for #UD, #GP, and #PF
Krust enters ring 3 at the initial process entry point
Krust tracks Ready, Running, BlockedOnEndpoint, and Exited process states
Receiver calls `sys_ipc_recv` before a message exists and blocks on demo-ipc
The cooperative scheduler round-robins to the sender
Krust validates syscall user buffers by walking user page tables
Bad `sys_write_serial`, `sys_ipc_send`, and `sys_ipc_recv` pointers return STATUS_BAD_BUFFER
Sender calls `sys_ipc_recv` and is rejected for missing receive rights
Sender calls `sys_ipc_send`
Sender wakes the blocked receiver through the endpoint
Krust switches back to the receiver after sender exit
Receiver prints the delivered IPC bytes through `sys_write_serial`
Receiver calls `sys_ipc_send` and is rejected for missing send rights
Krust halts after `IPC demo ok`
```

No dynamic heap allocator, timer/APIC setup, preemption, user page-fault
recovery, full interrupt handling, full JSON parsing in the kernel, full Vertex
IR integration, filesystem, network, or device drivers are part of M11. The
kernel consumes a compact KrustBoot manifest
compiled by hosted `vertexctl`; full graph interpretation remains a userspace
responsibility. The M9 syscall path still rejects the expected bad-pointer tests
before CPU faults, and the IDT handlers provide a defined serial-log-and-halt
path for unexpected `#UD`, `#GP`, and `#PF`.

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
`user/ipc/target/x86_64-unknown-none/debug/krust-ipc-sender`, and
`user/ipc/target/x86_64-unknown-none/debug/krust-ipc-receiver`.

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
KrustBoot boot modules: 2
KrustBoot processes: 2
KrustBoot endpoints: 1
KrustBoot grants: 2
  grant[0] process=ipc-sender cap[0] endpoint=demo-ipc rights=send
  grant[1] process=ipc-receiver cap[0] endpoint=demo-ipc rights=receive
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
Process table entries: 2
Endpoint table entries: 1
endpoint[0] id=1 name=demo-ipc
process[0] id=1 name=ipc-sender state=ready
process[1] id=2 name=ipc-receiver state=running
proc=ipc-sender cap[0] endpoint=1 rights=send
proc=ipc-receiver cap[0] endpoint=1 rights=receive
GDT initialized
IDT initialized: #UD #GP #PF
Syscall path initialized
Entering userspace process: ipc-receiver
Userspace sys_write_serial: ipc receiver started
IPC receive blocked: proc=ipc-receiver endpoint=1
Scheduler switch: from=ipc-receiver to=ipc-sender
Userspace sys_write_serial: ipc sender started
Bad pointer test: SYS_WRITE_SERIAL returned STATUS_BAD_BUFFER
IPC negative test: ipc-sender receive rejected: bad capability
Bad pointer test: SYS_IPC_SEND returned STATUS_BAD_BUFFER
IPC send accepted: endpoint=1 bytes=14
IPC receive delivered: endpoint=1 bytes=14
IPC wake receiver: proc=ipc-receiver endpoint=1
Userspace sys_write_serial: ipc sender sent message
Scheduler switch: from=ipc-sender to=ipc-receiver
Userspace sys_write_serial: ipc receiver received message
Userspace sys_write_serial: Krust IPC ping
Bad pointer test: SYS_IPC_RECV returned STATUS_BAD_BUFFER
IPC negative test: ipc-receiver send rejected: bad capability
IPC demo ok
```

QEMU runs with `-display none`, so all kernel output is written through the
serial console. Interrupt QEMU with `Ctrl-C`.

## Smoke Test

```sh
make smoke
```

The smoke test boots QEMU headlessly, captures serial output to
`build/serial.log`, and passes when it sees the M11 boot transcript. The same
check is available from the repository root:

```sh
scripts/krust-smoke.sh
```

The expected transcript includes:

```text
Krust Kernel booted
Limine memory map entries:
KrustBoot manifest generation: gen:hello-0001
KrustBoot processes: 2
KrustBoot endpoints: 1
KrustBoot grants: 2
grant[0] process=ipc-sender cap[0] endpoint=demo-ipc rights=send
grant[1] process=ipc-receiver cap[0] endpoint=demo-ipc rights=receive
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
IDT initialized: #UD #GP #PF
Process table entries: 2
Endpoint table entries: 1
endpoint[0] id=1 name=demo-ipc
process[0] id=1 name=ipc-sender state=ready
process[1] id=2 name=ipc-receiver state=running
proc=ipc-sender cap[0] endpoint=1 rights=send
proc=ipc-receiver cap[0] endpoint=1 rights=receive
IPC receive blocked: proc=ipc-receiver endpoint=1
Scheduler switch: from=ipc-receiver to=ipc-sender
Scheduler switch: from=ipc-sender to=ipc-receiver
IPC wake receiver: proc=ipc-receiver endpoint=1
Bad pointer test: SYS_WRITE_SERIAL returned STATUS_BAD_BUFFER
Bad pointer test: SYS_IPC_SEND returned STATUS_BAD_BUFFER
Bad pointer test: SYS_IPC_RECV returned STATUS_BAD_BUFFER
IPC send accepted: endpoint=1 bytes=14
IPC receive delivered: endpoint=1 bytes=14
IPC negative test: ipc-sender receive rejected: bad capability
IPC negative test: ipc-receiver send rejected: bad capability
Krust IPC ping
IPC demo ok
```

## Machine Notes

Linux x86_64 can add KVM later with QEMU flags such as `-enable-kvm -cpu host`.
That is not enabled by default so the M11 command stays portable:

```sh
QEMU_EXTRA="-enable-kvm -cpu host" make smoke
```

macOS Apple Silicon can run `qemu-system-x86_64` by emulation. It is slower than
native AArch64 virtualization, but it is enough for the M11 serial milestone.
