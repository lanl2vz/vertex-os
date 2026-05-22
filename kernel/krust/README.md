# Krust Kernel

Krust now covers the M14-M36 native graph-activation proof path and substrate
hardening. The planned M37-M40 roadmap is tracked in
`../../docs/krust-milestones.md`.

The target is intentionally small:

```text
QEMU boots a Limine ISO
Limine loads krust.elf
Krust enters 64-bit Rust code
Krust writes "Krust Kernel booted" to COM1 serial
Krust reads the Limine memory map response
Krust prints every memory map entry to serial
`vertexctl compile-boot-manifest` derives `hello-generation.krustboot` from the full Vertex IR graph
Limine loads `hello-generation.krustboot` as a boot module
Krust parses the versioned KrustBoot Manifest v1 wrapper and prints its generation ID
Krust prints the KrustBoot boot modules, processes, endpoints, grants, store objects, state volumes, network ports, and hardware capability objects
Krust builds a physical frame allocator from usable memory map entries
Krust allocates, frees, and reuses 4 KiB physical frames
Krust walks the active x86_64 page tables through Limine's HHDM
Krust maps a small fixed kernel-heap virtual range
Krust writes and reads through the mapped virtual pages
Krust allocates typed endpoint and process arenas from the kernel heap and checks capacity failure paths
Krust creates fixed kernel objects and boot capabilities
Krust prints the boot capability table
Limine loads native service ELFs for vertex-init, serial-driver, logd, netstack, block-driver, vertex-store, vertex-state, echo, model-reader, counter, state-reader, timer, flaky-service, cpu-hog, and faulty-service
Krust loads each declared process into a fresh low-half address space
Krust creates a runtime process table and endpoint table from the KrustBoot manifest
Krust allocates runtime process IDs and states from KrustBoot process records
Krust grants vertex-init cap[0] read rights to the manifest module
Krust grants vertex-init cap[1] send rights to the serial-log endpoint
Krust grants vertex-init cap[2] process-control authority with control, allocate, delegate, and revoke rights; cap[3] readiness receive authority; and per-endpoint attenuable endpoint authority starting at cap[4]
Krust grants I/O, MMIO, IRQ, DMA, store/state backend, and timer authority only to services that declare those capabilities
Krust installs a minimal IDT for #UD, #GP, #PF, and PIT IRQ0
Krust installs a TSS-backed ring-0 interrupt stack for user traps
Krust programs the PIT/PIC timer path and preempts CPU-bound userspace
Krust enters ring 3 at the initial process entry point
Krust tracks Declared, Ready, Running, BlockedOnEndpoint, Sleeping, and Exited process states
Krust validates syscall user buffers by walking user page tables
vertex-init reads the compact manifest through cap[0]
vertex-init logs through cap[1]
vertex-init computes a manifest-driven activation order and starts declared services through cap[2]
vertex-init proves endpoint quota enforcement and quota delegation bounds
logd reports readiness before echo starts
vertex-init derives and transfers attenuated endpoint authority
echo sends one message to logd through an explicit IPC capability
echo inspects, copies, moves, and revokes delegated authority
echo drops its endpoint capability and denied authority stays rejected
serial-driver writes COM1 through its own I/O port capability
block-driver owns virtio-blk shaped MMIO, IRQ, and DMA authority and serves block-read IPC
model-reader reads an immutable object through vertex-store and block-driver
counter-service and reader-service access mutable state through vertex-state
timer-service sleeps through its own timer capability without monopolizing the scheduler
cpu-hog proves a CPU-bound userspace loop cannot starve logd
faulty-service proves a direct userspace page fault kills only that process and can be restarted
echo proves bounded restart=always with delegated endpoint authority restored, and flaky-service proves restart=on-failure from a fresh restart context
logd receives the message and denial tests reject missing authority
Krust halts after `Native service activation ok`
```

No unbounded general-purpose allocator, APIC-backed timer, advanced fault
delivery, full interrupt handling, full JSON parsing in the kernel, filesystem,
network stack, real virtio queues, or filesystems are part of this native
proof. Timer deadlines now wake through the PIT interrupt path, bad userspace
page faults are contained as process failures, and hardware-shaped authority is
exposed only through explicit capability objects. The kernel consumes a compact
KrustBoot Manifest v1
artifact compiled by hosted `vertexctl`; graph interpretation and lifecycle
policy remain a userspace responsibility.

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

