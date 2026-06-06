#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd "$(dirname "$0")/.." && pwd)
KRUST_DIR=${KRUST_DIR:-"$ROOT_DIR/kernel/krust"}
LOG_DIR=${LOG_DIR:-"$KRUST_DIR/build/release-gate"}
KRUST_CASES=${KRUST_CASES:-"m14 manifest-cycle bad-cap readiness-timeout rollback store-state-services timer preemption user-fault restart manifest-v1 cap-lifecycle typed-arenas quotas m32 m33 m34 m35 m36 m37 m38 m40 m41 m42 m42-driver-fault m43 m43-bad-superblock m44 m45 m46 m47 m47-corrupt-executable m48 m49 m49-config-corrupt m50 m54 m55 m56 m57 m59 m60 m61 m62 m62-journal-replay m62-corrupt-journal m63 m64 m66 m67 m68 m69 m70 m71 m72 m73 m75 m76 m77 m78 m78-bad-superblock m78-journal-replay m78-journal-checkpoint-after-journal m78-journal-checkpoint-after-data m78-journal-checkpoint-after-inode m78-post-sync-remount m78-fsync-fault m79 m80 m81 m82 m82-vertexdisk-graph-corrupt manifest-truncated manifest-bad-magic manifest-raw-compact manifest-old-compact-magic manifest-graph-store-checksum manifest-graph-store-record manifest-unsupported-version manifest-oob-record manifest-missing-provider"}

fail() {
    echo "error: $*" >&2
    exit 1
}

step() {
    echo
    echo "==> $*"
}

run() {
    step "$*"
    "$@"
}

run_krust_fmt() {
    manifest=$1
    step "cargo fmt --manifest-path kernel/krust/$manifest -- --check"
    (cd "$KRUST_DIR" && cargo fmt --manifest-path "$manifest" -- --check)
}

require_doc_line() {
    file=$1
    pattern=$2
    grep -Fq "$pattern" "$ROOT_DIR/$file" || fail "$file does not mention: $pattern"
}

check_script() {
    script=$1
    path="$ROOT_DIR/$script"
    [ -f "$path" ] || fail "$script is missing"
    [ -x "$path" ] || fail "$script must be executable"
    sh -n "$path" || fail "$script failed POSIX shell syntax check"
}

check_no_trailing_whitespace() {
    file=$1
    path="$ROOT_DIR/$file"
    [ -f "$path" ] || fail "$file is missing"
    if grep -n '[[:blank:]]$' "$path"; then
        fail "$file has trailing whitespace"
    fi
}

require_absent() {
    path="$ROOT_DIR/$1"
    [ ! -e "$path" ] || fail "$1 is legacy and must stay removed"
}

mkdir -p "$LOG_DIR"
cd "$ROOT_DIR"

step "checking Krust script and Makefile hygiene"
check_script scripts/krust-smoke.sh
check_script scripts/krust-test.sh
check_script scripts/krust-release-gate.sh
make -C "$KRUST_DIR" -n iso >/dev/null
check_no_trailing_whitespace scripts/krust-smoke.sh
check_no_trailing_whitespace scripts/krust-test.sh
check_no_trailing_whitespace scripts/krust-release-gate.sh
check_no_trailing_whitespace kernel/krust/Makefile
check_no_trailing_whitespace kernel/krust/rust-toolchain.toml

