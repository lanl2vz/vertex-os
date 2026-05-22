#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd "$(dirname "$0")/.." && pwd)
KRUST_DIR=${KRUST_DIR:-"$ROOT_DIR/kernel/krust"}
BUILD_DIR=${BUILD_DIR:-"$KRUST_DIR/build"}
ISO_IMAGE=${ISO_IMAGE:-"$BUILD_DIR/krust.iso"}
SERIAL_LOG=${SERIAL_LOG:-"$BUILD_DIR/serial-test.log"}
QEMU=${QEMU:-qemu-system-x86_64}
QEMU_EXTRA=${QEMU_EXTRA:-}
CASE=${1:-m14}
FALLBACK_MANIFEST=

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
    readiness-timeout|readiness)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        required_lines='
logd ready
vertex-init observed ready: logd
Native readiness activation ok
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
    restart)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        required_lines='
flaky-service exits with status 1
vertex-init observes failure
restart policy = on-failure
vertex-init restarts flaky-service once
flaky-service exits 0
Native restart policy ok
'
        ;;
    *)
        echo "usage: scripts/krust-test.sh <m13|m14|valid-activation|manifest-cycle|bad-cap|readiness|readiness-timeout|rollback|store-state|timer|restart>" >&2
        exit 2
        ;;
esac

(cd "$KRUST_DIR" && make iso VERTEX_MANIFEST="$MANIFEST" FALLBACK_MANIFEST="$FALLBACK_MANIFEST")

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

for _ in 1 2 3 4 5 6 7 8; do
    missing=0
    while IFS= read -r line; do
        if [ -z "$line" ]; then
            continue
        fi
        if ! grep -Fq "$line" "$SERIAL_LOG" 2>/dev/null; then
            missing=1
            break
        fi
    done <<EOF
$required_lines
EOF

    if [ "$missing" -eq 0 ]; then
        cleanup
        pid=
        echo "krust test ok: $CASE"
        exit 0
    fi

    sleep 1
done

cleanup
pid=
echo "krust test failed: $CASE"
if [ -f "$SERIAL_LOG" ]; then
    cat "$SERIAL_LOG"
fi
exit 1
