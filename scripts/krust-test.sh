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
QEMU_STABILITY_ATTEMPTS=${QEMU_STABILITY_ATTEMPTS:-1}
QEMU_PREEMPTION_STABILITY_ATTEMPTS=${QEMU_PREEMPTION_STABILITY_ATTEMPTS:-3}
CASE=${1:-m14}
FALLBACK_MANIFEST=
KRUSTBOOT_CORRUPT=
EXPECT_ACTIVATION_SUCCESS=0
SUCCESS_STABILITY_ATTEMPTS=$QEMU_STABILITY_ATTEMPTS

case "$CASE" in
    m13|m14|valid-activation)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
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
        EXPECT_ACTIVATION_SUCCESS=1
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
        EXPECT_ACTIVATION_SUCCESS=1
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
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
Boot generation: gen:bad-0002
activation failed
falling back to generation: gen:hello-0001
Krust rollback generation accepted: target=gen:hello-0001
Boot generation: gen:hello-0001
Native service activation ok
'
        ;;
    store-state-services)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
model-reader reads bytes successfully
Native immutable store client ok
reader-service write rejected
Native state service client ok
'
        ;;
    timer)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
timer-service sleeps 10 ms
wakes
timer ok
Native timer ok
'
        ;;
    preemption|m30)
        MANIFEST="$ROOT_DIR/examples/krust-preemption-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        SUCCESS_STABILITY_ATTEMPTS=$QEMU_PREEMPTION_STABILITY_ATTEMPTS
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
        EXPECT_ACTIVATION_SUCCESS=1
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
        EXPECT_ACTIVATION_SUCCESS=1
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
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
KrustBoot Manifest v1 records: 9
proc=vertex-init cap[0] boot-module=krustboot-manifest rights=read
Boot module read accepted: proc=vertex-init module=krustboot-manifest bytes=
Native manifest-driven activation ok
'
        ;;
    cap-lifecycle)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
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
        EXPECT_ACTIVATION_SUCCESS=1
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
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
proc=vertex-init cap[2] process-control=process-control rights=control|allocate|delegate|revoke|inspect
service with quota=1 endpoint can create one endpoint
second endpoint creation fails
init can delegate smaller quota
delegated quota cannot exceed parent quota
service with no allocation authority cannot create endpoint
'
        ;;
    m32|io-substrate)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
proc=serial-driver cap[3] io-port=cap:io.com1 rights=read|write
serial-driver has COM1 I/O port capability
serial-driver can write byte
echo I/O write rejected
'
        ;;
    m33|serial-driver)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
serial-driver ready
logd sends log message
serial-driver writes message to COM1
logd cannot write COM1 directly
echo cannot write COM1 directly
Krust Kernel booted
'
        ;;
    m34|block-driver)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
block-driver ready
store-service requests block read
block-driver returns bytes
unauthorized service cannot talk to block-driver
unauthorized service cannot access MMIO, IRQ, or DMA capabilities
'
        ;;
    m35|store-service)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
model-reader asks for store:hello-text
vertex-store verifies hash
model-reader reads bytes
modified object fails hash check
unauthorized process cannot read object
'
        ;;
    m36|state-service)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
counter-service writes state
reader-service reads state
reader-service write denied
snapshot created
state restored
system generation rollback does not automatically roll back state unless policy says so
'
        ;;
    m37|generation-switch)
        MANIFEST="$ROOT_DIR/examples/krust-switch-a-generation.vertex.json"
        FALLBACK_MANIFEST="$ROOT_DIR/examples/krust-switch-b-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
Boot generation: gen:switch-a-0001
vertex-store exposes generation B manifest
vertex-init validates generation B
Krust generation switch accepted: from=gen:switch-a-0001 to=gen:switch-b-0002
Krust generation switch revoked old generation authority: generation=gen:switch-a-0001
old generation service loses old capability
Krust generation switch entering generation: gen:switch-b-0002
Boot generation: gen:switch-b-0002
service from B runs
vertex-init validates generation C
Krust generation switch rejected: requested=gen:switch-c-bad-0003
bad generation C fails
rollback to B
Native service activation ok
'
        ;;
    m38|introspection)
        MANIFEST="$ROOT_DIR/examples/krust-inspect-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
Boot generation: gen:inspect-0001
vertex-init delegates inspect authority to vertex-inspect
Runtime inspect accepted: proc=vertex-inspect
vertex-inspect started
vertex-inspect generation graph: gen:inspect-0001
native why echo log-sink
why: echo can send to log-sink because delegated endpoint authority has send rights
native who-can state:counter
who-can: vertex-state owns state:counter with rights=read|write|snapshot|restore
native cap provenance report
cap provenance: echo log-sink cap is derived from vertex-init endpoint authority
Native introspection service ok
Native service activation ok
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
        echo "usage: scripts/krust-test.sh <m13|m14|valid-activation|manifest-cycle|bad-cap|readiness|readiness-timeout|rollback|store-state-services|timer|preemption|m30|user-fault|m31|restart|manifest-v1|cap-lifecycle|typed-arenas|quotas|m32|io-substrate|m33|serial-driver|m34|block-driver|m35|store-service|m36|state-service|m37|generation-switch|m38|introspection|manifest-truncated|manifest-bad-magic|manifest-raw-compact|manifest-unsupported-version|manifest-oob-record|manifest-missing-provider>" >&2
        exit 2
        ;;
esac

forbidden_lines='
Krust exception
'

if [ "$EXPECT_ACTIVATION_SUCCESS" -eq 1 ]; then
    forbidden_lines="${forbidden_lines}
Native service activation failed
"
fi

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
present_forbidden=

check_transcript() {
    missing_required=
    present_forbidden=

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

    while IFS= read -r line; do
        if [ -z "$line" ]; then
            continue
        fi
        if grep -Fq "$line" "$SERIAL_LOG" 2>/dev/null; then
            present_forbidden="${present_forbidden}${line}
"
        fi
    done <<EOF
$forbidden_lines
EOF

    [ -z "$missing_required" ] && [ -z "$present_forbidden" ]
}

print_lines() {
    while IFS= read -r line; do
        if [ -n "$line" ]; then
            echo "  - $line"
        fi
    done
}

wait_for_stability() {
    stable_attempt=0
    while [ "$stable_attempt" -lt "$SUCCESS_STABILITY_ATTEMPTS" ]; do
        sleep "$QEMU_POLL_SECONDS"
        if ! check_transcript; then
            return 1
        fi
        stable_attempt=$((stable_attempt + 1))
    done
    return 0
}

attempt=1
while [ "$attempt" -le "$QEMU_ATTEMPTS" ]; do
    if check_transcript; then
        if wait_for_stability; then
            cleanup
            pid=
            echo "krust test ok: $CASE"
            exit 0
        fi
    fi

    if [ -n "$present_forbidden" ]; then
        break
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
if [ -n "$present_forbidden" ]; then
    echo "forbidden transcript lines were present:"
    printf '%s' "$present_forbidden" | print_lines
fi
if [ -f "$SERIAL_LOG" ]; then
    cat "$SERIAL_LOG"
fi
exit 1
