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
KrustBoot boot modules: 13
KrustBoot processes: 13
KrustBoot endpoints: 7
KrustBoot grants: 32
KrustBoot store objects: 0
KrustBoot state volumes: 1
KrustBoot network ports: 1
KrustBoot io port ranges: 1
KrustBoot mmio regions: 1
KrustBoot interrupt lines: 1
KrustBoot dma regions: 1
boot_module[0] name=vertex-init string=vertex-init
boot_module[1] name=serial-driver string=serial-driver
boot_module[2] name=logd string=logd
boot_module[3] name=netstack string=netstack
boot_module[4] name=block-driver string=block-driver
boot_module[5] name=vertex-store string=vertex-store
boot_module[6] name=vertex-state string=vertex-state
boot_module[7] name=echo string=echo
boot_module[8] name=model-reader string=model-reader
boot_module[9] name=counter-service string=counter
boot_module[10] name=reader-service string=state-reader
boot_module[11] name=timer-service string=timer
boot_module[12] name=flaky-service string=flaky
process[0] name=vertex-init module=vertex-init initial=yes
process[1] name=serial-driver module=serial-driver initial=no service=svc:serial-driver restart=0
process[2] name=logd module=logd initial=no service=svc:logd restart=1 health=ipc-ping
process[3] name=netstack module=netstack initial=no service=svc:netstack restart=1
process[4] name=block-driver module=block-driver initial=no service=svc:block-driver restart=0
process[5] name=vertex-store module=vertex-store initial=no service=svc:vertex-store restart=0
process[6] name=vertex-state module=vertex-state initial=no service=svc:vertex-state restart=0
process[7] name=echo module=echo initial=no service=svc:echo-server restart=2
process[8] name=model-reader module=model-reader initial=no service=svc:model-reader restart=0
process[9] name=counter-service module=counter initial=no service=svc:counter-service restart=0
process[10] name=reader-service module=state-reader initial=no service=svc:state-reader restart=0
process[11] name=timer-service module=timer initial=no service=svc:timer-service restart=0
process[12] name=flaky-service module=flaky initial=no service=svc:flaky-service restart=1
endpoint[0] name=serial-log
endpoint[1] name=readiness
endpoint[2] name=serial-console
endpoint[3] name=log-sink
endpoint[4] name=block-io
endpoint[5] name=store-hello-text-api
endpoint[6] name=state-counter-api
grant[0] process=vertex-init cap[1] endpoint=serial-log rights=send
grant[1] process=vertex-init cap[3] endpoint=readiness rights=receive
grant[15] process=serial-driver cap[0] endpoint=serial-console rights=send|receive
grant[19] process=block-driver cap[0] endpoint=block-io rights=send|receive
grant[21] process=vertex-store cap[0] endpoint=store-hello-text-api rights=send|receive
grant[23] process=vertex-state cap[0] endpoint=state-counter-api rights=send|receive
grant[25] process=serial-driver cap[3] io-port=cap:io.com1 rights=read|write
grant[26] process=block-driver cap[3] mmio-region=cap:mmio.virtio-blk0 rights=map
grant[27] process=block-driver cap[4] interrupt-line=cap:irq.virtio-blk0 rights=listen
grant[28] process=block-driver cap[5] dma-region=cap:dma.virtio-blk0 rights=read|write|map
grant[29] process=vertex-state cap[3] state-volume=state:counter rights=read|write|snapshot|restore
grant[30] process=echo cap[3] network-port=cap:net.tcp.8080 rights=listen
grant[31] process=timer-service cap[0] timer=monotonic-timer rights=control
state_volume[0] id=state:counter
network_port[0] id=cap:net.tcp.8080
io_port[0] id=cap:io.com1 base=0x00000000000003f8 length=0x0000000000000008
mmio_region[0] id=cap:mmio.virtio-blk0 base=0x0000000010001000 length=0x0000000000001000
interrupt_line[0] id=cap:irq.virtio-blk0 line=5
dma_region[0] id=cap:dma.virtio-blk0 base=0x0000000000000000 length=0x0000000000001000
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
Process table entries: 13
Endpoint table entries: 7
endpoint[0] id=1 name=serial-log
endpoint[1] id=2 name=readiness
endpoint[2] id=3 name=serial-console
endpoint[3] id=4 name=log-sink
endpoint[4] id=5 name=block-io
endpoint[5] id=6 name=store-hello-text-api
endpoint[6] id=7 name=state-counter-api
process[0] id=1 name=vertex-init state=running
process[1] id=2 name=serial-driver state=declared
process[2] id=3 name=logd state=declared
process[3] id=4 name=netstack state=declared
process[4] id=5 name=block-driver state=declared
process[5] id=6 name=vertex-store state=declared
process[6] id=7 name=vertex-state state=declared
process[7] id=8 name=echo state=declared
process[8] id=9 name=model-reader state=declared
process[9] id=10 name=counter-service state=declared
process[10] id=11 name=reader-service state=declared
process[11] id=12 name=timer-service state=declared
process[12] id=13 name=flaky-service state=declared
proc=vertex-init cap[0] boot-module=krustboot-manifest rights=read
proc=vertex-init cap[1] endpoint=serial-log rights=send
proc=vertex-init cap[2] process-control=process-control rights=control|allocate|delegate|revoke
proc=vertex-init cap[3] endpoint=readiness rights=receive
proc=vertex-init cap[4] endpoint=serial-console rights=send|receive
proc=vertex-init cap[5] endpoint=log-sink rights=send|receive
proc=vertex-init cap[6] endpoint=block-io rights=send|receive
proc=vertex-init cap[7] endpoint=store-hello-text-api rights=send|receive
proc=vertex-init cap[8] endpoint=state-counter-api rights=send|receive
proc=serial-driver cap[3] io-port=cap:io.com1 rights=read|write
proc=block-driver cap[3] mmio-region=cap:mmio.virtio-blk0 rights=map
proc=block-driver cap[4] interrupt-line=cap:irq.virtio-blk0 rights=listen
proc=block-driver cap[5] dma-region=cap:dma.virtio-blk0 base=0x0000000000000000 length=0x0000000000001000 rights=read|write|map
proc=vertex-store cap[0] endpoint=store-hello-text-api rights=send|receive
proc=vertex-state cap[3] state-volume=state:counter rights=read|write|snapshot|restore
proc=echo cap[3] network-port=cap:net.tcp.8080 rights=listen
proc=netstack cap[1] endpoint=serial-log rights=send
proc=echo cap[1] endpoint=serial-log rights=send
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
vertex-init boot modules: 13
vertex-init processes: 13
vertex-init endpoints: 7
vertex-init grants: 32
vertex-init network ports: 1
vertex-init store objects: 0
vertex-init state volumes: 1
vertex-init io ports: 1
vertex-init mmio regions: 1
vertex-init interrupt lines: 1
vertex-init dma regions: 1
service with quota=1 endpoint can create one endpoint
second endpoint creation fails
init can delegate smaller quota
delegated quota cannot exceed parent quota
vertex-init activation plan:
  1. serial-driver
  2. logd
  3. netstack
  4. block-driver
  5. vertex-store
  6. vertex-state
  7. echo
  8. model-reader
  9. counter-service
  10. reader-service
  11. timer-service
  12. flaky-service
