# Vertex OS

Vertex OS is a typed, reproducible, capability-secure operating-system prototype where a running machine is represented as a generation graph.

Krust Kernel is the native Rust kernel prototype that will enforce that graph's
runtime authority. The repository now has two active tracks: a hosted Linux
prototype for Vertex IR, `vertexctl`, supervisor behavior, and capability
semantics; and a bootable Krust kernel under `kernel/krust` that runs under
QEMU/Limine and has reached the M10 compiled boot-manifest milestone.

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
    vertex-init/         Planned first user-space activator
    logd/                Demo log service
    netstack/            Demo hosted network capability provider
    echo-server/         Demo service consuming log and network capabilities
  kernel/
    krust/               Bootable Krust kernel prototype, currently at M10 krustboot
  lang/
    vertex-lang/         Planned typed system-definition language
  nix/                   Nix support modules and builders
  flake.nix              Planned root flake entrypoint
```

## Krust M10

The current native boot-authority milestone lives in `kernel/krust`. It is
isolated from the hosted Cargo workspace and boots a Limine ISO under QEMU. The
ISO build first runs `vertexctl compile-boot-manifest` to turn the source
manifest's `krustBoot` section into `hello-generation.krustboot`, a compact
fixed-format native boot manifest. Krust parses that module, prints the
generation/process/endpoint/grant records, allocates runtime process IDs from
those records, uses the manifest grants to create the IPC endpoint and
process-local capabilities, loads two static userspace ELF modules, keeps the M9
IDT and safe user-copy checks, accepts the authorized send/receive path, rejects
unauthorized cross-operations, transfers `Krust IPC ping`, and halts after
`IPC demo ok`.

```sh
cd kernel/krust
make build
make smoke
```

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
