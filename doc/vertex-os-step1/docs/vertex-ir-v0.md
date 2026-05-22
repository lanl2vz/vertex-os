# Vertex IR v0: Generation Manifest Draft

Status: design seed v0.1  
Date: 2026-05-22  
Project: Vertex OS  
Kernel: Krust Kernel

## Purpose

Vertex IR, also called VIR, is the canonical intermediate representation of a Vertex OS generation.

It is the bridge between:

```text
vertex-lang / Haskell-like system definition
        ↓
Vertex IR generation manifest
        ↓
vertex-supervisor or vertex-init
        ↓
Krust Kernel capability objects and user-space services
```

The IR should describe **what the system is** before the system is activated.

The first draft uses JSON for readability. JSON is not necessarily the final wire format.

## Design constraints

VIR must be:

1. **Deterministic** — the same logical generation must serialize to the same canonical form.
2. **Inspectable** — humans and tools must be able to answer “why” questions.
3. **Typed** — vertices and edges must have kinds, rights, and validation rules.
4. **Hashable** — a generation manifest should have a stable identity.
5. **Bootable** — `vertex-init` should be able to activate it without evaluating a high-level language.
6. **Kernel-translatable** — capabilities in the manifest must map to compact Krust Kernel handles.
7. **Evolvable** — the schema must allow extension without making v0 unusable.

## Core idea

A generation is a graph:

```text
vertices: store objects, executables, services, capabilities, devices, state volumes, secrets
edges: requires, provides, grants, owns, controls, starts_after, depends_on
```

The manifest stores the graph in a structured form.

## Top-level shape

A v0 manifest has these top-level sections:

```json
{
  "schema": "vertex.ir.v0",
  "generation": {},
  "kernel": {},
  "init": {},
  "store": [],
  "executables": [],
  "devices": [],
  "stateVolumes": [],
  "secrets": [],
  "capabilities": [],
  "services": [],
  "activation": {},
  "policies": {}
}
```

Only a subset is required for the first Linux-hosted prototype.

## Identifier conventions

All graph objects use stable typed identifiers.

Suggested prefixes:

```text
gen:       generation
kernel:    kernel object
init:      init object
store:     immutable store object
exe:       executable
svc:       service
cap:       capability
state:     mutable state volume
secret:    secret
dev:       device
policy:    policy object
user:      user identity
group:     group identity
```

Examples:

```text
gen:hello-0001
svc:logd
svc:echo-server
cap:log.sink
cap:net.tcp.8080
state:postgres-data
store:blake3-f00d...-logd
```

IDs should be stable inside a manifest. Store IDs should eventually derive from content identity.

## Generation object

The `generation` object identifies the system generation.

Required fields:

- `id`
- `createdUtc`
- `description`

Optional fields:

- `parent`
- `source`
- `author`
- `manifestHash`
- `signatures`

Example:

```json
{
  "id": "gen:hello-0001",
  "createdUtc": "2026-05-22T00:00:00Z",
  "description": "Minimal Vertex OS hosted demo generation",
  "parent": null
}
```

## Kernel object

The `kernel` object declares the kernel artifact.

For a Linux-hosted prototype, this may be a placeholder.

For Krust-native boot, it identifies the Krust Kernel image.

Fields:

- `id`
- `kind`
- `storeObject`
- `abi`
- `target`

Example:

```json
{
  "id": "kernel:krust-qemu-x86_64",
  "kind": "krust-kernel",
  "storeObject": "store:krust-kernel-demo",
  "abi": "krust.abi.v0",
  "target": "x86_64-unknown-none"
}
```

## Init object

The `init` object declares the first user-space activator.

Fields:

- `id`
- `executable`
- `mode`

Modes:

- `hosted-linux`
- `krust-native`

Example:

```json
{
  "id": "init:vertex-init",
  "executable": "exe:vertex-init",
  "mode": "hosted-linux"
}
```

## Store objects

Store objects are immutable artifacts.

Fields:

- `id`
- `name`
- `kind`
- `path`
- `hashAlgorithm`
- `hash`
- `sizeBytes`
- `references`

Kinds:

- `kernel-image`
- `executable`
- `library`
- `data`
- `manifest`
- `source`
- `debug-symbols`

Example:

```json
{
  "id": "store:logd-demo",
  "name": "logd-demo",
  "kind": "executable",
  "path": "/vertex/store/blake3-demo-logd",
  "hashAlgorithm": "blake3",
  "hash": "demo-not-a-real-hash-logd",
  "sizeBytes": 65536,
  "references": []
}
```

