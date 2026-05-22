# Krust Milestones

This file tracks the native Krust path from the first QEMU boot to a real
native Vertex boot. The hosted Linux prototype remains the reference for Vertex
IR and graph semantics; Krust is the native enforcement path.

## Status Summary

Current status: M14-M24 are implemented and smoke-tested under
`qemu-system-x86_64` with Limine.

```sh
scripts/krust-smoke.sh
scripts/krust-test.sh manifest-cycle
scripts/krust-test.sh rollback
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
  -> Krust loads vertex-init, logd, and echo ELFs
  -> Krust marks non-initial services Declared
  -> vertex-init reads the manifest
  -> vertex-init starts logd through SYS_PROCESS_START
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
process[2] name=echo module=echo initial=no
process[1] id=2 name=logd state=declared
process[2] id=3 name=echo state=declared
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

M13 proves that native Krust can boot a compact generation and run a tiny
capability-enforced service graph. Full graph ordering, readiness, filesystems,
networking, device drivers, and native store objects remain future milestones.

## Post-M13 Direction

The next strategic target is native Krust boot activation of a real generation
graph, not a hardcoded demo graph.

M13 proves:

- Krust can boot.
- `vertex-init` can run natively.
- `vertex-init` can start declared services.
- Services communicate only through granted capabilities.
- Denied authority fails.

The next milestones should prove:

- `vertex-init` reads service dependencies from the compact manifest.
- Activation order is computed from the graph.
- Readiness and failure are explicit.
- Capabilities can be delegated and attenuated.
- Processes are supervised.
- Generation switch and rollback become native.

That keeps Vertex OS aligned with the core design: the running system is an
activation of a declared graph, and Krust only enforces compact authority.

## M14: Manifest-Driven Native Activation

Status: done.

Goal: remove the remaining demo shape from native `vertex-init`. M13 still
knows that it should start `logd` and `echo`. M14 should make `vertex-init`
discover declared services from the compact KrustBoot manifest, compute a valid
activation order, and start services from manifest records without
special-casing service names.

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
  2. echo
vertex-init starting service: logd
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
grant echo cap[0] send log-sink
grant logd cap[0] receive log-sink
start_after echo <- logd
```

Acceptance evidence:

```text
vertexctl compile-boot-manifest examples/hello-generation.vertex.json build/hello.krustboot
vertexctl explain-krustboot build/hello.krustboot
make smoke
```

Expected explanation:

```text
svc:echo-server receives send authority to endpoint log-sink
because it requires cap:log.sink/send
and svc:logd provides cap:log.sink
```

This is the key bridge from a native demo to one source graph that can target
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
SYS_CAP_TRANSFER(endpoint_slot, cap_slot, rights_mask)
```

Acceptance evidence:

```text
vertex-init derives send-only cap for echo from stronger endpoint authority
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
max_restarts
```

Acceptance evidence:

```text
flaky-service exits with status 1
vertex-init observes failure
restart policy = on-failure
vertex-init restarts flaky-service once
flaky-service exits 0
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
SYS_OBJECT_READ(cap_slot, offset, ptr, len)
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
rollback activation ok
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
SYS_STATE_WRITE(cap_slot, key, value)
SYS_STATE_READ(cap_slot, key, buffer)
```

Acceptance evidence:

```text
counter-service has write cap to state:counter
counter-service writes value
reader-service has read-only cap
reader-service reads value
reader-service write rejected
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
wakes
logs "timer ok"
```

This enables readiness timeouts, restart backoff, and activation failure
handling.

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

## Recommended Order

The next planned milestones should be implemented in this order:

```text
M14  Manifest-driven native activation
M15  Native readiness and service lifecycle
M16  Compile native KrustBoot from full Vertex IR
M17  Capability derivation, attenuation, drop, and transfer
M18  Native supervision and restart policy
M19  Native immutable store-object read capabilities
M20  Native generation identity and rollback selection
M21  Native state-volume read/write capabilities
M22  Standard native component protocol
M23  Timer/sleep capability
M24  QEMU native test suite
```

The first three are the highest priority:

```text
M14: no hardcoded service graph
M15: service readiness/failure semantics
M16: native boot manifest generated from real Vertex IR
```

These milestones bridge from "Krust can run services" to "Vertex OS can
activate a declared system."

## Deferred Work

Avoid these until native graph activation, readiness, compilation, supervision,
and rollback semantics are solid:

```text
filesystem driver
network driver
USB
GPU
POSIX compatibility
Linux syscall compatibility
desktop
Haskell DSL
full Nix replacement
dynamic package manager
preemptive scheduler
multicore
```

They matter eventually, but they distract from the next core proof:

```text
A native booted Vertex system should be fully determined by the generation graph.
```

M13 proves that native services can run under explicit authority. M14-M16
should prove that the graph itself decides which native services exist, when
they start, what they receive, and why they are allowed to communicate.
