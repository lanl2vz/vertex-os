# Vertex OS

Vertex OS is a typed, reproducible, capability-secure operating-system prototype where a running machine is represented as a generation graph.

The repository currently has one native OS target plus host-side development
tools:

1. Standalone Krust/Vertex OS path: QEMU/Limine boot, compact KrustBoot manifest, native
   `vertex-init`, process-local capabilities, safe user-copy validation,
   PIT-backed preemption, user page-fault containment, and native multi-service
   activation.
2. Host-side tooling and simulation: Vertex IR, `vertexctl`,
   `vertex-supervisor`, and demo userland services. These tools run on the
   development host; they are not the OS substrate.

## Repository Layout

```text
vertex-os/
  Makefile               Vertex OS root boot/build/test runner
  docs/                  Design notes and Vertex IR drafts
  schemas/               JSON schemas for manifest experiments
  examples/              Example generation manifests
  crates/
    vertex-ir/           Vertex IR model, loader, validation, graph helpers
    vertexctl/           CLI for validation, graphing, boot-manifest compilation, activation, inspection, and rollback
    vertex-supervisor/   Host-side graph activator and simulation tool
  userland/
    vertex-init/         First user-space activator for native Krust
    logd/                Demo log service
    netstack/            Demo network capability provider
    echo-server/         Demo service consuming log and network capabilities
    operator-shell/      Target-independent operator graph-shell command core
    package-import/      Target-independent native package import validation core
  targets/
    krust/
      user/              Krust ABI adapter workspace for native user programs
  kernel/
    krust/               Bootable Krust kernel prototype, currently covering M14-M88 plus M90-M93 native graph activation, substrate hardening, pinned tooling, ABI v1 IPC, console shell, virtio device I/O, VertexDisk v1 durability, native boot selection, verified store objects, native updates, store-loaded executables, dynamic process creation, native config/secrets, package/link/build import, the first appliance transcript, native user-space driver objects, netstack boundaries, capability namespaces, policy compilation, lifecycle supervision, a supported standalone appliance profile, owned frame reclamation, address-space teardown, failure-atomic kernel object creation, memory lifecycle soak gates, interrupt routing, DMA ownership, virtio recovery, device-fault isolation, VFS root authority, open-file handles, directory metadata operations, bounded block-cache writeback, image-backed VertexFS/mount-namespace gates, VFS coordination primitives, filesystem security/soak gates, the native VertexDisk graph-store read/provenance surface, native generation-manager install/rollback/recovery, native package closure import, declared state-object migration policy, native policy validation, the usable operator graph shell, the end-to-end appliance update gate, the typed filesystem-service protocol substrate, the VertexFS v2 durable format substrate, VertexFS-backed durable metadata operations, and bounded VertexFS vnode page-cache/writeback
  lang/
    vertex-lang/         Planned typed system-definition language
```

## Krust M14-M88 Plus M90-M93 And M87-1/M87-4

The current native activation path lives in `kernel/krust`. It is isolated
from the host-side Cargo workspace and boots a Limine ISO under QEMU. The ISO
build first runs `vertexctl compile-boot-manifest` to turn the source
generation graph into `hello-generation.krustboot`, a versioned KrustBoot
Manifest v1 native boot artifact.

