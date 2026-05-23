# Krust Milestones

This file tracks the native Krust path from the first QEMU boot to a real
native Vertex boot. The hosted Linux prototype remains the reference for Vertex
IR and graph semantics; Krust is the native enforcement path.

## Status Summary

Current status: M14-M38 are implemented and smoke-tested under
`qemu-system-x86_64` with Limine.

```sh
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
```

Next direction: M39-M40 continue hardening the M14-M38 graph-activation proof
into a small, reliable, extensible capability microkernel substrate.

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
  -> Krust marks non-initial services Declared
  -> vertex-init reads the manifest
  -> vertex-init starts logd through SYS_PROCESS_START
  -> vertex-init starts netstack and other declared services through SYS_PROCESS_START
  -> vertex-init starts echo through SYS_PROCESS_START
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
negative test: logd process-start rejected: bad capability
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
- `vertex-init` can start declared services.
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
SYS_PROCESS_STATUS
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
to the richer hosted graph. M16 should make hosted `vertexctl
compile-boot-manifest` derive native KrustBoot records from services,
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
grant vertex-init cap[4] send|receive log-sink
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

This is the key bridge from a native proof path to one source graph that can target
both hosted and native runtimes.

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
vertex-init observes failure
restart policy = on-failure
vertex-init restarts flaky-service once
Krust process restart reload: proc=flaky-service
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
SYS_OBJECT_READ(cap_slot, ptr, len)
```

Acceptance evidence:

```text
model-reader has read cap to store:hello-text
model-reader reads bytes successfully
echo lacks read cap
echo read rejected: bad capability
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

Candidate demo syscalls:

```text
SYS_STATE_WRITE(cap_slot, value)
SYS_STATE_READ(cap_slot, buffer)
```

Acceptance evidence:

```text
counter-service has state API cap
counter-service sends state write
reader-service has state API cap
reader-service receives state value
reader-service write rejected
Native state service client ok
```

This keeps immutable generation rollback and mutable state rollback separate.

## M22: Small Native Component Protocol

Status: done.

Goal: define `Vertex Native Protocol v0` so native services stop inventing
ad hoc byte strings as the graph grows.

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
vertex.log.v0
vertex.health.v0
vertex.supervision.v0
vertex.store.v0
vertex.state.v0
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
M36  Native state-volume service
M37  Native generation switch
M38  Native vertexctl-like introspection service
M39  Reproducible build environment
M40  Vertex Native Runtime ABI v1
```

M32 is the immediate priority. M39 can run in parallel with I/O and storage
work because reproducible development tooling is part of the system story, not
an afterthought.

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
done: all QEMU transcript checks have bounded polling windows
done: missing and forbidden transcript lines are reported explicitly
done: README.md, docs/krust-milestones.md, docs/krust-abi-v0.md, and kernel/krust/README.md agree
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
init observes failure through SYS_PROCESS_STATUS
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
SYS_IRQ_WAIT
SYS_MMIO_MAP
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
`DmaRegion` grants into the kernel runtime. `SYS_IO_READ`, `SYS_IO_WRITE`,
`SYS_IRQ_WAIT`, and `SYS_MMIO_MAP` resolve process-local capabilities before
touching or exposing any hardware-shaped resource.

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
virtio MMIO or PCI authority
block-read/block-write IPC protocol
```

Acceptance tests:

```text
done: block-driver ready
done: store-service requests block read
done: block-driver returns bytes
done: unauthorized service cannot talk to block-driver
done: unauthorized service cannot access MMIO, IRQ, or DMA capabilities
```

This may need to split into multiple sub-milestones if PCI enumeration, virtio
queues, DMA, and IRQ handling prove too large for one step.

