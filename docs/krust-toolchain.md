# Krust M39 Toolchain

M39 pins the native Krust build to exact host tool versions. The release gate
does not accept floating `stable`, minimum-version ranges, or legacy user crates.

Required tools:

```text
rustc 1.95.0
cargo 1.95.0
rustfmt 1.9.0-stable
qemu-system-x86_64 11.0.0
limine 12.3.0
xorriso 1.5.8.pl01
```

The native Rust toolchain is pinned by
`kernel/krust/rust-toolchain.toml`:

```text
channel = "1.95.0"
targets = ["x86_64-unknown-none"]
components = ["rustfmt"]
```

Install the pinned Rust toolchain with:

```sh
rustup toolchain install 1.95.0 --profile minimal --component rustfmt --target x86_64-unknown-none
```

Cargo dependencies are locked by `Cargo.lock` at the repository root, by
`kernel/krust/Cargo.lock`, and by the native userspace workspace lockfile at
`targets/krust/user/Cargo.lock`. Krust builds invoke Cargo with `--locked`.
The release gate also runs the top-level host-tool workspace with
`--locked --offline`.

Run the complete M39 gate from the repository root:

```sh
scripts/krust-release-gate.sh
```

Run the native tool check directly with:

```sh
make doctor
```