Krust parses that module, verifies native service executable store objects,
creates only the initial runtime process, and enters ring 3 at `vertex-init`.
Native `vertex-init` reads the manifest through cap[0], logs through cap[1],
creates service processes dynamically through cap[2] process-control, delegates
attenuated IPC authority by pid, starts services, waits for readiness, and
supervises process exits. The QEMU test path now proves Manifest v1 bounds
checks, capability provenance/revocation, typed arena allocation, resource
quotas, PIT timer preemption, user page-fault containment, explicit I/O
capabilities, user-space serial and block drivers, native store/state services,
native generation switching, native runtime introspection, exact M39 toolchain
checks, M40 directed request/reply IPC, M41 native console shell commands, M42
virtio-blk sector I/O over PCI/DMA authority, M43 VertexDisk v1
superblock/index/state/journal handling, M44 native generation selection and
fallback, M45 store-object hash verification, M46 native update transactions,
M47 store-loaded executable images, M48 dynamic process creation, M49 immutable
config objects, M50 native secrets, M51-M53 package/link/build import CLI
boundaries, M54 appliance behavior, M55 user-space driver object authority,
M56 virtio-console/rng/net device authority, M57 UDP network authority, M58
POSIX compatibility planning, M59 capability namespaces, M60 policy and typed
prototype compilation, M61 ABI/authority hardening, M62 storage durability,
M63 network service boundaries, M64 supervisor lifecycle semantics, M65 release
profile recording, M66 owned frame accounting, M67 address-space teardown, M68
failure-atomic object/capability creation, M69 lifecycle soak gates, M70
interrupt routing, M71 DMA ownership, M72 virtio recovery, M73 device-fault
isolation, M74 VFS object authority, M75 open-file handle lifecycle, M76
directory metadata operations, M77 bounded block-cache writeback, and a real
restart of `flaky-service`, not init-owned transcript logging. M78-M79 add an
initial separate `vertexfs` boot image mount, VertexFS-file backing for declared
and created files, service-local mount-root gates, corrupt VertexFS image
rejection, interrupted-journal replay, declared-inode `SYS_VFS_SYNC`
device-backed transactions, committed post-sync image remount, declared-file
journal checkpoint recovery, writable bind alias gates, and the current
read-only `servicefs` request/reply file route. M80-M81 add byte-range advisory
locks, directory watch events, VFS poll readiness, bounded pipe buffering,
revocation checks for live file authority, hostile VFS argument rejection, and a
100-cycle file churn security/soak gate. M82 adds a `KRUSTBOOTM82` version 13
compact graph header plus a native VertexDisk graph-store object, imports
runtime graph tables from that disk object, exposes graph
checksum/hash/object-count inspection, records process/capability graph
provenance in runtime inspect, and rejects malformed compact or disk
graph-store records before activation. M83 moves generation installation,
rollback, durable selected-generation metadata, and prepare/commit/rollback
recovery into the native generation-manager, block-driver, and staged kernel
runtime-build authority path. M84 adds native package closure import, including
compact graph-fragment parsing, store/config hash verification,
authority-delta reporting, separate negative import commands for undeclared
dependencies and excess grants, a graph-store-only candidate that is not
installable until package-import writes native generation metadata, an active
graph delta from base to candidate generation, canonical closure hashing,
activation, and rollback. M85 carries declared state
owner/schema/storage/migration/retention/sharing policy through the graph
store, native boot config, runtime inspection, generation staging, rollback,
and host validation. M86 updates the strict compact payload to
`KRUSTBOOTM86` version 19, carries hashed policy facts for capability grants,
mount roots, declared mounts, state paths, bootstrap authorities, and service
namespaces, rejects graph-consistent but policy-invalid authority before
activation, and exposes native policy-denial records through runtime
inspection. M87 adds an operator graph shell over the native console path:
current-generation, generation listing/status, generation diffs, authority
delta reports, policy-provenance `why`, graph-authorized `who-can`,
which-generation lookup, package/activation summaries, mark-known-good, and
generic activate/rollback through generation-manager authority.
M87-1 moves the operator shell's graph-answering semantics into
`userland/operator-shell`, a target-independent no_std Vertex userland package
with Krust kept as the syscall/IPC adapter.
M87-2 moves the Krust-built userspace workspace out of `kernel/krust/user` and
into `targets/krust/user`, making the OS/kernel boundary explicit: portable
Vertex behavior belongs in `userland/`, while target-specific Krust syscalls,
linker glue, and process entry points belong in the Krust target adapter
workspace.
M87-3 adds the repository-root Vertex OS runner, so `make run-gui` boots the OS
while Krust remains the selected native target behind `VERTEX_TARGET=krust`.
M87-4 upgrades the native operator console with `overview`, richer `help`,
service/capability/state discovery, and detail commands so authority proofs can
start from discoverable IDs instead of memorized internals.
M88 adds the end-to-end appliance update gate: a running Vertex OS imports a
package closure, activates the resulting generation, marks it known-good,
attempts an intentionally bad generation, rolls back to the known-good graph,
then proves package facts, state health, activation history, live graph-backed
capability provenance, and final system consistency through the operator shell.
The portable package-import verifier lives in `userland/package-import`; the
Krust package-import program is only the target adapter for store reads and
generation-manager IPC.
M90 adds the typed filesystem-service protocol substrate after the M89 soak was
deferred: VFS mount objects now report typed source metadata, `/state/service-report`
is an exact read-only `servicefs` mount, service-backed reads use a v2 `FS`
request envelope, `vertex-state` validates that envelope, and runtime inspect
reports filesystem-service health for the M90 gate.
M91 adds the default VertexFS v2 durable format: v2 images carry volume and
generation identity, strict feature flags, expanded inode/directory metadata,
a checked free-space bitmap, a replayable v2 journal, host-side
create/inspect/verify/corrupt/update tooling, and a QEMU proof that dynamic
create grows beyond the old v1 metadata limit.
M92 adds VertexFS-backed durable metadata semantics for create/open-create,
unlink with final-close reaping, same-filesystem rename and hard link, mkdir
and rmdir policy, truncate/append stat updates, directory watches, checkpoint
logs, and a 100-cycle metadata churn gate.
M93 adds a bounded kernel-owned VertexFS vnode page cache and writeback layer:
cached second reads avoid filesystem-service IPC, sequential reads record
read-ahead hits, dirty pages are not silently evicted under pressure, fsync
clears cached dirty pages only after ordered block-driver acknowledgement,
writeback failures retain dirty data and record errors, and runtime inspect
reports page-cache health.

