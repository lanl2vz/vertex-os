# Krust Milestones

This file tracks the native Krust path from the first QEMU boot to a standalone
Vertex OS appliance profile. Host-side tools and simulations remain useful for
Vertex IR development, but Krust is the native enforcement path rather than a
runtime layered over a host kernel.

## Status Summary

Current status: M14-M88 plus M90-M93 and M87-1 through M87-4 are implemented and
smoke-tested under `qemu-system-x86_64` with Limine. The current tree has an
image-backed VertexFS v2 mount, a strict VertexDisk v1 section carrying the
current VertexFS image, retained VertexFS v1 verifier/regression gates,
fixed journal replay, kernel-owned device-backed fsync transactions,
post-sync image remount, fsync fault/restart handling, a bounded kernel-owned
VertexFS vnode page cache/writeback layer, declared-file journal checkpoint
recovery, mount-namespace gates, and a read-only `servicefs`
filesystem-service route, plus VFS coordination and security/soak coverage for
locks, polls, directory watches, bounded pipe buffering, revocation with live
handles, hostile VFS arguments, and repeated file churn. M74-M77 VFS,
directory-metadata, and block-cache writeback work is now smoke-tested and
release-gate covered. M39
pins the native toolchain, M40 makes
native IPC directed, and M41 adds a native console shell path over explicit
console authority. M42 adds the first real virtio-blk sector I/O path over
PCI I/O and DMA capabilities. M43 adds the current VertexDisk v1 block-object
layout for immutable store reads, mutable state persistence, journal writeback,
and bad-superblock rejection. M44 adds native boot-manager fallback state, M45
verifies store objects by content identity, M46 performs native update
transactions, and M47 loads service executables only through verified native
store objects. M48 adds PID-based dynamic process creation, M49 adds immutable
config objects, M50 adds native secret authority, M51-M53 add package/link/build
graph CLI boundaries, M54 boots the first stateful appliance transcript, and
M55 formalizes user-space driver objects and native device ownership. M56 adds
virtio-console, virtio-rng, and virtio-net capability paths. M57 adds the first
cap-mediated network send/receive path. M58 records the POSIX compatibility
plan, M59 adds capability namespaces, and M60 adds human-readable policy and
typed prototype compilation into the existing boot path. M61 hardens the native
ABI and authority checks against hostile syscall inputs. M62 adds explicit
VertexDisk durability and storage corruption cases, M63 tightens the netstack
service boundary, M64 exposes native supervisor lifecycle semantics, and M65
defines the first supported standalone appliance release profile. M66-M69 add
owned frame accounting, address-space teardown, failure-atomic kernel object and
capability creation, and 100-cycle memory lifecycle soak gates for
create/start/exit, restart, endpoint churn, and fault/restart paths. M70-M73 add
blocking interrupt waits, DMA ownership/release accounting, virtio reset and
driver queue reporting, and the first device-fault isolation gate. M76-M77 add
bounded directory metadata operations, open-unlink lifetime, hard-link policy,
64-byte stat metadata, and a bounded VertexDisk state block cache with explicit
dirty/writeback inspection. M78-M79 add the first separate image-backed
`vertexfs` mount, per-service mount-root gate, declared bind-mount snapshot
gate, and current `servicefs` request/reply file route without treating old
state-volume paths as the new filesystem. M80-M81 add advisory byte-range
locks, metadata watches, poll readiness, bounded pipe buffering, capability
revocation checks against live handles and new opens, bad-buffer/path
rejection, VFS churn, and release-gated filesystem crash/security coverage.
M82 makes the active generation graph a native typed VertexDisk graph-store
object, records graph provenance for processes and capabilities, exposes graph
checksum/hash/object counts through runtime inspection, and rejects corrupt
compact or disk graph-store records before activation. M83 moves generation
installation, rollback, durable selected-generation metadata, and
prepare/commit/rollback recovery into the native generation-manager,
block-driver, and staged kernel runtime-build authority path. M84 adds a native
package-import service that reads compact graph fragments from immutable store
objects, verifies closure/config hashes, rejects standalone negative dependency
and excess-authority import commands without sending generation-manager install
requests, registers a graph-store-only candidate into native generation
metadata before activation, and proves canonical closure hashing is stable. M85
makes state volumes first-class policy records in the compact graph, validates
owner/schema/storage/migration/retention/sharing declarations on the host and
native boot paths, reports state policy and health through runtime inspection,
and gates generation staging on declared migration and rollback policy. M86
adds a strict hashed native policy section to the compact payload, validates
capability grants and service namespace authority before boot/runtime
activation, reports the accepted policy hash plus a bounded native denial ring
through runtime inspection, and rejects unknown policy versions or
graph-consistent policy-excess payloads.
M87 adds a native operator graph shell over the console command path, using
runtime inspect graph/policy facts to answer current-generation, generation
status, generation diff, authority delta, `why`, `who-can`, package-list,
activation-log, mark-known-good, activate, and rollback commands without
falling back to transcript guesses.
M87-1 moves the operator graph-shell command semantics and structured report
parsing into `userland/operator-shell`, a target-independent no_std Vertex
userland package. The Krust console-shell now acts as a runtime adapter for
logging, console output, generation-manager IPC, and exits.
M87-2 moves Krust-built user adapter crates into `targets/krust/user`, leaving
`kernel/krust` as kernel/substrate source and making `userland/` the home for
portable Vertex OS semantics.
M87-3 adds the repository-root Vertex OS runner, so users boot the OS with
`make run-gui` while Krust stays an implementation target selected by
`VERTEX_TARGET=krust`.
M87-4 upgrades the operator shell from a proof-oriented test prompt into a
usable discovery console with `overview`, richer `help`, real service,
capability, state, and device inventory commands, plus detail views that make
`why` and `who-can` discoverable rather than requiring the operator to know
internal IDs ahead of time.
M88 closes the first end-to-end appliance update loop: QEMU imports a package
closure, activates the new graph, marks it known-good, rejects an intentionally
bad generation through readiness failure, rolls back to the known-good graph,
and verifies package facts, state health, activation history, live graph-backed
capability provenance, and final system consistency from the operator shell.

M74-M82 are implemented as the current VFS, open-file, directory, block-cache,
VertexFS/mount-namespace, coordination, and security/soak substrate. The
current tree has a kernel VFS node graph, service-local mount roots,
per-process file handles, first-class `vfs-root` manifest authority, volatile
memory-file create/write/read/unlink/rename, mkdir/rmdir/link, monotonic stat
metadata, open-unlink final-close reaping, a live blocking `/proc/log-stream`
pipe with bounded buffering, a service-backed state-volume VFS transaction
path, a read-only `/state/service-report` `servicefs` route, advisory
whole-file and byte-range lock coverage, metadata watch events, VFS poll
readiness, and a bounded write-through block cache in `vertex-state`. The new
`/fs` mount is a distinct `vertexfs` source loaded from the boot VertexFS image
module with VertexFS-file backing, declared files, fixed journal replay, a
kernel-owned device-backed fsync transaction for declared and dynamic inodes,
create/write/read/fsync coverage, and a model-reader service rooted at
`/fs/app`. M91 makes the default boot image VertexFS v2 with expanded inode and
directory sections, a volume id, generation id, strict feature flags,
checksummed v2 metadata, and dynamic create growth beyond the v1 inode/dir
limit. The release gate now covers corrupt VertexFS images, interrupted
journals, checkpoint remount verification, fsync faults, live-handle revocation
semantics, a 100-cycle VFS churn probe, native graph-store checksum rejection,
invalid graph-record rejection, and native package closure import through
generation-manager activation/rollback, plus M85 state migration,
bad-migration rollback, sharing-policy, retention, state-health reporting, M86
native policy validation with grant and mount-root tamper rejection plus
inspectable policy-denial records, and M87 operator graph-shell commands with
policy-provenance answers plus generic generation-manager activation and
rollback. M87-1 adds the first Vertex-owned no_std userland package consumed by
the Krust image instead of owned under the kernel tree. M87-2 adds the Krust
target user adapter workspace at `targets/krust/user` and removes tracked
userspace manifests from `kernel/krust/user`. M87-3 adds top-level `make`
targets for booting, building, smoke-testing, and release-gating Vertex OS
without requiring users to invoke `make -C kernel/krust`. M87-4 makes the
native operator console usable for discovery: services and capabilities can be
listed before proof commands are run.
M88 adds the flagship appliance update transcript and verifier, including a
target-independent package-import validation core under `userland/package-import`.
M90 adds the typed filesystem-service protocol substrate after the M89 soak was
deferred: servicefs now has typed mount-source reporting, an exact read-only
mount, a v2 `FS` request envelope, provider-side v2 validation, and runtime
inspect filesystem-service health rows. M91 adds the default VertexFS v2
durable format while keeping explicit v1 creation and verifier gates available
for regression coverage. M92 moves normal create, open-create, unlink, rename,
link, mkdir/rmdir, truncate, append, stat, and watch semantics onto
VertexFS-backed VFS nodes with metadata checkpoint logs and 100-cycle churn
coverage while preserving the existing handle and authority model. General
vnode page cache integration is now implemented for VertexFS-backed files in
M93, including bounded clean eviction, dirty-page pressure rejection, ordered
fsync writeback, writeback-error accounting, sequential read-ahead counters,
and runtime inspect health rows. Broader synthetic/device filesystem breadth,
external filesystem-service shared-buffer page I/O, durable graph-store mutation, and
durable policy-denial history remain later work.

```sh
make doctor
make run-gui
scripts/krust-release-gate.sh
scripts/krust-smoke.sh
scripts/krust-test.sh manifest-cycle
scripts/krust-test.sh rollback
scripts/krust-test.sh manifest-v1
scripts/krust-test.sh cap-lifecycle
scripts/krust-test.sh typed-arenas
scripts/krust-test.sh quotas
scripts/krust-test.sh preemption
scripts/krust-test.sh user-fault
scripts/krust-test.sh m32
scripts/krust-test.sh m33
scripts/krust-test.sh m34
scripts/krust-test.sh m35
scripts/krust-test.sh m36
scripts/krust-test.sh m37
scripts/krust-test.sh m38
scripts/krust-test.sh m40
scripts/krust-test.sh m41
scripts/krust-test.sh m42
scripts/krust-test.sh m42-driver-fault
scripts/krust-test.sh m43
scripts/krust-test.sh m43-bad-superblock
scripts/krust-test.sh m44
scripts/krust-test.sh m45
scripts/krust-test.sh m46
scripts/krust-test.sh m47
scripts/krust-test.sh m48
scripts/krust-test.sh m49
scripts/krust-test.sh m49-config-corrupt
scripts/krust-test.sh m50
scripts/krust-test.sh m54
scripts/krust-test.sh m55
scripts/krust-test.sh m56
scripts/krust-test.sh m57
scripts/krust-test.sh m59
scripts/krust-test.sh m60
scripts/krust-test.sh m61
scripts/krust-test.sh m62
scripts/krust-test.sh m62-journal-replay
scripts/krust-test.sh m62-corrupt-journal
scripts/krust-test.sh m63
scripts/krust-test.sh m64
scripts/krust-test.sh m66
scripts/krust-test.sh m67
scripts/krust-test.sh m68
scripts/krust-test.sh m69
scripts/krust-test.sh m70
scripts/krust-test.sh m71
scripts/krust-test.sh m72
scripts/krust-test.sh m73
scripts/krust-test.sh m75
scripts/krust-test.sh m76
scripts/krust-test.sh m77
scripts/krust-test.sh m78
scripts/krust-test.sh m79
scripts/krust-test.sh m80
scripts/krust-test.sh m81
scripts/krust-test.sh m82
scripts/krust-test.sh manifest-graph-store-checksum
scripts/krust-test.sh manifest-graph-store-record
scripts/krust-test.sh m83
scripts/krust-test.sh m83-hostless
scripts/krust-test.sh m83-power-prepare
scripts/krust-test.sh m83-power-commit
scripts/krust-test.sh m83-power-rollback
scripts/krust-test.sh m84
scripts/krust-test.sh m85
scripts/krust-test.sh m86
scripts/krust-test.sh m87
scripts/krust-test.sh m88
scripts/krust-test.sh m90
scripts/krust-test.sh m91
scripts/krust-test.sh m92
scripts/krust-test.sh m93
scripts/krust-test.sh manifest-policy-version
scripts/krust-test.sh manifest-policy-hash
scripts/krust-test.sh manifest-policy-excess-grant
scripts/krust-test.sh manifest-policy-mount-root
scripts/krust-test.sh manifest-policy-state-root
```

Next direction: move richer state lifecycle policy, durable policy-denial
history, the appliance update loop, and the graph soak gate into first-class
Vertex OS surfaces on top of the M82 graph store, M83 generation-manager
substrate, M84 package-import path, and M87 operator graph shell while keeping
Krust small and capability-mediated.

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
space, enter ring 3, and let userspace call `SYS_LOG_WRITE` with an explicit log
capability.

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
IDT initialized: #UD #GP #PF IRQ0
Unknown userspace syscall: 1
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
Native manifest-driven activation ok
Native service activation ok
```

M12 proves the first native Vertex OS boot where Krust enforces boot authority
and native `vertex-init` activates a compact generation from the manifest. Full
service spawning is deliberately left for the next milestone.

Non-goals for M12: filesystems, networking, GPU, USB, timer preemption,
multicore, POSIX compatibility, host package stores, and the Haskell/typed DSL.

## M13: Native Service Activation

Status: done.

Goal: make native `vertex-init` start declared service processes from the
compact KrustBoot manifest instead of only proving that process-control
authority exists.

Target boot flow:

```text
Limine
  -> Krust
  -> Krust reads KrustBootManifest
  -> Krust loads vertex-init, logd, netstack, echo, and the M19-M23 service ELFs
  -> Krust records non-initial services as creation templates
  -> vertex-init reads the manifest
  -> vertex-init creates logd and starts its pid through SYS_PROCESS_START
  -> vertex-init creates netstack and other services and starts their pids through SYS_PROCESS_START
  -> vertex-init creates echo and starts its pid through SYS_PROCESS_START
  -> echo sends one message to logd
  -> denial tests prove missing authority is rejected
```

M13 extends the process states with:

```text
Declared
Ready
Running
BlockedOnEndpoint
Exited
```

Only `Ready` processes are scheduler candidates. Non-initial services loaded
from the manifest remain `Declared` until `vertex-init` starts them with
process-control authority.

Acceptance evidence in smoke:

```text
KrustBoot processes: 9
process[0] name=vertex-init module=vertex-init initial=yes
process[1] name=logd module=logd initial=no
process[2] name=netstack module=netstack initial=no
process[3] name=echo module=echo initial=no
process[1] id=2 name=logd state=declared
process[2] id=3 name=netstack state=declared
process[3] id=4 name=echo state=declared
vertex-init starting service: logd
Krust process start accepted: proc=vertex-init target=logd
vertex-init starting service: netstack
vertex-init starting service: echo
Krust process start accepted: proc=vertex-init target=echo
echo sent message to logd
logd received: hello from echo
negative test: echo receive rejected: bad capability
negative test: logd process-create rejected: bad capability
Native service activation ok
```

M13 proved that native Krust could boot a compact generation and run a tiny
capability-enforced service graph. M14-M24 build on that path with graph-derived
activation order, readiness, delegation, supervision, service-local store/state
authority, timer sleep, rollback selection, and QEMU test cases. Filesystems,
networking, and device drivers remain outside this proof.

## M14-M24 Direction

The M14-M24 strategic target is native Krust boot activation of a real
generation graph, not a hardcoded demo graph.

M13 proved:

- Krust can boot.
- `vertex-init` can run natively.
- `vertex-init` can create and start services.
- Services communicate only through granted capabilities.
- Denied authority fails.

M14-M24 prove:

- `vertex-init` reads service dependencies from the compact manifest.
- Activation order is computed from the graph.
- Readiness and failure are explicit.
- Capabilities can be delegated and attenuated.
- Processes are supervised.
- Generation switch and rollback metadata flow through the native manifest.
- Store, state, and timer authority is granted to the declaring service, not to init.

That keeps Vertex OS aligned with the core design: the running system is an
activation of a declared graph, and Krust only enforces compact authority.

## M14: Manifest-Driven Native Activation

Status: done.

Goal: make native `vertex-init` discover declared services from the compact
KrustBoot manifest, compute a valid activation order, and start services from
manifest records without special-casing service names. This replaces the older
M13 assumption that init knew it should start only `logd` and `echo`.

The compact manifest should extend the existing M10-M13 shape rather than
introducing a second native format. Additional process fields should include:

```text
process:
  name
  module
  initial
  service_id
  start_after[]
  requires_endpoint[]
  provides_endpoint[]
  health_kind
```

Native `vertex-init` should:

```text
parse manifest
build dependency graph
reject dependency cycles
reject missing providers
start services in topological order
```

Acceptance evidence:

```text
KrustBoot processes: 9
vertex-init activation plan:
  1. logd
  2. netstack
  3. echo
  4. model-reader
  5. counter-service
  6. reader-service
  7. timer-service
  8. flaky-service
vertex-init starting service: logd
vertex-init starting service: netstack
vertex-init starting service: echo
Native manifest-driven activation ok
```

Negative manifest to add:

```text
examples/krust-cycle-generation.vertex.json
```

Expected failure:

```text
vertex-init activation failed: dependency cycle
```

## M15: Native Readiness And Service Lifecycle

Status: done.

Goal: distinguish starting a service process from proving that the service is
usable. M15 adds service lifecycle semantics above the kernel process state.

Native service lifecycle states:

```text
Declared   -- process exists but is not allowed to run
Starting   -- process has been started
Ready      -- service passed readiness check
Failed     -- service exited or failed health check
Exited     -- service completed intentionally
```

Possible kernel/user ABI additions:

```text
SYS_PROCESS_WAIT
```

or a minimal event endpoint model:

```text
process-exit endpoint
readiness endpoint
```

For the first M15 version, keep readiness explicit and small:

```text
logd starts
logd sends "ready" to vertex-init
vertex-init starts echo only after logd is ready
```

Acceptance evidence:

```text
vertex-init starting service: logd
logd ready
vertex-init observed ready: logd
vertex-init starting service: echo
echo sent message to logd
Native readiness activation ok
```

This turns native activation into a service lifecycle model rather than only an
ordered process launcher.

## M16: Compile Native KrustBoot From Full Vertex IR

Status: done.

Goal: make the native boot manifest a compiled representation of the actual
Vertex graph. Today the example manifest has a smaller `krustBoot` section next
to the richer host-side graph. M16 should make `vertexctl compile-boot-manifest`
derive native KrustBoot records from services,
capabilities, providers, dependencies, policies, and lifecycle fields.

Pipeline:

```text
Vertex IR services/capabilities/policies
  -> vertexctl compile-boot-manifest
  -> KrustBoot compact manifest
  -> Krust + vertex-init native activation
```

Example compilation:

```text
"svc:logd" provides "cap:log.sink"
"svc:echo-server" requires "cap:log.sink"

compiles to:

