#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd "$(dirname "$0")/.." && pwd)
KRUST_DIR=${KRUST_DIR:-"$ROOT_DIR/kernel/krust"}
BUILD_DIR=${BUILD_DIR:-"$KRUST_DIR/build"}
ISO_IMAGE=${ISO_IMAGE:-"$BUILD_DIR/krust.iso"}
SERIAL_LOG=${SERIAL_LOG:-"$BUILD_DIR/serial-test.log"}
QEMU=${QEMU:-qemu-system-x86_64}
QEMU_EXTRA=${QEMU_EXTRA:-}
QEMU_ATTEMPTS=${QEMU_ATTEMPTS:-20}
QEMU_POLL_SECONDS=${QEMU_POLL_SECONDS:-1}
CASE=${1:-m14}
FALLBACK_MANIFEST=
KRUSTBOOT_CORRUPT=

case "$CASE" in
    m13|m14|valid-activation)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        required_lines='
Native manifest-driven activation ok
Native readiness activation ok
Native service activation ok
'
        ;;
    manifest-cycle)
        MANIFEST="$ROOT_DIR/examples/krust-cycle-generation.vertex.json"
        required_lines='
vertex-init activation failed: dependency cycle
activation failed
'
        ;;
    bad-cap)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        required_lines='
negative test: echo receive rejected: bad capability
echo read rejected: bad capability
echo send after drop rejected
negative test: logd process-start rejected: bad capability
reader-service write rejected
'
        ;;
    readiness)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        required_lines='
logd ready
vertex-init observed ready: logd
Native readiness activation ok
'
        ;;
    readiness-timeout)
        MANIFEST="$ROOT_DIR/examples/krust-readiness-timeout.vertex.json"
        required_lines='
vertex-init readiness timeout
activation failed
Native service activation failed
'
        ;;
    rollback)
        MANIFEST="$ROOT_DIR/examples/krust-rollback-bad-generation.vertex.json"
        FALLBACK_MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        required_lines='
Boot generation: gen:bad-0002
activation failed
falling back to generation: gen:hello-0001
Krust rollback generation accepted: target=gen:hello-0001
Boot generation: gen:hello-0001
Native service activation ok
'
        ;;
    store-state)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        required_lines='
model-reader reads bytes successfully
Native store-object read ok
reader-service write rejected
Native state-volume access ok
'
        ;;
    timer)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        required_lines='
timer-service sleeps 10 ms
wakes
timer ok
Native timer ok
'
        ;;
    preemption|m30)
        MANIFEST="$ROOT_DIR/examples/krust-preemption-generation.vertex.json"
        required_lines='
PIT timer interrupt initialized: vector=32 hz=100
Timer tick increments: ticks=1
Preemption disabled in kernel critical sections
cpu-hog starts without yielding
Scheduler preempted process without explicit yield: from=cpu-hog
logd received: hello from echo
'
        ;;
    user-fault|m31)
        MANIFEST="$ROOT_DIR/examples/krust-user-fault-generation.vertex.json"
        required_lines='
faulty-service triggers direct invalid load
User page fault: proc=faulty-service
User process fault contained: proc=faulty-service
vertex-init observes failure
restart policy = on-failure
vertex-init restarts faulty-service once
Krust process restart reload: proc=faulty-service
faulty-service exits 0 after restart
Native service activation ok
'
        ;;
    restart)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        required_lines='
flaky-service exits with status 1
vertex-init observes failure
restart policy = on-failure
vertex-init restarts flaky-service once
Krust process restart reload: proc=flaky-service
flaky-service exits 0
restart policy = always
vertex-init restarts echo once
Krust process restart reload: proc=echo
echo restart retained delegated log cap
Native restart policy ok
'
        ;;
    manifest-v1)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        required_lines='
KrustBoot Manifest v1 records: 9
proc=vertex-init cap[0] boot-module=krustboot-manifest rights=read
Boot module read accepted: proc=vertex-init module=krustboot-manifest bytes=
Native manifest-driven activation ok
'
        ;;
    cap-lifecycle)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        required_lines='
Capability inspect: proc=vertex-init
Capability inspect: proc=echo
cap inspect shows parent chain
Capability copy accepted: proc=echo
cap copy preserves source slot
Capability move accepted: proc=echo
cap move removes source slot
Capability revoke accepted: proc=echo
echo send after revoke rejected
'
        ;;
    typed-arenas)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        required_lines='
