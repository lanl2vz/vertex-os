# Vertex OS Philosophy v0

Status: design seed v0.1  
Date: 2026-05-22  
Project: Vertex OS  
Kernel: Krust Kernel

## One-sentence definition

**Vertex OS is a typed, reproducible, capability-secure operating system where the whole machine is represented as a generation graph, and Krust Kernel is the Rust microkernel that enforces that graph's runtime authority.**

A shorter slogan:

> **The system graph should be buildable, bootable, and runnable as a value.**

## Why the name Vertex?

In Vertex OS, every meaningful system object is a vertex:

- package
- executable
- service
- driver
- filesystem
- block device
- GPU queue
- network endpoint
- secret
- user identity
- state volume
- capability
- boot generation
- running process

Edges describe explicit relationships:

- `requires`
- `provides`
- `depends_on`
- `can_read`
- `can_write`
- `can_send`
- `can_receive`
- `controls`
- `owns`
- `starts_after`

The operating system is not merely configured by a graph. The operating system **is** the graph, activated and enforced.

## Philosophical Inheritance

Vertex OS inherits the strongest ideas from reproducible declarative operating
systems:

1. **Declarative systems** — the desired system is described, not hand-mutated into existence.
2. **Reproducibility** — build inputs and system inputs should be explicit.
3. **Immutable artifacts** — built outputs live in immutable store objects.
4. **Atomic upgrades** — switching system versions should be all-or-nothing.
5. **Rollback** — previous working generations should remain bootable or activatable.
6. **Graph thinking** — software is a dependency graph, not a pile of global files.

But Vertex OS changes the target. Existing declarative systems primarily make
the system **buildable and deployable** as an immutable closure. Vertex OS
should make the system **runnable** as an explicit graph whose runtime authority
is also declared.

## Core problem with ordinary operating systems

Traditional Unix-like systems expose a great deal of ambient authority:

- A process runs as a user and inherits broad filesystem access.
- Devices appear under global namespaces such as `/dev`.
- Runtime dependency discovery often happens by path lookup, environment variables, sockets, or convention.
- Kernel drivers and system services often hold much more authority than their actual task requires.
- Mutable state is scattered through `/var`, home directories, sockets, caches, pid files, and logs.

Declarative package and system managers improve the build and configuration
side dramatically, but they usually still run on the Linux/Unix authority model.

Vertex OS exists to remove this mismatch.

## Design law 1: the generation manifest is the source of truth

A running Vertex OS system is an activation of a generation manifest.

A generation manifest says:

- which kernel image is booted
- which init program starts
- which immutable store objects are live
- which services exist
- which capabilities exist
- which services receive which capabilities
- which state volumes exist
- which devices are controlled by which drivers
- which secrets may be opened by which services
- which services may communicate
- which system transition policy applies

The running system should be explainable by the manifest.

Every important operational question should have an answer in the graph:

- Why does this process exist?
- Why is this binary running?
- Why can this process talk to that service?
- Why can this service read this state volume?
- Why can this driver access this device?
- Why is this secret visible here?
- Why did this generation start these services in this order?

The answer should be: because a typed edge exists in the generation graph.

## Design law 2: explicit runtime authority

Explicit dependencies should not stop at build time.

Vertex OS should extend explicitness into runtime authority:

```text
build-time dependency  -> store reference
runtime dependency     -> capability reference
mutable state access   -> state capability
hardware access        -> device capability
secret access          -> secret capability
network exposure       -> endpoint capability
```

A service should not access the world by default. It should receive only the capabilities granted to it by the generation graph.

A service launched without a filesystem capability should not be able to walk arbitrary paths. A service launched without a network capability should not be able to bind or open arbitrary sockets. A service launched without a device capability should not be able to discover or control arbitrary hardware.

## Design law 3: capabilities are authority, names are not authority

Names are for humans and manifests. Capabilities are authority.

A service may know the name `postgres.socket`, but the name alone should not grant access. It must receive a concrete capability handle at launch or through an authorized delegation path.

This distinction prevents accidental authority leaks.

## Design law 4: Krust Kernel is an enforcer, not the whole OS

Krust Kernel should be small.

Its responsibilities:

- address spaces
- threads
- scheduling
- memory objects
- IPC endpoints
- capability tables
- interrupt routing
- timers
- device memory mapping
- IOMMU / DMA authority
- boot handoff to `vertex-init`

Its non-responsibilities:

- package management
- service graph evaluation
- high-level policy language
- filesystem policy
- user database management
- build orchestration
- Haskell-like module evaluation
- desktop environment

The kernel should enforce compact capabilities. The policy engine and graph evaluator should live in user space.

## Design law 5: the graph is control plane, not data plane

The typed graph should decide what exists and what is authorized. It should not be consulted for every byte of I/O.

Good design:

```text
evaluate graph -> grant compact capability -> use cheap kernel handle -> move data by shared memory / DMA / fast IPC
```

Bad design:

```text
every read, write, packet, and message re-evaluates the whole graph
```

The expensive reasoning should occur during build, validation, activation, service launch, capability delegation, and generation transition.

Hot paths should use:

- O(1) capability handle checks
- shared memory buffers
- batched IPC
- async I/O queues
- zero-copy transfer where practical
- IOMMU-controlled DMA for drivers

## Design law 6: mutable state is first-class

Mutable state is real. Vertex OS should not pretend otherwise.

State should be represented as explicit vertices:

- database volume
- cache volume
- log stream
- user home volume
- model-weight cache
- state snapshot
- backup target

A service does not merely write under `/var/lib`. It receives a state capability.

System rollback and state rollback must be separate concepts:

```text
system rollback: switch to an older immutable generation
state rollback: restore a mutable volume snapshot
coordinated rollback: restore generation and selected state checkpoints together
```

This distinction should exist from the beginning.

## Design law 7: compatibility must be contained

Vertex OS should eventually run existing POSIX/Linux software, but compatibility should be a personality layer, not the core model.

Native Vertex services should use explicit capabilities.

Legacy software may run inside a compatibility environment that maps explicit capabilities into familiar Unix-like namespaces:

- synthetic `/dev`
- synthetic `/proc`
- filesystem namespace
- socket namespace
- user/group mapping
- limited Linux/POSIX syscall personality
- VM fallback when necessary

The compatibility layer may emulate ambient authority internally, but it must be bounded by explicit Vertex capabilities from the outside.

## Design law 8: reproducibility must be native, not outsourced

Early Vertex OS should be buildable with ordinary, pinned, documented tools:
Rust, Cargo, QEMU, Limine, xorriso, and the repository's own scripts.

External package managers can be useful on individual developer machines, but
the repo should not require one as a functional dependency.

Vertex OS should define its own generation, store, state, and activation model.
The practical project rule is:

> **Vertex-native artifacts and manifests are the source of truth.**

## Principal components

### Krust Kernel

Rust microkernel / hybrid microkernel.

Primary goal: enforce isolation and capabilities with as little policy as possible.

### vertex-init

The first user-space process.

Responsibilities:

- receive boot manifest pointer
- verify manifest identity and store roots
- create initial user-space authority graph
- start required root services
- hand off to `vertex-supervisor`

### vertex-supervisor

Graph-native service manager.

Responsibilities:

- activate service vertices
- pass capabilities
- monitor health
- restart services according to policy
- coordinate generation transitions
- expose structured runtime introspection

### vertex-store

Immutable content-addressed store manager.

Responsibilities:

- expose read-only store objects
- manage store roots
- identify live generation closures
- coordinate garbage collection
- provide provenance metadata

### vertex-state

Mutable state manager.

Responsibilities:

- create state volumes
- grant state capabilities
- snapshot state
- coordinate backup and restore policy
- distinguish system rollback from state rollback

### vertexctl

Human and machine control tool.

Early commands:

- `vertexctl validate <manifest>`
- `vertexctl graph <manifest>`
- `vertexctl why <service> <capability>`
- `vertexctl activate <generation>`
- `vertexctl rollback`
- `vertexctl who-can <capability>`
- `vertexctl provenance <process-or-store-object>`

### vertex-lang

Typed functional system-definition language.

It may be implemented in Haskell, embedded in Haskell, or inspired by Haskell. The important requirement is that it produces deterministic Vertex IR without ambient host IO.

## Native service model

A native service is not just a command line.

A service vertex contains:

- service identity
- executable reference
- arguments
- environment
- required capabilities
- provided capabilities
- state volumes
- secrets
- restart policy
- resource budget
- health checks
- upgrade behavior

Example conceptually:

```text
service: prisma-api
  executable: store:prisma-api
  requires:
    - cap:postgres.socket/sendrecv
    - cap:gpu0.compute/submit
    - cap:secret.r2-token/read
    - cap:store.model-weights/read
  provides:
    - cap:http.8080/listen
  state:
    - none
  restart:
    - on-failure
```

## Minimal MVP target

The first working prototype should not be a desktop OS.

The first milestone should be a Linux-hosted Vertex generation runner:

```text
manifest -> validate -> activate -> inspect -> switch -> rollback
```

The second milestone should be a QEMU-bootable Krust Kernel that runs a tiny manifest:

```text
Krust boots
vertex-init reads manifest
logd starts
echo-service starts
echo-service sends to logd through a capability
serial output confirms the graph was enforced
```

## Non-goals for v0

Do not build these first:

- desktop environment
- GPU driver
- browser compatibility
- complete POSIX layer
- Linux syscall compatibility
- package-manager replacement
- network stack
- USB stack
- audio stack
- complex Haskell module system

The first goal is not broad usability. The first goal is to make the Vertex model real.

## Early threat model

Vertex OS v0 should assume:

- services may be buggy
- services may be compromised
- device drivers may crash
- manifests may be malformed
- capabilities may be accidentally over-granted by policy
- mutable state may diverge from immutable system state

Vertex OS v0 should not yet claim:

- formal verification
- complete side-channel resistance
- perfect rollback of all state
- safe execution of arbitrary Linux malware
- high-performance GPU support

## Success criteria for v0

A v0 prototype succeeds if it can demonstrate:

1. A generation manifest describes a small system.
2. The manifest validates.
3. Services start from the manifest.
4. Capabilities are passed explicitly.
5. A service cannot use a capability it was not granted.
6. `vertexctl graph` can display the system.
7. `vertexctl why` can explain authority.
8. A generation can be switched.
9. A previous generation can be reactivated.
10. The same conceptual manifest can eventually be booted by Krust Kernel.

## Open questions

- Should Vertex IR use canonical JSON, CBOR, Cap'n Proto, FlatBuffers, or a custom binary format?
- How should capability revocation work for long-lived handles?
- Which state-snapshot model should be assumed first: copy-on-write filesystem, content-addressed chunks, or external backend?
- Should the first Haskell-like system language be embedded Haskell, Dhall-like, Nickel-like, or a custom typed DSL?
- How much POSIX compatibility belongs in user-space libraries versus a compatibility server?
- How should store object provenance be represented without bloating manifests?
- Should Krust adopt seL4-like capability semantics, or a simpler design first?
- How can GPU and high-throughput networking be represented as explicit capabilities without destroying performance?

## Guiding sentence

> **First make the graph real. Then make the kernel enforce it.**