process logd
process echo
endpoint log-sink
grant logd cap[0] receive log-sink
grant vertex-init cap[4] send log-sink
start_after echo <- logd
```

`vertex-init` derives and transfers the endpoint capability requested by each
consumer before starting that consumer, so consumers do not receive static boot
grants for delegated endpoint authority. The compact manifest records endpoint
requirements with rights, and `vertex-init` uses the matching per-endpoint
authority slot instead of assuming one hardcoded log-sink path.

Acceptance evidence:

```text
vertexctl compile-boot-manifest examples/hello-generation.vertex.json build/hello.krustboot
vertexctl explain-krustboot examples/hello-generation.vertex.json
make smoke
```

Expected explanation:

```text
svc:echo-server receives send authority to endpoint log-sink
because it requires cap:log.sink/send
and svc:logd provides cap:log.sink
```

This is the key bridge from a native proof path to one source graph that can
target both the host-side simulator and the native runtime.

## M17: Capability Derivation, Attenuation, And Transfer

Status: done.

Goal: move beyond static boot grants and support controlled authority flow.
A process should be able to derive a weaker capability from a stronger one:

```text
cap A: endpoint log-sink, rights = send | receive
derive cap B: endpoint log-sink, rights = send
```

It must never amplify authority:

```text
send cannot derive receive
read cannot derive write
```

Candidate syscalls:

```text
SYS_CAP_DERIVE(parent_slot, new_slot, rights_mask)
SYS_CAP_DROP(slot)
SYS_CAP_TRANSFER(control_slot, target_process, packed_source_target_and_rights)
```

Acceptance evidence:

```text
vertex-init derives endpoint cap for echo from endpoint[2] rights=send
echo can send
echo cannot receive
echo drops cap
echo send after drop rejected
```

This is required for future services that receive authority through the graph
rather than directly from boot.

## M18: Native Supervision And Restart Policy

Status: done.

Goal: add the smallest Vertex-native supervisor semantics. Do not build a full
service manager; implement only the restart policy subset needed to prove
native process supervision.

Policies:

```text
restart = never
restart = on-failure
restart = always
max_restarts = 1 in the ABI v0 native proof
```

Acceptance evidence:

```text
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
restart policy = always
vertex-init restarts echo once
Krust process restart reload: proc=echo
echo restart retained delegated log cap
Native restart policy ok
```

Negative evidence:

```text
restart = never
service exits 1
activation fails
```

This turns `vertex-init` from a launcher into a minimal generation supervisor.

## M19: Native Store-Object Read Capability

Status: done.

Goal: introduce immutable store objects without implementing a filesystem.
Limine can load store objects as boot modules; Krust can expose them as
boot-module-backed store capabilities.

Compact manifest records:

```text
store_object:
  id
  module_name
  hash
  size
```

Capability kind:

```text
StoreObjectRead
```

Candidate syscall:

```text
historical: SYS_OBJECT_READ(cap_slot, ptr, len)
current: VFS open/read handles after M74
```

Acceptance evidence:

```text
model-reader has read cap to store:hello-text
model-reader reads bytes successfully
echo lacks read cap
echo VFS open rejected: permission
```

Store objects should be immutable byte blobs at this stage. Filesystems,
mounting, and path resolution remain out of scope.

## M20: Native Generation Identity And Boot Selection

Status: done.

Goal: introduce the native foundation for graph-level generation switch and
rollback. For the first version, multiple generation records can be embedded in
the ISO as separate KrustBoot modules.

Objects and fields:

```text
generation table
current generation
previous generation
booted generation
activation result
```

Boot selection:

```text
limine.conf selects generation B
Krust passes selected generation to vertex-init
vertex-init activates selected generation
```

Failure behavior:

```text
if selected generation fails activation,
fall back to previous generation
```

Acceptance evidence:

```text
Boot generation: gen:bad-0002
activation failed
falling back to generation: gen:hello-0001
Krust rollback generation accepted: target=gen:hello-0001
Boot generation: gen:hello-0001
Native service activation ok
```

This is where Vertex begins to become graph-native rollback rather than only a
capability-kernel demo.

## M21: Native State-Volume Capabilities

Status: done.

Goal: introduce the mutable state object model without building a disk driver.
State should become first-class in the manifest, services should receive state
authority only if declared, writes should be simulated in memory, and rollback
semantics should stay explicit.

Object kind:

```text
StateVolume
```

Rights:

```text
read
write
snapshot
restore
```

Initial demo protocol:

```text
counter-service -> /state/counter/value VFS write
reader-service -> /state/counter/value VFS read
explicit lifecycle owner -> /state/counter/control VFS shutdown command
kernel -> vertex-state kernel-owned state-vfs-request/state-vfs-reply endpoints
```

Acceptance evidence:

```text
counter-service has VFS state file
counter-service writes state through VFS
reader-service has VFS state file
reader-service receives state value
reader-service write rejected
Native state service client ok
```

This keeps immutable generation rollback and mutable state rollback separate.

## M22: Small Native Component Protocol

Status: done.

Goal: define the Vertex native protocol envelope so native services stop
inventing ad hoc byte strings as the graph grows. M40 carries these protocol
families forward as v1 identifiers.

Minimal message envelope:

```text
u16 protocol
u16 message_kind
u32 length
u64 correlation_id
payload bytes
optional transferred cap slots
```

Initial protocol families:

```text
vertex.log.v1
vertex.health.v1
vertex.supervision.v1
vertex.store.v1
vertex.state-vfs.v1
```

Keep this deliberately small. The purpose is a simple graph-first component
protocol, not a large service framework.

## M23: Minimal Timer And Sleep

Status: done.

Goal: add time only after graph activation, readiness, and supervision
semantics are stable.

Objects and syscalls:

```text
Timer object
Timer capability
SYS_SLEEP_MS
SYS_TIMER_WAIT
```

Acceptance evidence:

```text
timer-service sleeps 10 ms
Timer sleep accepted: proc=timer-service timer=monotonic-timer ms=10
Timer sleep blocked: proc=timer-service
Timer wake: proc=timer-service
wakes
logs "timer ok"
```

This enables readiness timeouts, restart backoff, and activation failure
handling without keeping the sleeping process runnable. M23 still uses a
cooperative TSC-polled idle wait when no process is ready; interrupt-driven
timer wakeup is outside this milestone.

## M24: Hostless Native Test Runner

Status: done.

Goal: grow `scripts/krust-smoke.sh` into a QEMU-native test suite before adding
drivers or persistent storage.

Suggested commands:

```text
scripts/krust-test.sh m13
scripts/krust-test.sh manifest-cycle
scripts/krust-test.sh bad-cap
scripts/krust-test.sh readiness-timeout
scripts/krust-test.sh rollback
```

Each test should build an ISO, run QEMU, collect serial output, and assert
required transcript lines.

Test cases:

```text
valid activation
dependency cycle rejected
missing provider rejected
bad capability rejected
bad user pointer rejected
service failure rollback
read-only store access
state write denial
```

This test runner is infrastructure for keeping Krust stable as native graph
semantics become richer.

## Phase 3: Krust Substrate Hardening

M14-M24 prove that Krust can activate a declared native service graph under
explicit authority. The next phase should stop expanding the demo graph and
make the substrate durable: manifest ABI, capability lifecycle, resource
accounting, scheduling, fault recovery, I/O authority, user-space drivers,
persistent store/state services, generation switching, native introspection,
and a reproducible build environment.

Recommended order:

```text
M25  Reproducible clean-clone release gate
M26  KrustBoot Manifest v1
M27  Capability model v1: provenance, revocation, and audit
M28  Kernel heap and typed object arenas
M29  Resource accounting and quotas
M30  Real timer interrupt and preemptive scheduling
M31  User page-fault handling and process death
M32  I/O capability substrate
M33  Move serial logging toward user space
M34  First real block-device path
M35  Native immutable store service
M36  Native state service
M37  Native generation switch
M38  Native vertexctl-like introspection service
M39  Reproducible build environment
M40  Vertex Native Runtime ABI v1
```

M40 is now complete. The M39 build environment pin keeps the proof repeatable
while ABI v1 removes the old shared bidirectional endpoint protocol.

## M25: Reproducible Clean-Clone Release Gate

Status: done.

Goal: make the current M14-M24 proof boring and repeatable from a clean clone
before adding new kernel mechanisms.

The release gate covers:

```sh
scripts/krust-release-gate.sh
```

Acceptance criteria:

```text
done: all release-gate scripts are shell-syntax checked, executable, and trailing-whitespace checked
done: Rust formatting and milestone Markdown whitespace are checked by the gate
done: Makefile recipes are parsed by make before the gate proceeds
done: all M14-M24 QEMU tests are run from the gate
done: M26-M29 manifest, capability, arena, quota, and malformed-manifest QEMU tests are run from the gate
done: M30-M31 timer-preemption and user-fault containment QEMU tests are run from the gate
done: M32-M38 I/O, serial-driver, block-driver, store-service, state-service, generation-switch, and introspection QEMU tests are run from the gate
done: M39 exact toolchain, Cargo lockfiles, and locked offline Cargo metadata are checked by the gate
done: M40 directed request/reply IPC is checked by the gate
done: M41 native console shell is checked by the gate
done: M42 minimal virtio-block driver is checked by the gate
done: M43 VertexDisk layout is checked by the gate
done: M44 native boot manager fallback is checked by the gate
done: M45 store-object verification failure is checked by the gate
done: M46 native update transactions are checked by the gate
done: M47 store-loaded executables and corrupt executable rejection are checked by the gate
done: all QEMU transcript checks have bounded polling windows
done: missing and forbidden transcript lines are reported explicitly
done: README.md, docs/krust-milestones.md, docs/krust-abi-v1.md, and kernel/krust/README.md agree
done: offline build failure reports the Cargo cache/vendor prerequisite
```

This milestone protects the current proof from becoming fragile as the kernel
gains more moving parts.

## M26: KrustBoot Manifest v1

Status: done.

Goal: replace the current fixed-offset proof manifest with a versioned native
ABI artifact that can evolve without silent parser breakage.

Manifest header fields:

```text
magic
version
total_size
header_size
record_table_offset
record_count
generation_id
parent_generation_id
checksum_or_hash
```

Records should be self-describing:

```text
kind
id
offset
length
```

Initial record kinds:

```text
BootModule
Process
Endpoint
Grant
StoreObject
StateVolume
Timer
Generation
Policy
```

Acceptance tests:

```text
done: valid manifest boots
done: truncated manifest rejected
done: bad magic rejected
done: unwrapped compact payload rejected
done: unsupported version rejected
done: out-of-bounds record rejected
done: cyclic dependency rejected
done: missing provider rejected
```

The parser now accepts only a versioned Manifest v1 wrapper around the compact
payload, validates record bounds and checksum before exposing records, rejects
unwrapped compact payloads at the boot-module boundary, and keeps full JSON and
graph interpretation outside Krust.

## M27: Capability Model v1: Provenance, Revocation, And Audit

Status: done.

Goal: make authority lineage explicit so generation switches and supervision can
revoke obsolete service authority.

Capability metadata:

```text
cap_id
object_id
rights
owner_process
parent_cap_id
generation_id
delegated_by
revoked
```

Operations:

```text
SYS_CAP_REVOKE
SYS_CAP_INSPECT
SYS_CAP_MOVE
SYS_CAP_COPY
```

Semantic rule: derived capabilities must not outlive revoked parent authority
unless they are explicitly marked as independently rooted.

Acceptance tests:

```text
done: init derives send-only cap for echo
done: echo can send
done: delegated capability revocation makes later send fail
done: cap inspect shows parent chain
done: cap transfer cannot amplify rights
done: cap move removes source slot
done: cap copy preserves source slot
```

Capability records now carry provenance metadata, generation identity, delegate
identity, and revocation state. Lookup rejects revoked caps and caps with
revoked ancestors.

## M28: Kernel Heap And Typed Object Arenas

Status: done.

Goal: move beyond fixed proof tables without adding an unbounded general-purpose
runtime to the kernel.

Add:

```text
frame allocator to page mapper path
small kernel heap
typed process arena
typed endpoint arena
typed capability arena
typed timer arena
typed store/state object arenas
```

The first implementation should prefer fixed-size typed arenas allocated from a
real heap, with explicit capacity and failure paths.

Acceptance tests:

```text
done: create 32 endpoints
done: create 32 processes
done: allocate, free, and reuse kernel objects
done: allocation failure returns a controlled error
done: no silent object-table overwrite
```

Krust now has a small heap-backed typed arena primitive for fixed-capacity
kernel object families, with explicit failure instead of table overwrite.

## M29: Resource Accounting And Quotas

Status: done.

Goal: make resource ownership as explicit as authority. A capability OS should
not let one service consume all kernel objects by accident.

Process quota fields:

```text
max_caps
max_endpoints
max_memory_pages
max_child_processes
max_ipc_bytes
```

Resource rights:

```text
allocate
delegate
control
revoke
```

Acceptance tests:

```text
done: service with no allocation authority cannot create endpoint
done: service with quota=1 endpoint can create one endpoint
done: second endpoint creation fails
done: init can delegate smaller quota
done: delegated quota cannot exceed parent quota
```

Quotas are enforced by the kernel syscall boundary together with capability
rights: allocation requires explicit authority and available quota, and
delegation cannot exceed the caller's quota.

## M30: Real Timer Interrupt And Preemptive Scheduling

Status: done.

Goal: let Krust regain control without relying on userspace to yield.

Start with the simplest QEMU-friendly path:

```text
PIT or local APIC timer
IDT interrupt entry
tick counter
sleep queue
preemptive scheduler option
critical kernel regions where preemption is disabled
```

Acceptance tests:

```text
timer tick increments
process sleeping 10 ms wakes
CPU-bound process cannot starve logd
scheduler preempts process without explicit yield
preemption can be disabled for critical kernel regions
```

Acceptance evidence:

```text
PIT timer interrupt initialized: vector=32 hz=100
Timer tick increments: ticks=1
Preemption disabled in kernel critical sections
cpu-hog starts without yielding
Scheduler preempted process without explicit yield: from=cpu-hog
logd received: hello from echo
```

Do not over-engineer fairness in this milestone. The first target is control
recovery and correct wakeups.

## M31: User Page-Fault Handling And Process Death

Status: done.

Goal: turn bad userspace memory behavior into process failure instead of kernel
failure.

Behavior:

```text
userspace page fault identifies current process
bad userspace fault marks only that process Failed or Exited
init observes failure through SYS_PROCESS_WAIT
restart policy can restart the failed service
kernel continues running
```

Acceptance tests:

```text
bad-pointer syscall returns STATUS_BAD_BUFFER
direct invalid userspace load kills only that process
init observes service failure
restart policy can restart it
kernel does not panic
```

Acceptance evidence:

```text
faulty-service triggers direct invalid load
User page fault: proc=faulty-service
User process fault contained: proc=faulty-service
vertex-init observes failure
vertex-init restarts faulty-service once
faulty-service exits 0 after restart
Native service activation ok
```

This milestone should be kept narrow: process containment first, advanced fault
delivery later.

## M32: I/O Capability Substrate

Status: done.

Goal: expose hardware authority through kernel objects before adding real
drivers.

Kernel object kinds:

```text
IoPortRange
MmioRegion
InterruptLine
DmaRegion
```

Syscalls:

```text
SYS_IO_READ
SYS_IO_WRITE
SYS_IO_READ16
SYS_IO_WRITE16
SYS_IO_READ32
SYS_IO_WRITE32
SYS_IRQ_WAIT
SYS_MMIO_MAP
SYS_DMA_MAP
```

Acceptance tests:

```text
done: serial-driver has COM1 I/O port capability
done: serial-driver can write byte
done: echo lacks I/O capability
done: echo I/O write rejected
```

The enforcement rule is simple: only services with explicit I/O capabilities can
touch hardware resources.

KrustBoot now carries `IoPortRange`, `MmioRegion`, `InterruptLine`, and
`DmaRegion` grants into the kernel runtime. The I/O, IRQ, MMIO, and DMA syscalls
resolve process-local capabilities before touching or exposing any
hardware-shaped resource.

## M33: Move Serial Logging Toward User Space

Status: done.

Goal: keep kernel serial as a debug and panic path, but move normal logging
toward a user-space driver model.

Flow:

```text
Krust grants serial-driver I/O port capability
serial-driver owns COM1
logd sends messages to serial-driver
serial-driver writes to serial
```

Acceptance tests:

```text
done: serial-driver ready
done: logd sends log message
done: serial-driver writes message to COM1
done: logd cannot write COM1 directly
done: echo cannot write COM1 directly
done: kernel debug serial still works for panic path
```

This aligns Krust with the microkernel direction: drivers belong in userspace
when the kernel can safely enforce the resource boundary.

Normal demo logging now includes a user-space path where `logd` sends to
`serial-driver`, and `serial-driver` writes COM1 through `SYS_IO_WRITE`. The
kernel serial writer remains available for early boot, debug transcripts, and
panic paths.

## M34: First Real Block-Device Path

Status: done.

Goal: add storage transport without adding a filesystem yet.

Preferred first target:

```text
QEMU virtual disk
virtio-blk driver service
virtio PCI I/O authority
block-read/block-write IPC protocol
```

Acceptance tests:

```text
done: virtio-blk driver ready
done: store-service requests block read
done: block-driver returns bytes
done: unauthorized service cannot talk to block-driver
done: unauthorized service cannot access PCI I/O, IRQ, or DMA capabilities
```

The original proof transport has been upgraded rather than kept as a legacy
compatibility path. `block-driver` now owns the virtio-blk PCI I/O, IRQ, and
DMA capabilities, discovers the QEMU PCI device, configures a single virtqueue,
reads sector 0, writes a test sector, verifies readback, and serves directed
block-read IPC from a real raw disk image.

## M35: Native Immutable Store Service

Status: done.

Goal: implement the first native Vertex store feature without implementing
POSIX or a general filesystem.

Service:

```text
vertex-store
  reads content-addressed objects from block service
  exposes StoreObjectRead capabilities
  verifies object hashes
```

Object identity:

```text
store:blake3:<hash>
```

Acceptance tests:

```text
done: model-reader asks for store:hello-text
done: vertex-store verifies hash
done: model-reader reads bytes
done: modified object fails hash check
done: unauthorized process cannot open ungranted VFS file
done: legacy object-read syscall is rejected
```

This turns the current boot-module store proof into a real immutable object
service.

`model-reader` now talks to `vertex-store` over an explicit store IPC endpoint.
`vertex-store` requests bytes from `block-driver`, verifies the expected object
content, rejects a modified-object negative check, and replies to the reader.

## M36: Native State Service

Status: done.

Goal: add real mutable state while keeping it distinct from immutable store
objects.

Service:

```text
vertex-state
  owns block ranges or state objects
  exposes state through explicit VFS files
  supports read, write, snapshot, and restore semantics
```

Acceptance tests:

```text
done: counter-service writes state through VFS
done: reader-service reads state
done: reader-service write rejected by read-only VFS authority
done: state owner shuts down the state service after VFS state clients quiesce
done: state restored
done: system generation rollback does not automatically roll back state unless policy says so
```

Immutable system rollback and mutable state rollback are related policy
decisions, not the same operation.

`counter-service` and `reader-service` use a `vertex-state` IPC endpoint for
state operations. The old direct kernel state syscall path was upgraded into
service IPC and removed from the current ABI surface. M43 then moved
`vertex-state` persistence onto VertexDisk block IPC, so there is no native
state backend capability to preserve.

## M37: Native Generation Switch

Status: done.

Goal: make generation switching real instead of only boot-time generation
selection and rollback metadata.

Flow:

```text
vertex-init activates generation A
vertex-store exposes generation B manifest
vertex-init validates B
vertex-init starts B services
old generation authority is revoked
if B fails, rollback to A
```

Acceptance tests:

```text
done: boot generation A
done: switch to generation B
done: service from A loses old capability
done: service from B runs
done: bad generation C fails and records last_failed_generation
done: native boot manager falls back to B
```

`SYS_ACTIVATE_GENERATION` now resolves the requested generation ID against the
registered native KrustBoot runtime configs, records the previous generation as
the rollback target, rebuilds the process/object/capability tables for the new
generation, and enters the new `vertex-init`. The M37 QEMU case boots
`gen:switch-a-0001`, has `vertex-init` obtain the generation-B marker from
`vertex-store` over the declared store IPC endpoint, switches to
`gen:switch-b-0002`, proves old authority was discarded with the old runtime
tables, then switches to registered bad generation C. C fails activation,
records `last_failed_generation`, journals the fallback decision, and invokes
rollback to B through the native boot manager. The older re-entry rejection
transcript is not kept as a compatibility path.

## M38: Native Vertexctl-Like Introspection Service

Status: done.

Goal: bring Vertex explainability into the native system.

Service:

```text
vertex-inspect
  reads the generation graph through delegated manifest-read authority
  asks the kernel for the process and capability graph
  emits native why/who-can/provenance answers from the structured report