`make doctor` prints the resolved Rust, QEMU, Limine, xorriso, and Limine asset
paths, and checks that the `x86_64-unknown-none` Rust target is installed.

## Build

```sh
make build
```

This builds `target/x86_64-unknown-none/debug/krust` and the native user
programs under `user/*/target/x86_64-unknown-none/debug/`: `vertex-init`,
`serial-driver`, `logd`, `echo`, `netstack`, `block-driver`, `vertex-store`,
`vertex-state`, `model-reader`, `counter`, `state-reader`, `timer`, and
`flaky`.

## Build ISO

```sh
make iso
```

This runs `vertexctl compile-boot-manifest`, writes
`build/hello-generation.krustboot` plus `build/fallback-generation.krustboot`,
and creates `build/krust.iso`. By default the fallback manifest is the same
generation; pass `FALLBACK_MANIFEST=/path/to/previous.vertex.json` to package a
real fallback generation.

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
KrustBoot Manifest v1 records: 9
KrustBoot boot modules: 13
KrustBoot processes: 13
KrustBoot endpoints: 7
KrustBoot grants: 32
KrustBoot store objects: 0
KrustBoot state volumes: 1
KrustBoot network ports: 1
KrustBoot io port ranges: 1
KrustBoot mmio regions: 1
KrustBoot interrupt lines: 1
KrustBoot dma regions: 1
  grant[0] process=vertex-init cap[1] endpoint=serial-log rights=send
  grant[11] process=logd cap[0] endpoint=log-sink rights=receive
  grant[12] process=vertex-init cap[4] endpoint=log-sink rights=send|receive
  grant[13] process=echo cap[3] network-port=cap:net.tcp.8080 rights=listen
  grant[14] process=model-reader cap[0] store-object=store:hello-text rights=read
  grant[16] process=reader-service cap[0] state-volume=state:counter rights=read|snapshot|restore
  grant[17] process=timer-service cap[0] timer=monotonic-timer rights=control
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
Kernel heap arena allocation ok
Typed endpoint arena created 32 endpoints
Typed process arena created 32 processes
Process table entries: 13
Endpoint table entries: 7
endpoint[0] id=1 name=serial-log
endpoint[1] id=2 name=readiness
endpoint[2] id=3 name=log-sink
process[0] id=1 name=vertex-init state=running
process[1] id=2 name=serial-driver state=declared
process[2] id=3 name=logd state=declared
process[3] id=4 name=netstack state=declared
process[4] id=5 name=block-driver state=declared
process[5] id=6 name=vertex-store state=declared
process[6] id=7 name=vertex-state state=declared
proc=vertex-init cap[0] boot-module=krustboot-manifest rights=read
proc=vertex-init cap[1] endpoint=serial-log rights=send
proc=vertex-init cap[2] process-control=process-control rights=control|allocate|delegate|revoke
proc=vertex-init cap[3] endpoint=readiness rights=receive
proc=vertex-init cap[4] endpoint=log-sink rights=send|receive
proc=serial-driver cap[3] io-port=cap:io.com1 rights=read|write
proc=block-driver cap[3] mmio-region=cap:mmio.virtio-blk0 rights=map
proc=block-driver cap[4] interrupt-line=cap:irq.virtio-blk0 rights=listen
proc=block-driver cap[5] dma-region=cap:dma.virtio-blk0 rights=read|write|map
proc=logd cap[0] endpoint=log-sink rights=receive
proc=model-reader cap[0] endpoint=store-hello-text-api rights=send|receive
proc=reader-service cap[0] endpoint=state-counter-api rights=send|receive
proc=timer-service cap[0] timer=monotonic-timer rights=control
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
vertex-init boot modules: 13
vertex-init processes: 13
vertex-init endpoints: 7
vertex-init grants: 32
vertex-init network ports: 1
vertex-init io ports: 1
vertex-init mmio regions: 1
vertex-init interrupt lines: 1
vertex-init dma regions: 1
service with quota=1 endpoint can create one endpoint
second endpoint creation fails
init can delegate smaller quota
delegated quota cannot exceed parent quota
vertex-init activation plan:
  1. serial-driver
  2. logd
  3. netstack
  4. block-driver
  5. vertex-store
  6. vertex-state
  7. echo
  8. model-reader
  9. counter-service
  10. reader-service
  11. timer-service
  12. flaky-service