vertex-init starting service: serial-driver
Krust process start accepted: proc=vertex-init target=serial-driver
vertex-init starting service: logd
Krust process start accepted: proc=vertex-init target=logd
logd ready
vertex-init observed ready: logd
vertex-init starting service: netstack
Krust process start accepted: proc=vertex-init target=netstack
vertex-init starting service: block-driver
Krust process start accepted: proc=vertex-init target=block-driver
vertex-init starting service: vertex-store
Krust process start accepted: proc=vertex-init target=vertex-store
vertex-init starting service: vertex-state
Krust process start accepted: proc=vertex-init target=vertex-state
vertex-init derives endpoint cap for echo from endpoint[3] rights=send
Capability inspect: proc=vertex-init
Capability transfer accepted: proc=vertex-init target=echo slot=0 rights=send
vertex-init starting service: echo
Krust process start accepted: proc=vertex-init target=echo
serial-driver ready
serial-driver has COM1 I/O port capability
serial-driver can write byte
logd sends log message
serial-driver writes message to COM1
echo sent message to logd
service with no allocation authority cannot create endpoint
echo I/O write rejected
echo cannot write COM1 directly
logd cannot write COM1 directly
unauthorized service cannot talk to block-driver
unauthorized service cannot access MMIO, IRQ, or DMA capabilities
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
block-driver ready
MMIO map accepted: proc=block-driver mmio-region=cap:mmio.virtio-blk0
IRQ wait accepted: proc=block-driver interrupt-line=cap:irq.virtio-blk0
block-driver DMA is distinct from MMIO authority
vertex-store ready
vertex-state ready
model-reader asks for store:hello-text
store-service requests block read
block-driver received block-read request
block-driver returns bytes
vertex-store verifies hash
modified object fails hash check
model-reader reads bytes
model-reader reads bytes successfully
Native immutable store client ok
counter-service has state API cap
counter-service sends state write
State write accepted: proc=vertex-state state=state:counter
counter-service writes state
reader-service has state API cap
State read accepted: proc=vertex-state state=state:counter
snapshot created
reader-service reads state
reader-service receives state value
reader-service write denied
reader-service write rejected
state restored
system generation rollback does not automatically roll back state unless policy says so
Native immutable store service ok
Native state-volume service ok
Native state service client ok
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
echo "smoke failed: serial output did not contain the full M14-M36 native activation transcript after $QEMU_ATTEMPTS checks"
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
