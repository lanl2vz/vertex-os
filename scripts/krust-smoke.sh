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
KrustBoot processes: 2
KrustBoot endpoints: 1
KrustBoot grants: 2
grant[0] process=ipc-sender cap[0] endpoint=demo-ipc rights=send
grant[1] process=ipc-receiver cap[0] endpoint=demo-ipc rights=receive
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
IDT initialized: #UD #GP #PF
Process table entries: 2
Endpoint table entries: 1
endpoint[0] id=1 name=demo-ipc
process[0] id=1 name=ipc-sender state=ready
process[1] id=2 name=ipc-receiver state=running
proc=ipc-sender cap[0] endpoint=1 rights=send
proc=ipc-receiver cap[0] endpoint=1 rights=receive
IPC receive blocked: proc=ipc-receiver endpoint=1
Scheduler switch: from=ipc-receiver to=ipc-sender
Scheduler switch: from=ipc-sender to=ipc-receiver
IPC wake receiver: proc=ipc-receiver endpoint=1
Bad pointer test: SYS_WRITE_SERIAL returned STATUS_BAD_BUFFER
Bad pointer test: SYS_IPC_SEND returned STATUS_BAD_BUFFER
Bad pointer test: SYS_IPC_RECV returned STATUS_BAD_BUFFER
IPC send accepted: endpoint=1 bytes=14
IPC receive delivered: endpoint=1 bytes=14
IPC negative test: ipc-sender receive rejected: bad capability
IPC negative test: ipc-receiver send rejected: bad capability
Krust IPC ping
IPC demo ok
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
        echo "smoke ok: Krust Kernel booted, parsed KrustBoot manifest, installed IDT handlers, rejected bad user buffers, enforced process capability tables, blocked IPC receive, woke receiver, and ran userspace IPC"
        exit 0
    fi

    sleep 1
done

cleanup
pid=
echo "smoke failed: serial output did not contain the full M11 boot, scheduler, IPC, and safety transcript"
if [ -f "$SERIAL_LOG" ]; then
    cat "$SERIAL_LOG"
fi
exit 1
