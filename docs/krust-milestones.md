# Krust Milestones

This file tracks the native Krust path from the first QEMU boot to a standalone
Vertex OS appliance profile. Host-side tools and simulations remain useful for
Vertex IR development, but Krust is the native enforcement path rather than a
runtime layered over a host kernel.

## Status Summary

Current status: M14-M73 are implemented and smoke-tested under
`qemu-system-x86_64` with Limine. M39 pins the native toolchain, M40 makes
native IPC directed, and M41 adds a native console shell path over explicit
console authority. M42 adds the first real virtio-blk sector I/O path over
PCI I/O and DMA capabilities. M43 adds the first VertexDisk v0 block-object
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
driver queue reporting, and the first device-fault isolation gate.

```sh
make -C kernel/krust doctor
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
```

Next direction: keep broadening blocked until the M70-M73 device-failure gate
stays boring under longer optional soak runs and hardware profiles beyond the
current legacy PIC/QEMU target are deliberately selected.

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

Initial demo protocol:

```text
counter-service -> vertex-state request endpoint
reader-service -> vertex-state request endpoint
vertex-state -> private reply endpoints
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
vertex.state.v1
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
done: unauthorized process cannot read object
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
  exposes a directed state service endpoint
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
done: single release-gate script runs the clean-clone M14-M73 proof with the M14-M73 QEMU matrix
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
done: vertex-init delegates inspect-only authority to console-shell
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
console-shell receives only shell request, console output, console control, and inspect authority
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

## M43: VertexDisk v0 Layout

Status: done.

Goal: stop relying only on ISO boot modules by defining a minimal disk format
for generations, immutable store objects, state volumes, and update journals.

Initial layout:

```text
VertexDisk v0
  superblock
  generation metadata area
  immutable store index
  immutable store data
  mutable state index
  mutable state data
  journal area
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
done: vertex-store reads object index from disk
done: vertex-state reads state volume from disk
done: vertex-state writes journal record before state data/index writeback
done: vertex-state writes state volume to disk
done: reboot preserves state value
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
logd reads config object
echo cannot read logd config
config hash mismatch fails activation
vertex-inspect shows config authority without dumping large content
```

Implementation note: `config:logd` is a native immutable object carried in
KrustBoot/VertexDisk metadata and granted only to logd. Reads verify the
content identity before returning bytes; the corrupt-config QEMU case rejects a
hash mismatch during activation.

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
  instead of accepting them as a compatibility format. M65 supersedes this with
  `KRUSTBOOTM65` version 8 and rejects M61 payloads as legacy.
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
- Keep store and state traffic separated by endpoint identity, as in M43-M57.
- Prefer explicit error surfaces in `vertex-inspect` and the appliance shell over
  silent retry loops.
- The block-driver self-test no longer overwrites the live journal sector. It
  writes a scratch journal sector, reports virtio completion status, and keeps
  store and state endpoint ranges separate.
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
KrustBoot Manifest v1 / compact payload KRUSTBOOTM65 version 8
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
- The compact native payload identity is now `KRUSTBOOTM65` version 8. M61 and
  older compact payload identities are rejected rather than retained as
  compatibility formats.
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
driver exit releases DMA buffers and user DMA mappings
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

## Later Direction

Avoid these until the appliance release profile, storage durability, network
service boundary, update path, supervisor semantics, memory lifecycle, and
interrupt/device failure model are solid:

```text
USB
GPU
full filesystem
Linux syscall compatibility
desktop
multicore
self-hosting
```

They matter eventually, but they distract from the next core proof:

```text
A native booted Vertex system should be able to boot from persistent storage,
install verified generations, run dynamically created services, preserve
mutable state, explain its authority graph, recover from failed updates, and
survive repeated process, memory, interrupt, DMA, and device failures without
leaking resources or losing isolation.
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
device-fault isolation before broadening the platform.
