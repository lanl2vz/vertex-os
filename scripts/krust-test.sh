#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd "$(dirname "$0")/.." && pwd)
KRUST_DIR=${KRUST_DIR:-"$ROOT_DIR/kernel/krust"}
BUILD_DIR=${BUILD_DIR:-"$KRUST_DIR/build"}
ISO_IMAGE=${ISO_IMAGE:-"$BUILD_DIR/krust.iso"}
BLOCK_IMAGE=${BLOCK_IMAGE:-"$BUILD_DIR/krust-block.img"}
SERIAL_LOG=${SERIAL_LOG:-"$BUILD_DIR/serial-test.log"}
QEMU=${QEMU:-qemu-system-x86_64}
QEMU_EXTRA=${QEMU_EXTRA:-}
QEMU_MACHINE=${QEMU_MACHINE:-}
QEMU_BLOCK=${QEMU_BLOCK:-"-drive if=none,id=vertexblk,file=$BLOCK_IMAGE,format=raw -device virtio-blk-pci,drive=vertexblk,disable-modern=on,queue-size=8"}
QEMU_ATTEMPTS=${QEMU_ATTEMPTS:-20}
QEMU_POLL_SECONDS=${QEMU_POLL_SECONDS:-1}
QEMU_STABILITY_ATTEMPTS=${QEMU_STABILITY_ATTEMPTS:-1}
QEMU_PREEMPTION_STABILITY_ATTEMPTS=${QEMU_PREEMPTION_STABILITY_ATTEMPTS:-3}
CASE=${1:-m14}
FALLBACK_MANIFEST=
BAD_GENERATION_MANIFEST=
KRUSTBOOT_CORRUPT=
EXPECT_ACTIVATION_SUCCESS=0
SUCCESS_STABILITY_ATTEMPTS=$QEMU_STABILITY_ATTEMPTS
USE_SERIAL_PIPE=0
SERIAL_INPUT=

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
virtio-blk driver ready
block-driver reads sector 0
block-driver writes test sector
readback matches
store-service requests block read
block-driver returns bytes
unauthorized service cannot talk to block-driver
unauthorized service cannot access PCI I/O, IRQ, or DMA capabilities
'
        ;;
    m42|virtio-block)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
KrustBoot grants: 43
KrustBoot io port ranges: 3
KrustBoot mmio regions: 0
io_port[1] id=cap:io.pci-config base=0x0000000000000cf8 length=0x0000000000000008
io_port[2] id=cap:io.virtio-blk0 base=0x000000000000c000 length=0x0000000000001000
interrupt_line[0] id=cap:irq.virtio-blk0 line=11
dma_region[0] id=cap:dma.virtio-blk0 base=
proc=block-driver cap[4] io-port=cap:io.pci-config rights=read|write
proc=block-driver cap[7] io-port=cap:io.virtio-blk0 rights=read|write
virtio-blk PCI device discovered
DMA map accepted: proc=block-driver dma-region=cap:dma.virtio-blk0
virtio-blk driver ready
block-driver reads sector 0
block-driver writes test sector
readback matches
block-driver received block-read request
block-driver returns bytes
vertex-store verifies hash
unauthorized service cannot talk to block-driver
unauthorized service cannot access PCI I/O, IRQ, or DMA capabilities
Native service activation ok
'
        ;;
    m42-driver-fault|block-driver-fault)
        MANIFEST="$ROOT_DIR/examples/krust-block-driver-fault-generation.vertex.json"
        required_lines='
Boot generation: gen:block-driver-fault-0001
KrustBoot grants: 44
proc=block-driver cap[8] timer=monotonic-timer rights=control
block-driver fault injection triggers direct invalid load
User page fault: proc=block-driver
User process fault contained: proc=block-driver
vertex-init readiness timeout
activation failed
Native service activation failed
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
        BAD_GENERATION_MANIFEST="$ROOT_DIR/examples/krust-switch-c-bad-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