```sh
scripts/krust-smoke.sh
```

The clean-clone release gate validates the host-side build and tests, checks
the Krust toolchain, rebuilds the standalone ISO from clean kernel artifacts,
and runs the M14-M88 plus M90-M93 substrate gate plus M87-1/M87-4 layout checks with the
M14-M88 plus M90-M93 QEMU test matrix:

```sh
scripts/krust-release-gate.sh
```

See [docs/krust-milestones.md](docs/krust-milestones.md) for M0 through M88
current status and the appliance OS MVP profile,
[docs/krust-toolchain.md](docs/krust-toolchain.md) for the pinned M39 toolchain,
and [docs/krust-abi-v1.md](docs/krust-abi-v1.md) for the current syscall,
capability, process, and IPC ABI.
The M58 compatibility plan is tracked in
[docs/posix-personality-v0.md](docs/posix-personality-v0.md).

The current Krust milestone status and deferred work are tracked in
[docs/krust-milestones.md](docs/krust-milestones.md).

## Boot Vertex OS

Boot Vertex OS in a QEMU window from the repository root:

```sh
make run-gui
```

This is the supported OS-level entry point. It selects the current native
target with `VERTEX_TARGET=krust`, builds the Krust kernel/substrate, builds
the Krust target user adapters from `targets/krust/user`, creates the
VertexDisk/VertexFS boot artifacts, and opens the QEMU window. Krust remains an
implementation target; the command you run is Vertex OS.

Useful root commands:

```sh
make doctor
make iso
make run
make smoke
make release-gate
```

`make run` boots headlessly with serial in the terminal. `make run-gui` uses a
QEMU window and writes GUI-run serial output under
`kernel/krust/build/serial-gui.log`.

## Current Demo

Build and validate the example generation:

```sh
cargo build --locked --offline
target/debug/vertexctl validate examples/hello-generation.vertex.json
target/debug/vertexctl validate examples/hello-stateful-generation.vertex.json
target/debug/vertexctl validate examples/deny-log-generation.vertex.json
target/debug/vertexctl graph examples/hello-generation.vertex.json
target/debug/vertexctl why examples/hello-generation.vertex.json svc:echo-server cap:log.sink
target/debug/vertexctl who-can examples/hello-generation.vertex.json cap:log.sink --json
target/debug/vertexctl compile-boot-manifest examples/hello-generation.vertex.json /private/tmp/hello-generation.krustboot
```

Materialize the host-side simulation demo into a local store tree:

```sh
target/debug/vertexctl materialize-demo examples/hello-generation.vertex.json /private/tmp/vertex-os-demo
```

Run the host-side activation simulation:

```sh
target/debug/vertex-supervisor --run-once /private/tmp/vertex-os-demo/hello-generation.hosted.vertex.json
```

In restricted sandboxes, local socket binding may require running the supervisor outside the sandbox.

Track host-side generation activation state under `.vertex/`:

```sh
target/debug/vertexctl activate /private/tmp/vertex-os-demo/hello-generation.hosted.vertex.json --run-once
target/debug/vertexctl generations
target/debug/vertexctl status --json
target/debug/vertexctl inspect current --json
target/debug/vertexctl rollback --run-once
```

Use `--state-root <dir>` with host-side activation and inspection commands to keep generation metadata somewhere other than `.vertex/`. Host-side activation records live under `<state-root>/activations/`, current and previous pointers are stored as JSON, activation history is appended to `<state-root>/history.jsonl`, and supervisor runtime events are appended to `<state-root>/runtime-events.jsonl`.

The host-side supervisor keeps typed capability and state grants internally before encoding them for child processes. Runtime events include concrete granted capabilities, provided capabilities, state volume paths, provider readiness checks, and activation failures. `examples/deny-log-generation.vertex.json` is a negative authority demo: `cap:log.sink` exists, but `svc:echo-server` does not declare it, so the supervisor does not pass the grant and the service exits. This is a development simulation of grant enforcement; standalone enforcement lives in Krust.