```

Queries:

```text
why can echo send to log-sink?
who can access state:counter?
which generation started vertex-inspect?
which caps were derived from delegated endpoint authority?
which service owns state:counter?
```

Acceptance tests:

```text
done: native why echo log-sink
done: native who-can state:counter
done: native which-generation vertex-inspect
done: native delegated endpoint cap enumeration
done: native cap provenance report
```

`SYS_RUNTIME_INSPECT` requires `inspect` rights on process-control and returns
a structured text report containing the runtime generation, process states,
current capability spaces, initial capability spaces, rights, cap IDs, parent
cap IDs, generation IDs, and revocation state. `vertex-init` delegates only
`inspect` and manifest `read` authority to `vertex-inspect`; the service then
proves the native counterpart to host-side `vertexctl why` and `who-can` without
receiving control, allocation, delegate, or revoke authority.

## M39: Reproducible Build Environment

Status: done.

Goal: make the build environment itself reproducible without requiring an
external functional package manager.

Implemented:

```text
done: documented host tool versions for Rust, qemu, limine, xorriso, and cargo tools
done: locked Cargo dependencies for the top-level host-tool workspace, Krust kernel, and native userspace crates
done: kernel/krust/rust-toolchain.toml pins Rust 1.95.0, rustfmt, and x86_64-unknown-none
done: make doctor checks every required tool and reports actionable fixes
done: legacy hello/ipc userspace crates are removed instead of carried forward
done: single release-gate script runs the clean-clone M14-M88 plus M90-M93 substrate proof with the current QEMU matrix
```

Acceptance tests:

```text
done: cargo metadata --locked --offline --no-deps --format-version 1
done: cargo build --locked --offline
done: target/debug/vertexctl validate examples/hello-generation.vertex.json
done: cd kernel/krust && make doctor && make smoke
done: scripts/krust-test.sh restart
done: scripts/krust-test.sh timer
done: scripts/krust-test.sh store-state-services
```

The pinned tool list lives in `docs/krust-toolchain.md`. The release gate
rejects floating Rust channels, missing lockfiles, stale legacy userspace
crates, and unlocked or online Cargo resolution.

## M40: Vertex Native Runtime ABI v1

Status: done.

Goal: freeze a small native ABI subset after the substrate has enough real use
to reveal the unstable parts.

ABI set:

```text
KrustBoot Manifest v1
Krust Syscall ABI v1
Vertex Native Protocol v1
Capability Rights v1
Process Lifecycle v1
Store Object v1
State Volume v1
```

This does not mean Vertex OS is stable. It means the prototype has a durable
base for the next phase.

Implemented:

```text
done: kernel IPC endpoints keep a fixed four-message FIFO for one-way request queues
done: native endpoint requirements are send-only; provider receive authority is derived from provides
done: native endpoint grants are one-way send or receive, never shared send|receive
done: store, block, and state protocols use request endpoints plus private reply endpoints
done: vertex-init fetches generation B through an attenuated receive-only private dynamic reply endpoint
done: provider readiness replaces scheduling-yield assumptions for serial, block, store, state, and log services
done: Vertex IR examples and schema remove the legacy bidirectional endpoint right
done: scripts/krust-test.sh m40 proves the directed IPC ABI and FIFO queue behavior
```

## Phase 4: Vertex OS v0 Appliance System

Status: done.

Goal: turn the M14-M73 native proof into a small real operating system target:
bootable in QEMU, persistent, inspectable, updateable, and capable of running
several native services under explicit authority.

The design target is an idealized NixOS shape, not Linux compatibility:

```text
immutable store objects
explicit runtime graph
capability-derived service authority
generation switch and rollback
persistent mutable state with policy
reproducible build inputs
inspectable provenance and authority
```

The practical target is a v0.1 appliance, not a desktop:

```text
QEMU boots Vertex OS from a disk image
serial console shell is available
store and state persist across reboot
new generations install transactionally
corrupted store objects fail verification
generation rollback works from disk metadata
native services can be explained from inside the VM
```

Ordering rule: do not build high-level language, package manager, POSIX, or
networking features before the persistent appliance loop works end to end.

## M41: Native Console Shell

Status: done.

Goal: make the system interactable from inside Vertex OS using the existing
serial substrate, without introducing ambient terminal authority.

Add:

```text
done: console-driver owns COM1 I/O authority in the M41 generation
done: console-driver reads serial input and forwards complete command lines to console-shell
done: console output, shell input, and driver control use separate directed endpoints
done: vertex-init delegates inspect and update authority to console-shell
done: generation, services, and why commands are backed by the runtime inspect report
done: scripts/krust-test.sh m41 checks the native console transcript
```

M41 shell transcript commands:

```text
help
generation
services
why <service> <capability>
halt
```

The broader command surface remains the direction for later appliance
milestones once persistent store, state, update, and boot-selection services
exist.

Rules:

```text
console-driver owns COM1 I/O authority
console-shell receives only shell request, console output, console control, counter request, state lifecycle control, inspect, and update authority
ordinary services can write console output but cannot inject shell commands
serial log remains available for tests
shell output is a service protocol, not kernel println as an API
```

Acceptance tests:

```text
Vertex shell ready
user types: help
commands: generation services why halt
user types: generation
current generation: gen:console-0001
user types: services
services: vertex-init=<state> logd=<state> vertex-store=<state> vertex-state=<state> console-shell=<state>
user types: why svc:echo cap:log.sink
svc:echo has send authority because generation graph granted cap slot 0
```

## M42: Minimal Virtio-Block Driver

Status: done.

Goal: add the first real persistent block I/O path before defining a disk
layout that depends on it.

Scope:

```text
virtio-blk PCI discovery path for the QEMU device already modeled by caps
single request queue
read fixed-size sectors
write fixed-size sectors
block-driver owns PCI I/O, IRQ, and DMA caps
clients use directed request/reply IPC
```

Rules:

```text
no filesystem yet
no global disk authority
no unprivileged PCI I/O, IRQ, DMA, or raw block access
kernel grants hardware-shaped authority; user-space driver owns the protocol
```

Acceptance tests:

```text
done: virtio-blk driver ready
done: virtio-blk PCI device discovered
done: block-driver reads sector 0
done: block-driver writes test sector
done: readback matches
done: block-driver received block-read request
done: block-driver returns bytes
done: vertex-store verifies hash
done: echo cannot access block hardware authority
done: block-driver fault does not crash kernel
```

## M43: VertexDisk v1 Layout

Status: done.

Goal: stop relying only on ISO boot modules by defining a minimal disk format
for generations, immutable store objects, state volumes, and update journals.

Initial layout:

```text
VertexDisk v1
  superblock
  generation metadata area
  immutable store index
  immutable store data
  mutable state index
  mutable state data
  journal area
  VertexFS image area
```

Rules:

```text
custom block-object format first
no full filesystem
no path namespace yet
all metadata has version, checksum, and bounds
state writes are explicit service operations
```

Acceptance tests:

```text
done: QEMU boots with VertexDisk image attached
done: VertexDisk superblock accepted
done: VertexDisk v1 superblock exposes a bounded VertexFS image section
done: vertex-store reads object index from disk
done: vertex-state reads state volume from disk
done: vertex-state writes journal record before state data/index writeback
done: vertex-state writes state volume to disk
done: reboot preserves state value
done: reboot preserves state:scratch value
done: bad superblock is rejected without panic
```

Implementation note: native M43 uses custom VertexDisk block IPC through
`block-driver`; store and state clients use separate request endpoints so the
driver can enforce read-only store access and state-only read/write sector
bounds. `vertex-state` writes a journal record before state data/index
writeback and can replay that record if the index is stale. It does not keep
the legacy kernel state syscall or native state backend capability path alive.

## M44: Native Boot Manager and Generation Selector

Status: done.

Goal: make generation selection and fallback disk-native instead of
QEMU-scripted or boot-module-only.

Generation selector state:

```text
selected_generation
previous_generation
known_good_generation
last_failed_generation
boot_attempt_counter
```

Boot behavior:

```text
try selected_generation
if activation succeeds, mark selected_generation as known_good_generation
if activation fails, mark selected_generation as last_failed_generation
fallback to previous known_good_generation
record the decision in the journal
```

Acceptance tests:

```text
Boot gen:A
gen:A activation ok
Install gen:B
Boot gen:B
gen:B activation fails
Fallback to gen:A
gen:A activation ok
journal records failed gen:B and fallback gen:A
```

Implementation note: Krust now keeps selected, previous, known-good,
last-failed, and boot-attempt state in the native boot manager. Failed
activation marks the attempted generation as failed, falls back to the previous
known-good generation, and records the decision in the native journal log. The
M44 QEMU case boots a bad generation, falls back to `gen:hello-0001`, and
checks the boot-manager transcript.

## M45: Store Object Hashing and Verification

Status: done.

Goal: make the native store trustworthy enough to activate generations from
disk.

Object identity:

```text
store:blake3:<hash>
```

Metadata:

```text
id
hash
size
kind
references[]
build_provenance optional
```

Rules:

```text
activation fails if any required object is missing or corrupted
reads come from a verified index or re-verify before use
services never receive corrupted object bytes as success
hash mismatch is a security event in inspect output
```

Acceptance tests:

```text
store object hash matches -> read ok
store object modified on disk -> verification fails
service denied corrupted object
generation activation fails if required object is corrupted
```

Implementation note: KrustBoot store objects now carry
`store:blake3:<hash>` identities, exact byte sizes, and executable module
bindings. Kernel store reads re-verify object bytes before success, VertexDisk
store entries carry hash metadata, and corrupted store data emits an inspectable
security event instead of being delivered to services. The M45 QEMU case
corrupts the on-disk store object and expects activation failure.

## M46: Native Update Transaction

Status: done.

Goal: install a new generation as an atomic disk transaction.

Flow:

```text
receive or import new generation manifest
verify manifest hash and optional signature
verify every referenced store object
install missing objects
write generation metadata
fsync-equivalent journal commit at block layer
set selected_generation to the new generation
activate live or reboot into it
```

Rules:

```text
a generation is not bootable until its full closure is present and verified
partial installs are recoverable
rollback metadata is updated only after commit
update authority is an explicit capability
```

Acceptance tests:

```text
install gen:B
simulate missing store object
install rejected
selected_generation unchanged
install gen:C
all objects verified
selected_generation updated
boot gen:C succeeds
power-loss simulation before commit leaves old generation selected
```

Implementation note: generation activation now verifies the target manifest and
store closure before committing selection changes. Missing store closure rejects
the transaction and leaves the selected generation unchanged; a verified
generation writes a journal-commit transcript and becomes selected. The M46
QEMU case installs generation B, rejects a missing-object transaction, and boots
the selected generation.

## M47: Executables Loaded From Native Store

Status: done.

Goal: move native service executables out of boot-module-only activation and
into verified store objects.

Flow:

```text
vertex-init reads generation graph
vertex-store verifies executable store object
kernel creates process image from verified bytes
vertex-init grants declared capabilities
vertex-init starts service
```

Rules:

```text
ELF bytes must be verified before process creation
kernel does not trust service-provided executable bytes blindly
Limine boot modules remain only for compact manifests and the native VertexDisk image
```

Acceptance tests:

```text
logd executable loaded from store object
echo executable loaded from store object
store hash verified before process creation
corrupted executable rejected
generation activation fails cleanly
```

Implementation note: process records must resolve to verified native store
objects. The old boot-module-only executable path is removed for declared
processes: process creation first resolves the matching VertexDisk-backed store
object, verifies its BLAKE3 identity, checksum, and size, and logs the store
identity before loading the ELF.
The M47 QEMU case checks `logd` and `echo` executable objects.

## M48: Dynamic Process Creation Authority

Status: done.

Goal: replace fixed process slots with capability-controlled process
creation from vertex-init.

Add:

```text
SYS_PROCESS_CREATE
SYS_PROCESS_START
SYS_PROCESS_KILL
SYS_PROCESS_WAIT
```

Process-factory rights:

```text
create
start
kill
wait
inspect
```

Rules:

```text
only holders of process-factory authority can create processes
initial capability table is supplied as explicit grants
arguments and environment-like metadata are immutable launch objects
process IDs and cap IDs appear in runtime inspect output
```

Acceptance tests:

```text
vertex-init creates logd dynamically from verified store object
vertex-init grants only declared caps
unprivileged service calls SYS_PROCESS_CREATE
request rejected: bad capability
vertex-init waits for service exit status
```

Implementation note: Krust now boots only `vertex-init` into the runtime process
table. Non-initial manifest process records are templates; `vertex-init` calls
`SYS_PROCESS_CREATE`, receives a pid, transfers or delegates only the declared
authority for that pid, then calls `SYS_PROCESS_START` and `SYS_PROCESS_WAIT`.

done: M48 dynamic process creation authority is checked by the gate

## M49: Immutable Config Objects

Status: done.

Goal: make service configuration explicit, immutable, hashable, and
inspectable without returning to ambient config files.

Config object:

```text
id
bytes
hash
schema optional
```

Rules:

```text
config is passed as a capability
config can be shared explicitly between services
config hash mismatch fails activation
services cannot read configs they were not granted
```

Acceptance tests:

```text
logd reads config through VFS handle
echo cannot read logd config
config hash mismatch fails activation
vertex-inspect shows config authority without dumping large content
```

Implementation note: `config:logd` is a native immutable object carried in
KrustBoot/VertexDisk metadata and granted only to logd. M74 moved config reads
onto VFS file handles; the legacy object-read syscall is rejected. Reads verify
the content identity before returning bytes; the corrupt-config QEMU case
rejects a hash mismatch during activation.

done: M49 immutable config objects and hash-mismatch rejection are checked by the gate

## M50: Native Secrets Model

Status: done.

Goal: add first-class secret authority without treating secrets as normal
config or store content.

Secret rights:

```text
read
derive
seal
unseal
inspect-metadata
```

Rules:

```text
secrets are not printed in inspect output
secrets are not stored in plaintext generation manifests
secret access logs metadata, never content
early v0 may keep secrets in memory only
later versions can bind secrets to TPM, encrypted state, or sealed keys
```

Acceptance tests:

```text
service with secret cap reads secret
service without cap rejected
vertex-inspect shows which services have secret access
vertex-inspect does not print secret value
```

Implementation note: `secret:logd-token` is registered as an in-memory native
secret object. `SYS_SECRET_READ` requires explicit secret authority, logs only
metadata, and runtime inspection redacts the value while showing which process
holds the secret cap.

done: M50 native secret authority is checked by the gate

## M51: Native Package Boundary

Status: done.

Goal: define the metadata boundary for reusable Vertex-native software without
building a full package manager yet.

Package contents:

```text
executable store objects
library store objects
config schemas
declared runtime needs
service templates
metadata and provenance
```

Acceptance tests:

```text
vertexctl package inspect logd.vertexpkg
vertexctl package instantiate logd.vertexpkg
output graph fragment contains service, executable, config, and cap needs
```

Implementation note: `vertexctl package inspect` and `vertexctl package
instantiate` read `.vertexpkg` metadata and expose the package boundary used by
the current package examples.

done: M51 package inspection and instantiation are checked by the gate

## M52: Vertex Graph Linker

Status: done.

Goal: link package graph fragments into a concrete generation graph and store
closure.

The linker resolves provider/consumer relationships:

```text
logd requires serial-output/send
serial-driver provides serial-output
linker emits endpoint capability and grant path
```

Acceptance tests:

```text
input: serial-driver package, logd package, echo package
output: generation graph
output: store closure
output: KrustBoot or disk generation metadata
linked graph boots in QEMU
```

Implementation note: `vertexctl graph-link` accepts package fragments, resolves
their service provider/consumer capabilities into a concrete generation graph,
validates the result, and emits a store closure plus KrustBoot metadata seed for
the current package proof.

done: M52 graph linking is checked by the gate

## M53: Reproducible Build Graph Interface

Status: done.

Goal: define Vertex's own build graph boundary so external builders can feed
verified artifacts into Vertex without making Vertex depend on any one build
system.

Pipeline:

```text
source
  -> external reproducible build
  -> store object
  -> Vertex package metadata
  -> generation graph
  -> disk image and KrustBoot seed
