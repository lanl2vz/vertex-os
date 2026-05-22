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
KrustBoot boot modules: 1
KrustBoot processes: 1
KrustBoot endpoints: 1
KrustBoot grants: 1
boot_module[0] name=vertex-init string=vertex-init
process[0] name=vertex-init module=vertex-init initial=yes
endpoint[0] name=serial-log
grant[0] process=vertex-init cap[1] endpoint=serial-log rights=send
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
IDT initialized: #UD #GP #PF
Process table entries: 1
Endpoint table entries: 1
endpoint[0] id=1 name=serial-log
process[0] id=1 name=vertex-init state=running
proc=vertex-init cap[0] boot-module=krustboot-manifest rights=read
proc=vertex-init cap[1] endpoint=1 rights=send
proc=vertex-init cap[2] process-control=process-control rights=control
Entering userspace process: vertex-init
vertex-init started
Boot module read accepted: proc=vertex-init module=krustboot-manifest bytes=
vertex-init received cap[0]=manifest-read
vertex-init received cap[1]=serial-log
vertex-init received cap[2]=process-control
vertex-init manifest generation: gen:hello-0001
vertex-init boot modules: 1
vertex-init processes: 1
vertex-init endpoints: 1
vertex-init grants: 1
Krust process authority accepted: proc=vertex-init generation=gen:hello-0001
Krust native generation activation ok
vertex-init activated generation: gen:hello-0001
Native vertex-init boot ok
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
        echo "smoke ok: Krust Kernel booted, loaded native vertex-init, enforced boot caps, and activated the compact generation"
        exit 0
    fi

    sleep 1
done

cleanup
pid=
echo "smoke failed: serial output did not contain the full M12 native vertex-init boot transcript"
if [ -f "$SERIAL_LOG" ]; then
    cat "$SERIAL_LOG"
fi
exit 1
