#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd "$(dirname "$0")/.." && pwd)
KRUST_DIR=${KRUST_DIR:-"$ROOT_DIR/kernel/krust"}
LOG_DIR=${LOG_DIR:-"$KRUST_DIR/build/release-gate"}
KRUST_CASES=${KRUST_CASES:-"m14 manifest-cycle bad-cap readiness-timeout rollback store-state-services timer preemption user-fault restart manifest-v1 cap-lifecycle typed-arenas quotas m32 m33 m34 m35 m36 m37 m38 m40 m41 m42 m42-driver-fault m43 m43-bad-superblock m44 m45 m46 m47 m47-corrupt-executable m48 m49 m49-config-corrupt m50 m54 manifest-truncated manifest-bad-magic manifest-raw-compact manifest-unsupported-version manifest-oob-record manifest-missing-provider"}

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
check_no_trailing_whitespace docs/krust-toolchain.md
check_no_trailing_whitespace kernel/krust/README.md

step "checking Krust status documentation"
require_doc_line README.md "M14-M54"
require_doc_line README.md "scripts/krust-release-gate.sh"
require_doc_line README.md "docs/krust-toolchain.md"
require_doc_line README.md "docs/krust-abi-v1.md"
require_doc_line docs/krust-milestones.md "Current status: M14-M54"
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
require_doc_line docs/krust-milestones.md "## M42: Minimal Virtio-Block Driver"
require_doc_line docs/krust-milestones.md "## M43: VertexDisk v0 Layout"
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
require_doc_line kernel/krust/README.md "M26-M54 Substrate"
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

run "$ROOT_DIR/target/debug/vertexctl" validate "$ROOT_DIR/examples/hello-generation.vertex.json"
run "$ROOT_DIR/target/debug/vertexctl" package inspect "$ROOT_DIR/examples/packages/logd.vertexpkg"
run "$ROOT_DIR/target/debug/vertexctl" package instantiate "$ROOT_DIR/examples/packages/logd.vertexpkg"

run make -C "$KRUST_DIR" doctor
run make -C "$KRUST_DIR" clean
run make -C "$KRUST_DIR" smoke

mkdir -p "$LOG_DIR/graph-link" "$LOG_DIR/build-import"
run "$ROOT_DIR/target/debug/vertexctl" graph-link "$LOG_DIR/graph-link" "$ROOT_DIR/examples/packages/serial-driver.vertexpkg" "$ROOT_DIR/examples/packages/logd.vertexpkg" "$ROOT_DIR/examples/packages/echo.vertexpkg"
run "$ROOT_DIR/target/debug/vertexctl" build-import "$ROOT_DIR/examples/build-output.json" --output "$LOG_DIR/build-import"

for case_name in $KRUST_CASES; do
    run "$ROOT_DIR/scripts/krust-test.sh" "$case_name"
done

echo
echo "Krust release gate ok: clean-clone M14-M54 proof is repeatable."
