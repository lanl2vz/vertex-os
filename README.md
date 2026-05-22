# Vertex OS

Vertex OS is a typed, reproducible, capability-secure operating-system prototype where a running machine is represented as a generation graph.

Krust Kernel is the Rust kernel planned to enforce that graph's runtime authority. The current repository starts with the hosted Linux prototype: Vertex IR, `vertexctl`, a hosted supervisor, and small userland services that demonstrate explicit capability passing.

## Repository Layout

```text
vertex-os/
  docs/                  Design notes and Vertex IR drafts
  schemas/               JSON schemas for manifest experiments
  examples/              Example generation manifests
  crates/
    vertex-ir/           Vertex IR model, loader, validation, graph helpers
    vertexctl/           CLI for validation, graphing, activation, inspection, and rollback
    vertex-supervisor/   Hosted Linux graph activator prototype
  userland/
    vertex-init/         Planned first user-space activator
    logd/                Demo log service
    netstack/            Demo hosted network capability provider
    echo-server/         Demo service consuming log and network capabilities
  kernel/
    krust/               Planned Krust Kernel prototype
  lang/
    vertex-lang/         Planned typed system-definition language
  nix/                   Nix support modules and builders
  flake.nix              Planned root flake entrypoint
```

## Krust M2

The first native Vertex manifest module milestone lives in `kernel/krust`. It
is isolated from the hosted Cargo workspace and boots a Limine ISO under QEMU
until Krust prints `Krust Kernel booted`, reads Limine's memory map, finds the
packaged `hello-generation.vertex.json` boot module, prints its generation ID
to the serial console, and halts.

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