vertex-init starting service: serial-driver
serial-driver ready
vertex-init starting service: logd
Krust process start accepted: proc=vertex-init target=logd
logd ready
logd sends log message
serial-driver writes message to COM1
vertex-init observed ready: logd
vertex-init starting service: netstack
Krust process start accepted: proc=vertex-init target=netstack
vertex-init derives endpoint cap for echo from endpoint[2] rights=send
Capability derive accepted: proc=vertex-init parent=4 new=31 rights=send
Capability inspect: proc=vertex-init
Capability transfer accepted: proc=vertex-init target=echo slot=0 rights=send
vertex-init starting service: echo
Krust process start accepted: proc=vertex-init target=echo
vertex-init starting service: model-reader
vertex-init starting service: counter-service
vertex-init starting service: reader-service
vertex-init starting service: timer-service
vertex-init starting service: flaky-service
echo sent message to logd
service with no allocation authority cannot create endpoint
cap inspect shows parent chain
cap copy preserves source slot
cap move removes source slot
echo send after revoke rejected
logd received: hello from echo
negative test: echo receive rejected: bad capability
echo read rejected: bad capability
echo send after drop rejected
negative test: logd process-start rejected: bad capability
block-driver ready
store-service requests block read
block-driver returns bytes
vertex-store verifies hash
Native store-object read ok
counter-service writes state
reader-service reads state
reader-service write rejected
snapshot created
state restored
Native state-volume access ok
Timer sleep accepted: proc=timer-service timer=monotonic-timer ms=10
Timer sleep blocked: proc=timer-service
Timer wake: proc=timer-service
Native timer ok
vertex-init observes exit
restart policy = always
vertex-init restarts echo once
Krust process restart reload: proc=echo
echo restart retained delegated log cap
flaky-service exits with status 1
vertex-init observes failure
restart policy = on-failure
vertex-init restarts flaky-service once
Krust process restart reload: proc=flaky-service
flaky-service exits 0
Native restart policy ok
Native manifest-driven activation ok
Native readiness activation ok
Native service activation ok
```

QEMU runs with `-display none`, so all kernel output is written through the
serial console. Interrupt QEMU with `Ctrl-C`.

## Smoke Test

```sh
make smoke
```

The smoke test boots QEMU headlessly, captures serial output to
`build/serial.log`, and passes when it sees the M14-M36 boot transcript. The same
check is available from the repository root:

```sh
scripts/krust-smoke.sh
```

## M26-M36 Substrate Gate

Run the clean-clone gate from the repository root:

```sh
scripts/krust-release-gate.sh
```

Or from this directory:

```sh
make release-gate
```

The gate checks script executability and shell syntax, verifies Makefile recipe
parsing, checks Rust formatting and milestone Markdown whitespace, confirms the
M14-M36 documentation anchors, runs `cargo build --offline`, validates
`examples/hello-generation.vertex.json`, runs `make doctor`, rebuilds from
`make clean`, runs `make smoke`, and then runs the M14-M36 QEMU cases: `m14`,
`manifest-cycle`, `bad-cap`, `readiness-timeout`, `rollback`, `store-state`,
`timer`, `preemption`, `user-fault`, `restart`, `manifest-v1`, `cap-lifecycle`,
`typed-arenas`, `quotas`, `m32`, `m33`, `m34`, `m35`, `m36`, and the
malformed-manifest cases. If the offline build
fails, the gate prints the Cargo cache or vendoring prerequisite explicitly.

The expected transcript includes:

```text
Krust Kernel booted
Limine memory map entries:
KrustBoot manifest generation: gen:hello-0001
KrustBoot Manifest v1 records: 9
KrustBoot boot modules: 13
KrustBoot processes: 13
KrustBoot endpoints: 7
KrustBoot grants: 32
KrustBoot network ports: 1
KrustBoot io port ranges: 1
KrustBoot mmio regions: 1
KrustBoot interrupt lines: 1
KrustBoot dma regions: 1
grant[0] process=vertex-init cap[1] endpoint=serial-log rights=send
grant[11] process=logd cap[0] endpoint=log-sink rights=receive
grant[12] process=vertex-init cap[4] endpoint=log-sink rights=send|receive
grant[13] process=echo cap[3] network-port=cap:net.tcp.8080 rights=listen
grant[...] process=serial-driver cap[3] io-port=cap:io.com1 rights=read|write
grant[...] process=block-driver cap[3] mmio-region=cap:mmio.virtio-blk0 rights=map
grant[...] process=block-driver cap[4] interrupt-line=cap:irq.virtio-blk0 rights=listen
grant[...] process=block-driver cap[5] dma-region=cap:dma.virtio-blk0 rights=read|write|map
network_port[0] id=cap:net.tcp.8080
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
Kernel heap arena allocation ok
Typed endpoint arena created 32 endpoints
Typed process arena created 32 processes
IDT initialized: #UD #GP #PF
Process table entries: 13
Endpoint table entries: 7
endpoint[0] id=1 name=serial-log
endpoint[1] id=2 name=readiness
endpoint[2] id=3 name=log-sink
process[0] id=1 name=vertex-init state=running
process[1] id=2 name=serial-driver state=declared
process[2] id=3 name=logd state=declared
process[3] id=4 name=netstack state=declared
process[...] name=block-driver state=declared
process[...] name=vertex-store state=declared
process[...] name=vertex-state state=declared
proc=vertex-init cap[0] boot-module=krustboot-manifest rights=read
proc=vertex-init cap[1] endpoint=serial-log rights=send
proc=vertex-init cap[2] process-control=process-control rights=control|allocate|delegate|revoke
proc=vertex-init cap[3] endpoint=readiness rights=receive
proc=vertex-init cap[4] endpoint=log-sink rights=send|receive
proc=logd cap[0] endpoint=log-sink rights=receive
proc=serial-driver cap[3] io-port=cap:io.com1 rights=read|write
proc=model-reader cap[0] endpoint=store-hello-text-api rights=send|receive
proc=reader-service cap[0] endpoint=state-counter-api rights=send|receive
proc=timer-service cap[0] timer=monotonic-timer rights=control
vertex-init started
Boot module read accepted: proc=vertex-init module=krustboot-manifest bytes=
vertex-init manifest generation: gen:hello-0001
vertex-init network ports: 1
vertex-init io ports: 1
vertex-init mmio regions: 1
vertex-init interrupt lines: 1
vertex-init dma regions: 1
service with quota=1 endpoint can create one endpoint
second endpoint creation fails
service with no allocation authority cannot create endpoint
cap inspect shows parent chain
cap copy preserves source slot
cap move removes source slot
echo send after revoke rejected
vertex-init activation plan:
  1. serial-driver
  2. logd
  ...
  12. flaky-service