## Executables

Executables bind a runnable entrypoint to a store object.

Fields:

- `id`
- `storeObject`
- `entrypoint`
- `abi`
- `argsDefault`

Example:

```json
{
  "id": "exe:logd",
  "storeObject": "store:logd-demo",
  "entrypoint": "bin/logd",
  "abi": "hosted-linux-process.v0",
  "argsDefault": []
}
```

## Capabilities

A capability is runtime authority.

Fields:

- `id`
- `kind`
- `provider`
- `rights`
- `properties`

Early capability kinds:

```text
ipc-endpoint
clock
log-sink
network-port
store-read
state-volume
secret-read
device-control
process-control
```

Rights are capability-specific.

Common rights:

```text
read
write
send
receive
sendrecv
bind
listen
connect
map
execute
control
snapshot
restore
delegate
revoke
```

Example:

```json
{
  "id": "cap:log.sink",
  "kind": "ipc-endpoint",
  "provider": "svc:logd",
  "rights": ["send"],
  "properties": {
    "protocol": "vertex.log.v0"
  }
}
```

Important rule: a capability declaration describes possible authority. A service receives authority only when it has a matching `requires` entry and the activation policy grants it.

## Services

Services are managed runtime vertices.

Fields:

- `id`
- `name`
- `executable`
- `args`
- `env`
- `requires`
- `provides`
- `state`
- `secrets`
- `restart`
- `resources`
- `health`
- `lifecycle`

Example:

```json
{
  "id": "svc:echo-server",
  "name": "echo-server",
  "executable": "exe:echo-server",
  "args": ["--listen", "cap:net.tcp.8080"],
  "env": {},
  "requires": [
    { "capability": "cap:log.sink", "rights": ["send"] },
    { "capability": "cap:net.tcp.8080", "rights": ["listen"] }
  ],
  "provides": ["cap:echo.api"],
  "state": [],
  "secrets": [],
  "restart": "on-failure",
  "resources": {
    "memoryMaxBytes": 67108864,
    "cpuShares": 100
  },
  "health": {
    "kind": "ipc-ping",
    "target": "cap:echo.api"
  },
  "lifecycle": {
    "startAfter": ["svc:logd", "svc:netstack"],
    "stopBefore": []
  }
}
```

## State volumes

A state volume is mutable storage with explicit ownership and snapshot policy.

Fields:

- `id`
- `name`
- `kind`
- `owner`
- `mountIntent`
- `snapshotPolicy`
- `backupPolicy`

Example:

```json
{
  "id": "state:postgres-data",
  "name": "postgres-data",
  "kind": "local-cow-volume",
  "owner": "svc:postgres",
  "mountIntent": "private",
  "snapshotPolicy": {
    "enabled": true,
    "keepDaily": 7,
    "keepWeekly": 8
  },
  "backupPolicy": {
    "enabled": false
  }
}
```

## Secrets

A secret is not stored directly in the manifest.

The manifest may reference a secret identity and policy.

Fields:

- `id`
- `name`
- `provider`
- `requiredAt`
- `rotationPolicy`

Example:

```json
{
  "id": "secret:r2-token",
  "name": "r2-token",
  "provider": "external-agent",
  "requiredAt": "service-start",
  "rotationPolicy": "manual"
}
```

## Devices

A device vertex represents hardware or virtual hardware.

Fields:

- `id`
- `kind`
- `selector`
- `driver`
- `properties`

Example:

```json
{
  "id": "dev:gpu0",
  "kind": "pci-device",
  "selector": {
    "pciVendor": "10de",
    "pciDevice": "*"
  },
  "driver": "svc:nvidia-driver",
  "properties": {
    "role": "gpu-compute"
  }
}
```

For v0, devices may be omitted.

## Activation object

The activation object tells `vertex-init` or `vertex-supervisor` how to activate the graph.

Fields:

- `rootService`
- `startOrder`
- `rollbackPolicy`
- `onFailure`

Example:

```json
{
  "rootService": "svc:vertex-supervisor",
  "startOrder": ["svc:logd", "svc:netstack", "svc:echo-server"],
  "rollbackPolicy": {
    "default": "system-only",
    "state": "preserve-unless-explicit"
  },
  "onFailure": "stop-activation"
}
```

## Policies object

The policies object contains generation-wide rules.

Early policies:

- default deny all ungranted authority
- forbid undeclared device access
- forbid undeclared secret access
- require every service executable to resolve to a store object
- require every required capability to exist
- require every provided capability to name its provider

Example:

```json
{
  "defaultAuthority": "deny",
  "allowAmbientFilesystem": false,
  "allowAmbientNetwork": false,
  "allowAmbientDevices": false,
  "capabilityDelegation": "explicit-only",
  "unknownReferences": "reject"
}
```

## Validation rules v0

A manifest validator must reject a manifest when:

1. `schema` is not `vertex.ir.v0`.
2. Any ID is duplicated across the same namespace.
3. A service references an executable that does not exist.
4. An executable references a store object that does not exist.
5. A service requires a capability that does not exist.
6. A service provides a capability whose `provider` is a different service.
7. A capability provider does not exist.
8. A state volume owner does not exist.
9. A secret reference does not exist.
10. The activation `startOrder` references an unknown service.
11. A `startAfter` reference names an unknown service.
12. A required right is not included in the capability's rights set.
13. A service receives authority not represented in `requires`.
14. Ambient filesystem, network, or device access is requested while policy forbids it.

A validator should warn, not reject, when:

1. `startOrder` omits a service that is not root or not required.
2. A capability is declared but unused.
3. A store object is declared but unreachable from any executable or kernel object.
4. A service has no health check.
5. A state volume has no snapshot policy.

## Canonicalization v0

The first implementation may use readable JSON.

For stable generation identity, canonicalization should eventually specify:

- UTF-8 only
- sorted object keys
- sorted arrays where order is not semantically meaningful
- no insignificant whitespace
- normalized timestamps
- explicit nulls only where allowed
- no host-dependent paths except store paths

`manifestHash` should be computed after canonicalization with a specified algorithm such as BLAKE3 or SHA-256.

## Hosted Linux interpretation

The first prototype can simulate capabilities on Linux.

Possible mappings:

```text
ipc-endpoint     -> Unix domain socket or inherited file descriptor
network-port     -> supervisor binds socket and passes fd
store-read       -> read-only bind mount or path env var
state-volume     -> temporary directory or bind mount
secret-read      -> inherited pipe/fd, never environment variable if avoidable
clock            -> ordinary host clock access, later explicit syscall/cap
log-sink         -> pipe or Unix socket to logd
```

The hosted prototype should preserve Vertex semantics even if Linux cannot fully enforce them.

## Krust-native interpretation

In Krust-native mode:

1. `vertex-init` receives the manifest from bootloader/initrd/store.
2. It verifies the generation identity.
3. It asks Krust Kernel to create initial IPC endpoints, memory objects, and process objects.
4. It maps manifest capabilities to kernel capability handles.
5. It launches services with initial capability tables.
6. `vertex-supervisor` takes over lifecycle management.

The kernel does not evaluate the high-level graph. It only sees compact capability objects and process tables.

## Minimal example graph

The first meaningful graph should have:

- `svc:logd` providing `cap:log.sink`
- `svc:netstack` providing `cap:net.tcp.8080`
- `svc:echo-server` requiring both and providing `cap:echo.api`

This is sufficient to prove:

- service dependency
- explicit capability grant
- graph validation
- graph explanation
- generation activation
- generation rollback

## Example `vertexctl why`

Command:

```text
vertexctl why examples/hello-generation.vertex.json svc:echo-server cap:log.sink
```

Expected explanation:

```text
svc:echo-server can use cap:log.sink because:

1. cap:log.sink exists and is kind ipc-endpoint.
2. cap:log.sink is provided by svc:logd.
3. svc:echo-server declares a requirement for cap:log.sink with right send.
4. cap:log.sink grants right send.
5. generation policy defaultAuthority=deny does not grant anything else.
```

## Evolution plan

VIR v0 is for a hosted prototype.

VIR v1 should add:

- formal capability kind registry
- explicit edge list form
- canonical binary serialization
- manifest signing
- store closure hashing
- richer state snapshot model
- typed resource budgets
- versioned ABI negotiation
- device-driver authority model
- compatibility personality declarations

VIR v2 should be suitable for Krust-native boot.

## Non-goals for VIR v0

VIR v0 does not attempt to specify:

- complete package build derivations
- full POSIX compatibility
- dynamic service discovery
- distributed deployment
- formal verification model
- high-performance driver IPC
- exact Krust syscall ABI

VIR v0 is the design bridge from philosophy to implementation.