Boot generation: gen:switch-a-0001
vertex-store exposes generation B manifest
vertex-init attenuates private store reply endpoint to receive-only
vertex-init uses private store reply endpoint
vertex-init validates generation B
Krust generation switch accepted: from=gen:switch-a-0001 to=gen:switch-b-0002
Krust generation switch revoked old generation authority: generation=gen:switch-a-0001
old generation service loses old capability
Krust generation switch entering generation: gen:switch-b-0002
Boot generation: gen:switch-b-0002
service from B runs
vertex-init validates generation C
Krust generation switch accepted: from=gen:switch-b-0002 to=gen:switch-c-bad-0003
Krust generation switch entering generation: gen:switch-c-bad-0003
Boot generation: gen:switch-c-bad-0003
activation failed
falling back to generation: gen:switch-b-0002
Krust rollback generation accepted: target=gen:switch-b-0002
Krust rollback entering generation: gen:switch-b-0002
Krust generation switch rejected: requested=gen:switch-c-bad-0003
bad generation C fails
rollback to B
Native service activation ok
'
        ;;
    m40|directed-ipc)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
KrustBoot endpoints: 10
KrustBoot grants: 43
IPC FIFO regression: queued sends preserve FIFO order
IPC FIFO regression: queue-full send rejected
IPC FIFO regression: receiver-specific dequeue preserves eligible ordering
IPC FIFO regression: multiple blocked receivers match eligible messages
IPC FIFO regression ok
endpoint[4] name=block-read-request
endpoint[5] name=vertex-store-block-reply
endpoint[6] name=store-hello-text-request
endpoint[7] name=model-reader-store-reply
endpoint[8] name=state-counter-request
endpoint[9] name=state-reader-state-reply
grant[23] process=block-driver cap[0] endpoint=block-read-request rights=receive
grant[25] process=vertex-store cap[3] endpoint=vertex-store-block-reply rights=receive
grant[27] process=vertex-store cap[0] endpoint=store-hello-text-request rights=receive
grant[29] process=model-reader cap[0] endpoint=model-reader-store-reply rights=receive
grant[31] process=vertex-state cap[0] endpoint=state-counter-request rights=receive
grant[33] process=reader-service cap[0] endpoint=state-reader-state-reply rights=receive
vertex-init observed ready: serial-driver
vertex-init observed ready: block-driver
vertex-init observed ready: vertex-store
vertex-init observed ready: vertex-state
vertex-init derives endpoint cap for vertex-store from endpoint[4] rights=send
vertex-init derives endpoint cap for vertex-store from endpoint[7] rights=send
vertex-init derives endpoint cap for model-reader from endpoint[6] rights=send
vertex-init derives endpoint cap for counter-service from endpoint[8] rights=send
vertex-init derives endpoint cap for reader-service from endpoint[8] rights=send
model-reader reads bytes successfully
reader-service write rejected
Native service activation ok
'
        ;;
    m41|console-shell)
        MANIFEST="$ROOT_DIR/examples/krust-console-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        USE_SERIAL_PIPE=1
        SERIAL_INPUT='help
generation
services
why svc:echo cap:log.sink
halt
'
        required_lines='