serial-driver ready
serial-driver can write byte
logd sends log message
serial-driver writes message to COM1
vertex-init starting service: logd
Krust process start accepted: proc=vertex-init target=logd
vertex-init observed ready: logd
vertex-init starting service: netstack
vertex-init starting service: echo
Krust process start accepted: proc=vertex-init target=echo
echo sent message to logd
echo I/O write rejected
unauthorized service cannot talk to block-driver
unauthorized service cannot access MMIO, IRQ, or DMA capabilities
logd received: hello from echo
negative test: echo receive rejected: bad capability
echo send after drop rejected
negative test: logd process-start rejected: bad capability
store-service requests block read
block-driver returns bytes
vertex-store verifies hash
modified object fails hash check
model-reader reads bytes
counter-service writes state
reader-service reads state
reader-service write rejected
snapshot created
state restored
Timer sleep accepted: proc=timer-service timer=monotonic-timer ms=10
Timer sleep blocked: proc=timer-service
Timer wake: proc=timer-service
vertex-init observes exit
restart policy = always
vertex-init restarts echo once
Krust process restart reload: proc=echo
echo restart retained delegated log cap
flaky-service exits with status 1
restart policy = on-failure
vertex-init restarts flaky-service once
Krust process restart reload: proc=flaky-service
flaky-service exits 0
Native manifest-driven activation ok
Native readiness activation ok
Native service activation ok
```

## Machine Notes

Linux x86_64 can add KVM later with QEMU flags such as `-enable-kvm -cpu host`.
That is not enabled by default so the Krust smoke command stays portable:

```sh
QEMU_EXTRA="-enable-kvm -cpu host" make smoke
```

macOS Apple Silicon can run `qemu-system-x86_64` by emulation. It is slower than
native AArch64 virtualization, but it is enough for the Krust serial tests.
