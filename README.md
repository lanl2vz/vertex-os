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
  kernel/
    krust/               Bootable Krust kernel prototype, currently covering M14-M77 native graph activation, substrate hardening, pinned tooling, ABI v1 IPC, console shell, virtio device I/O, VertexDisk v0 durability, native boot selection, verified store objects, native updates, store-loaded executables, dynamic process creation, native config/secrets, package/link/build import, the first appliance transcript, native user-space driver objects, netstack boundaries, capability namespaces, policy compilation, lifecycle supervision, a supported standalone appliance profile, owned frame reclamation, address-space teardown, failure-atomic kernel object creation, memory lifecycle soak gates, interrupt routing, DMA ownership, virtio recovery, device-fault isolation, VFS root authority, open-file handles, directory metadata operations, and bounded block-cache writeback
  lang/
    vertex-lang/         Planned typed system-definition language
```

## Krust M14-M77

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
virtio-blk sector I/O over PCI/DMA authority, M43 VertexDisk v0
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
restart of `flaky-service`, not init-owned transcript logging.

```sh
scripts/krust-smoke.sh
```

The clean-clone release gate validates the host-side tool build, checks the
Krust toolchain, rebuilds the standalone ISO from clean kernel artifacts, and
runs the M14-M77 substrate gate with the M14-M77 QEMU test matrix:

```sh
scripts/krust-release-gate.sh
```

See [docs/krust-milestones.md](docs/krust-milestones.md) for M0 through M77
current status and the appliance OS MVP profile,
[docs/krust-toolchain.md](docs/krust-toolchain.md) for the pinned M39 toolchain,
and [docs/krust-abi-v1.md](docs/krust-abi-v1.md) for the current syscall,
capability, process, and IPC ABI.
The M58 compatibility plan is tracked in
[docs/posix-personality-v0.md](docs/posix-personality-v0.md).

The current Krust milestone status and deferred work are tracked in
[docs/krust-milestones.md](docs/krust-milestones.md).

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