```

Rules:

```text
Vertex records artifact hashes, provenance, package metadata, and runtime graph
Nix may be one adapter, but not the privileged or required build model
the Vertex store identity is content-addressed and builder-independent
the runtime graph must be reproducible from declared inputs
```

Acceptance tests:

```text
vertexctl build-import build-output.json
produces krust.elf
produces VertexDisk image
produces store objects
produces generation manifest
produces bootable QEMU target
optional Nix adapter produces the same build-output.json shape
```

Implementation note: `vertexctl build-import` imports a declared
`build-output.json`, rejects missing kernel or artifact paths, verifies declared
artifact hashes when present, materializes actual artifact bytes and generation
metadata, emits `krust.elf`, creates a VertexDisk image, and writes a QEMU
target descriptor.

done: M53 build graph import is checked by the gate

## M54: First Appliance Release Target

Status: done.

Goal: ship one tiny appliance that demonstrates the whole model with polish.

Initial target: stateful counter appliance.

Required behavior:

```text
boot from disk
show shell
run counter service
persist counter state
install a new generation
rollback system generation
preserve or rollback state according to policy
explain authority graph from shell
```

Acceptance transcript:

```text
Vertex OS v0 appliance booted
install generation gen:new
counter value: 41
increment -> 42
rollback to gen:old
counter state policy: preserve
counter value: 42
why svc:counter state:counter
svc:counter has state authority from generation graph
```

Implementation note: the M54 QEMU case boots the console generation, drives the
native shell over serial input, and verifies the counter, install, rollback, and
authority-explanation transcript.

done: M54 appliance transcript is checked by the gate

## M55: User-Space Driver Framework

Status: done.

Goal: formalize native drivers as capability-bound services.

Driver object types:

```text
IoPortRange
MmioRegion
InterruptLine
DmaRegion
PciDevice
VirtioDevice
```

Rules:

```text
drivers own hardware caps
drivers provide service endpoints
driver health checks are mandatory
driver restart policy is explicit
unprivileged services never receive hardware caps by default
```

Acceptance tests:

```text
serial-driver owns COM1 I/O ports
block-driver owns virtio-blk device
unprivileged service cannot access hardware authority
driver crash does not crash kernel
driver restart preserves or reinitializes protocol state explicitly
```

Implementation notes:

- KrustBoot payload version 5 removes the old compact payload identity and adds
  first-class `PciDevice` and `VirtioDevice` object records.
- `vertexctl` derives PCI and virtio device caps only for the declared native
  driver service; non-driver services are rejected if they request hardware
  authority.
- Native validation rejects driver devices without health checks and rejects
  legacy transport declarations instead of keeping compatibility modes.
- The M55 QEMU case checks COM1 ownership, virtio-blk PCI/virtio ownership,
  mandatory driver health, explicit restart policy, hardware denial for
  unprivileged services, and the existing user-fault containment path for a
  crashing block driver.

done: M55 user-space driver framework is checked by the gate

## M56: Virtio Device Stack

Status: done.

Goal: grow beyond virtio-blk into the QEMU-friendly device set needed for a
usable appliance and development loop.

Priority:

```text
virtio-console
virtio-rng
virtio-net
```

Acceptance tests:

```text
virtio-console replaces raw serial shell transport
virtio-rng provides random bytes through explicit cap
virtio-net driver can send and receive raw frames
unauthorized service cannot access virtio devices
```

Implementation notes:

- KrustBoot payload version 6 is the only accepted compact device-manifest
  format. It carries the M56-M60 virtio and namespace sections directly; the
  previous M55 payload identity is not accepted as a compatibility mode.
- `virtio-console`, `virtio-rng`, and `virtio-net` are native
  `VirtioDevice` capability objects. `vertexctl` grants them only to the
  declared driver service.
- `SYS_VIRTIO_DEVICE_PROBE`, `SYS_VIRTIO_RNG_READ`, `SYS_VIRTIO_NET_TX`, and
  `SYS_VIRTIO_NET_RX` all resolve through the caller's process-local
  capability table and reject the wrong object kind or missing rights.
- `serial-driver` probes the console device, `netstack` reads RNG bytes and
  sends/receives raw net frames, and `echo` proves an unauthorized virtio-net
  call is rejected.

done: M56 virtio device stack is checked by the gate

## M57: Networking v0

Status: done.

Goal: add the smallest useful network path after storage and the appliance
model are stable.

Likely order:

```text
Ethernet frames
ARP
IPv4
ICMP ping
UDP
TCP later
```

Acceptance tests:

```text
QEMU user-mode network attached
Vertex replies to ping or sends ICMP echo
echo queues a UDP payload through a network-port capability
netstack transmits the queued UDP payload through raw virtio-net authority
network authority is endpoint/capability mediated
unauthorized service cannot use network device
```

Implementation notes:

- The M57 QEMU case attaches QEMU user-mode networking and the generation graph
  exposes only `cap:net.udp.9000` as a network-port authority.
- `SYS_NETWORK_SEND_UDP` consumes a `NetworkPort` capability with bind/listen
  rights and queues the payload for `netstack`. `netstack` receives queued
  payloads through its control cap on the same network-port object, then uses
  its separate raw virtio-net device capability to transmit frames.
- `netstack` proves raw frame RX/TX and ICMP-style echo handling in the native
  transcript; `echo` proves UDP send authority through the endpoint-style
  network-port capability and proves it still cannot use the virtio-net device.

done: M57 networking v0 is checked by the gate

## M58: POSIX Compatibility Plan

Status: done.

Goal: write the compatibility architecture before implementing it.

Design layers:

```text
Vertex-native services
WASI personality service
POSIX personality service
Linux personality research service
VM fallback
```

Rule: compatibility services may emulate ambient authority internally, but
they must themselves be launched with explicit Vertex capabilities.

Acceptance artifact:

```text
docs/posix-personality-v0.md
```

done: M58 POSIX compatibility plan is checked by the gate

## M59: Capability Namespace Service

Status: done.

Goal: add path-like convenience without creating a global Unix namespace.

Namespace service maps names to capabilities:

```text
/bin/logd -> store object cap
/state/counter -> state service endpoint cap
/dev/serial -> endpoint cap
```

Rules:

```text
namespace itself is a capability
different services can receive different namespaces
resolution returns capabilities, not ambient access
```

Acceptance tests:

```text
service A namespace contains /state/a
service B namespace contains /state/b
service A cannot resolve /state/b
inspect shows namespace grants
```

Implementation notes:

- Namespace objects are first-class KrustBoot objects and runtime capabilities,
  not a global filesystem. The only namespace right is `resolve`.
- Each namespace entry maps an absolute path to an existing non-namespace
  capability object plus attenuated rights. Resolution installs a derived cap in
  the caller-selected target slot.
- The hello generation grants `svc:echo-server` a namespace containing
  `/state/a` and `svc:state-reader` a different namespace containing
  `/state/b`; the M59 case checks both grants and the `/state/b` denial from
  service A.

done: M59 capability namespace service is checked by the gate

## M60: Human-Readable Policy and Typed Vertex Prototype

Status: done.

Goal: introduce a readable system definition layer only after the runtime
semantics have been proven by the appliance path.

Stage 1:

```text
small .vertex policy syntax or structured TOML/YAML
compiles to Vertex IR
boots through existing pipeline
```

Stage 2:

```text
typed Vertex language prototype
services, capabilities, state, store, secrets, drivers, namespaces
compile-time rejection of missing capability wiring
```

Acceptance tests:

```text
vertexctl compile policy.vertex -> generation manifest
generation manifest -> disk generation metadata
typed system definition compiles
invalid missing capability rejected before boot
valid system boots in QEMU
```

Implementation notes:

- `vertexctl compile-policy` reads `examples/policy.vertex`, validates the
  service/capability declarations against the template manifest, and emits a
  normal generation manifest.
- `vertexctl compile-typed` uses the same validation path for the typed
  prototype syntax. Missing capability wiring is rejected before any boot
  artifact is generated.
- The M60 case compiles policy and typed examples, rejects
  `examples/invalid-missing-capability.vertex`, compiles the resulting
  KrustBoot manifest, writes VertexDisk metadata, and boots the valid policy
  generation under QEMU.

done: M60 policy and typed prototype are checked by the gate

## M61: Kernel ABI and Authority Hardening

Status: done.

Goal: make the native ABI hostile-input resistant before adding breadth.

Scope:

```text
syscall argument bounds and alignment
exact object-kind dispatch for every syscall
exact typed device IDs for device-specific syscalls
rights-mask subset checks on every derived or transferred capability
namespace resolution limited to explicitly allowed non-hardware object types
revocation and parent-cap provenance invariants
generation identity checks on process, cap, and inspect paths
no legacy payload, transport, or compatibility fallback path
```

Acceptance tests:

```text
wrong object kind is rejected for every syscall family
missing rights are rejected for every syscall family
malformed user buffers fail without kernel fault
namespace cannot resolve io-port, mmio, interrupt, dma, pci, or virtio authority
virtio-rng rejects every non-RNG virtio-device ID
virtio-net TX/RX reject every non-network virtio-device ID
legacy markers in device kind, selector, or properties are rejected case-insensitively
capability inspect shows parent and generation provenance after derive/transfer/revoke
```

Implementation notes:

- Add a syscall negative-test table instead of one-off transcript checks. Each
  syscall should have at least one wrong-object, wrong-rights, and bad-buffer
  case.
- Keep the ABI small. If an old behavior conflicts with current authority rules,
  remove it instead of preserving compatibility.
- Treat M61 as the security regression baseline for every later milestone.

Implementation notes:

- M61 introduced `KRUSTBOOTM61` version 7 and rejected M60 compact payloads
  instead of accepting them as a compatibility format. M82 supersedes the M79
  compact identity with `KRUSTBOOTM82` version 13 and rejects older compact
  payloads as legacy.
- The kernel rejects current-process capabilities whose generation provenance
  does not match the active runtime generation, rejects `SYS_CAP_MOVE` before
  clearing the source when the target slot is occupied or invalid, checks DMA
  mapping output alignment after exact DMA authority validation, and requires
  the expected `virtio-pci-io` transport on virtio device syscalls.
- The M61 QEMU case runs native negative tests for wrong object kind, missing
  rights, bad user buffers, virtio RNG/net device mismatches, timer wrong-kind
  dispatch, inspect-only process-control denial, and capability
  parent/generation provenance.
- Host-side validation rejects namespace entries targeting hardware authority
  and rejects `legacy` markers in device kind, selector, and properties
  case-insensitively.

done: M61 ABI and authority hardening is checked by the gate

## M62: Storage Reliability and VertexDisk Durability

Status: done.

Goal: make the block and state path reliable enough for appliance updates.

Scope:

```text
virtio-blk request completion and error propagation
sector-range and alignment enforcement
read-only immutable store objects
state write bounds and owner checks
journal replay after interrupted writes
generation update commit atomicity
explicit durability model for flush/barrier support
block-driver failure and restart semantics
```

Acceptance tests:

```text
store reads survive repeated boot
state write survives reboot
interrupted state journal replays or rolls back deterministically
corrupt state journal is detected and reported
corrupt store object is rejected by hash before process launch
update commit interrupted before final pointer leaves previous generation bootable
update commit interrupted after final pointer boots the new verified generation
block-driver fault during request fails the client request without kernel fault
```

Implementation notes:

- Define the exact VertexDisk durability contract before adding a filesystem.
  A small, auditable store/state layout is more valuable than broad POSIX file
  semantics at this stage.
- Keep store and state block traffic separated by endpoint identity, as in
  M43-M57, while client-facing state access moves through VFS.
- Prefer explicit error surfaces in `vertex-inspect` and the appliance shell over
  silent retry loops.
- The block-driver self-test no longer overwrites the live journal sector. It
  writes a scratch journal sector, reports virtio completion status, and keeps
  store and state block-client ranges separate.
- `vertex-state` replays an interrupted journal record deterministically and
  reports corrupt journal records before rolling back to indexed state.
- The update transaction path now reports both pre-final-pointer and
  post-final-pointer interruption outcomes.

done: M62 storage durability cases are checked by the gate

## M63: Network Service Boundary

Status: done.

Goal: move networking authority behind `netstack` so applications consume
network-port capabilities instead of raw virtio-net device syscalls.

Scope:

```text
raw virtio-net authority granted only to netstack
ARP cache owned by netstack
IPv4 packet validation
ICMP echo request/reply for diagnostics
UDP send through network-port capability
UDP request delivery through the netstack-owned network-port boundary
network-port bind/listen rights enforced by netstack and kernel objects
```

Acceptance tests:

```text
netstack owns device:virtio-net0 and initializes QEMU user-mode networking
echo sends UDP through cap:net.udp.9000 without a raw virtio-device cap
netstack receives the UDP request through the network-port boundary
netstack transmits the UDP packet for the network-port client
unauthorized service cannot bind or send on cap:net.udp.9000
unauthorized service cannot call raw virtio-net TX/RX
ICMP echo from the appliance shell reaches the QEMU gateway
inspect shows network-port authority and raw driver authority separately
```

Implementation notes:

- Keep raw virtio-net syscalls as driver-facing ABI only. Application-facing
  networking should be endpoint/capability mediated.
- Use the current QEMU ARP/ICMP proof as the device smoke test, then add service
  IPC around it instead of exposing Ethernet frames to applications.
- TCP, DNS, DHCP, and POSIX sockets remain later work.
- `netstack` is the only service with raw virtio-net authority in the supported
  manifest. Application code uses `cap:net.udp.9000`, and the negative table
  checks both missing bind/listen rights and attempts to call raw virtio-net
  syscalls without a virtio-device cap.
- `echo` can only queue UDP through bind/listen authority. The provider-side
  network-port control cap is granted to `netstack`, which drains queued
  payloads and performs the virtio-net transmit path.
- Runtime inspect output shows network-port authority separately from raw
  virtio-device authority.

done: M63 network service boundary is checked by the gate

## M64: Supervisor Semantics and Service Lifecycle

Status: done.

Goal: make native activation behave like an appliance supervisor, not only a
boot transcript.

Scope:

```text
manifest dependency graph for startup ordering
readiness timeout policy per service
health-check protocol per driver/service
restart budget and backoff
dependency failure propagation
fault attribution in inspect output
service state machine exposed through runtime inspect
operator-visible activation log
```

Acceptance tests:

```text
service starts only after declared providers are ready
readiness timeout marks the correct service failed
restart=never exits once and stays failed
restart=on-failure restarts within budget and then reports exhausted
restart=always restarts after clean exit only when policy allows it
dependent service is not started when required provider fails
inspect reports declared, starting, ready, failed, restarting, and exited states
appliance shell shows last failure reason and generation id
```

Implementation notes:

- Keep lifecycle policy in native `vertex-init`/supervisor userspace where
  possible. The kernel should enforce authority and provide inspectable state,
  not grow a full policy engine.
- Make restart restoration explicit: capabilities, quotas, and initial process
  context must come from the generation graph, not from stale runtime state.
- Native `vertex-init` now emits declared, starting, ready, restarting, failed,
  and exited lifecycle events, performs timer-backed restart backoff sleeps,
  and keeps readiness timeout attribution service-specific. The kernel only
  records `ready` lifecycle events when they arrive on the readiness endpoint
  for `vertex-init` and the ready payload names the sending process.
- Runtime inspect and the shell consume the same generation/process/capability
  state used by supervisor decisions.

done: M64 supervisor lifecycle semantics are checked by the gate

## M65: Supported Appliance Release Profile

Status: done.

Goal: define the first supported standalone Krust profile and gate it as a
release artifact.

Supported profile:

```text
x86_64 one CPU
Limine boot
KrustBoot Manifest v1 / compact payload KRUSTBOOTM86 version 19
QEMU virtio-blk, virtio-rng, virtio-net, and virtio-console
VertexDisk store/state/update layout
no legacy transport or payload compatibility
no POSIX personality in the base profile
```

Acceptance tests:

```text
clean checkout builds host tools offline
standalone ISO rebuilds from clean kernel artifacts
M56-M64 QEMU cases run in the release gate
storage corruption cases run in the release gate
network authority cases run in the release gate
malformed manifest cases run in the release gate
release artifact records exact toolchain, manifest hash, kernel hash, and store closure
README and docs describe Krust as standalone, with host-side tools only as build/simulation utilities
```

Implementation notes:

- This is the first place to write down what is supported. Everything outside the
  profile is explicitly experimental.
- Do not expand to SMP, USB, GPU, a full filesystem, or Linux/POSIX
  compatibility until the profile can boot, update, recover, and explain its
  authority graph repeatably.
- The compact native payload identity is now `KRUSTBOOTM86` version 19. M85,
  M82, M79, M75, M65, M61, and older compact payload identities are rejected
  rather than retained as compatibility formats.
- The release gate records a supported profile artifact containing the exact
  toolchain, manifest hash, KrustBoot hash, kernel hash, VertexDisk hash, and
  store closure.

done: M65 supported appliance release profile is checked by the gate

## M66: Owned Physical Frames And Reclamation

Status: done.

Goal: make every allocated physical frame owned, inspectable, and reclaimable
instead of treating frame allocation as a mostly one-way boot-time resource.

Scope:

```text
frame owner metadata for kernel, page tables, process memory, DMA, and scratch allocations
allocated/free/reserved frame accounting visible through runtime inspect
contiguous allocation ownership records
double-free and foreign-free rejection
frame zeroing policy before reuse
allocator exhaustion reported without corrupting allocator state
```

Acceptance tests:

```text
process exit returns all process-owned frames to the allocator
restart reuses reclaimed frames without stale userspace bytes
double-free is rejected and leaves accounting unchanged
failed contiguous allocation leaves accounting unchanged
runtime inspect reports total, allocated, free, reclaimed, and owner-class counts
allocator exhaustion fails process creation cleanly
```

Implementation notes:

- Add an explicit frame ledger before adding a general heap. The ledger is the
  source of truth for ownership and is what later teardown paths consume.
- Keep page-table frames, user segment frames, user stacks, DMA buffers, and
  kernel scratch frames as distinct owner classes.
- Zero frames on allocation or before reuse, but make the policy explicit and
  testable.
- Do not reclaim Limine, kernel image, boot modules, or HHDM backing ranges.

done: M66 owned frame ledger, reclaim counters, double-free/foreign-free checks,
and failed contiguous allocation accounting are checked by the gate

## M67: Address Space Teardown And Process Reaping

Status: done.

Goal: fully tear down a process address space on exit, fault, kill, failed
start, and failed restart.

Scope:

```text
walk user half of PML4 for owned mappings
free user leaf frames
free process page-table frames
unmap user device mappings
clear blocked IPC/sleep scheduler state on reaped processes
reap faulted, killed, and normally exited services through one path
```

Acceptance tests:

```text
faulty-service frees its old address space before restart
kill_process releases process frames and removes wait/sleep/block state
failed userspace load frees all frames allocated before the failure
repeated create/start/exit cycles reach a stable frame count
blocked receiver killed while waiting is removed from endpoint wakeups
runtime inspect shows no live mappings for reaped pids
```

Implementation notes:

- This milestone should introduce an address-space object or equivalent
  teardown API so process exit does not know page-table internals.
- Treat process teardown as idempotent. Calling reap twice should be harmless
  and observable as a no-op.
- Keep kernel half mappings shared and never freed by process teardown.

done: M67 process exit, restart, fault, and create/start/exit churn reclaim user
leaf frames and page-table frames and inspect reports reaped pids with no live CR3

## M68: Kernel Object Lifetime And Failure Atomicity

Status: done.

Goal: make kernel object creation and capability installation transactional so
failed syscalls do not leak objects, caps, quotas, or IDs.

Scope:

```text
object table free-list or generation-tagged slots
capability slot rollback on partial failure
quota charge and refund helpers
endpoint_create failure atomicity
process_create failure atomicity
namespace_resolve failure atomicity
cap_transfer failure atomicity
runtime inspect leak report for unreachable objects
```

Acceptance tests:

```text
endpoint_create with occupied target slot does not consume object slots or quota
process_create failure after partial cap grants leaves no process table entry
namespace_resolve with occupied target slot does not allocate a cap id
cap_transfer failure leaves target capability space unchanged
repeated failed syscalls do not change live object/cap/quota counts
inspect reports zero unreachable kernel objects after smoke
```

Implementation notes:

- Prefer small transaction structs around existing fixed tables over a general
  allocator.
- IDs may remain monotonic if that is useful for audit, but live object slots
  and quotas must not leak.
- Keep failure atomicity local and explicit. Do not add broad rollback machinery
  that obscures authority checks.

done: M68 endpoint, cap grant, cap transfer, namespace resolution, process
creation, and dynamic endpoint lifetime paths are checked for live object/cap
leak deltas by the gate

## M69: Memory Pressure, Limits, And Soak Gate

Status: done.

Goal: prove the M66-M68 memory lifecycle holds under repeated restarts,
allocation pressure, and hostile syscall inputs.

Scope:

```text
memory pressure test service
bounded process and endpoint churn
repeated fault/restart loops
runtime high-water marks
leak-delta checks in smoke and release gate
out-of-memory error path coverage
```

Acceptance tests:

```text
100 create/start/exit cycles return to baseline frame and object counts
100 fault/restart cycles return to baseline frame and object counts
memory pressure service reaches configured limit and receives a clean error
endpoint churn reaches quota limit and returns to baseline after owner exit
release gate fails on nonzero frame/object/cap leak delta
inspect shows memory high-water marks and current live counts
```

Implementation notes:

- This is a hardening milestone, not a new feature milestone. The main output is
  confidence that existing services can fail repeatedly without degrading the
  system.
- Keep cycle counts small enough for CI/QEMU portability, but high enough to
  catch monotonic leaks.
- Add a longer optional soak script separately from the default release gate.

done: M69 100-cycle create/start/exit, restart, endpoint churn, and fault/restart
soak checks are run by the Krust QEMU gate with frame/object/cap leak deltas

## M70: Interrupt Routing And Blocking IRQ Delivery

Status: done.

Goal: replace IRQ stubs and polling-only waits with a real interrupt delivery
path from hardware IRQs to authorized driver processes.

Scope:

```text
IRQ object wait queues
per-IRQ pending counters
EOI ordering rules
driver blocking on interrupt-line caps
timeout-aware irq_wait
spurious interrupt accounting
interrupt attribution in runtime inspect
```

Acceptance tests:

```text
block-driver sleeps on virtio-blk IRQ instead of polling for completion
netstack sleeps on virtio-net IRQ instead of polling for RX completion
irq_wait without listen rights is rejected
irq_wait timeout returns timeout without consuming future interrupts
pending IRQ before wait wakes the next authorized waiter
spurious IRQ is counted and does not wake unrelated drivers
inspect reports IRQ line, owner, pending count, waiters, and spurious count
```

Implementation notes:

- Keep the first implementation on legacy PIC/PIT/QEMU hardware if that is what
  the supported profile uses. APIC can be a later replacement.
- Preserve capability isolation: an interrupt-line cap should wake only the
  process authorized for that IRQ object.
- Avoid running driver logic in interrupt context. Interrupt handlers should
  acknowledge, record, and schedule.

done: M70 blocking IRQ wait, timeout, authority rejection, net/block interrupt
wait evidence, and runtime IRQ attribution are checked by the Krust QEMU gate

## M71: DMA Ownership, Pinning, And Bounds Safety

Status: done.

Goal: make DMA memory explicit, owned, bounded, and unmapped on teardown so
drivers cannot use stale or overlapping DMA windows.

Scope:

```text
DMA allocation records tied to process/device objects
page-pinned DMA buffers
DMA virtual mapping teardown
overlap checks for physical and user DMA windows
device-visible length and alignment validation
DMA zeroing on allocation and release
```

Acceptance tests:

```text
persistent block-driver owns its DMA mapping while serving storage
restarted driver receives a fresh DMA mapping with zeroed contents
overlapping DMA region in manifest is rejected
unaligned DMA region in manifest is rejected
oversized DMA region is rejected before mapping
DMA map twice for the same object rejects or returns the same mapping without leaking frames
unauthorized service cannot map or inspect another driver's DMA region
```

Implementation notes:

- Without an IOMMU, Krust cannot fully contain a malicious device. The supported
  profile should state that DMA safety is driver/process bookkeeping plus
  manifest validation, not hardware remapping.
- Keep DMA buffers out of the general user heap. DMA mappings should live in
  reserved user VA windows with explicit ownership.
- Device reset paths in M72 must release or reinitialize DMA descriptors through
  this ownership model.

done: M71 DMA ownership, repeat-map idempotence, release-on-teardown, manifest
range rejection, and inspect accounting are checked by the Krust QEMU gate

## M72: Virtio Reset, Error Recovery, And Async Completion

Status: done.

Goal: make virtio drivers recover from device errors, queue timeouts, and
driver restarts without rebooting the kernel.

Scope:

```text
virtqueue state machines
descriptor ownership tracking
device reset and reinitialize path
timeout-to-reset policy
completion events delivered through IRQ wait queues
driver restart rebinds device authority safely
virtio status/error counters in inspect
```

Acceptance tests:

```text
virtio-blk request timeout resets the device and fails only the request
block-driver fault releases virtqueue ownership before restart
restarted block-driver reinitializes virtio-blk and completes later requests
virtio-net RX timeout does not wedge netstack
virtio-rng timeout returns a clean syscall error
wrong virtio device type cannot enter a typed driver reset path
inspect reports virtio queue state, last error, reset count, and owner process
```

Implementation notes:

- Keep one queue per supported QEMU virtio device until reset/recovery semantics
  are boring. Multiple queues are later work.
- Driver-owned requests should complete through a bounded kernel record, not by
  trusting driver-local memory after the driver has faulted.
- Recovery should favor clean failure and supervisor restart over hidden retry
  loops.

done: M72 virtio queue reports, timeout-to-reset paths, owner release,
wrong-device rejection, and runtime queue/error counters are checked by the
Krust QEMU gate

## M73: Device Isolation And Fault Injection Gate

Status: done.

Goal: prove that driver faults, bad manifests, bad DMA/IRQ authority, and device
timeouts are isolated from unrelated services and from the kernel.

Scope:

```text
device fault-injection test matrix
driver kill/fault while IRQ waiter is registered
driver kill/fault while DMA is mapped
driver kill/fault while request is in flight
manifest negative cases for overlapping I/O, MMIO, DMA, and IRQ authority
operator-visible device failure reports
release-gate integration for memory and device leak deltas
```

Acceptance tests:

```text
block-driver fault during in-flight request fails client request without kernel fault or leaks
netstack fault releases virtio-net IRQ/DMA ownership and leaves other services running
serial-driver fault does not revoke unrelated console-shell authority
bad manifest with overlapping I/O ranges is rejected
bad manifest with overlapping DMA ranges is rejected
bad manifest with duplicate IRQ ownership is rejected
release gate checks memory/object/cap/DMA/IRQ leak deltas after fault injection
appliance shell reports last device failure reason and owner process
```

Implementation notes:

- This milestone closes the M66-M72 hardening loop. It should not introduce new
  device classes.
- Fault injection should be deterministic and scriptable under QEMU so failures
  are reproducible.
- Keep the operator report tied to the same inspect data used by the gate.

done: M73 device-fault isolation, DMA/IRQ/virtio leak deltas, bad hardware
manifest rejection, and operator-visible failure reporting are checked by the
Krust QEMU gate

## M74: VFS Object Model And Authority

Status: done.

Goal: introduce a real VFS object model that unifies files, directories,
devices, immutable store objects, and mutable state volumes behind explicit
capability-mediated authority.

Scope:

```text
VFS object kinds: regular file, directory, device node, pipe, synthetic node
stable vnode/inode identities independent from user path strings
file rights: lookup, read, write, create, unlink, rename, mount, stat, execute
filesystem capability objects in the boot manifest
root directory caps and service-local VFS roots
typed VFS syscall status values in the native ABI
runtime inspect report for live VFS objects and authority
```

Acceptance tests:

```text
service with only read authority cannot create or unlink a file
service with no lookup authority cannot resolve a child path
directory cap can attenuate into read-only subtree authority
device node open requires both VFS authority and underlying device authority
immutable store object appears as read-only regular file
mutable state volume appears below an explicit mounted root
inspect reports vnode id, object kind, rights, owner process, and mount source
old store/state-only access paths are rejected once VFS mode is selected
```

Current implementation state:

```text
done: kernel VFS nodes cover /, /store, /state, /dev, /proc
done: node kinds cover regular file, directory, device node, pipe, synthetic node
done: immutable store/config objects appear as read-only regular files
done: /state/a exists as a kernel memory-file prototype below /state
done: virtio devices appear as /dev nodes tied to underlying device caps
done: boot manifest carries first-class VFS root authority objects
done: service-local VFS root caps can open configured VFS paths
done: compact process records carry a validated `mount_root`; service
      `mountRoot` is mandatory in hosted manifests and selects the
      process-local VFS root without retaining a legacy version-10
      process-record parser or implicit `/` default
