# Krust Target

This directory contains Vertex OS code that is specific to the Krust native
target.

Krust itself remains under `kernel/krust`: boot, memory, syscalls, IPC,
scheduling, capability enforcement, device authority, VFS, and native graph
activation substrate.

Krust userspace adapter crates live under `targets/krust/user`. They are built
against the Krust ABI and may contain syscall bindings, IPC transport, linker
configuration, and target-specific process entry points. Portable OS semantics
belong in `userland/`.

