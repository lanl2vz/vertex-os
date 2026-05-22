# Krust Milestones

This file tracks the native Krust path from the first QEMU boot to a real
native Vertex boot. The hosted Linux prototype remains the reference for Vertex
IR and graph semantics; Krust is the native enforcement path.

## Status Summary

Current status: M12 is implemented and smoke-tested under
`qemu-system-x86_64` with Limine.

```sh
scripts/krust-smoke.sh
```

## M0: Serial Boot

Status: done.

Goal: QEMU boots a Limine ISO, Limine loads `krust.elf`, Krust enters 64-bit
Rust code, prints `Krust Kernel booted` to COM1 serial, then halts.

This proved the repo can build and boot freestanding Rust code without relying
on a Linux kernel.

## M1: Boot Information

Status: done.

Goal: read Limine boot information and print the memory map to serial.

This proved Krust can consume bootloader-provided machine information.

## M2: Boot Manifest Module

Status: done.

Goal: load an initial Vertex-oriented boot module and prove Krust can find it
from Limine module metadata.

This was intentionally lightweight and preceded the compact KrustBoot manifest.

## M3: Physical Memory Allocator

Status: done.

Goal: parse the Limine memory map, track usable 4 KiB frames, allocate/free
frames, and print allocator stats.

## M4: Virtual Memory Basics

Status: done.

Goal: walk x86_64 page tables through Limine's HHDM, map a fixed kernel heap
range, and verify read/write access through virtual mappings.

## M5: Boot Capability Table

Status: done.

Goal: create a kernel object table and boot capability table containing
`MemoryObject`, `IpcEndpoint`, `Process`, `Thread`, and `BootModule` shaped
objects.

This established the kernel-side vocabulary for explicit authority.

## M6: First Userspace Process

Status: done.

Goal: load a tiny static ELF from a boot module, create a userspace address
space, enter ring 3, and let userspace call `sys_write_serial`.

## M7: First IPC Capability Demo

Status: done.

Goal: load two userspace ELF modules, grant endpoint capabilities, send a
message from one process, receive it in another, and print the message through
the syscall path.

## M8: Runtime Capability Enforcement

Status: done.

Goal: make syscalls consult the current process's real process-local capability
table instead of hardcoded sender/receiver identity checks.

Acceptance evidence in smoke:

```text
proc=ipc-sender cap[0] endpoint=1 rights=send
proc=ipc-receiver cap[0] endpoint=1 rights=receive
IPC negative test: ipc-sender receive rejected: bad capability
IPC negative test: ipc-receiver send rejected: bad capability
```

## M9: Exceptions And Safe User Memory

Status: done.

Goal: install a minimal IDT for `#UD`, `#GP`, and `#PF`, and make syscall user
copies validate low-half canonical ranges, present user pages, and write
permissions before copying.

Acceptance evidence in smoke:

```text
IDT initialized: #UD #GP #PF
Bad pointer test: SYS_WRITE_SERIAL returned STATUS_BAD_BUFFER
Bad pointer test: SYS_IPC_SEND returned STATUS_BAD_BUFFER
Bad pointer test: SYS_IPC_RECV returned STATUS_BAD_BUFFER
```

## M10: Compact KrustBoot Manifest

Status: done.

Goal: keep full JSON/graph interpretation out of the kernel. Hosted
`vertexctl compile-boot-manifest` compiles `hello-generation.vertex.json` into
`hello-generation.krustboot`, and Krust uses that compact manifest to create
boot modules, processes, endpoints, and grants.

## M11: Tiny Cooperative Scheduler

Status: done.

Goal: replace the hardcoded sender-exit-starts-receiver path with a fixed-size
process table, process states, round-robin selection, and blocking IPC receive.

Current process states:

```text
Ready
Running
BlockedOnEndpoint
Exited
```

Acceptance evidence in smoke:

```text
IPC receive blocked: proc=ipc-receiver endpoint=1
Scheduler switch: from=ipc-receiver to=ipc-sender
IPC wake receiver: proc=ipc-receiver endpoint=1
Scheduler switch: from=ipc-sender to=ipc-receiver
IPC demo ok
```

## M12: Native vertex-init

Status: done.

Goal: replace the sender/receiver demo with a native `vertex-init` userspace
program.

Target boot flow:

```text
Limine
  -> Krust
  -> Krust reads KrustBootManifest
  -> Krust creates boot caps
  -> Krust loads vertex-init ELF
  -> vertex-init receives boot capabilities
  -> vertex-init activates a tiny generation
```

Initial `vertex-init` capability shape:

```text
cap[0] = manifest module read
cap[1] = serial/log endpoint
cap[2] = process-control authority, temporary
```

Acceptance evidence in smoke:

```text
KrustBoot boot modules: 1
process[0] name=vertex-init module=vertex-init initial=yes
proc=vertex-init cap[0] boot-module=krustboot-manifest rights=read
proc=vertex-init cap[1] endpoint=1 rights=send
proc=vertex-init cap[2] process-control=process-control rights=control
vertex-init manifest generation: gen:hello-0001
Krust process authority accepted: proc=vertex-init generation=gen:hello-0001
vertex-init activated generation: gen:hello-0001
Native vertex-init boot ok
```

M12 proves the first native Vertex OS boot where Krust enforces boot authority
and native `vertex-init` activates a compact generation from the manifest. Full
service spawning is deliberately left for the next milestone.

Non-goals for M12: filesystems, networking, GPU, USB, timer preemption,
multicore, POSIX compatibility, the Nix store, and the Haskell/typed DSL.