done: VFS-root path syscalls resolve absolute user paths under the current
      process mount root, so a service rooted at /state opens /a as /state/a
      while direct store/config empty-path opens remain direct object opens
done: device-node open through `/dev` requires both VFS root path authority and
      the underlying virtio-device control cap; direct device caps are rejected
      as VFS path authority
done: SYS_VFS_DERIVE_ROOT creates covered directory subtree root caps for
      explicit path attenuation
done: VFS roots without `resolve` cannot traverse child paths, even when they
      retain read rights
done: directory handles can enumerate child vnode entries with stable vnode ids
      through native SYS_VFS_READDIR
done: explicit VFS mount objects exist for rootfs, storefs, volatile state,
      devfs, and procfs, and runtime inspect reports their root vnode, source,
      flags, and dynamic status
done: SYS_VFS_MOUNT and SYS_VFS_UNMOUNT create and remove empty volatile
      mounted directory roots with explicit mount authority and busy-root
      rejection
done: manifest-granted VFS root authority can create, write, read, and unlink
      a volatile regular memory file below /state
done: SYS_VFS_RENAME requires explicit rename authority on both source and
      destination parent directories, moves volatile memory-file vnodes without
      changing vnode identity, and rejects destination replacement
done: unlink rejects directories, non-volatile VFS nodes, and non-empty subtrees
done: direct legacy SYS_OBJECT_READ is rejected in VFS mode
done: inspect reports vnode id, kind, parent, backing, mount source, owner process,
      live handle slot, open-file description, rights, flags, offsets, and refs
done: VFS syscalls reject M59 namespace caps as filesystem authority
done: VFS syscalls return typed STATUS_VFS_* values for filesystem errors
      instead of collapsing them into generic bad-capability status
done: compact native `state_volumes` records are accepted without direct
      `OBJECT_STATE` grants and are installed as explicit VFS state-volume
      mount roots below `/state/<state-id suffix>`
done: manifest-declared state volumes expose `/state/<suffix>/value` as regular
      VFS files; read/write/stat operations validate user buffers, block the
      caller, and transact with `vertex-state` through a native versioned request
      carrying the state id instead of a hardcoded state name
done: service-backed state-value stat asks `vertex-state` for the durable
      current value length and returns a normal 64-byte VFS stat record
done: `/state/counter/control` is a VFS control file for native shutdown, so
      clients no longer need a direct state-service endpoint to terminate the
      demo state service, and opening it requires explicit `control` authority
done: `vertex-state` serves VFS state read/write/stat requests and persists writes
      for every indexed VertexDisk state volume through the journal/data/index
      block protocol
deferred: /state/a remains a volatile kernel memory-file fixture below an
          explicit volatile mount, while state-volume value files are durable
          service-backed files
deferred: general VertexFS metadata transactions, non-state service-backed
          metadata routing, and full crash recovery are M78-M81 work rather
          than M74 substrate requirements
```

Implementation notes:

- Keep the VFS name and object authority in the kernel, but do not put a full
  filesystem implementation in the kernel yet. Filesystem services should own
  persistence and metadata policy behind typed kernel objects.
- Do not introduce POSIX compatibility as a hidden requirement. The ABI should
  expose Krust-native file authority first; a POSIX personality can translate
  later.
- The boot manifest should select the VFS ABI generation explicitly. Do not
  accept older compact payloads as a compatibility mode.

## M75: Open Files, Handles, And Descriptor Lifecycle

Status: done.

Goal: make open files first-class kernel-owned handles with explicit lifetime,
rights, offsets, blocking behavior, and cleanup on process exit or fault.

Scope:

```text
per-process handle table separate from capability slots
open file descriptions with offset, rights, flags, and vnode reference
open, close, read, write, pread, pwrite, seek, stat, sync syscalls
handle duplication and attenuation rules
O_APPEND, O_TRUNC, O_CREATE equivalent semantics as native flags
nonblocking whole-file advisory lock and unlock syscalls
process-exit and fault cleanup for open handles
bounded per-process and global handle quotas
```

Acceptance tests:

```text
read-only handle rejects write even when another process has write authority
duplicated handle shares offset only when explicitly requested
pread and pwrite do not mutate the shared offset
process exit closes all handles and releases vnode references
faulted process cannot leave a file locked or pinned
handle quota exhaustion fails without leaking handles or vnodes
close of invalid or already closed handle returns a controlled error
```

Current implementation state:

```text
done: per-process VFS handle table is separate from capability slots
done: global open-file descriptions hold vnode, rights, flags, offset, owner,
      and reference count
done: open, close, read, write, pread, pwrite, seek, stat, sync, dup syscalls
      are allocated in the ABI
done: duplicated handles share offsets only with an explicit dup flag; otherwise
      the offset is copied into an independent open-file description
done: native open-create creates missing volatile memory files only after
      validating create authority and requested file-handle rights
done: open-create handle-quota failure rolls back the newly created vnode and
      memory-file backing instead of leaking an unreachable file
done: trunc and append open flags are implemented for volatile memory files
      and covered by the smoke gate
done: unlink of an open volatile memory file detaches the pathname while the
      existing handle remains readable until final close
done: close, process exit, process fault, restart reload, kill, and reap paths
      release VFS handles/open-file descriptions
done: nonblocking whole-file advisory locks reject conflicting open-file
      descriptions, are shared by shared-duped handles, and are released on
      final close or process cleanup
done: the user-fault restart gate proves a faulted process cannot leave a VFS
      advisory lock behind
done: per-process handle quota is bounded and rejected without leaking handles
done: invalid close returns a controlled error
done: create/unlink are implemented for manifest-granted volatile memory files
      and covered by the smoke gate, including open-unlink final-close cleanup
done: `/proc/log-stream` is a live VFS pipe; read on an empty stream blocks the
      process in `blocked-vfs` state and the next kernel log write copies bytes
      into the saved user buffer and wakes the reader
done: service-backed state-volume value-file read, write, and stat use saved
      syscall frames, transact through kernel-owned
      `state-vfs-request`/`state-vfs-reply` endpoints, and wake after
      `vertex-state` completes the state-id-scoped transaction
done: `/state/counter/control` uses the same blocked VFS transaction machinery
      for service shutdown
done: write/pwrite/trunc/append and create/unlink are implemented for the
      volatile memory-file backing; general durable metadata variants are M78
      work
done: rename exists for volatile memory-file vnodes with stable vnode identity;
      general durable filesystem-service rename is M78 work
done: blocking file behavior exists for the live log-stream pipe and the
      service-backed state-volume value files
deferred: durable filesystem-service lock integration, non-state
          filesystem-service transaction routing, and VertexFS metadata
          integration are M78-M81 work rather than M75 handle-lifecycle
          requirements
```

Implementation notes:

- Keep capability slots as authority to obtain handles, not as the handles
  themselves. That distinction matters for revocation, restart, and audit.
- Use the existing M66-M69 leak accounting discipline for handle/vnode
  lifecycle from the first implementation.
- Avoid inherited Unix assumptions such as ambient current working directory
  until mount namespaces and process profiles define them explicitly.

## M76: Directory Operations And Atomic Metadata Changes

Status: done for the current Krust VFS substrate.

Goal: support a useful directory tree with atomic create, unlink, rename, link,
and metadata update semantics backed by filesystem-service transactions.

Scope:

```text
component-by-component path walk with bounded path and component lengths
directory entry cache with generation validation
create file, mkdir, unlink, rmdir, rename, hard-link policy
stat metadata: type, size, generation, link count, timestamps or monotonic version
atomic same-filesystem rename
negative lookup caching with invalidation
metadata operation journal records
```

Acceptance tests:

```text
rename is atomic: readers see old name or new name, never a missing middle state
rename cannot cross filesystem or independent volatile mount boundaries
unlink of open file keeps existing handle readable until final close
rmdir rejects non-empty directory
hard links cannot cross filesystem boundaries
path traversal cannot escape a service's namespace root
very long paths and components are rejected before allocation side effects
crash during create/unlink/rename replays to a valid directory tree
```

Current implementation state:

```text
done: component-by-component path validation rejects empty, `.`, `..`, overly
      long, and trailing-slash components before allocation side effects
done: stat records are 64 bytes and report type, size, vnode id, rights,
      monotonic metadata version, and link count
done: same-filesystem volatile rename is atomic inside the kernel VFS node graph
      and bumps the vnode metadata version while preserving open handles
done: cross-mount rename is rejected before changing the VFS node graph
done: SYS_VFS_MKDIR and SYS_VFS_RMDIR create directories and reject non-empty
      or open directories with controlled VFS errors
done: unlink of an open volatile file detaches the path, keeps existing handles
      readable, and reaps the vnode/backing on final close
done: SYS_VFS_LINK creates same-filesystem volatile hard links, shares the
      memory-file backing, reports link count, rejects cross-filesystem links,
      and rejects links across independent volatile mount instances
done: metadata versions for hard-linked volatile files advance for every vnode
      sharing the backing on write, truncate, link, and unlink
done: scripts/krust-test.sh m76 covers rename metadata, mkdir/rmdir,
      open-unlink, hard-link metadata, mount-instance hard-link policy, long
      path rejection, traversal denial, and cross-mount rename denial
deferred: persistent metadata journals for a general on-disk filesystem move to
          M78 VertexFS rather than being bolted onto the volatile fixture
```

Implementation notes:

- Treat metadata correctness as security-critical. Path lookup must never fall
  back to string-prefix checks once vnodes exist.
- Keep cross-filesystem rename out of scope. Return a clean error and require
  copy plus unlink at a higher layer later.
- If timestamps are awkward under the current timer model, use monotonic
  metadata versions first and document the limitation.

## M77: Block Cache, Page Cache, And Writeback

Status: done for the VertexDisk state-volume block path.

Goal: add a bounded cache layer for the VertexDisk state-volume block path with
explicit dirty tracking and ordered synchronous writeback before state VFS
responses are released.

Implemented scope:

```text
state-volume block cache keyed by logical VertexDisk sector
dirty/clean state for state superblock, index, journal, and data sectors
ordered journal, data, and index writeback through block-driver
clean-entry replacement inside a fixed eight-entry cache
dirty-entry non-eviction by failing the state service if no clean slot is available
writeback error accounting on failed cache writeback paths
cache inspection transcript with dirty, pinned, and writeback-error counts
```

Release-gate acceptance tests:

```text
repeated state-volume reads hit the vertex-state block cache
state writes dirty journal/data/index entries, write them back, and reply only after clean writeback
cache inspect reports dirty=0, pinned=0, writeback_errors=0 after the exercised write/read path
cache policy probe proves clean-entry eviction under pressure
cache policy probe proves dirty pages are not selected for eviction under pressure
cache policy probe drives the failed cache-writeback branch, leaves the dirty page dirty, and increments error accounting
state write and read transactions complete through the VFS state-service path
```

Current implementation state:

```text
done: vertex-state owns an eight-entry bounded block cache keyed by logical
      VertexDisk sector for state superblock, index, journal, and data sectors
done: repeated state-volume reads hit the cache and log an inspectable cache-hit
      transcript for the release gate
done: state writes dirty the journal/data/index cache entries, synchronously
      write them back through block-driver, and mark them clean before the VFS
      transaction response is sent
done: clean entries can be evicted by bounded replacement; dirty entries are not
      evicted and instead fail the service if no clean slot is available
done: the cache policy probe forces the failed cache-writeback branch without
      issuing stale fallback I/O, verifies the page stays dirty, and verifies
      writeback-error accounting increments
done: scripts/krust-test.sh m77 asserts cache hit, writeback clean, dirty=0,
      pinned=0, writeback_errors=0, clean eviction under pressure, dirty-page
      non-eviction, failed-writeback dirty retention, writeback-error
      accounting, state write, and state read transcripts
done later: VertexFS fsync syscall semantics land in M78 and the bounded
      VertexFS vnode page-cache/writeback layer lands in M93
deferred: pending VFS transaction cleanup after filesystem-service exit,
          block-driver restart cache ownership tests, and broader
          filesystem-service writeback semantics belong to later
          fault/soak gates
```

Implementation notes:

- Do not optimize before the crash and memory-pressure behavior is boring.
  Correct dirty tracking and bounded memory use matter more than throughput.
- Keep DMA buffers and cache pages separate unless a later design proves safe
  zero-copy ownership transfer.
- The cache should be inspectable enough for release gates to assert dirty
  counts, pinned counts, writeback errors, and high-water marks.

## M78: VertexFS v1 Persistent Filesystem

Status: done for the current VertexFS v1 substrate; image-backed mount, fixed
journal-sector replay, device-backed fsync fault/restart handling,
declared-file journal checkpoint recovery, and initial VFS operations are
release-gate covered.

Goal: define and implement the first general-purpose native on-disk filesystem
format for Krust, replacing the special-purpose VertexDisk store/state layout
for normal file workloads.

Scope:

```text
VertexFS superblock with feature flags and generation identity
inode table or equivalent object table
directory record format
extent or block-map file data layout
free-space allocator with checksums
metadata journal or copy-on-write transaction log
offline verifier and image builder in vertexctl
```

Acceptance tests:

```text
fresh VertexFS namespace mounts and exposes declared root files
file create/write/fsync/readback preserves data and metadata in the mounted namespace
corrupt superblock is rejected without mounting
corrupt directory block is detected and reported
corrupt free-space metadata cannot allocate overlapping file extents
journal replay after interrupted write and every declared-file fsync checkpoint
returns the new file
vertexctl can build, inspect, and verify a VertexFS image reproducibly
```

Current implementation state:

```text
done: `/fs` is mounted as a distinct `vertexfs` source rather than an alias for
      storefs, state-volume, or volatilefs
done: VertexFS regular files use a separate `VfsBacking::VertexFsFile` table
      with dirty state and checksums, so creates inside `/fs` do not fall back
      to volatile memory-file backing
done: `/fs/readme` and `/fs/app/a` are declared at boot and exposed through
      ordinary VFS open/read/stat paths
