# POSIX Personality v0

Status: M58 design artifact.

POSIX support is a Vertex service, not a kernel compatibility mode. A POSIX
personality may expose ambient-looking file descriptors, process IDs, signals,
paths, and sockets inside its own sandbox, but the personality service itself is
started only with explicit Vertex capabilities.

## Layers

1. Vertex-native services remain the preferred interface.
2. A WASI personality maps preopened directories, clocks, random, and sockets
   from explicit Vertex capabilities.
3. A POSIX personality maps a service-local `/`, `/dev`, `/tmp`, process table,
   and socket table from explicit namespace, state, device, and network caps.
4. A Linux research personality may emulate Linux syscalls behind the same
   capability boundary.
5. VM fallback is used when syscall emulation would create hidden authority.

## Rules

- The compatibility namespace is a capability passed to the personality.
- Resolution returns capabilities or personality-local handles, never global
  kernel access.
- Device, network, state, store, secret, and time access must be declared in the
  generation graph before the personality starts.
- No legacy global `/dev`, process table, or filesystem root is retained in the
  native Krust runtime.

## First Tests

- Launch one POSIX personality with a namespace containing `/state/a`.
- Launch another with a namespace containing `/state/b`.
- Verify the first cannot resolve `/state/b`.
- Verify runtime inspect reports the namespace capabilities without dumping
  secret or config contents.