step "checking Krust Rust and Markdown formatting"
run cargo fmt --all -- --check
run_krust_fmt Cargo.toml
for manifest in "$KRUST_DIR"/user/*/Cargo.toml; do
    run_krust_fmt "${manifest#"$KRUST_DIR"/}"
done
check_no_trailing_whitespace README.md
check_no_trailing_whitespace docs/krust-milestones.md
check_no_trailing_whitespace docs/krust-abi-v1.md
check_no_trailing_whitespace docs/posix-personality-v0.md
check_no_trailing_whitespace docs/krust-toolchain.md
check_no_trailing_whitespace kernel/krust/README.md

step "checking Krust status documentation"
require_doc_line README.md "M14-M82"
require_doc_line README.md "scripts/krust-release-gate.sh"
require_doc_line README.md "docs/krust-toolchain.md"
require_doc_line README.md "docs/krust-abi-v1.md"
require_doc_line docs/krust-milestones.md "declared-file journal checkpoint recovery"
require_doc_line docs/krust-milestones.md "## M25: Reproducible Clean-Clone Release Gate"
require_doc_line docs/krust-milestones.md "done: all M14-M24 QEMU tests are run from the gate"
require_doc_line docs/krust-milestones.md "done: M26-M29 manifest, capability, arena, quota, and malformed-manifest QEMU tests are run from the gate"
require_doc_line docs/krust-milestones.md "done: M30-M31 timer-preemption and user-fault containment QEMU tests are run from the gate"
require_doc_line docs/krust-milestones.md "done: M39 exact toolchain, Cargo lockfiles, and locked offline Cargo metadata are checked by the gate"
require_doc_line docs/krust-milestones.md "done: M40 directed request/reply IPC is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M41 native console shell is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M42 minimal virtio-block driver is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M43 VertexDisk layout is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M44 native boot manager fallback is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M45 store-object verification failure is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M46 native update transactions are checked by the gate"
require_doc_line docs/krust-milestones.md "done: M47 store-loaded executables and corrupt executable rejection are checked by the gate"
require_doc_line docs/krust-milestones.md "done: M48 dynamic process creation authority is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M49 immutable config objects and hash-mismatch rejection are checked by the gate"
require_doc_line docs/krust-milestones.md "done: M50 native secret authority is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M51 package inspection and instantiation are checked by the gate"
require_doc_line docs/krust-milestones.md "done: M52 graph linking is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M53 build graph import is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M54 appliance transcript is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M55 user-space driver framework is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M56 virtio device stack is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M57 networking v0 is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M58 POSIX compatibility plan is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M59 capability namespace service is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M60 policy and typed prototype are checked by the gate"
require_doc_line docs/krust-milestones.md "done: M61 ABI and authority hardening is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M62 storage durability cases are checked by the gate"
require_doc_line docs/krust-milestones.md "done: M63 network service boundary is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M64 supervisor lifecycle semantics are checked by the gate"
require_doc_line docs/krust-milestones.md "done: M65 supported appliance release profile is checked by the gate"
require_doc_line docs/krust-milestones.md "done: M66 owned frame ledger, reclaim counters, double-free/foreign-free checks,"
require_doc_line docs/krust-milestones.md "done: M67 process exit, restart, fault, and create/start/exit churn reclaim user"
require_doc_line docs/krust-milestones.md "done: M68 endpoint, cap grant, cap transfer, namespace resolution, process"
require_doc_line docs/krust-milestones.md "done: M69 100-cycle create/start/exit, restart, endpoint churn, and fault/restart"
require_doc_line docs/krust-milestones.md "done: M70 blocking IRQ wait, timeout, authority rejection, net/block interrupt"
require_doc_line docs/krust-milestones.md "done: M71 DMA ownership, repeat-map idempotence, release-on-teardown, manifest"
require_doc_line docs/krust-milestones.md "done: M72 virtio queue reports, timeout-to-reset paths, owner release,"
require_doc_line docs/krust-milestones.md "done: M73 device-fault isolation, DMA/IRQ/virtio leak deltas, bad hardware"
require_doc_line docs/krust-milestones.md "done: M80 advisory file locks, directory watch events, bounded pipe buffering,"
require_doc_line docs/krust-milestones.md "done: M81 capability revocation with live handles, 100-cycle VFS churn,"
require_doc_line docs/krust-milestones.md "done: KrustBoot compact payload identity is \`KRUSTBOOTM82\` version 13"
require_doc_line docs/krust-milestones.md "## M42: Minimal Virtio-Block Driver"
require_doc_line docs/krust-milestones.md "## M43: VertexDisk v1 Layout"
require_doc_line docs/krust-milestones.md "done: unwrapped compact payload rejected"
require_doc_line docs/krust-milestones.md "## M39: Reproducible Build Environment"
require_doc_line docs/krust-milestones.md "## M40: Vertex Native Runtime ABI v1"
require_doc_line docs/krust-milestones.md "Status: done."
require_doc_line docs/krust-milestones.md "## M29: Resource Accounting And Quotas"
require_doc_line docs/krust-toolchain.md "rustc 1.95.0"
require_doc_line docs/krust-toolchain.md "cargo 1.95.0"
require_doc_line docs/krust-toolchain.md "qemu-system-x86_64 11.0.0"
require_doc_line docs/krust-toolchain.md "limine 12.3.0"
require_doc_line docs/krust-toolchain.md "xorriso 1.5.8.pl01"
require_doc_line docs/krust-abi-v1.md "M40 freezes ABI v1"
require_doc_line docs/posix-personality-v0.md "Status: M58 design artifact."
require_doc_line kernel/krust/README.md "M14-M82"
require_doc_line kernel/krust/README.md "scripts/krust-release-gate.sh"
require_doc_line kernel/krust/README.md "rustc 1.95.0"
require_doc_line kernel/krust/README.md "directed IPC"

step "checking removed legacy Krust userspace crates"
require_absent kernel/krust/user/hello/Cargo.toml
require_absent kernel/krust/user/ipc/Cargo.toml

step "checking locked offline Cargo metadata"
run cargo metadata --locked --offline --no-deps --format-version 1

step "cargo build --locked --offline"
offline_log="$LOG_DIR/cargo-build-offline.log"
if ! cargo build --locked --offline >"$offline_log" 2>&1; then
    cat "$offline_log"
    fail "cargo build --locked --offline failed. Populate the Cargo cache from Cargo.lock or configure a vendored Cargo source before running the M40 gate offline."
fi

step "cargo test --locked --offline -p vertex-ir -p vertexctl"
test_log="$LOG_DIR/cargo-test-offline.log"
if ! cargo test --locked --offline -p vertex-ir -p vertexctl >"$test_log" 2>&1; then
    cat "$test_log"
    fail "cargo test --locked --offline -p vertex-ir -p vertexctl failed"
fi

run "$ROOT_DIR/target/debug/vertexctl" validate "$ROOT_DIR/examples/hello-generation.vertex.json"
run "$ROOT_DIR/target/debug/vertexctl" package inspect "$ROOT_DIR/examples/packages/logd.vertexpkg"
run "$ROOT_DIR/target/debug/vertexctl" package instantiate "$ROOT_DIR/examples/packages/logd.vertexpkg"
run "$ROOT_DIR/target/debug/vertexctl" compile-policy "$ROOT_DIR/examples/policy.vertex" "$LOG_DIR/m60-policy-generation.vertex.json"
run "$ROOT_DIR/target/debug/vertexctl" compile-typed "$ROOT_DIR/examples/typed-system.vertex" "$LOG_DIR/m60-typed-generation.vertex.json"
if "$ROOT_DIR/target/debug/vertexctl" compile-typed "$ROOT_DIR/examples/invalid-missing-capability.vertex" "$LOG_DIR/m60-invalid-generation.vertex.json"; then
    fail "typed policy unexpectedly accepted missing capability wiring"
fi
run "$ROOT_DIR/target/debug/vertexctl" compile-boot-manifest "$LOG_DIR/m60-policy-generation.vertex.json" "$LOG_DIR/m60-policy.krustboot"
run "$ROOT_DIR/target/debug/vertexctl" create-vertex-disk "$LOG_DIR/m60-policy.img" "$LOG_DIR/m60-policy-generation.vertex.json"

run make -C "$KRUST_DIR" doctor
run make -C "$KRUST_DIR" clean
run make -C "$KRUST_DIR" smoke
mkdir -p "$LOG_DIR"
release_profile="$LOG_DIR/m82-release-profile.txt"
step "$ROOT_DIR/target/debug/vertexctl release-profile $ROOT_DIR/examples/hello-generation.vertex.json $KRUST_DIR/build/hello-generation.krustboot $KRUST_DIR/target/x86_64-unknown-none/debug/krust $KRUST_DIR/build/krust-block.img"
"$ROOT_DIR/target/debug/vertexctl" release-profile "$ROOT_DIR/examples/hello-generation.vertex.json" "$KRUST_DIR/build/hello-generation.krustboot" "$KRUST_DIR/target/x86_64-unknown-none/debug/krust" "$KRUST_DIR/build/krust-block.img" >"$release_profile"
cat "$release_profile"
grep -Fq "krustboot=Manifest v1 compact KRUSTBOOTM82 version 13" "$release_profile" || fail "release profile missing M82 KrustBoot identity"
grep -Fq "base-profile=no POSIX personality, no legacy transport, no legacy payload" "$release_profile" || fail "release profile missing supported base profile"

mkdir -p "$LOG_DIR/graph-link" "$LOG_DIR/build-import"
run "$ROOT_DIR/target/debug/vertexctl" graph-link "$LOG_DIR/graph-link" "$ROOT_DIR/examples/packages/serial-driver.vertexpkg" "$ROOT_DIR/examples/packages/logd.vertexpkg" "$ROOT_DIR/examples/packages/echo.vertexpkg"
run "$ROOT_DIR/target/debug/vertexctl" build-import "$ROOT_DIR/examples/build-output.json" --output "$LOG_DIR/build-import"

for case_name in $KRUST_CASES; do
    run "$ROOT_DIR/scripts/krust-test.sh" "$case_name"
done

echo
echo "Krust release gate ok: clean-clone M14-M82 substrate proof is repeatable."