Boot generation: gen:console-0001
KrustBoot boot modules: 15
KrustBoot processes: 15
KrustBoot endpoints: 12
KrustBoot grants: 50
proc=console-driver cap[0] endpoint=console-output rights=receive
proc=console-driver cap[3] endpoint=console-driver-control rights=receive
proc=console-shell cap[0] endpoint=console-shell-request rights=receive
proc=console-driver cap[5] io-port=cap:io.com1 rights=read|write
vertex-init delegates inspect authority to console-shell
console-driver ready
vertex-init observed ready: console-driver
console-shell ready
vertex-init observed ready: console-shell
Runtime inspect accepted: proc=console-shell
console-driver wrote console output
Vertex shell ready
console-driver forwarded serial command: help
commands: generation services why halt
console-driver forwarded serial command: generation
current generation: gen:console-0001
console-driver forwarded serial command: services
console-shell service state: vertex-init=
console-shell service state: logd=
console-shell service state: vertex-store=
console-shell service state: vertex-state=
console-shell service state: console-shell=
console-driver forwarded serial command: why svc:echo cap:log.sink
console-shell why result: svc:echo cap:log.sink send slot 0
console-driver forwarded serial command: halt
Native console shell ok
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
native which-generation vertex-inspect
generation: vertex-inspect started in gen:inspect-0001
native delegated endpoint cap report
derived endpoint cap: proc=echo cap[0] endpoint=log-sink
derived endpoint caps from vertex-init:
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
        echo "usage: scripts/krust-test.sh <m13|m14|valid-activation|manifest-cycle|bad-cap|readiness|readiness-timeout|rollback|store-state-services|timer|preemption|m30|user-fault|m31|restart|manifest-v1|cap-lifecycle|typed-arenas|quotas|m32|io-substrate|m33|serial-driver|m34|block-driver|m35|store-service|m36|state-service|m37|generation-switch|m38|introspection|m40|directed-ipc|m41|console-shell|m42|virtio-block|m42-driver-fault|block-driver-fault|manifest-truncated|manifest-bad-magic|manifest-raw-compact|manifest-unsupported-version|manifest-oob-record|manifest-missing-provider>" >&2
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

(cd "$KRUST_DIR" && make iso VERTEX_MANIFEST="$MANIFEST" FALLBACK_MANIFEST="$FALLBACK_MANIFEST" BAD_GENERATION_MANIFEST="$BAD_GENERATION_MANIFEST" KRUSTBOOT_CORRUPT="$KRUSTBOOT_CORRUPT")

mkdir -p "$(dirname "$SERIAL_LOG")"
rm -f "$SERIAL_LOG"

pid=
cat_pid=
feeder_pid=
serial_pipe=
cleanup() {
    if [ -n "$feeder_pid" ]; then
        kill "$feeder_pid" >/dev/null 2>&1 || true
        wait "$feeder_pid" >/dev/null 2>&1 || true
    fi
    if [ -n "$pid" ]; then
        kill "$pid" >/dev/null 2>&1 || true
        wait "$pid" >/dev/null 2>&1 || true
    fi
    if [ -n "$cat_pid" ]; then
        kill "$cat_pid" >/dev/null 2>&1 || true
        wait "$cat_pid" >/dev/null 2>&1 || true
    fi
    if [ -n "$serial_pipe" ]; then
        rm -f "$serial_pipe.in" "$serial_pipe.out"
    fi
}
trap cleanup EXIT INT TERM

serial_arg="file:$SERIAL_LOG"
if [ "$USE_SERIAL_PIPE" -eq 1 ]; then
    serial_pipe="$BUILD_DIR/serial-test-pipe"
    rm -f "$serial_pipe.in" "$serial_pipe.out"
    mkfifo "$serial_pipe.in" "$serial_pipe.out"
    cat "$serial_pipe.out" >"$SERIAL_LOG" &
    cat_pid=$!
    serial_arg="pipe:$serial_pipe"
fi

"$QEMU" $QEMU_EXTRA \
    $QEMU_MACHINE \
    $QEMU_BLOCK \
    -m 256M \
    -serial "$serial_arg" \
    -monitor none \
    -display none \
    -no-reboot \
    -no-shutdown \
    -cdrom "$ISO_IMAGE" &
pid=$!

if [ "$USE_SERIAL_PIPE" -eq 1 ]; then
    (
        input_attempt=1
        while [ "$input_attempt" -le "$QEMU_ATTEMPTS" ]; do
            if grep -Fq "Vertex shell ready" "$SERIAL_LOG" 2>/dev/null; then
                break
            fi
            sleep "$QEMU_POLL_SECONDS"
            input_attempt=$((input_attempt + 1))
        done
        printf '%s' "$SERIAL_INPUT" >"$serial_pipe.in" 2>/dev/null || true
    ) &
    feeder_pid=$!
fi

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
