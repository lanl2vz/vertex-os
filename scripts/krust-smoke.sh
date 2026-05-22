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
KrustBoot boot modules: 9
KrustBoot processes: 9
KrustBoot endpoints: 3
KrustBoot grants: 17
KrustBoot store objects: 1
KrustBoot state volumes: 1
boot_module[0] name=vertex-init string=vertex-init
boot_module[1] name=logd string=logd
boot_module[2] name=netstack string=netstack
boot_module[3] name=echo string=echo
boot_module[4] name=model-reader string=model-reader
boot_module[5] name=counter-service string=counter
boot_module[6] name=reader-service string=state-reader
boot_module[7] name=timer-service string=timer
boot_module[8] name=flaky-service string=flaky
process[0] name=vertex-init module=vertex-init initial=yes
process[1] name=logd module=logd initial=no service=svc:logd restart=1 health=ipc-ping
process[2] name=netstack module=netstack initial=no service=svc:netstack restart=1
process[3] name=echo module=echo initial=no service=svc:echo-server restart=1
process[4] name=model-reader module=model-reader initial=no service=svc:model-reader restart=0
process[5] name=counter-service module=counter initial=no service=svc:counter-service restart=0
process[6] name=reader-service module=state-reader initial=no service=svc:state-reader restart=0
process[7] name=timer-service module=timer initial=no service=svc:timer-service restart=0
process[8] name=flaky-service module=flaky initial=no service=svc:flaky-service restart=1
endpoint[0] name=serial-log
endpoint[1] name=readiness
endpoint[2] name=log-sink
grant[0] process=vertex-init cap[1] endpoint=serial-log rights=send
grant[1] process=vertex-init cap[3] endpoint=readiness rights=receive
grant[11] process=logd cap[0] endpoint=log-sink rights=receive
grant[12] process=vertex-init cap[4] endpoint=log-sink rights=send|receive
grant[13] process=model-reader cap[0] store-object=store:hello-text rights=read
grant[14] process=counter-service cap[0] state-volume=state:counter rights=write
grant[15] process=reader-service cap[0] state-volume=state:counter rights=read|snapshot|restore
grant[16] process=timer-service cap[0] timer=monotonic-timer rights=control
store_object[0] id=store:hello-text module=store-hello-text
state_volume[0] id=state:counter
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
IDT initialized: #UD #GP #PF
Process table entries: 9
Endpoint table entries: 3
endpoint[0] id=1 name=serial-log
endpoint[1] id=2 name=readiness
endpoint[2] id=3 name=log-sink
process[0] id=1 name=vertex-init state=running
process[1] id=2 name=logd state=declared
process[2] id=3 name=netstack state=declared
process[3] id=4 name=echo state=declared
process[4] id=5 name=model-reader state=declared
process[5] id=6 name=counter-service state=declared
process[6] id=7 name=reader-service state=declared
process[7] id=8 name=timer-service state=declared
process[8] id=9 name=flaky-service state=declared
proc=vertex-init cap[0] boot-module=krustboot-manifest rights=read
proc=vertex-init cap[1] endpoint=serial-log rights=send
proc=vertex-init cap[2] process-control=process-control rights=control
proc=vertex-init cap[3] endpoint=readiness rights=receive
proc=vertex-init cap[4] endpoint=log-sink rights=send|receive
proc=logd cap[0] endpoint=log-sink rights=receive
proc=netstack cap[1] endpoint=serial-log rights=send
proc=echo cap[1] endpoint=serial-log rights=send
proc=model-reader cap[0] store-object=store:hello-text rights=read
proc=counter-service cap[0] state-volume=state:counter rights=write
proc=reader-service cap[0] state-volume=state:counter rights=read|snapshot|restore
proc=timer-service cap[0] timer=monotonic-timer rights=control
proc=logd cap[2] endpoint=readiness rights=send
Entering userspace process: vertex-init
vertex-init started
Boot module read accepted: proc=vertex-init module=krustboot-manifest bytes=
vertex-init received cap[0]=manifest-read
vertex-init received cap[1]=serial-log
vertex-init received cap[2]=process-control
Boot generation: gen:hello-0001
vertex-init manifest generation: gen:hello-0001
vertex-init boot modules: 9
vertex-init processes: 9
vertex-init endpoints: 3
vertex-init grants: 17
vertex-init store objects: 1
vertex-init state volumes: 1
vertex-init activation plan:
  1. logd
  2. netstack
  3. echo
  4. model-reader
  5. counter-service
  6. reader-service
  7. timer-service
  8. flaky-service
vertex-init starting service: logd
Krust process start accepted: proc=vertex-init target=logd
logd ready
vertex-init observed ready: logd
vertex-init starting service: netstack
Krust process start accepted: proc=vertex-init target=netstack
vertex-init derives send-only cap for echo from stronger endpoint authority
Capability derive accepted: proc=vertex-init parent=4 new=9 rights=send
Capability transfer accepted: proc=vertex-init target=echo slot=0 rights=send
vertex-init starting service: echo
Krust process start accepted: proc=vertex-init target=echo
echo sent message to logd
logd received: hello from echo
negative test: echo receive rejected: bad capability
echo read rejected: bad capability
echo drops cap
echo send after drop rejected
negative test: logd process-start rejected: bad capability
netstack ready
Object read accepted: proc=model-reader object=store:hello-text bytes=22
model-reader has read cap to store:hello-text
model-reader reads bytes successfully
Native store-object read ok
State write accepted: proc=counter-service state=state:counter
counter-service has write cap to state:counter
counter-service writes value
State read accepted: proc=reader-service state=state:counter
reader-service has read-only cap
reader-service reads value
reader-service write rejected
Native state-volume access ok
timer-service sleeps 10 ms
Timer sleep accepted: proc=timer-service timer=monotonic-timer ms=10
wakes
timer ok
Native timer ok
flaky-service exits with status 1
vertex-init observes failure
restart policy = on-failure
vertex-init restarts flaky-service once
flaky-service exits 0
Native restart policy ok
Native manifest-driven activation ok
Native readiness activation ok
Native service activation ok
'

forbidden_lines='
proc=vertex-init cap[5] store-object=store:hello-text rights=read
proc=vertex-init cap[6] state-volume=state:counter rights=read|write
proc=vertex-init cap[8] timer=monotonic-timer rights=control
Object read accepted: proc=vertex-init
State write accepted: proc=vertex-init
Timer sleep accepted: proc=vertex-init
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
        while IFS= read -r line; do
            if [ -z "$line" ]; then
                continue
            fi
            if grep -Fq "$line" "$SERIAL_LOG" 2>/dev/null; then
                missing=1
                break
            fi
        done <<EOF
$forbidden_lines
EOF
    fi

    if [ "$missing" -eq 0 ]; then
        cleanup
        pid=
        echo "smoke ok: Krust completed manifest-driven activation, readiness, cap flow, service-local store/state/timer access, restart, and native service activation"
        exit 0
    fi

    sleep 1
done

cleanup
pid=
echo "smoke failed: serial output did not contain the full M14-M24 native activation transcript"
if [ -f "$SERIAL_LOG" ]; then
    cat "$SERIAL_LOG"
fi
exit 1