done: the kernel loads the `vertexfs-v1` Limine module, validates exact image
      size, superblock magic/version/sector size/feature flags, expanded
      multi-sector inode and directory section layout, checksums, inode
      records, 52-byte directory-name records, file extents, free-space
      allocation bytes, journal sector metadata, and file payload checksums
      before mounting `/fs`; pending v1 journal records can cover only the
      exact target file payload they name
done: model-reader opens `/a` from its declared `/fs/app` mount root, reads the
      declared VertexFS file, rewrites `/a`, calls `SYS_VFS_SYNC`, reopens it,
      and verifies the journaled declared-inode readback; it also creates
      `/created` through `/created11`, writes them, syncs expanded dynamic
      inodes 5-15, reopens them, and verifies mounted-namespace readback
done: `SYS_VFS_SYNC` on a VertexFS v1 inode writes a pending journal record into
      the runtime shadow image, rewrites file extents and metadata checksums,
      clears the journal sector, queues the ordered journal/data/metadata sector
      writes through the block driver, and clears the dirty bit only after every
      device write is acknowledged
done: bounded dynamic VertexFS create under `/fs/app` allocates expanded v1
      dynamic slots in order: inodes 5-15, directory entries 4-14, data sectors
      9-19, and matching free-map bits in the shadow image; `SYS_VFS_SYNC`
      commits each created file through the same journaled transaction path,
      and the twelfth create returns no space instead of falling back to
      volatile storage
done: `vertexctl create-vertexfs`, `inspect-vertexfs`, and `verify-vertexfs`
      build reproducible VertexFS v1 images with a strict superblock, inode
      table, directory block, extent records, free-space map, journal-v1
      feature flag and sector, generation id, and checksums
done: `vertexctl create-vertex-disk` writes a strict `VERTEXDISKV1` version-3
      VertexDisk image with explicit VertexFS and graph-store sections; the
      kernel, block-driver, vertex-store, and vertex-state reject the old
      `VERTEXDISKV0`/version-1 identity instead of parsing it as a fallback
done: block-driver validates the VertexDisk VertexFS section, reads its
      VertexFS v1 superblock from the real virtio-backed image, writes the same
      sector back, and verifies readback before serving client requests
done: the kernel owns `vertexfs-device-request` and `vertexfs-device-reply`
      endpoints, grants them only to `block-driver`, and routes VertexFS fsync
      writeback sectors through that device path instead of treating the runtime
      shadow image as durable state
done: `vertexctl corrupt-vertexfs` produces bad-superblock, bad-directory,
      overlapping-extent, and free-space-overlap fixtures that the verifier
      rejects without falling back to VertexDisk or volatile metadata
done: `vertexctl corrupt-vertexfs interrupted-journal` produces a verifiable
      pending journal record; the kernel replays it during mount, and
      model-reader accepts the resulting new `/fs/app/a` file content
done: `vertexctl corrupt-vertexfs journal-checkpoint-after-journal`,
      `journal-checkpoint-after-data`, and `journal-checkpoint-after-inode`
      produce verifiable declared-file fsync checkpoint images for the current
      v1 sector order without accepting unrelated checksum mismatches
done: `vertexctl update-vertexfs-file` performs a strict journaled update
      transaction on a current VertexFS image: it writes a pending journal
      record, updates the file extent and inode checksum, clears the journal,
      and verifies the committed image
done: scripts/krust-test.sh m78 gates the VertexFS mount, declared-file read,
      VertexDisk-backed VertexFS section read/write proof, declared-inode
      fsync device transaction, dynamic created-file inodes 5-15
      fsync/readback, strict expanded dynamic capacity denial, reproducible
      image build, inspect/verify, and corrupt metadata rejection
done: scripts/krust-test.sh m78-bad-superblock boots a corrupted VertexFS image
      and verifies the kernel rejects it during native runtime init without
      falling back to an in-kernel fixture
done: scripts/krust-test.sh m78-journal-replay boots an interrupted-journal
      image and proves replay returns the new declared file contents
done: scripts/krust-test.sh m78-journal-checkpoint-after-journal,
      m78-journal-checkpoint-after-data, and
      m78-journal-checkpoint-after-inode boot every pending-journal checkpoint
      before the clean-journal sector is written and prove remount reads the
      replayed new declared-file contents
done: scripts/krust-test.sh m78-post-sync-remount boots a committed post-sync
      VertexFS image and proves the kernel remount reads the committed file
      contents
done: scripts/krust-test.sh m78-fsync-fault injects a block-driver exit during
      a VertexFS fsync, proves the kernel aborts the blocked transaction with
      STATUS_VFS_UNSUPPORTED, keeps the runtime dirty file readable without
      committing stale device writes, and restarts block-driver under the
      declared on-failure policy
```

Implementation notes:

- Pick one durable update model for v1. A small journal is acceptable; a small
  copy-on-write tree is acceptable. Mixing both early will make recovery harder.
- Include an on-disk feature/version field and reject unsupported versions.
  Do not silently mount older experimental layouts.
- Keep immutable verified store objects available as files, but move the common
  persistent namespace to VertexFS instead of expanding special-purpose store
  records.

## M79: Mount Namespaces And Filesystem Services

Status: done for the current mount-namespace and initial filesystem-service
substrate; declared mount roots, declared bind-mount snapshots, dynamic bind
cleanup, and the read-only `servicefs` route are release-gate covered.

Goal: make mount trees explicit per service or service group, with filesystem
services, block devices, and synthetic files connected through capability
grants rather than global ambient paths.

Scope:

```text
mount objects with source filesystem, root vnode, target vnode, and flags
per-process mount namespace root
bind mounts and read-only mounts
read-only service-backed filesystem file over the current `FS` request envelope
kernel-owned device request/reply route for VertexFS fsync writeback
mount and unmount rights with busy checks
manifest syntax for initial mount namespaces
```

Current release-gate acceptance tests:

```text
two services can see different files at the same absolute path
read-only bind mount rejects write through all aliases
unmount of busy filesystem is rejected without losing references
service without mount right cannot attach a filesystem
service-backed filesystem file exposes runtime data without process-control rights
service-backed filesystem file rejects write-capable opens
restart restores the service's declared mount namespace exactly
restart drops process-owned dynamic bind mounts
```

Current implementation state:

```text
done: compact process records carry validated `mountRoot` values; hosted
      `vertexctl` rejects services that omit `mountRoot`, and Krust resolves all
      VFS syscall paths through the current process mount root before authority
      checks
done: M79 compact process records carried explicit per-process declared
      bind-mount snapshot entries; M82 supersedes that compact identity with
      `KRUSTBOOTM82` version 13, and old M75/M79 records are rejected instead
      of parsed as compatibility input
done: hosted `vertexctl` validates service `mounts` entries as explicit
      `{path, source, readOnly}` records and rejects malformed or implicit mount
      records before emitting a boot manifest
done: model-reader is rooted at `/fs/app` and echo remains rooted at `/state`;
      both can use `/a` as a service-local absolute path without escaping to a
      global ambient root
done: mount objects expose source filesystem, root vnode, root path, flags, and
      dynamic state through inspect output
done: dynamic volatile mounts still require explicit mount authority and busy
      unmount checks; `servicefs` and VertexFS device routes remain
      endpoint/capability-mediated
done: `SYS_VFS_MOUNT` supports strict bind mounts and read-only bind flags;
      `echo` proves `/state` bound at `/ro` remains readable while write,
      create, and unlink through the read-only alias are rejected
done: read-only bind flags propagate when a read-only alias is rebound; `echo`
      derives a root at `/ro`, requests a nested writable bind at `/ro/nested`,
      and proves writes and creates through the nested alias are still denied
done: writable bind aliases are release-gated; `echo` writes through `/rw/a`,
      reads the change through `/a`, creates metadata through `/rw`, observes it
      through the source path, unlinks it through `/rw`, and unmounts cleanly
done: bind unmount checks live handles in the mounted subtree and rejects
      unmount while a file opened through the bind alias is still live
done: echo restart logs that its declared `/state` mount root was restored
      after process reload
done: echo declares `/declared-ro` as a read-only bind of its service-local root
      `/`; kernel process creation restores the owned declared bind mount from
      the immutable process template, echo proves the alias is readable and
      write-denied, process reap removes the owned declared bind mount, and the
      restarted echo process proves `/declared-ro` was restored from the
      manifest snapshot
done: process-owned dynamic bind mounts are reaped on process restart; `echo`
      leaves `/restart-bind` mounted before exit and the restarted process
      proves the alias is not inherited
done: kernel-owned VertexFS device endpoints are not ambient filesystem service
      authority; only `block-driver` receives the request/reply caps used by the
      current VertexFS fsync writeback route
done: `/state/service-report` is mounted from the current `servicefs` source as
      a read-only file; kernel VFS queues the native `FS` version-1 service-read
      request to `vertex-state`, wakes the blocked reader from the state VFS
      reply endpoint, rejects write-capable opens, and never treats old short
      commands as a compatibility protocol
done: scripts/krust-test.sh m79 gates distinct service-local roots and restart
      mount-root restoration, explicit mount authority, volatile busy unmount,
      direct and nested read-only bind denial, writable bind alias propagation,
      bind busy-unmount behavior, declared bind-mount snapshot restoration, and
      process-owned dynamic bind cleanup on restart, plus the `servicefs`
      request/reply file route
```

Implementation notes:

- Mount namespaces should reuse the M59 capability namespace lesson but operate
  on VFS vnodes and mount objects, not arbitrary string-to-cap records.
- Keep global `/` as an implementation detail. Service profiles should receive
  explicit roots and mounts from the manifest.
- Synthetic filesystems must be read-only unless a specific write protocol and
  authority are defined.

## M80: File Locking, Notifications, And Streaming Objects

Status: done.

Goal: add the coordination primitives needed for real services: advisory file
locks, filesystem change notifications, pipes, and stream-like device files.

Scope:

```text
advisory whole-file locks and byte-range locks
poll readiness for readable, writable, and metadata-change events
pipe objects with bounded buffers and backpressure
device-file read/write dispatch through typed driver protocols
directory watch events for create, unlink, rename, and writeback errors
deadlock-resistant lock cleanup on process exit
```

Acceptance tests:

```text
conflicting exclusive locks block or fail according to requested mode
process exit releases locks and wakes waiters
pipe writer blocks or returns would-block when buffer is full
pipe reader blocks until stream data arrives and empty pipe polls as not readable
directory watcher receives create, rename, and unlink events in order
poll on file and pipe handles reports readiness only for authorized handles
faulted process cannot strand a lock, pipe endpoint, or watcher
```

done: M80 advisory file locks, directory watch events, bounded pipe buffering,
      and VFS poll readiness are checked by scripts/krust-test.sh m80
done: byte-range exclusive locks allow disjoint ranges and reject overlapping
      ranges with `STATUS_VFS_BUSY`
done: directory watchers deliver create, rename, and unlink records in order
      through per-open-description watch cursors
done: `/proc/log-stream` keeps blocking VFS reads and now reports empty-pipe
      readiness accurately before the writer wakes the reader
done: VFS poll rejects unauthorized readable/writable event requests before
      reporting readiness
done: process-exit lock cleanup remains covered by the M31 user-fault/restart
      test path, which reacquires a VFS lock after fault cleanup

Implementation notes:

- Start with advisory locks only. Mandatory locks create surprising authority
  interactions and should not be in the first mature VFS pass.
- Reuse scheduler blocking primitives from IPC/IRQ/timer instead of inventing a
  separate wait subsystem.
- Device files should remain authority wrappers around typed device services,
  not a backdoor to raw hardware.

## M81: Filesystem Crash, Security, And Soak Gate

Status: done.

Goal: prove the VFS/filesystem stack survives hostile paths, process faults,
device faults, interrupted writes, restarts, and repeated mount/open/write
cycles without leaks, authority bypass, or persistent corruption.

Scope:

```text
malformed path and syscall argument matrix
capability revocation while handles and mounts are live
filesystem-service kill/fault during metadata transaction
block-driver fault during dirty writeback
forced power-loss checkpoints in QEMU image tests
long-run create/write/rename/unlink/open/close soak
fsck/verifier integration into release gate
operator-visible filesystem health report
```

Acceptance tests:

```text
revoked directory authority prevents new opens but preserves existing handle semantics
revoked file authority prevents handle duplication and new opens
filesystem-service fault rolls back or completes the active transaction
block-driver fault during fsync reports error without marking dirty data clean
100-cycle file churn returns to baseline handle, vnode, frame, and cache counts
forced crash at each journal checkpoint remounts to a verified filesystem
path traversal, integer overflow, and bad user buffers are rejected before side effects
release gate checks VFS/fs leak deltas and verifier output
```

done: M81 capability revocation with live handles, 100-cycle VFS churn,
      hostile path/buffer rejection, VertexFS verifier cases, journal checkpoint
      remount checks, and fsync-fault reporting are release-gate covered
done: revoked VFS-root authority prevents new opens while existing handles keep
      their original file semantics
done: handles opened through revoked authority cannot be duplicated into fresh
      live authority
done: scripts/krust-test.sh m81 gates create/write/rename/open/read/close/unlink
      churn for 100 cycles and the hostile path/syscall argument summary
done: scripts/krust-test.sh m78-bad-superblock, m78-journal-replay,
      m78-journal-checkpoint-after-journal,
      m78-journal-checkpoint-after-data,
      m78-journal-checkpoint-after-inode, m78-post-sync-remount, and
      m78-fsync-fault keep filesystem crash/fault coverage in the release gate
done: release-gate documentation checks require the M80 and M81 proof lines
      before running the QEMU matrix

Implementation notes:

- This milestone is the equivalent of M66-M73 for VFS. Do not treat the
  filesystem as mature until this gate is boring.
- The verifier should be useful both offline through `vertexctl` and in the
  QEMU gate against the post-test disk image.
- Preserve the capability model: revocation and namespace restrictions must be
  tested against handles, cached vnodes, mount aliases, and synthetic files.

## M82: Native Graph Store

Status: done.

Goal: make the booted system persist and query its own generation graph as a
native, typed VertexDisk graph-store object instead of treating the graph as
only a host-compiled boot artifact that disappears after activation.

Implemented:

```text
done: KrustBoot compact payload identity is `KRUSTBOOTM82` version 13
done: hosted `vertexctl` emits native typed graph records for generation,
      service, endpoint, store-object, config, state-volume, device,
      namespace, vfs-root, timer, secret, activation, and capability edges
done: graph node and edge counts plus checksum live in the compact graph header
done: VertexDisk v1 carries a required graph-store object section with graph
      magic, generation identity, node/edge counts, checksum, BLAKE3 hash, and
      typed graph records
done: Krust validates graph checksum, graph record bounds, node references,
      duplicate node/edge IDs, and graph coverage for services, endpoints,
      store/config/state/device objects, VFS roots, and grants before activation
done: kernel imports runtime graph nodes and edges from the VertexDisk graph
      object after cross-checking it against the compact boot graph hash and
      checksum
done: kernel runtime config stores the VertexDisk graph-store hash, checksum,
      source, nodes, edges, process graph nodes, and capability graph provenance
done: runtime inspect exposes graph-store generation, hash, checksum, object
      counts, representative graph nodes, process graph nodes, and capability
      graph edges to inspect-authorized userspace with `source=vertexdisk`
done: `vertex-inspect` queries the native runtime report through inspect-only
      authority and proves generation, service, store-object, state-volume, and
      device graph nodes plus process/capability graph provenance
done: `vertex-init` delegates graph inspect authority to `vertex-inspect`
      without delegating manifest-read authority
done: corrupt compact graph-store checksum and invalid graph-record inputs are
      rejected as malformed KrustBoot manifests before activation
done: corrupt VertexDisk graph-store objects are rejected before native runtime
      activation
done: old compact payload identities are rejected rather than accepted as
      native graph-store fallbacks
```

Acceptance tests:

```text
done: scripts/krust-test.sh m82
done: scripts/krust-test.sh m82-vertexdisk-graph-corrupt
done: scripts/krust-test.sh manifest-graph-store-checksum
done: scripts/krust-test.sh manifest-graph-store-record
done: scripts/krust-test.sh manifest-old-compact-magic
done: scripts/krust-test.sh m38 remains compatible with the current inspect
      generation graph
```

Implementation notes:

- Treat the VertexDisk graph-store object as the OS truth, not as
  documentation for the kernel tables. Kernel tables should remain the
  enforcement cache derived from the graph.
- Keep host `vertexctl` useful for building and debugging graph images, but the
  booted system must be able to inspect the graph without host assistance.
- Do not introduce JSON parsing in the kernel. Prefer a compact typed graph
  object format validated by userspace services and exposed to the kernel only
  through narrow boot/runtime records.
- M82 deliberately updates stale inspect-generation graph data to the current
  declared mount contract instead of carrying a compatibility path in echo,
  vertex-init, or the kernel.
- Durable native graph mutation and graph-store garbage collection remain
  M84-M89 responsibilities; M82 establishes the booted system's durable typed
  graph-store read/provenance substrate, and M83 uses that substrate for native
  candidate generation installation.

## M83: Native Generation Manager

Status: done.

Goal: move generation install, activation, rollback, and failure recording into
a native service so the running Vertex system can manage generations without
host-side activation glue.

Scope:

```text
done: native generation-manager service receives install, rollback, and
      shutdown commands through explicit endpoint authority
done: vertex-init delegates generation update authority to generation-manager
      instead of retaining it in the supervisor process
done: candidate generation manifests are stored as deterministic native
      VertexDisk store objects (`store:krustboot:<generation>`)
done: VertexDisk generation metadata records selected, previous, known-good,
      transaction state, transaction target, failure reason, and candidate
      generation IDs
done: kernel registers candidate generation configs from VertexDisk metadata
      and graph-store/store-closure input without requiring a host fallback
      boot-module manifest
done: generation-manager install verifies manifest hash and store closure
      before switching to the candidate generation
done: generation-manager writes prepare/commit/rollback/abort generation
      metadata checkpoints through native block-driver authority, scoped to the
      VertexDisk generation metadata section
done: generation-manager asks the kernel to verify and stage/build the
      candidate runtime before committing durable selected-generation metadata
done: kernel activation and rollback enter previously staged generation
      runtimes, so runtime construction failure aborts before durable commit
done: rollback returns to the previous known-good generation and preserves the
      counter state according to the current graph state policy
done: activation failure records service, dependency, policy, and reason
      details in the native generation-manager state and runtime inspect report
done: VertexDisk recovery metadata handles prepare, commit, and rollback
      checkpoints and remounts to exactly one selected generation on boot
done: console shutdown waits for finite state clients to drain before issuing
      the state-service control write, so generation activation completion does
      not race in-flight state VFS transactions
done: richer state migration, rollback, retention, sharing, and health-report
      policy moves to the M85 state-object implementation
```

Acceptance tests:

```text
done: booted system installs a new candidate generation from native graph-store input
done: selected generation changes durably only after candidate verification
      and staged runtime construction succeed
done: failed activation restores the previous known-good generation
done: activation failure records service, dependency, and policy error details
done: rollback preserves state according to the current graph policy
done: power loss during prepare, commit, and rollback remounts to one selected generation
done: host-side activation metadata is not required for native generation switching
done: live generation-manager writes are served by the native block-driver and
      are limited to the VertexDisk generation metadata object
