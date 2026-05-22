#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd "$(dirname "$0")/.." && pwd)
KRUST_DIR=${KRUST_DIR:-"$ROOT_DIR/kernel/krust"}
BUILD_DIR=${BUILD_DIR:-"$KRUST_DIR/build"}
ISO_IMAGE=${ISO_IMAGE:-"$BUILD_DIR/krust.iso"}
SERIAL_LOG=${SERIAL_LOG:-"$BUILD_DIR/serial.log"}
QEMU=${QEMU:-qemu-system-x86_64}
QEMU_EXTRA=${QEMU_EXTRA:-}
SKIP_BUILD=0

if [ "${1:-}" = "--no-build" ]; then
    SKIP_BUILD=1
fi

if [ "$SKIP_BUILD" -eq 0 ]; then
    (cd "$KRUST_DIR" && make iso)
fi

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

# QEMU_EXTRA is intentionally word-split so callers can pass flags like
# QEMU_EXTRA="-enable-kvm -cpu host".
"$QEMU" $QEMU_EXTRA \
    -m 256M \
    -serial "file:$SERIAL_LOG" \
    -monitor none \
    -display none \
    -no-reboot \
    -no-shutdown \
    -cdrom "$ISO_IMAGE" &
pid=$!

required_lines='
Krust Kernel booted
Limine memory map entries:
KrustBoot manifest generation: gen:hello-0001
KrustBoot boot modules: 3
KrustBoot processes: 3
KrustBoot endpoints: 2
KrustBoot grants: 5
boot_module[0] name=vertex-init string=vertex-init
boot_module[1] name=logd string=logd
boot_module[2] name=echo string=echo
process[0] name=vertex-init module=vertex-init initial=yes
process[1] name=logd module=logd initial=no
process[2] name=echo module=echo initial=no
endpoint[0] name=serial-log
endpoint[1] name=log-sink
grant[0] process=vertex-init cap[1] endpoint=serial-log rights=send
grant[1] process=logd cap[0] endpoint=log-sink rights=receive
grant[3] process=echo cap[0] endpoint=log-sink rights=send
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
IDT initialized: #UD #GP #PF
Process table entries: 3
Endpoint table entries: 2
endpoint[0] id=1 name=serial-log
endpoint[1] id=2 name=log-sink
process[0] id=1 name=vertex-init state=running
process[1] id=2 name=logd state=declared
process[2] id=3 name=echo state=declared
proc=vertex-init cap[0] boot-module=krustboot-manifest rights=read
proc=vertex-init cap[1] endpoint=serial-log rights=send
proc=vertex-init cap[2] process-control=process-control rights=control
proc=logd cap[0] endpoint=log-sink rights=receive
proc=echo cap[0] endpoint=log-sink rights=send
Entering userspace process: vertex-init
vertex-init started
Boot module read accepted: proc=vertex-init module=krustboot-manifest bytes=
vertex-init received cap[0]=manifest-read
vertex-init received cap[1]=serial-log
vertex-init received cap[2]=process-control
vertex-init manifest generation: gen:hello-0001
vertex-init boot modules: 3
vertex-init processes: 3
vertex-init endpoints: 2
vertex-init grants: 5
vertex-init starting service: logd
Krust process start accepted: proc=vertex-init target=logd
vertex-init starting service: echo
Krust process start accepted: proc=vertex-init target=echo
echo sent message to logd
logd received: hello from echo
negative test: echo receive rejected: bad capability
negative test: logd process-start rejected: bad capability
Native service activation ok
'

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
        echo "smoke ok: Krust booted native vertex-init, started declared services, enforced IPC caps, and completed native service activation"
        exit 0
    fi

    sleep 1
done

cleanup
pid=
echo "smoke failed: serial output did not contain the full M13 native service activation transcript"
if [ -f "$SERIAL_LOG" ]; then
    cat "$SERIAL_LOG"
fi
exit 1
