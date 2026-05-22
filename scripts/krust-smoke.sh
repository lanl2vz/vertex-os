#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd "$(dirname "$0")/.." && pwd)
KRUST_DIR=${KRUST_DIR:-"$ROOT_DIR/kernel/krust"}
BUILD_DIR=${BUILD_DIR:-"$KRUST_DIR/build"}
ISO_IMAGE=${ISO_IMAGE:-"$BUILD_DIR/krust.iso"}
SERIAL_LOG=${SERIAL_LOG:-"$BUILD_DIR/serial.log"}
QEMU=${QEMU:-qemu-system-x86_64}
QEMU_EXTRA=${QEMU_EXTRA:-}
QEMU_ATTEMPTS=${QEMU_ATTEMPTS:-20}
QEMU_POLL_SECONDS=${QEMU_POLL_SECONDS:-1}
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
KrustBoot Manifest v1 records: 9
KrustBoot boot modules: 9
KrustBoot processes: 9
KrustBoot endpoints: 3
KrustBoot grants: 18
KrustBoot store objects: 1
KrustBoot state volumes: 1
KrustBoot network ports: 1
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
process[3] name=echo module=echo initial=no service=svc:echo-server restart=2
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
grant[13] process=echo cap[3] network-port=cap:net.tcp.8080 rights=listen
grant[14] process=model-reader cap[0] store-object=store:hello-text rights=read
grant[15] process=counter-service cap[0] state-volume=state:counter rights=write
grant[16] process=reader-service cap[0] state-volume=state:counter rights=read|snapshot|restore
grant[17] process=timer-service cap[0] timer=monotonic-timer rights=control
store_object[0] id=store:hello-text module=store-hello-text
state_volume[0] id=state:counter
network_port[0] id=cap:net.tcp.8080
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
Kernel heap arena allocation ok
Typed endpoint arena created 32 endpoints
Typed process arena created 32 processes
Typed arena free and reuse ok
Typed arena allocation failure returned controlled error
Typed object arenas no silent overwrite ok
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
proc=vertex-init cap[2] process-control=process-control rights=control|allocate|delegate|revoke
proc=vertex-init cap[3] endpoint=readiness rights=receive
proc=vertex-init cap[4] endpoint=log-sink rights=send|receive
proc=echo cap[3] network-port=cap:net.tcp.8080 rights=listen
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
vertex-init grants: 18
vertex-init network ports: 1
vertex-init store objects: 1
vertex-init state volumes: 1
service with quota=1 endpoint can create one endpoint
second endpoint creation fails
init can delegate smaller quota
delegated quota cannot exceed parent quota
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
vertex-init derives endpoint cap for echo from endpoint[2] rights=send
Capability derive accepted: proc=vertex-init parent=4 new=31 rights=send
Capability inspect: proc=vertex-init
Capability transfer accepted: proc=vertex-init target=echo slot=0 rights=send
vertex-init starting service: echo
Krust process start accepted: proc=vertex-init target=echo
echo sent message to logd
service with no allocation authority cannot create endpoint
Capability inspect: proc=echo
cap inspect shows parent chain
Capability copy accepted: proc=echo
cap copy preserves source slot
Capability move accepted: proc=echo
cap move removes source slot
Capability revoke accepted: proc=echo
echo send after revoke rejected
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
Timer sleep blocked: proc=timer-service
Timer wake: proc=timer-service
wakes
timer ok
Native timer ok
vertex-init observes exit
restart policy = always
vertex-init restarts echo once
Krust process restart reload: proc=echo
echo restart retained delegated log cap
flaky-service exits with status 1
vertex-init observes failure
restart policy = on-failure
vertex-init restarts flaky-service once
Krust process restart reload: proc=flaky-service
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

attempt=1
while [ "$attempt" -le "$QEMU_ATTEMPTS" ]; do
    if check_transcript; then
        cleanup
        pid=
        echo "smoke ok: Krust completed manifest v1, typed arenas, cap lifecycle, quotas, service-local store/state/timer access, restart, and native service activation"
        exit 0
    fi

    sleep "$QEMU_POLL_SECONDS"
    attempt=$((attempt + 1))
done

cleanup
pid=
echo "smoke failed: serial output did not contain the full M14-M29 native activation transcript after $QEMU_ATTEMPTS checks"
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
