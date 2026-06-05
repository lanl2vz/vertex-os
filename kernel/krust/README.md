# Krust Kernel

Krust now covers the M14-M81 native graph-activation proof path, substrate
hardening, reproducible build environment, directed IPC ABI v1, and native
console shell plus virtio device I/O, VertexDisk v1 persistence, native boot
selection, verified store objects, native update transactions, and store-loaded
service executables, dynamic process creation, native config and secret
authority, package/link/build import boundaries, the first appliance
transcript, first-class native driver objects, capability namespaces,
policy/typed generation compilation, storage durability, network boundaries,
supervisor lifecycle semantics, the supported appliance release profile, owned
frame reclamation, address-space teardown, failure-atomic kernel object
creation, memory lifecycle soak gates, interrupt routing, DMA ownership,
virtio recovery, device-fault isolation, VFS root authority, service-local
mount roots, service-backed state-volume VFS transactions, and kernel-owned
open-file handles, directory metadata operations, and bounded block-cache
writeback, plus image-backed VertexFS journal checkpoint recovery and
mount-namespace gates, including the current read-only `servicefs`
request/reply file route, advisory byte-range locks, directory watch events,
VFS poll readiness, bounded pipe buffering, revocation checks for live file
authority, and the current VFS security/soak gate. M44-M81 are tracked in
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
Limine loads the native VertexDisk image plus compact generation manifests
Krust resolves each declared process executable from the native store, verifies its BLAKE3 identity and checksum, and then loads it into a fresh low-half address space
Krust creates a runtime endpoint table and one initial runtime process from the KrustBoot manifest
Krust records non-initial KrustBoot process records as templates and allocates runtime process IDs only through SYS_PROCESS_CREATE
Krust grants vertex-init cap[0] read rights to the manifest module
Krust grants vertex-init cap[1] send rights to the serial-log endpoint
Krust grants vertex-init cap[2] process-control authority with control, allocate, delegate, revoke, inspect, create, start, kill, and wait rights; cap[3] readiness receive authority; cap[30] monotonic timer control for supervised restart backoff; and per-endpoint attenuable endpoint authority starting at cap[4]
Krust applies I/O port, IRQ, DMA, virtio-device, network-port, namespace, VFS-root, store-object, config, secret, endpoint, and service timer authority when a service is dynamically created from a matching template
Krust installs a minimal IDT for #UD, #GP, #PF, and PIT IRQ0
Krust installs a TSS-backed ring-0 interrupt stack for user traps
Krust programs the PIT/PIC timer path and preempts CPU-bound userspace
Krust enters ring 3 at the initial process entry point
Krust tracks Declared, Ready, Running, BlockedOnEndpoint, Sleeping, and Exited process states
Krust validates syscall user buffers by walking user page tables
vertex-init reads the compact manifest through cap[0]
vertex-init logs through cap[1]
vertex-init computes a manifest-driven activation order, creates services dynamically through cap[2], starts them by pid, and waits for exits by pid
vertex-init proves endpoint quota enforcement and quota delegation bounds
logd reports readiness before echo starts
serial-driver, block-driver, vertex-store, vertex-state, and logd report
readiness before dependent clients start
vertex-init derives and transfers attenuated endpoint authority
request endpoints use a fixed FIFO, providers hold receive-only endpoint caps,
consumers hold send-only caps, and replies use private reply endpoints
echo sends one message to logd through an explicit IPC capability
echo inspects, copies, moves, and revokes delegated authority
echo drops its endpoint capability and denied authority stays rejected
serial-driver writes COM1 through its own I/O port capability
block-driver owns virtio-blk PCI I/O, IRQ, DMA, PCI-device, and virtio-device authority and serves separate VertexDisk block IPC endpoints for store and state
serial-driver owns virtio-console authority, and netstack owns virtio-rng plus virtio-net authority
echo sends a UDP probe through a network-port capability and proves it cannot use the raw virtio-net device
echo and reader-service receive different namespace capabilities for object aliasing; echo receives separate VFS-root caps for filesystem paths
echo runs with process mount root /state and proves /a resolves as /state/a without accepting the old process-record format
model-reader reads an immutable object through vertex-store and VertexDisk/block-driver
counter-service and reader-service access mutable state through vertex-state persisted on VertexDisk
vertex-inspect reads the generation graph and asks the kernel for a process/capability graph through inspect-only authority
logd reads immutable config and secret caps, echo proves those objects stay inaccessible without explicit grants, and runtime inspection redacts secret values
the native boot manager records selected, previous, known-good, last-failed, and boot-attempt state for generation fallback
native update transactions verify manifest/store closure before committing selected_generation
console-driver owns COM1 in the M41 generation, and console-shell prints runtime-inspect backed commands through directed console request/reply IPC
timer-service sleeps through its own timer capability without monopolizing the scheduler
cpu-hog proves a CPU-bound userspace loop cannot starve logd
faulty-service proves a direct userspace page fault kills only that process and can be restarted
echo proves bounded restart=always with delegated endpoint authority restored, and flaky-service proves restart=on-failure from a freshly loaded address space
Krust reclaims exited process address spaces, owned dynamic endpoints, and process page-table frames
runtime inspect reports frame owner counts, reclaimed/high-water counters, live cap/object counts, unreachable object counts, and reaped exited pids with no live CR3
logd receives the message and denial tests reject missing authority
Krust halts after `Native service activation ok`
```

No unbounded general-purpose allocator, APIC-backed timer, advanced fault
delivery, full interrupt handling, full JSON parsing in the kernel, full TCP/IP
stack, or filesystems are part of this native proof. Timer deadlines
now wake through the PIT interrupt path, bad userspace page faults are contained
as process failures, and hardware-shaped authority is exposed only through
explicit capability objects. The kernel consumes a compact KrustBoot Manifest v1
artifact compiled by host-side `vertexctl`; graph interpretation and lifecycle
policy remain a userspace responsibility inside the standalone Krust system.

## Prerequisites

M39 pins exact host tool versions. See `../../docs/krust-toolchain.md` for the
full toolchain contract.

- `rustc 1.95.0` with the `x86_64-unknown-none` target.
- `cargo 1.95.0`.
- `rustfmt 1.9.0-stable`.
- `qemu-system-x86_64 11.0.0`.
- `limine 12.3.0`.
- `xorriso 1.5.8.pl01`.
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

On macOS with Homebrew, install the pinned tools or set `QEMU`, `LIMINE`,
`XORRISO`, and `LIMINE_DIR` to exact-version binaries/assets:

```sh
brew install limine xorriso qemu
make doctor
```

On Linux, install the same tools through the host distribution package manager
and run:

```sh
make doctor
```

`make doctor` prints the resolved Rust, Cargo, rustfmt, QEMU, Limine, xorriso,
and Limine asset paths. It rejects wrong versions, missing Cargo lockfiles, and
a missing `x86_64-unknown-none` Rust target with concrete fix commands.

## Build

```sh
make build
```

This builds `target/x86_64-unknown-none/debug/krust` and the supported native
user programs under `user/*/target/x86_64-unknown-none/debug/`: `vertex-init`,
`serial-driver`, `logd`, `echo`, `netstack`, `block-driver`, `vertex-store`,
`vertex-state`, `vertex-inspect`, `model-reader`, `counter`, `state-reader`,
`timer`, `flaky`, `cpu-hog`, and `faulty-service`.

## Build ISO

```sh
make iso
```

This runs `vertexctl compile-boot-manifest`, writes
`build/hello-generation.krustboot`, `build/fallback-generation.krustboot`, and
`build/bad-generation.krustboot`, then creates `build/krust.iso`. By default
the fallback and bad-generation manifests are the same generation; pass
`FALLBACK_MANIFEST=/path/to/previous.vertex.json` and
`BAD_GENERATION_MANIFEST=/path/to/bad.vertex.json` to package explicit
generation-switch rollback inputs.

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
KrustBoot endpoints: 10
KrustBoot grants: 65
KrustBoot store objects: 14
KrustBoot state volumes: 1
  state_volume[0] id=state:counter
KrustBoot network ports: 1
KrustBoot io port ranges: 3
KrustBoot mmio regions: 0
KrustBoot interrupt lines: 1
KrustBoot dma regions: 1
KrustBoot pci devices: 4
KrustBoot virtio devices: 4
KrustBoot namespaces: 2
KrustBoot vfs roots: 8
  grant[0] process=vertex-init cap[1] endpoint=serial-log rights=send
  grant[11] process=logd cap[0] endpoint=log-sink rights=receive
  grant[12] process=vertex-init cap[4] endpoint=log-sink rights=send
  grant[13] process=echo cap[3] network-port=cap:net.udp.9000 rights=bind|listen
  grant[...] process=block-driver cap[6] io-port=cap:io.pci-config rights=read|write
  grant[...] process=block-driver cap[7] interrupt-line=cap:irq.virtio-blk0 rights=listen
  grant[...] process=block-driver cap[8] dma-region=cap:dma.virtio-blk0 rights=read|write|map
  grant[...] process=block-driver cap[9] io-port=cap:io.virtio-blk0 rights=read|write
  grant[...] process=block-driver cap[10] vfs-root=cap:vfs.block-dev-blk0 rights=read|resolve
  grant[...] process=block-driver cap[11] pci-device=device:virtio-blk0 rights=control
  grant[...] process=block-driver cap[12] virtio-device=device:virtio-blk0 rights=control
  grant[...] process=serial-driver cap[5] virtio-device=device:virtio-console0 rights=control
  grant[...] process=netstack cap[3] virtio-device=device:virtio-rng0 rights=control
  grant[...] process=netstack cap[5] virtio-device=device:virtio-net0 rights=control
  grant[...] process=netstack cap[6] network-port=cap:net.udp.9000 rights=control
  grant[...] process=echo cap[4] namespace=cap:namespace.echo rights=resolve
  grant[...] process=echo cap[5] vfs-root=cap:vfs.echo-state-a rights=read|resolve
  grant[...] process=echo cap[6] vfs-root=cap:vfs.echo-state-writer rights=read|write|resolve|create|unlink|rename|mount
  grant[...] process=echo cap[7] vfs-root=cap:vfs.echo-state-control rights=control|resolve
  grant[42] process=timer-service cap[0] timer=monotonic-timer rights=control
io_port[1] id=cap:io.pci-config base=0x0000000000000cf8 length=0x0000000000000008
io_port[2] id=cap:io.virtio-blk0 base=0x000000000000c000 length=0x0000000000001000
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
Kernel heap arena allocation ok
Typed endpoint arena created 32 endpoints
Typed process arena created 32 processes
Process table entries: 1
Endpoint table entries: 12
endpoint[0] id=1 name=serial-log
endpoint[1] id=2 name=readiness
endpoint[2] id=3 name=serial-console
endpoint[3] id=4 name=log-sink
endpoint[4] id=5 name=vertex-store-block-request
endpoint[5] id=6 name=vertex-state-block-request
endpoint[6] id=7 name=vertex-store-block-reply
endpoint[7] id=8 name=vertex-state-block-reply
endpoint[8] id=9 name=store-hello-text-request
endpoint[9] id=10 name=model-reader-store-reply
endpoint[10] id=11 name=state-vfs-request
endpoint[11] id=12 name=state-vfs-reply
process[0] id=1 name=vertex-init state=running
proc=vertex-init cap[0] boot-module=krustboot-manifest rights=read
proc=vertex-init cap[1] endpoint=serial-log rights=send
proc=vertex-init cap[2] process-control=process-control rights=control|allocate|delegate|revoke|inspect|create|start|kill|wait
proc=vertex-init cap[3] endpoint=readiness rights=receive
proc=vertex-init cap[4] endpoint=log-sink rights=send
proc=vertex-init cap[30] timer=monotonic-timer rights=control
proc=serial-driver cap[3] io-port=cap:io.com1 rights=read|write
proc=block-driver cap[6] io-port=cap:io.pci-config rights=read|write
proc=block-driver cap[7] interrupt-line=cap:irq.virtio-blk0 rights=listen
proc=block-driver cap[8] dma-region=cap:dma.virtio-blk0 rights=read|write|map
proc=block-driver cap[9] io-port=cap:io.virtio-blk0 rights=read|write
proc=block-driver cap[10] vfs-root=cap:vfs.block-dev-blk0 root=/dev/device:virtio-blk0 rights=read|resolve
proc=block-driver cap[11] pci-device=device:virtio-blk0 kind=virtio-blk-pci rights=control
proc=block-driver cap[12] virtio-device=device:virtio-blk0 transport=virtio-pci-io rights=control
proc=logd cap[0] endpoint=log-sink rights=receive
proc=model-reader cap[0] endpoint=store-hello-text-request rights=send
proc=model-reader cap[4] vfs-root=cap:vfs.model-reader-vertexfs root=/fs/app rights=read|write|create|resolve
proc=counter-service cap[0] vfs-root=cap:vfs.counter-state root=/state/counter rights=read|write|resolve
proc=reader-service cap[0] vfs-root=cap:vfs.state-reader-state root=/state/counter rights=read|resolve
proc=echo cap[7] vfs-root=cap:vfs.echo-state-control root=/state/counter/control rights=control|resolve
proc=timer-service cap[0] timer=monotonic-timer rights=control
GDT initialized
IDT initialized: #UD #GP #PF IRQ0
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
vertex-init endpoints: 12
vertex-init grants: 65
vertex-init network ports: 1
vertex-init store objects: 14
vertex-init state volumes: 1
vertex-init io ports: 3
vertex-init mmio regions: 0
vertex-init interrupt lines: 1
vertex-init dma regions: 1
vertex-init pci devices: 4
vertex-init virtio devices: 4
vertex-init namespaces: 2
vertex-init vfs roots: 4
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
Capability transfer accepted: proc=vertex-init target=echo
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
echo VFS open rejected: permission
echo send after drop rejected
negative test: logd process-create rejected: bad capability
virtio-blk PCI device discovered
DMA map accepted: proc=block-driver dma-region=cap:dma.virtio-blk0
virtio-blk driver ready
block-driver reads sector 0
block-driver writes test sector
readback matches
QEMU boots with VertexDisk image attached
VertexDisk superblock accepted
VFS state volume mounted: state=state:counter path=/state/counter source=vertex-state
VFS state volume value file mounted: state=state:counter path=/state/counter/value source=vertex-state
VFS state volume control file mounted: state=state:counter path=/state/counter/control source=vertex-state
store-service requests block read
block-driver received block-read request
block-driver returns bytes
vertex-store reads object index from disk
vertex-store verifies hash
Native immutable store service ok
counter-service writes state through VFS
vertex-state reads state volume from disk
VFS state transaction request: proc=echo state=state:counter op=write file=value
VFS state transaction wake: proc=echo file=value op=write result=2
vertex-state serves VFS state write
VFS state transaction request: proc=echo state=state:counter op=read file=value
VFS state transaction wake: proc=echo file=value op=read result=2
vertex-state serves VFS state read
mounted state volume value uses VFS service transaction
VFS state transaction request: proc=echo state=state:counter op=stat file=value
VFS state transaction wake: proc=echo file=value op=stat result=64
vertex-state serves VFS state stat
service-backed state value stat reports durable length
vertex-state writes state volume to disk
reader-service reads state
reader-service write rejected
VFS state transaction request: proc=reader-service state=state:counter op=control file=control
VFS state transaction wake: proc=reader-service file=control op=control result=1
state restored
Native state service client ok
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
flaky-service creates quota-backed endpoint
vertex-init observes failure
restart policy = on-failure
restart backoff sleep elapsed
vertex-init restarts flaky-service once
Krust process restart reload: proc=flaky-service
Krust process restart restores quota baseline: proc=flaky-service
flaky-service restart quota restored
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
`build/serial.log`, and passes when it sees the M14-M81 directed IPC, console,
virtio-block, VertexDisk, verified store, update, store-executable, dynamic
process, config, secret, package-boundary, appliance, virtio device, networking,
namespace, policy, ABI-hardening, storage durability, network-boundary, and
lifecycle, memory-lifecycle, soak, interrupt, DMA, virtio recovery, and
device-fault, VFS coordination, and filesystem security transcripts. The same
check is available from the repository root:

```sh
scripts/krust-smoke.sh
```

## M26-M81 Substrate Gate

Run the clean-clone gate from the repository root:

```sh
scripts/krust-release-gate.sh
```

Or from this directory:

```sh
make release-gate
```

The gate checks script executability and shell syntax, verifies Makefile recipe
parsing, checks Rust formatting and Markdown whitespace, confirms the M14-M81
documentation anchors, checks the pinned M39 toolchain and Cargo lockfiles, runs
`cargo metadata --locked --offline` and `cargo build --locked --offline`,
validates `examples/hello-generation.vertex.json`, runs `make doctor`, rebuilds
from `make clean`, runs `make smoke`, checks package/link/build import commands,
and then runs the M14-M81 QEMU cases:
`m14`,
`manifest-cycle`, `bad-cap`, `readiness-timeout`, `rollback`, `store-state-services`,
`timer`, `preemption`, `user-fault`, `restart`, `manifest-v1`, `cap-lifecycle`,
`typed-arenas`, `quotas`, `m32`, `m33`, `m34`, `m35`, `m36`, `m37`, `m38`, `m40`,
`m41`, `m42`, `m42-driver-fault`, `m43`, `m43-bad-superblock`, `m44`, `m45`,
`m46`, `m47`, `m47-corrupt-executable`, `m48`, `m49`,
`m49-config-corrupt`, `m50`, `m54`, `m55`, `m56`, `m57`, `m59`, `m60`, `m61`,
`m62`, `m62-journal-replay`, `m62-corrupt-journal`, `m63`, `m64`, `m66`,
`m67`, `m68`, `m69`, `m70`, `m71`, `m72`, `m73`, `m75`, `m76`, `m77`, `m78`,
`m78-bad-superblock`, `m78-journal-replay`,
`m78-journal-checkpoint-after-journal`, `m78-journal-checkpoint-after-data`,
`m78-journal-checkpoint-after-inode`, `m78-post-sync-remount`,
`m78-fsync-fault`, `m79`, `m80`, `m81`, and the malformed-manifest cases. If the offline
build fails, the gate prints the Cargo cache or vendoring prerequisite
explicitly.

The expected transcript includes:

```text
Krust Kernel booted
Limine memory map entries:
KrustBoot manifest generation: gen:hello-0001
KrustBoot Manifest v1 records: 9
KrustBoot boot modules: 13
KrustBoot processes: 13
KrustBoot endpoints: 10
KrustBoot grants: 65
KrustBoot store objects: 14
KrustBoot network ports: 1
KrustBoot io port ranges: 3
KrustBoot mmio regions: 0
KrustBoot interrupt lines: 1
KrustBoot dma regions: 1
KrustBoot pci devices: 4
KrustBoot virtio devices: 4
KrustBoot namespaces: 2
KrustBoot vfs roots: 8
grant[0] process=vertex-init cap[1] endpoint=serial-log rights=send
grant[11] process=logd cap[0] endpoint=log-sink rights=receive
grant[12] process=vertex-init cap[4] endpoint=log-sink rights=send
grant[13] process=echo cap[3] network-port=cap:net.udp.9000 rights=bind|listen
grant[...] process=serial-driver cap[3] io-port=cap:io.com1 rights=read|write
grant[...] process=block-driver cap[6] io-port=cap:io.pci-config rights=read|write
grant[...] process=block-driver cap[7] interrupt-line=cap:irq.virtio-blk0 rights=listen
grant[...] process=block-driver cap[8] dma-region=cap:dma.virtio-blk0 rights=read|write|map
grant[...] process=block-driver cap[9] io-port=cap:io.virtio-blk0 rights=read|write
grant[...] process=block-driver cap[10] vfs-root=cap:vfs.block-dev-blk0 rights=read|resolve
grant[...] process=block-driver cap[11] pci-device=device:virtio-blk0 rights=control
grant[...] process=block-driver cap[12] virtio-device=device:virtio-blk0 rights=control
grant[...] process=serial-driver cap[5] virtio-device=device:virtio-console0 rights=control
grant[...] process=netstack cap[3] virtio-device=device:virtio-rng0 rights=control
grant[...] process=netstack cap[5] virtio-device=device:virtio-net0 rights=control
grant[...] process=netstack cap[6] network-port=cap:net.udp.9000 rights=control
grant[...] process=echo cap[4] namespace=cap:namespace.echo rights=resolve
grant[...] process=echo cap[5] vfs-root=cap:vfs.echo-state-a rights=read|resolve
grant[...] process=echo cap[6] vfs-root=cap:vfs.echo-state-writer rights=read|write|resolve|create|unlink|rename|mount
grant[...] process=echo cap[7] vfs-root=cap:vfs.echo-state-control rights=control|resolve
network_port[0] id=cap:net.udp.9000
io_port[1] id=cap:io.pci-config base=0x0000000000000cf8 length=0x0000000000000008
io_port[2] id=cap:io.virtio-blk0 base=0x000000000000c000 length=0x0000000000001000
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
Kernel heap arena allocation ok
Typed endpoint arena created 32 endpoints
Typed process arena created 32 processes
IDT initialized: #UD #GP #PF IRQ0
Process table entries: 1
Endpoint table entries: 12
endpoint[0] id=1 name=serial-log
endpoint[1] id=2 name=readiness
endpoint[2] id=3 name=log-sink
endpoint[10] id=11 name=state-vfs-request
endpoint[11] id=12 name=state-vfs-reply
process[0] id=1 name=vertex-init state=running
proc=vertex-init cap[0] boot-module=krustboot-manifest rights=read
proc=vertex-init cap[1] endpoint=serial-log rights=send
proc=vertex-init cap[2] process-control=process-control rights=control|allocate|delegate|revoke|inspect|create|start|kill|wait
proc=vertex-init cap[3] endpoint=readiness rights=receive
proc=vertex-init cap[4] endpoint=log-sink rights=send
Native VFS state request grant: process=vertex-state endpoint=state-vfs-request rights=receive
Native VFS state reply grant: process=vertex-state endpoint=state-vfs-reply rights=send
proc=vertex-state cap[6] endpoint=state-vfs-reply rights=send
proc=vertex-state cap[7] endpoint=state-vfs-request rights=receive
Krust process create accepted: proc=vertex-init target=logd
vertex-init dynamically created service: logd
proc=logd cap[0] endpoint=log-sink rights=receive
proc=logd cap[4] vfs-root=cap:vfs.logd-log-stream root=/proc/log-stream rights=read|resolve
proc=logd cap[5] config=config:logd rights=read
proc=logd cap[6] secret=secret:logd-token value=<redacted> rights=read|inspect-metadata
VFS open accepted: proc=logd file=log-stream
VFS read blocked: proc=logd
VFS pipe wake reader: proc=logd file=log-stream
VFS pipe read blocks until writer log
proc=serial-driver cap[3] io-port=cap:io.com1 rights=read|write
proc=block-driver cap[6] io-port=cap:io.pci-config rights=read|write
proc=block-driver cap[7] interrupt-line=cap:irq.virtio-blk0 rights=listen
proc=block-driver cap[8] dma-region=cap:dma.virtio-blk0 base=
proc=block-driver cap[9] io-port=cap:io.virtio-blk0 rights=read|write
proc=block-driver cap[10] vfs-root=cap:vfs.block-dev-blk0 root=/dev/device:virtio-blk0 rights=read|resolve
proc=block-driver cap[11] pci-device=device:virtio-blk0 kind=virtio-blk-pci rights=control
proc=block-driver cap[12] virtio-device=device:virtio-blk0 transport=virtio-pci-io rights=control
proc=vertex-init cap[30] timer=monotonic-timer rights=control
proc=model-reader cap[0] endpoint=store-hello-text-request rights=send
proc=model-reader cap[4] vfs-root=cap:vfs.model-reader-vertexfs root=/fs/app rights=read|write|create|resolve
proc=counter-service cap[0] vfs-root=cap:vfs.counter-state root=/state/counter rights=read|write|resolve
proc=reader-service cap[0] vfs-root=cap:vfs.state-reader-state root=/state/counter rights=read|resolve
proc=echo cap[7] vfs-root=cap:vfs.echo-state-control root=/state/counter/control rights=control|resolve
proc=timer-service cap[0] timer=monotonic-timer rights=control
vertex-init started
Boot module read accepted: proc=vertex-init module=krustboot-manifest bytes=
vertex-init manifest generation: gen:hello-0001
vertex-init network ports: 1
vertex-init io ports: 3
vertex-init mmio regions: 0
vertex-init interrupt lines: 1
vertex-init dma regions: 1
vertex-init pci devices: 4
vertex-init virtio devices: 4
vertex-init namespaces: 2
vertex-init vfs roots: 4
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
unauthorized service cannot access PCI I/O, IRQ, or DMA capabilities
Native driver framework ok
virtio-console replaces raw serial shell transport
virtio-rng provides random bytes through explicit cap
virtio-net driver can receive raw frames
virtio-net driver can send raw frames
UDP send queued for netstack: proc=echo network-port=cap:net.udp.9000 bytes=13
echo submits UDP request to netstack boundary
Network-port UDP request delivered to netstack: network-port=cap:net.udp.9000 bytes=13
netstack received UDP request through network-port boundary
netstack transmitted UDP packet for network-port client
UDP send transmitted: proc=netstack network-port=cap:net.udp.9000 bytes=13
service A namespace contains /state/a
service A cannot resolve /state/b
logd received: hello from echo
negative test: echo receive rejected: bad capability
echo send after drop rejected
negative test: logd process-create rejected: bad capability
store-service requests block read
virtio-blk PCI device discovered
DMA map accepted: proc=block-driver dma-region=cap:dma.virtio-blk0
block-driver reads sector 0
block-driver writes test sector
readback matches
block-driver received block-read request
block-driver returns bytes
vertex-store verifies hash
modified object fails hash check
model-reader reads bytes
counter-service writes state through VFS
reader-service reads state
reader-service write rejected
VFS state transaction request: proc=reader-service state=state:counter op=control file=control
VFS state transaction wake: proc=reader-service file=control op=control result=1
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
flaky-service creates quota-backed endpoint
restart policy = on-failure
restart backoff sleep elapsed
vertex-init restarts flaky-service once
Krust process restart reload: proc=flaky-service
Krust process restart restores quota baseline: proc=flaky-service
flaky-service restart quota restored
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
