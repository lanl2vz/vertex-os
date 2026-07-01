# Krust User Adapter Workspace

This workspace builds the user programs that run on the Krust native target.

The crates here are target adapters. They may depend on Vertex-owned userland
packages, translate those packages to Krust syscalls and directed IPC, and own
Krust linker/runtime glue. They should not be the primary home for portable
Vertex OS semantics.

For example, `console-shell` adapts the target-independent operator shell core
from `../../../userland/operator-shell` to the Krust console, inspect, and
generation-manager capabilities.