```

Acceptance gates:

```text
done: scripts/krust-test.sh m83
done: scripts/krust-test.sh m83-hostless
done: scripts/krust-test.sh m83-power-prepare
done: scripts/krust-test.sh m83-power-commit
done: scripts/krust-test.sh m83-power-rollback
```

Implementation notes:

- The generation manager should own generation-selection state; the kernel
  should enforce only the minimal boot-selection and capability checks needed
  to keep the manager honest.
- Reuse the M44-M46 boot-manager/update transaction lessons, but remove any
  special case that assumes one hard-coded example generation.
- Rollback policy must be graph data, not activation script convention.
- The current implementation makes generation selection durable and recoverable
  as VertexDisk metadata. The live path is native: `gen-manager` holds only
  scoped send/receive endpoint authority, `block-driver` validates the request
  against the generation metadata section, and the kernel verifies candidate
  generation closure and stages a buildable runtime before the manager commits
  selected-generation metadata.
- M83 intentionally kept migration policy minimal. M85 now carries the richer
  state-object migration, rollback, retention, sharing, and health-report path.

## M84: Native Package And Closure Import

Status: done.

Goal: make package graph fragments and closure materialization native OS
inputs, so packages can be imported, verified, and linked inside the running
system.

Scope:

```text
done: native package-import service
done: package import runs under the current `KRUSTBOOTM86` version 19 strict compact payload
done: package graph fragment parser for the compact typed graph format
done: store-object hash verification before materialization
done: closure linking against existing graph-store objects
done: authority-delta report for imported services
done: rejection of undeclared dependencies, excess grants, and missing store objects
done: base generation excludes the imported active service graph; candidate generation adds it
done: candidate graph-store can be present without initial installable generation metadata
done: package import registers the candidate generation in native metadata before install
```

Acceptance tests:

```text
done: native package import adds a service graph fragment to a candidate generation
done: store-object hashes are verified before executable or config use
done: missing dependency rejects the package without generation-manager register/install requests
done: package requesting undeclared authority is rejected without generation-manager register/install requests
done: duplicate package import is idempotent and does not duplicate store objects
done: imported package can be activated and then removed by generation rollback
done: graph-link closure hash is canonical across package input order
done: scripts/krust-test.sh m84
```

Implementation notes:

- Keep package import separate from activation. Importing a package should not
  grant authority or start a process until a generation transaction activates
  the resulting graph.
- Packages should carry graph fragments, not imperative install scripts.
- The first target is deterministic closure materialization, not a public
  package ecosystem.
- The current implementation intentionally uses a compact deterministic
  `pkg:logd` fragment and config proof object as the native proof path. General
  package removal, dependency solving, and persistent graph-store mutation are
  later work.

## M85: State Objects And Migration Policy

Status: done.

Goal: make mutable state a declared graph object with explicit ownership,
schema, migration, retention, sharing, and rollback policy.

Scope:

```text
state-object graph records with owner, schema version, and storage class
declared state sharing and read/write authority
state migration plan attached to generation transitions
state retention policy with unsafe delete modes rejected until durable GC exists
rollback semantics for preserve, fork, discard, and migrate cases
state health report and migration journal
```

Acceptance tests:

```text
generation switch runs a declared state migration exactly once
failed migration rolls back graph selection and leaves old state readable
service cannot open undeclared state even when a path alias exists
shared state requires explicit graph sharing policy and attenuated rights
rollback follows state policy rather than blindly preserving all volumes
delete-when-unreferenced retention is rejected until durable state deletion exists
state health reports owner, schema, generation, migration status, and last error
done: scripts/krust-test.sh m85
```

Implementation notes:

- done: state-volume graph records now carry owner, schemaVersion,
  storageClass, migrationPolicy, retentionPolicy, and sharingPolicy in the
  M85 compact payload, graph labels, native boot parser, and runtime config
- done: host validation rejects missing state policy, unsupported policy modes,
  unknown sharing services, non-owner state use without explicit sharing, and
  state path aliases without matching read/write/control sharing authority
- done: native boot validation rejects compact state records that omit owner,
  schema, storage, migration, retention, or sharing policy fields
- done: generation staging validates the complete state transition before any
  migration journal is emitted, accepts `migrate` schema changes at commit,
  aborts bad migrations, and leaves selected_generation unchanged
- done: rollback logs state policy application during staging and rollback
  journal records after commit instead of blindly preserving every state object
- done: runtime inspection emits state-policy and state-health records with
  owner, schema, storage, migration, retention, sharing, generation,
  migration status, and last error
- done: hosted supervisor state grants carry explicit rights, allow
  writer/controller non-owner consumers, and reject reader-only directory grants
  because hosted paths cannot enforce read-only attenuation
- done: `scripts/krust-test.sh m85` boots
  `gen:state-migration-0001`, rejects
  `gen:state-migration-bad-0003`, installs
  `gen:state-migration-new-0002`, rolls back to the base generation, verifies
  retention defers GC for referenced state, and queries state health before and
  after migration/rollback
- This is the point where Vertex must improve on `/var`: every mutable object
  should have a graph owner and lifecycle policy.
- Avoid migration scripts with ambient authority. A migration should receive
  only the old state, new state, and declared helper capabilities.
- State policy should be testable without relying on POSIX path conventions.

## M86: Native Policy Validation

Status: done.

Goal: make graph policy validation part of the trusted native activation path,
so invalid authority edges cannot be installed or activated even when the host
tooling is bypassed.

Scope:

```text
native policy validator gate in the boot/runtime activation path
typed capability edge validation against service requirements and providers
mount, device, state, config, and network authority checks
policy explain records for denied graph edges
activation-time policy hash in runtime inspection
negative matrix for malformed graph and hostile authority requests
```

Acceptance tests:

```text
done: native validator rejects missing providers before process creation
done: native validator rejects excess capability grants before process creation
done: device, network, state, config, and mount authority checks match kernel enforcement
done: policy explanation names the rejected edge, source node, and required rule
done: runtime inspection exposes a bounded native denial ring with generation, policy hash, source, target, rule, and reason
done: host-validated but tampered graph is rejected inside the booted system
done: policy hash is recorded in runtime inspection
done: legacy or unknown policy versions are rejected rather than interpreted loosely
```

Implementation notes:

- The strict compact payload is version 19. It appends a policy section after
  graph records: capability facts, requirement facts, provide facts, mount-root
  facts, declared-mount facts, state-path facts, bootstrap-authority facts, and
  a BLAKE3 hash over the policy records.
- The boot parser verifies policy version/hash and rejects any grant,
  `mount_root`, declared bind mount, state-bearing VFS root, secret grant, or
  synthetic bootstrap authority that is only graph/config-backed but not
  policy-backed. The installable-generation runtime validator repeats the same
  gate before generation-manager stage/install paths.
- Native validation emits denial records naming source, target, rule, and
  reason on the serial path and records a bounded native denial ring with
  generation and policy hash. `SYS_RUNTIME_INSPECT` exposes the accepted
  policy hash, fact counts, `policy-denials v=1`, and `policy-denial[...]`
  records.
- Keep the kernel enforcement model simple: it should consume validated compact
  records, not become the graph policy engine.
- No compatibility fallback for older policy formats. Add a new version only
  when the validator and gate understand it.
- Tests: `scripts/krust-test.sh m86`, `m86-policy-denial-report`,
  `manifest-policy-version`, `manifest-policy-hash`,
  `manifest-policy-excess-grant`, `manifest-policy-mount-root`, and
  `manifest-policy-state-root`.

## M87: Operator Graph Shell

Status: done.

Goal: build the native operator surface for an ideal NixOS-like graph OS:
asking what is running, why authority exists, what changed between generations,
and how to activate or roll back.

Scope:

```text
native graph-shell commands over console request/reply IPC
current-generation, generations, generation-status
diff-generation and planned-authority-delta
why service capability, who-can object, which-generation process
state-health, package-list, activation-log
activate, rollback, and mark-known-good commands through generation-manager authority
```

Acceptance tests:

```text
operator shell prints current generation from native graph-store state
diff-generation reports service, package, capability, state, and device changes
why command explains a live capability through graph policy provenance
who-can command lists all graph-authorized writers for a state object
activate command queues a candidate generation through generation-manager IPC
rollback command restores previous generation and reports state policy outcome
mark-known-good verifies the active generation and commits durable metadata
```

Done:

```text
done: runtime inspect emits operator-report, operator-generation,
      operator-service, operator-node, operator-edge, operator-capability,
      operator-requirement, operator-state, and operator-state-path facts for
      every registered generation
done: console-shell current-generation, generations, generation-status,
      diff-generation, planned-authority-delta, why, who-can,
      which-generation, package-list, activation-log, activate, rollback, and
      mark-known-good are backed by native graph/policy/runtime facts
done: diff-generation and planned-authority-delta report same-ID semantic
      changes instead of collapsing changed authorities or graph nodes into
      unchanged entries
done: why requires policy requirement, policy capability, graph edge, and a
      live non-revoked runtime capability whose graph target is the exact
      policy capability object with sufficient rights
done: mark-known-good rejects non-active or unverifiable generations and uses
      SYS_MARK_KNOWN_GOOD after generation-manager writes clean metadata
done: console-shell normalizes operator command verbs case-insensitively and
      reports explicit usage for missing or extra command arguments instead of
      falling through to unknown-command behavior
done: invalid operator proofs such as WHY a b are rejected explicitly, keep the
      shell process alive, return a prompt, and do not fall back to guessed
      answers
done: help and other multi-line shell output handle console IPC backpressure
      without killing the shell, and scripted M87 input proves a command after
      help still runs
done: shell commands that send to generation-manager, package-import,
      counter-service, or console-driver shutdown use bounded IPC backpressure
      retries instead of one-shot sends with command-specific failure behavior
done: malformed read-only operator commands for missing policy requirements,
      unsupported objects, and unknown generations reject explicitly and the
      M87 transcript proves the next valid shell command still runs
done: exact legacy shell commands are dispatched before generic operator verbs
      so M87 operator grammar does not change appliance-era commands such as
      `why svc:counter state:counter`
done: scripts/krust-test.sh m87
done: scripts/krust-release-gate.sh includes m87 in the default QEMU matrix
```

Implementation notes:

- This shell is not a POSIX shell. It is a graph operator console with explicit
  commands and explicit authority.
- Prefer structured command replies that can later back a richer UI.
- Every operator answer should identify the generation and policy hash it came
  from so stale answers are visible.

## M87-1: Vertex-Owned Operator Shell Package

Status: done.

Goal: make Vertex OS userland real by moving operator-shell semantics out of
the Krust tree while preserving the native Krust appliance behavior.

Scope:

```text
target-independent no_std userland/operator-shell package
Vertex.toml package metadata for service identity and target adapter
structured operator report parser with explicit errors
Krust console-shell as a thin syscall, IPC, logging, and console adapter
host unit tests for negative why proofs and same-ID authority diffs
```

Acceptance tests:

```text
operator-shell builds in the top-level Vertex workspace
Krust console-shell depends on userland/operator-shell by package path
why rejects live capabilities whose graph target does not match the policy object
diff-generation reports same-ID capability changes through the shared package
M87 QEMU transcript remains unchanged except for the richer diff fields
```

Done:

```text
done: userland/operator-shell exposes target-independent current-generation,
      generation-status, diff-generation, planned-authority-delta, why,
      who-can, which-generation, package-list, activation-log, activate,
      rollback, and mark-known-good request logic
done: userland/operator-shell/Vertex.toml declares pkg:vertex.operator-shell,
      svc:console-shell, and the Krust adapter path
done: targets/krust/user/console-shell keeps runtime transport only and calls
      vertex-operator-shell for operator graph answers
done: operator-shell unit tests cover mismatched graph_target rejection,
      same-ID authority changes, and target-neutral generation-manager requests
done: scripts/krust-test.sh m87 continues to validate the QEMU appliance path
```

Implementation notes:

- M87-1 does not add a kernel syscall. It changes source ownership: Krust owns
  enforcement and boot plumbing, while Vertex userland owns operator semantics.
- Future userland services should follow this package-plus-target-adapter
  pattern before moving code under a kernel target directory.

## M87-2: Krust Target User Adapter Workspace

Status: done.

Goal: make the OS/kernel split normal by moving Krust-built user programs out
of the kernel source tree and into a target-specific adapter workspace.

Scope:

```text
targets/krust/user workspace for Krust ABI adapter crates
kernel/krust remains kernel and substrate source only
Makefile builds Krust userspace from the target workspace
vertexctl store-object lookup uses the canonical target workspace path
release gate checks the target workspace and rejects old kernel-owned manifests
```

Acceptance tests:

```text
make -C kernel/krust build builds kernel plus targets/krust/user programs
vertexctl resolves native user binaries from targets/krust/user/target
kernel/krust/user/Cargo.toml is absent
kernel/krust/user/console-shell/Cargo.toml is absent
operator-shell Vertex.toml points to targets/krust/user/console-shell
release gate documentation checks mention M87-2 and targets/krust/user
```

Done:

```text
done: Krust user workspace moved from kernel/krust/user to targets/krust/user
done: kernel/krust/Makefile builds and cleans the target user workspace through
      KRUST_USER_DIR
done: vertexctl KrustBoot and VertexDisk store-object path resolution uses
      targets/krust/user as the canonical native userspace artifact location
done: userland/operator-shell/Vertex.toml declares the Krust adapter under
      targets/krust/user/console-shell
done: scripts/krust-release-gate.sh formats targets/krust/user crates and
      rejects legacy kernel-owned userspace manifests
done: targets/krust/README.md and targets/krust/user/README.md document the
      adapter boundary
```

Implementation notes:

- M87-2 does not add a kernel syscall or change runtime authority. It changes
  source layout and build ownership.
- `targets/krust/user` crates may contain Krust syscalls, IPC transport,
  linker files, and entry points. Portable Vertex behavior belongs in
  `userland/`.
- Keeping the adapter workspace outside `kernel/krust` prevents target-specific
  user programs from being mistaken for kernel responsibilities.

## M87-3: Vertex OS Root Boot Runner

Status: done.

Goal: remove the last user-facing kernel target leak from the normal boot path.

Scope:

```text
repository-root Makefile with Vertex OS commands
make run-gui as the supported QEMU window boot path
make run, iso, smoke, doctor, release-gate, and clean as root OS commands
explicit VERTEX_TARGET=krust selection for the current native target
release gate checks for the root runner and documentation
```

Acceptance tests:

```text
make run-gui dry-runs from the repository root without requiring -C kernel/krust
make iso dry-runs from the repository root and delegates to the selected target
unsupported VERTEX_TARGET values fail explicitly
README documents make run-gui as the OS-level boot command
release gate checks the root Makefile
```

Done:

```text
done: repository-root Makefile exposes Vertex OS boot/build/test commands
done: make run-gui delegates to the Krust target while keeping the user command
      at the OS root
done: make run keeps the existing headless serial path available as an OS-level
      command
done: scripts/krust-release-gate.sh checks root Makefile syntax, whitespace,
      docs, and dry-run parsing
done: docs describe Krust as the selected native target rather than the command
      users invoke to boot Vertex OS
```

Implementation notes:

- M87-3 does not change the runtime image or syscall ABI. It changes the
  operator/developer entry point so the product boundary matches the source
  boundary.
- `kernel/krust` remains callable for target maintainers, but normal users boot
  Vertex OS from the repository root.

## M87-4: Usable Operator Console

Status: done.

Goal: make the native shell feel like an operator console instead of a shallow
test harness, while preserving explicit graph authority and avoiding guessed
answers.

Scope:

```text
portable discovery renderers in userland/operator-shell
overview and richer help output
service list and service detail commands
capability list, capability detail, and capabilities-for-service commands
state list and state detail commands
device inventory output without dropping the existing device-failure signal
Krust console-shell streams bounded operator lines and keeps old transcript logs
```

Acceptance tests:

```text
help explains discovery and proof commands with examples
overview reports active generation and inventory counts from operator-report
services lists real operator-service facts with live process state
service svc:echo-server shows requirements and state paths
capabilities and capability cap:log.sink reveal discoverable capability IDs
state state:counter shows policy and service path authority
M87 QEMU transcript proves discovery commands run before why/who-can proofs
```

Done:

```text
done: userland/operator-shell owns overview, help, services, service,
      capabilities, capability, states, state, devices, and device renderers
done: console-shell streams bounded lines from the portable renderer and uses
      Krust only for runtime-inspect, IPC, console output, and legacy signals
done: services is no longer a hard-coded tiny status line; it renders active
      operator-service records with live process state and restart policy
done: capabilities and capability detail reveal capability IDs, providers,
      rights, object bindings, and consumers so why commands are discoverable
done: service detail links requirements to providers and state paths to rights
done: devices preserves the appliance device-failure line while also reporting
      graph device inventory when available
done: operator-shell host tests cover the discovery renderers
done: scripts/krust-test.sh m87 includes overview, services, service detail,
      capabilities, capability detail, states, and state detail before proofs
```

Implementation notes:

- M87-4 does not add a syscall or make the shell POSIX-like. It improves the
  user-facing operator model over existing runtime inspection facts.
- Every rendered inventory line is bounded for console IPC and either comes
  from an explicit operator report field or fails with a specific error.
- This is the UX bridge that makes `why` useful: operators can now discover
  service and capability IDs before asking for authority proofs.

## M88: End-To-End Appliance Update Gate

Status: done.

Goal: prove the complete Vertex OS loop inside QEMU: import package closure,
construct a candidate graph, validate policy, install generation, activate it,
handle failure, roll back, and preserve or migrate state by policy.

Scope:

```text
multi-generation QEMU appliance scenario
native package import and graph linking
native policy validation
native generation install and activation
intentional failed generation and rollback
state migration and preservation checks
post-rollback graph explanation and health report
```

Acceptance tests:

```text
QEMU installs a new package closure into a candidate generation while booted
candidate generation activates and starts only graph-declared services
new service receives only graph-declared capabilities
second candidate intentionally fails activation and rolls back automatically
state survives, forks, migrates, or is discarded according to declared policy
operator graph shell explains both the successful activation and failed rollback
post-test verifier reports no leaked graph, store, state, process, or capability objects
```

Implementation notes:

- This should become the flagship “ideal NixOS” proof: a running system changes
  itself by graph transaction, not by imperative activation scripts.
- Keep the appliance profile narrow. Do not add desktop, USB, or general POSIX
  scope to make the demo look bigger.
- The gate should fail if any step requires host-side activation state after
  the initial test image is booted.

Done:

- `userland/package-import` owns target-independent package-fragment and
  authority-delta validation; `targets/krust/user/package-import` is the Krust
  IPC/store adapter.
- Native graph package facts are explicit graph nodes and runtime inspect emits
  `operator-package[...]` rows with `package_facts=graph-v1`; `package-list`
  rejects absent package facts instead of guessing.
- The kernel keeps explicit selected/fallback/bad boot-module configs separate
  from disk-discovered generation configs, so a bad candidate cannot overwrite
  the known-good fallback slot.
- The operator shell verifier proves active selected graph/store/state/process
  consistency, accepted policy, package facts, clean state health, and live
  graph-backed capability provenance without accepting legacy fallback behavior.
- `scripts/krust-test.sh m88` gates base boot, package import, successful
  activation, mark-known-good, intentional failed activation, rollback,
  activation-log, package-list, state-health, `verify-system`, and halt.

## M89: Long-Run Graph OS Soak

Status: planned; intentionally skipped for now so M90 can establish the next
VFS/filesystem protocol substrate first.

Goal: stress the native graph OS model over many install, activation, rollback,
state migration, service restart, and crash cycles until graph/object/state
leaks and authority drift are boringly absent.

Scope:

```text
100-1000 generation install/activate/rollback cycles
repeated package import, package removal, and closure reuse
state migration churn with preserve, fork, discard, and rollback cases
service restart churn under changing generation graphs
power-loss checkpoints across graph-store, package-store, and state migrations
leak accounting for graph nodes, store objects, state objects, caps, handles, and vnodes
operator health report after every cycle batch
```

Acceptance tests:

```text
100 native generation cycles return graph, cap, process, vnode, and state counts to baseline
100 package import/remove cycles reuse closure objects without leaks
state migration churn preserves declared data and drops only policy-declared garbage
crash at every graph transaction phase remounts to a valid selected generation
service restart churn never grants authority from a previous generation
operator health report stays clean after every cycle batch
release gate checks final graph-store verifier, filesystem verifier, and leak deltas
```

Implementation notes:

- This is the graph-OS equivalent of M66-M81. It should be run before adding
  broad hardware, POSIX personalities, desktop ambitions, or self-hosting.
- Prefer deterministic cycle scenarios first; randomized stress can come after
  the deterministic gate is stable.
- The main output is confidence that the native graph transaction model does
  not drift under time, crashes, restarts, and repeated updates.

## M90: Filesystem-Service Protocol And VFS Authority Contract

Status: done for the current filesystem-service protocol substrate; typed mount
source reporting, exact read-only `servicefs` mount authority, v2
filesystem-service request envelope, provider-side v2 validation, and release
gate coverage are implemented. Broader lookup/page/metadata operations remain
the M91-M93 expansion.

Goal: split the next filesystem layer cleanly: Krust keeps VFS authority,
handles, mount namespaces, vnode identity, cache ownership, and blocking
syscall state; filesystem services own durable metadata policy, allocation, and
recovery behind explicit graph-authorized mounts.

Scope:

```text
typed kernel-to-filesystem-service request/reply protocol
mount source objects with typed source ids instead of string-only source names
lookup, getattr, setattr, create, unlink, rename, link, readdir operations
readpage, writepage, fsync, syncfs, forget operations
kernel-side vnode cache keyed by mount id and filesystem vnode id
service-local roots and declared bind mounts preserved across service restart
read-only, service-backed, volatile, durable, and bind flags enforced uniformly
filesystem-service fault handling for in-flight metadata and page transactions
runtime inspect rows for filesystem service health and active transactions
```

Acceptance tests:

```text
service without VFS authority cannot trigger filesystem-service lookup
service with read-only authority cannot create, unlink, rename, writepage, or fsync dirty data
same path under two service-local roots resolves to different authority where declared
filesystem service receives metadata requests only after kernel authority checks pass
filesystem-service crash aborts blocked callers and leaves mount state inspectable
read-only bind remains read-only after nested bind and service restart
kernel can forget idle filesystem vnodes without losing live-handle semantics
operator report shows mount source id, service owner, dirty counts, and fault state
```

Implementation notes:

- Names remain selectors, not authority. The filesystem service must never
  grant access by returning a vnode; the kernel must already have verified the
  caller's VFS root, mount, and open rights.
- The generation graph and policy validator decide which mounts and service
  capabilities exist. They are not consulted for every read or write.
- Use compact request records and page-sized shared buffers before introducing
  vectored I/O or larger async queues.
- Keep the first service implementation in-memory or fixture-backed if needed;
  the purpose of M90 is the authority boundary, request model, restart cleanup,
  and observability.

Current implementation state:

```text
done: VFS mount objects now carry typed source metadata alongside display source
      labels, with runtime inspect reporting `source_kind` and `source_id`
