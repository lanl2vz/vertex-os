#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd "$(dirname "$0")/.." && pwd)
KRUST_DIR=${KRUST_DIR:-"$ROOT_DIR/kernel/krust"}
LOG_DIR=${LOG_DIR:-"$KRUST_DIR/build/release-gate"}
KRUST_CASES=${KRUST_CASES:-"m14 manifest-cycle bad-cap readiness-timeout rollback store-state timer restart manifest-v1 cap-lifecycle typed-arenas quotas manifest-truncated manifest-bad-magic manifest-raw-compact manifest-unsupported-version manifest-oob-record manifest-missing-provider"}

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

step "checking Krust Rust and Markdown formatting"
run cargo fmt --all -- --check
for manifest in "$KRUST_DIR"/Cargo.toml "$KRUST_DIR"/user/*/Cargo.toml; do
    run cargo fmt --manifest-path "$manifest" -- --check
done
check_no_trailing_whitespace README.md
check_no_trailing_whitespace docs/krust-milestones.md
check_no_trailing_whitespace docs/krust-abi-v0.md
check_no_trailing_whitespace kernel/krust/README.md

step "checking Krust status documentation"
require_doc_line README.md "M14-M29"
require_doc_line README.md "scripts/krust-release-gate.sh"
require_doc_line docs/krust-milestones.md "Current status: M14-M29"
require_doc_line docs/krust-milestones.md "## M25: Reproducible Clean-Clone Release Gate"
require_doc_line docs/krust-milestones.md "done: all M14-M24 QEMU tests are run from the gate"
require_doc_line docs/krust-milestones.md "done: M26-M29 manifest, capability, arena, quota, and malformed-manifest QEMU tests are run from the gate"
require_doc_line docs/krust-milestones.md "done: unwrapped compact payload rejected"
require_doc_line docs/krust-milestones.md "## M29: Resource Accounting And Quotas"
require_doc_line docs/krust-abi-v0.md "M26-M29 add"
require_doc_line kernel/krust/README.md "M26-M29 Substrate"
require_doc_line kernel/krust/README.md "scripts/krust-release-gate.sh"

step "cargo build --offline"
offline_log="$LOG_DIR/cargo-build-offline.log"
if ! cargo build --offline >"$offline_log" 2>&1; then
    cat "$offline_log"
    fail "cargo build --offline failed. Populate the Cargo cache from Cargo.lock or configure a vendored Cargo source before running the M25 gate offline."
fi

run "$ROOT_DIR/target/debug/vertexctl" validate "$ROOT_DIR/examples/hello-generation.vertex.json"

run make -C "$KRUST_DIR" doctor
run make -C "$KRUST_DIR" clean
run make -C "$KRUST_DIR" smoke

for case_name in $KRUST_CASES; do
    run "$ROOT_DIR/scripts/krust-test.sh" "$case_name"
done

echo
echo "Krust release gate ok: clean-clone M14-M29 proof is repeatable."
