# Vertex OS

Vertex OS is a typed, reproducible, capability-secure operating-system prototype where a running machine is represented as a generation graph.

The repository currently has two active paths:

1. Hosted Vertex prototype on Linux: Vertex IR, `vertexctl`,
   `vertex-supervisor`, and demo userland services.
2. Native Krust path: QEMU/Limine boot, compact KrustBoot manifest, native
   `vertex-init`, process-local capabilities, safe user-copy validation,
   PIT-backed preemption, user page-fault containment, and native multi-service
   activation.

## Repository Layout

```text
vertex-os/
  docs/                  Design notes and Vertex IR drafts
  schemas/               JSON schemas for manifest experiments
  examples/              Example generation manifests
  crates/
    vertex-ir/           Vertex IR model, loader, validation, graph helpers
    vertexctl/           CLI for validation, graphing, boot-manifest compilation, activation, inspection, and rollback
    vertex-supervisor/   Hosted Linux graph activator prototype
  userland/
    vertex-init/         Planned hosted first user-space activator
    logd/                Demo log service
    netstack/            Demo hosted network capability provider
    echo-server/         Demo service consuming log and network capabilities
  kernel/
    krust/               Bootable Krust kernel prototype, currently covering M14-M31 native graph activation and substrate hardening
  lang/
    vertex-lang/         Planned typed system-definition language
```

## Krust M14-M31

The current native activation path lives in `kernel/krust`. It is isolated
from the hosted Cargo workspace and boots a Limine ISO under QEMU. The ISO
build first runs `vertexctl compile-boot-manifest` to turn the source
generation graph into `hello-generation.krustboot`, a versioned KrustBoot
Manifest v1 native boot artifact.

Krust parses that module, loads the native service ELFs declared by the compact
manifest, creates process-local capabilities, marks non-initial services
`declared`, and enters ring 3 at `vertex-init`. Native `vertex-init` reads the
manifest through cap[0], logs through cap[1], starts declared services through
cap[2] process-control, waits for readiness, delegates attenuated IPC authority,
and supervises process exits. The QEMU smoke path now proves Manifest v1 bounds
checks, capability provenance/revocation, typed arena allocation, resource
quotas, service-local store/state/timer access, PIT timer preemption,
user page-fault containment, and a real restart of `flaky-service`, not
init-owned transcript logging.

```sh
scripts/krust-smoke.sh
```

The clean-clone release gate validates the hosted build, checks the Krust
toolchain, rebuilds the ISO from clean kernel artifacts, and runs the M14-M31
QEMU test matrix:

```sh
scripts/krust-release-gate.sh
```

See [docs/krust-milestones.md](docs/krust-milestones.md) for M0 through M31
completion status and the planned M32-M40 substrate-hardening roadmap,
and [docs/krust-abi-v0.md](docs/krust-abi-v0.md) for the current syscall,
capability, process, and IPC ABI.

The current Krust milestone status and deferred work are tracked in
[docs/krust-milestones.md](docs/krust-milestones.md).

## Current Demo

Build and validate the example generation:

```sh
cargo build --offline
target/debug/vertexctl validate examples/hello-generation.vertex.json
target/debug/vertexctl validate examples/hello-stateful-generation.vertex.json
target/debug/vertexctl validate examples/deny-log-generation.vertex.json
target/debug/vertexctl graph examples/hello-generation.vertex.json
target/debug/vertexctl why examples/hello-generation.vertex.json svc:echo-server cap:log.sink
target/debug/vertexctl who-can examples/hello-generation.vertex.json cap:log.sink --json
target/debug/vertexctl compile-boot-manifest examples/hello-generation.vertex.json /private/tmp/hello-generation.krustboot
```

Materialize the hosted demo into a local store tree:

```sh
target/debug/vertexctl materialize-demo examples/hello-generation.vertex.json /private/tmp/vertex-os-demo
```

Run the hosted activation:

```sh
target/debug/vertex-supervisor --run-once /private/tmp/vertex-os-demo/hello-generation.hosted.vertex.json
```

In restricted sandboxes, local socket binding may require running the supervisor outside the sandbox.

Track hosted generation activation state under `.vertex/`:

```sh
target/debug/vertexctl activate /private/tmp/vertex-os-demo/hello-generation.hosted.vertex.json --run-once
target/debug/vertexctl generations
target/debug/vertexctl status --json
target/debug/vertexctl inspect current --json
target/debug/vertexctl rollback --run-once
```

Use `--state-root <dir>` with hosted activation and inspection commands to keep generation metadata somewhere other than `.vertex/`. Hosted activation records live under `<state-root>/activations/`, current and previous pointers are stored as JSON, activation history is appended to `<state-root>/history.jsonl`, and supervisor runtime events are appended to `<state-root>/runtime-events.jsonl`.

The hosted supervisor now keeps typed capability and state grants internally before encoding them for child processes. Runtime events include concrete granted capabilities, provided capabilities, state volume paths, provider readiness checks, and activation failures. `examples/deny-log-generation.vertex.json` is a negative authority demo: `cap:log.sink` exists, but `svc:echo-server` does not declare it, so the supervisor does not pass the grant and the service exits. This is hosted grant enforcement; host-level process isolation remains a later milestone.