Kernel heap arena allocation ok
Typed endpoint arena created 32 endpoints
Typed process arena created 32 processes
Typed arena free and reuse ok
Typed arena allocation failure returned controlled error
Typed object arenas no silent overwrite ok
'
        ;;
    quotas)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        required_lines='
proc=vertex-init cap[2] process-control=process-control rights=control|allocate|delegate|revoke
service with quota=1 endpoint can create one endpoint
second endpoint creation fails
init can delegate smaller quota
delegated quota cannot exceed parent quota
service with no allocation authority cannot create endpoint
'
        ;;
    manifest-truncated)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        KRUSTBOOT_CORRUPT=truncated
        required_lines='
KrustBoot manifest parse failed: truncated
KrustBoot manifest unavailable
'
        ;;
    manifest-bad-magic)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        KRUSTBOOT_CORRUPT=bad-magic
        required_lines='
KrustBoot manifest parse failed: bad magic
KrustBoot manifest unavailable
'
        ;;
    manifest-raw-compact)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        KRUSTBOOT_CORRUPT=raw-compact
        required_lines='
KrustBoot manifest parse failed: bad magic
KrustBoot manifest unavailable
'
        ;;
    manifest-unsupported-version)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        KRUSTBOOT_CORRUPT=unsupported-version
        required_lines='
KrustBoot manifest parse failed: unsupported version
KrustBoot manifest unavailable
'
        ;;
    manifest-oob-record)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        KRUSTBOOT_CORRUPT=out-of-bounds-record
        required_lines='
KrustBoot manifest parse failed: out-of-bounds record
KrustBoot manifest unavailable
'
        ;;
    manifest-missing-provider)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        KRUSTBOOT_CORRUPT=missing-provider
        required_lines='
vertex-init activation failed: missing provider
activation failed
'
        ;;
    *)
        echo "usage: scripts/krust-test.sh <m13|m14|valid-activation|manifest-cycle|bad-cap|readiness|readiness-timeout|rollback|store-state|timer|preemption|m30|user-fault|m31|restart|manifest-v1|cap-lifecycle|typed-arenas|quotas|manifest-truncated|manifest-bad-magic|manifest-raw-compact|manifest-unsupported-version|manifest-oob-record|manifest-missing-provider>" >&2
        exit 2
        ;;
esac

(cd "$KRUST_DIR" && make iso VERTEX_MANIFEST="$MANIFEST" FALLBACK_MANIFEST="$FALLBACK_MANIFEST" KRUSTBOOT_CORRUPT="$KRUSTBOOT_CORRUPT")

mkdir -p "$(dirname "$SERIAL_LOG")"
rm -f "$SERIAL_LOG"

pid=
cleanup() {
    if [ -n "$pid" ]; then
        kill "$pid" >/dev/null 2>&1 || true
        wait "$pid" >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT INT TERM

"$QEMU" $QEMU_EXTRA \
    -m 256M \
    -serial "file:$SERIAL_LOG" \
    -monitor none \
    -display none \
    -no-reboot \
    -no-shutdown \
    -cdrom "$ISO_IMAGE" &
pid=$!

missing_required=

check_transcript() {
    missing_required=

    while IFS= read -r line; do
        if [ -z "$line" ]; then
            continue
        fi
        if ! grep -Fq "$line" "$SERIAL_LOG" 2>/dev/null; then
            missing_required="${missing_required}${line}
"
        fi
    done <<EOF
$required_lines
EOF

    [ -z "$missing_required" ]
}

print_lines() {
    while IFS= read -r line; do
        if [ -n "$line" ]; then
            echo "  - $line"
        fi
    done
}

attempt=1
while [ "$attempt" -le "$QEMU_ATTEMPTS" ]; do
    if check_transcript; then
        cleanup
        pid=
        echo "krust test ok: $CASE"
        exit 0
    fi

    sleep "$QEMU_POLL_SECONDS"
    attempt=$((attempt + 1))
done

cleanup
pid=
echo "krust test failed: $CASE after $QEMU_ATTEMPTS checks"
echo "serial log: $SERIAL_LOG"
if [ -n "$missing_required" ]; then
    echo "missing required transcript lines:"
    printf '%s' "$missing_required" | print_lines
fi
if [ -f "$SERIAL_LOG" ]; then
    cat "$SERIAL_LOG"
fi
exit 1
