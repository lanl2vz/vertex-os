# Vertex OS Step 1 Starter Pack

Status: design seed v0.1  
Date: 2026-05-22  
Project: Vertex OS with Krust Kernel

This starter pack implements the first project step: writing down the initial philosophy and the first draft of the Vertex IR / generation manifest.

## Files

- `docs/philosophy.md` — the core operating-system philosophy and design laws.
- `docs/vertex-ir-v0.md` — the first draft of the Vertex generation manifest / IR.
- `schemas/vertex-ir-v0.schema.json` — an intentionally incomplete but useful JSON Schema for early experiments.
- `examples/hello-generation.vertex.json` — a minimal example generation containing `logd`, `netstack`, and `echo-server`.

## How to use this pack

Treat this as the first commit of the design, not as a final specification.

Suggested next implementation order:

1. Implement a small Rust crate named `vertex-ir` that can load, validate, and pretty-print `examples/hello-generation.vertex.json`.
2. Implement `vertexctl validate`, `vertexctl graph`, and `vertexctl why` against this IR.
3. Build a Linux-hosted `vertex-supervisor` that activates the example manifest with ordinary Linux subprocesses and simulated capabilities.
4. Only then begin the first QEMU-bootable Krust Kernel prototype.

## Source anchors

The design is inspired by, but not equivalent to, existing systems:

- Nix / NixOS: reproducible, declarative, reliable system construction and rollback.
- seL4: capability-oriented microkernel design and high-assurance kernel research.
- Redox OS: Rust-written microkernel operating-system work with many components in user space.

Vertex OS should not be a clone of any of these. Its distinct target is: the entire machine as a typed, reproducible, capability-enforced graph.