The first path is deliberately still a proof transport: `block-driver` owns the
virtio-blk shaped MMIO, IRQ, and DMA capabilities and serves a small
block-read IPC protocol. Real virtio queue setup and a real disk image remain
future driver work.

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
done: unauthorized process cannot read object
```

This turns the current boot-module store proof into a real immutable object
service.

`model-reader` now talks to `vertex-store` over an explicit store IPC endpoint.
`vertex-store` requests bytes from `block-driver`, verifies the expected object
content, rejects a modified-object negative check, and replies to the reader.

## M36: Native State-Volume Service

Status: done.

Goal: add real mutable state while keeping it distinct from immutable store
objects.

Service:

```text
vertex-state
  owns block ranges or state objects
  exposes state-volume capabilities
  supports read, write, snapshot, and restore semantics
```

Acceptance tests:

```text
done: counter-service writes state
done: reader-service reads state
done: reader-service write denied
done: snapshot created
done: state restored
done: system generation rollback does not automatically roll back state unless policy says so
```

Immutable system rollback and mutable state rollback are related policy
decisions, not the same operation.

`counter-service` and `reader-service` now use a `vertex-state` IPC endpoint
instead of direct kernel state syscalls. `vertex-state` owns the state-volume
backend capability, performs the write/read/deny flow, and demonstrates
explicit snapshot and restore policy separation.

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
done: bad generation C fails
done: rollback to B
```

`SYS_ACTIVATE_GENERATION` now resolves the requested generation ID against the
registered native KrustBoot runtime configs, records the previous generation as
the rollback target, rebuilds the process/object/capability tables for the new
generation, and enters the new `vertex-init`. The M37 QEMU case boots
`gen:switch-a-0001`, has `vertex-init` obtain the generation-B marker from
`vertex-store` over the declared store IPC endpoint, switches to
`gen:switch-b-0002`, proves old authority was discarded with the old runtime
tables, then rejects an unavailable bad generation C and remains on B.

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
done: native cap provenance report
```

`SYS_RUNTIME_INSPECT` requires `inspect` rights on process-control and returns
a structured text report containing the runtime generation, process states,
current capability spaces, initial capability spaces, rights, cap IDs, parent
cap IDs, generation IDs, and revocation state. `vertex-init` delegates only
`inspect` and manifest `read` authority to `vertex-inspect`; the service then
proves the native counterpart to hosted `vertexctl why` and `who-can` without
receiving control, allocation, delegate, or revoke authority.

## M39: Reproducible Build Environment

Status: planned.

Goal: make the build environment itself reproducible without requiring an
external functional package manager.

Add:

```text
documented host tool versions for Rust, qemu, limine, xorriso, and cargo tools
locked Cargo dependencies
kernel/krust/rust-toolchain.toml as the native Krust toolchain pin
make doctor checks every required tool and reports actionable fixes
single release-gate script that runs the clean-clone M14-M38 proof
```

Acceptance tests:

```text
cargo build --offline
target/debug/vertexctl validate examples/hello-generation.vertex.json
cd kernel/krust && make doctor && make smoke
scripts/krust-test.sh restart
scripts/krust-test.sh timer
scripts/krust-test.sh store-state-services
```

This can land earlier than M39 as a parallel track. The milestone number marks
the point by which the project should have first-class, repo-native build
reproducibility instead of placeholder external tooling.

## M40: Vertex Native Runtime ABI v1

Status: planned.

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

## Immediate Issue List

Completed first slices:

```text
done: M32.1  Add first I/O capability substrate path
done: M33.1  Move serial logging toward user space
```

## Deferred Work

Avoid these until substrate hardening, generation switching, store/state, and
I/O capability enforcement are solid:

```text
USB
GPU
full filesystem
POSIX compatibility
Linux syscall compatibility
network stack
package manager replacement
desktop
Haskell DSL
multicore
```

They matter eventually, but they distract from the next core proof:

```text
A native booted Vertex system should be able to activate, switch, inspect,
revoke, and persist declared generation graphs under explicit authority.
```

M13 proved that native services can run under explicit authority. M14-M38 prove
that the graph itself decides which native services exist, when they start,
what they receive, why they are allowed to communicate, and how authority and
resources are bounded, while timer preemption and user fault containment keep
the kernel in control. M39-M40 should make that model reliable enough to become
the long-lived Vertex native runtime base.