done: `/state/service-report` is installed as an exact read-only `servicefs`
      mount, so write-capable opens are denied by kernel VFS authority before
      the filesystem-service provider receives a request
done: service-backed file reads use a version-2 `FS` request envelope carrying
      operation, typed source kind, vnode id, and open-file-description id
done: `vertex-state` validates the v2 filesystem-service request envelope and
      rejects malformed or stale short requests before serving the report
done: runtime inspect emits `vfs-filesystem-service v=2` health with active
      transaction and writeback-error fields for operator/gate checks
done: `vertex-init` verifies typed VFS mount-source rows and filesystem-service
      health rows in the runtime inspect report
done: `scripts/krust-test.sh m90` gates the typed servicefs mount source, v2
      request envelope, provider-side v2 validation, pre-provider write denial,
      runtime inspect proof rows, and native activation success
```

## M91: VertexFS v2 Durable Format

Status: done for the current durable-format substrate; VertexFS v2 images,
offline tooling, mount-time verification/replay, v2 disk-section generation,
unsupported-feature rejection, and dynamic create growth beyond the v1 metadata
limit are release-gate covered. Full durable metadata mutation semantics remain
M92 work; the bounded VertexFS page-cache/writeback layer is implemented in
M93.

Goal: replace the current fixed VertexFS v1 substrate with an extensible
durable on-disk format that can support normal filesystem metadata growth
without falling back to volatile nodes or fixed boot-image tables.

Scope:

```text
VertexFS v2 superblock with volume id, generation id, feature flags, and checksum
extensible inode table or inode-tree records with stable inode ids
directory records with checked parent/child identity and bounded names
extent-backed regular files with checksummed metadata and file payload checks
free-space bitmap first, with room for later extent-tree allocation
metadata journal with begin, commit, sequence, target, checksum, and replay records
strict offline create, inspect, verify, corrupt, and update support in vertexctl
mount-time verifier that either replays to a valid tree or rejects the volume
compatibility rejection for unsupported v2 feature flags or stale experimental layouts
```

Acceptance tests:

```text
fresh VertexFS v2 image mounts and exposes declared files and directories
offline verifier rejects bad superblock, bad inode, bad directory, bad free map, and bad journal
interrupted journal checkpoints replay idempotently to one valid tree
unsupported feature flag rejects without falling back to v1 or volatile storage
dynamic create grows beyond the current v1 fixed inode and directory slots
vertexctl create/inspect/verify/corrupt/update are reproducible across runs
post-sync remount reads the committed v2 contents from the VertexDisk section
```

Implementation notes:

- Pick one durable update model for v2. The preferred first step is a metadata
  journal with ordered replay, not simultaneous copy-on-write snapshots.
- Keep the v1 verifier and corruption gates until v2 fully covers the existing
  M78 behavior.
- The on-disk format should be small and explicit, but not hard-coded to one
  application directory or fixed file count.

Current implementation state:

```text
done: VertexFS v2 uses a 64-sector image compatible with the existing
      VertexDisk section while reallocating internal metadata to 8-sector inode
      and directory regions, a free-space bitmap, a v2 journal sector, and a
      data section starting after the journal
done: the v2 superblock carries strict magic/version/sector count, a volume id,
      generation id, feature flags, a section table, and checksum validation
done: v2 inode and directory records keep stable inode ids, checked
      parent/child identity, bounded names, file extent metadata, and payload
      checksums
done: the v2 journal carries a pending/clean state plus target, payload length,
      payload checksum, sequence, and replay fields; interrupted checkpoints
      verify and mount through the replay path idempotently
done: `vertexctl create-vertexfs --format v2`, `inspect-vertexfs`,
      `verify-vertexfs`, `corrupt-vertexfs`, and `update-vertexfs-file` cover
      reproducible v2 create/inspect/verify/update plus bad superblock, inode,
      directory, free-map, journal, and unsupported-feature rejection
done: `vertexctl create-vertex-disk --vertexfs-format v2` writes a matching v2
      VertexFS image into the durable VertexDisk section used by the block
      driver
done: Krust accepts neutral, v1, and v2 VertexFS module labels, verifies v2 at
      mount time, reports `VertexFS v2` mount/replay/fsync logs, and keeps v1
      images accepted for explicit regression gates
done: `model-reader` proves v2 dynamic create can allocate inode 16
      (`/created12`), beyond the v1 dynamic inode/dir limit, while v1 still
      returns `STATUS_VFS_NO_SPACE` at the old capacity
done: `scripts/krust-test.sh m91` gates the v2 offline verifier/corrupt/update
      tooling, fresh v2 mount, v2 fsync transactions, VertexDisk v2 section,
      dynamic create growth, and native activation success
```

## M92: Durable Metadata Operations

Status: implemented.

Goal: move normal metadata operations from the volatile/memory-file-only path
onto VertexFS v2 transactions while preserving the current VFS handle and
authority semantics.

Scope:

```text
done: VertexFS-backed create and open-create allocate dynamic VertexFS file
      backing under the mounted `/fs/app` authority path and emit metadata
      checkpoints before the new name becomes visible
done: VertexFS-backed unlink detaches open files from their directory name,
      preserves existing handle reads, updates shared metadata versions, and
      reaps the runtime backing on final close
done: same-filesystem VertexFS rename uses the existing atomic VFS node move,
      rejects cross-mount moves, updates metadata versions, and emits a
      checkpointed rename event
done: same-filesystem VertexFS hard links share the underlying durable inode
      identity in `SYS_VFS_STAT`, update link counts across sibling names, and
      retain the existing cross-filesystem rejection policy
done: VertexFS directory mkdir/rmdir now participates in the same metadata
      event/checkpoint path; rmdir rejects non-empty and open directories
done: VertexFS truncate and append update byte length, metadata version, stat
      fields, and fsync persistence through the v2 file writer
done: VertexFS directory watches deliver create, rename, and unlink metadata
      events through the existing `SYS_VFS_WATCH`/`SYS_VFS_POLL` contract
done: metadata checkpoint serial records cover create, open-create, unlink,
      rename, link, rmdir, truncate/write, and fsync phases for the v2 gate
```

Acceptance tests:

```text
done: `model-reader` proves rename visibility by requiring the old name to be
      gone and the new name readable after a VertexFS v2 rename
done: `model-reader` proves unlink of an open VertexFS file removes the path
      while the existing handle still reads the old payload
done: `model-reader` proves hard-link stat identity and link-count sharing,
      then unlinks one sibling and observes the count fall to one
done: `model-reader` proves rmdir rejects both open and non-empty VertexFS
      directories before removing the empty directory
done: `model-reader` proves append and truncate update size and monotonic
      metadata version, then fsyncs the truncated payload
done: `model-reader` proves VertexFS directory watches receive create, rename,
      and unlink events in order
done: `model-reader` runs a 100-cycle create/write/rename/open/read/close/unlink
      VertexFS metadata churn loop using reusable unsynced dynamic inodes
done: `scripts/krust-test.sh m92` gates the v2 metadata proof and reuses the
      v2 offline verifier/corrupt/update block so the tested image remains
      accepted by `vertexctl verify-vertexfs`
```

Implementation notes:

- M92 is the kernel-authoritative VFS metadata surface for VertexFS v2. The
  v2 on-disk writer still serializes the canonical dynamic inode/name model
  introduced in M91; richer multi-name replay records and sticky writeback
  error watch events belong with the M93 page-cache/writeback layer.
- Cross-filesystem rename and hard link remain unsupported and return a
  controlled VFS error.
- The volatile memory-file metadata path remains for tmp/state tests and shares
  the same stat identity/link-count accounting model where possible.

## M93: Vnode Page Cache, Writeback, And Fast I/O

Status: implemented for the current VertexFS durable-file substrate; external
filesystem-service shared-buffer page I/O, `syncfs`, and fsync-range style APIs
remain future work.

Goal: add a bounded, inspectable page-cache and writeback layer for durable
filesystem I/O so common reads and writes do not require service IPC or graph
policy evaluation on every byte.

Scope:

```text
done: kernel-owned VertexFS vnode page cache keyed by mount id, inode id, and page offset
done: clean-page eviction and dirty-page non-eviction under pressure
done: per-mount and global dirty byte limits for cached VertexFS pages
done: SYS_VFS_SYNC ordered writeback through the VertexFS image writer and block driver
deferred: syncfs, fsync-range, and external filesystem-service shared-buffer page I/O
done: writeback error accounting with dirty data retained after fsync failure
done: simple read-ahead hit accounting for sequential reads
done: runtime inspect rows for clean pages, dirty pages, pinned pages, writeback errors, and high-water marks
```

Acceptance tests:

```text
done: second read of a cached page completes without filesystem-service readpage IPC
done: dirty page is not evicted silently under memory pressure
done: fsync replies only after ordered writeback reaches the VertexFS writer and block driver
done: writeback failure leaves dirty data dirty and reports an error on fsync
done: cache limits reject writers before exhausting kernel memory
done: sequential read path records read-ahead hits without changing file authority
done: operator health report exposes dirty, pinned, and writeback-error counts
```

Implementation notes:

- The graph remains control plane. Once a handle is open, the hot path should be
  handle-right checks, page lookup, and bounded copy or shared-buffer transfer.
- Start with page-sized buffers and synchronous fsync. Add async queues,
  vectored I/O, and zero-copy only after the bounded cache behavior is stable.
- Keep DMA buffers separate from cache pages until an ownership-transfer design
  is explicit and testable.

Current implementation state:

```text
done: RuntimeState owns eight fixed-size VertexFS page-cache slots using the
      existing 512-byte sector/page size, with cache keys carrying mount id,
      inode id, backing slot, and page offset for reuse safety
done: VertexFS VFS reads use page lookup and bounded user copy; cache hits,
      misses, and sequential read-ahead hits emit serial records with
      `no_service_ipc=yes`
done: VertexFS writes and truncates dirty cache pages, mirror the current
      in-memory VertexFS file buffer, and reject new dirty pages once the
      configured per-mount or global dirty-byte limit would be exceeded
done: clean cached pages are evictable; dirty/writeback pages are not evicted
      silently and instead force a controlled `STATUS_VFS_NO_SPACE` pressure
      rejection
done: `SYS_VFS_SYNC` pins dirty cached pages for ordered writeback, clears them
      only after the VertexFS device transaction commits, and records sticky
      writeback errors on rejected, queued-failed, or aborted block-driver
      transactions
done: runtime inspect emits `vfs-page-cache source=vertexfs` with clean, dirty,
      pinned, writeback-error, high-water, hit/miss, readahead, eviction,
      limit, and writeback counters; `vertex-init` verifies the health row
done: `scripts/krust-test.sh m93` gates cached second-read behavior, readahead
      accounting, ordered fsync writeback, dirty pressure rejection,
      operator-visible inspect health, and native activation success
done: `scripts/krust-test.sh m78-fsync-fault` now also proves a block-driver
      fsync abort records a page-cache writeback error while keeping dirty data
      readable
```

## M94: State Volume Backend Integration

Status: planned.

Goal: let graph-declared state volumes use the mature filesystem substrate
without losing the Vertex distinction between immutable system generations and
mutable state policy.

Scope:

```text
state volume records mapped to filesystem-backed volume roots or subvolumes
state owner, schema, migration, sharing, rollback, and retention policy preserved as graph facts
statefs mount authority generated from state graph records, not ambient paths
state migration jobs receive only old state, new state, and declared helper caps
state health report includes filesystem backend, dirty/writeback status, snapshot status, and last error
rollback policy chooses preserve, fork, migrate, discard, or reject before activation
```

Acceptance tests:

```text
service cannot open undeclared state even if a filesystem path alias exists
shared state requires graph sharing policy and attenuated VFS rights
state migration reads old backend and writes new backend exactly once
failed migration rolls back generation selection and leaves old state readable
rollback applies declared state policy instead of preserving every volume blindly
state health reports owner, schema, backend volume, migration status, and filesystem health
```

Implementation notes:

- State remains first-class graph state, not just files under a conventional
  path such as `/var`.
- `vertex-state` should own lifecycle policy even if VertexFS owns the block
  and metadata format below it.
- Avoid unsafe delete or garbage collection until snapshots, reference counts,
  and recovery gates are mature.

## M95: Snapshots, Forks, And Coordinated Rollback

Status: planned.

Goal: add durable filesystem snapshots and state forks after VertexFS v2
journaling, metadata transactions, and writeback are already stable.

Scope:

```text
snapshot records for filesystem volumes and state volumes
copy-on-write extent or snapshot-manifest design selected explicitly
state fork for candidate generation activation
coordinated generation and selected-state rollback checkpoints
snapshot retention policy and safe reference accounting
operator-visible snapshot list, owner, generation, policy, and health
crash checkpoints across snapshot create, fork, activate, rollback, and delete
```

Acceptance tests:

```text
snapshot create is atomic across crash checkpoints
candidate generation can fork selected state without mutating active state
failed activation discards or preserves forked state according to policy
rollback restores selected generation and selected state checkpoint together
referenced snapshots cannot be deleted
snapshot verifier catches missing extents, leaked extents, and invalid reference counts
100 snapshot/fork/rollback cycles return graph, vnode, extent, and state counts to baseline
```

Implementation notes:

- Do not mix snapshots into the initial v2 journal work. Snapshots add a second
  persistence model and should arrive only after ordinary metadata recovery is
  boring.
- Snapshot policy is graph data. Imperative rollback scripts must not decide
  which mutable state is preserved or discarded.

## M96: POSIX Personality Filesystem Mapping

Status: planned.

Goal: map POSIX-like filesystem expectations onto explicit Vertex VFS roots
without making a global ambient filesystem part of the native model.

Scope:

```text
personality-owned fd table mapped to native VFS handles
declared cwd and chroot-like root derived from explicit VFS authority
errno mapping for native STATUS_VFS_* values
synthetic /dev, /proc, tmp, and socket namespaces bounded by granted capabilities
openat-style path resolution within declared roots
limited stat, readdir, rename, link, unlink, chmod-like metadata translation
compatibility audit report showing every synthetic namespace source capability
```

Acceptance tests:

```text
POSIX personality cannot escape its declared root through cwd, openat, .., symlink-like aliases, or bind mounts
program with no device capability sees no usable synthetic device node
program with read-only root cannot write through POSIX compatibility aliases
native revocation prevents new compatibility opens while preserving existing fd semantics
compatibility audit explains which Vertex capability backs each visible namespace entry
```

Implementation notes:

- POSIX compatibility is a personality layer, not the core filesystem model.
- It may emulate ambient Unix authority internally, but the boundary outside
  the personality must remain explicit Vertex capabilities and mounts.
- Keep symlink semantics out until path authority, loop detection, and audit
  output are well specified.

## Later Direction

Avoid these until the appliance release profile, storage durability, network
service boundary, update path, supervisor semantics, memory lifecycle,
interrupt/device failure model, VFS/filesystem model, and native graph OS
transaction loop are solid:

```text
USB
GPU
Linux syscall compatibility
desktop
multicore
self-hosting
```

They matter eventually, but they distract from the next core proof:

```text
A native booted Vertex system should be able to boot from persistent storage,
install verified generations, run dynamically created services, preserve
mutable state as files, explain its authority graph, recover from failed
updates, and survive repeated process, memory, interrupt, DMA, device, VFS, and
filesystem failures without leaking resources, corrupting persistent state, or
losing isolation.
```

M13 proved that native services can run under explicit authority. M14-M73 prove
that the graph itself decides which native services exist, when they start,
what they receive, why they are allowed to communicate, and how authority and
resources are bounded, while timer preemption and user fault containment keep
the kernel in control. M40 freezes the first ABI subset for the long-lived
Vertex native runtime base, M41 adds the first in-VM operator surface for
asking what generation and authority graph are running, M42 adds the first
persistent block I/O path while preserving hardware-shaped authority, and M43
turns that block path into a checked VertexDisk layout with persistent state.
M44-M47 move boot selection, store-object verification, update commits, and
service executable loading onto the native verified-store path. M48-M55 move
process creation, config, secrets, package/link/build boundaries, and the first
appliance transcript onto that same native path. M56-M60 add the remaining
QEMU-friendly virtio devices, the first UDP-capable network path, the POSIX
compatibility plan, capability namespaces, and human-readable policy plus typed
prototype compilation. M61 turns those surfaces into an ABI and authority
regression baseline, and M62-M65 turn that baseline into the first supported
standalone appliance profile. M66-M69 harden resource lifetime with owned frame
reclamation, address-space teardown, object failure atomicity, and soak gates.
M70-M73 add interrupt routing, DMA ownership, virtio reset/recovery, and
device-fault isolation. M74-M81 should turn the existing special-purpose
store/state persistence into a mature capability-mediated VFS and filesystem
stack. M82 moves the active generation graph into a native typed graph-store
read/provenance substrate, M83 moves generation management into the native
system, and M84 moves package closure import onto that native path. M85-M89
should move policy validation, state lifecycle, operator shell, appliance
update loop, and long-run graph soak into the native system before broadening
the platform.
