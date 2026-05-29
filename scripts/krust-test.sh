#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd "$(dirname "$0")/.." && pwd)
KRUST_DIR=${KRUST_DIR:-"$ROOT_DIR/kernel/krust"}
BUILD_DIR=${BUILD_DIR:-"$KRUST_DIR/build"}
ISO_IMAGE=${ISO_IMAGE:-"$BUILD_DIR/krust.iso"}
BLOCK_IMAGE=${BLOCK_IMAGE:-"$BUILD_DIR/krust-block.img"}
SERIAL_LOG=${SERIAL_LOG:-"$BUILD_DIR/serial-test.log"}
QEMU=${QEMU:-qemu-system-x86_64}
QEMU_EXTRA=${QEMU_EXTRA:-"-object rng-random,filename=/dev/urandom,id=vertexrng -device virtio-rng-pci,rng=vertexrng,disable-modern=on -netdev user,id=vertexnet -device virtio-net-pci,netdev=vertexnet,mac=52:54:00:12:34:56,disable-modern=on"}
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
VERTEX_DISK_CORRUPT=
EXPECT_ACTIVATION_SUCCESS=0
SUCCESS_STABILITY_ATTEMPTS=$QEMU_STABILITY_ATTEMPTS
USE_SERIAL_PIPE=0
SERIAL_INPUT_DELAYED=0
SERIAL_INPUT_DELAY_SECONDS=2
SERIAL_INPUT=
REBOOT_REQUIRED_LINES=
case_forbidden_lines=

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
negative test: logd process-create rejected: bad capability
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
M69 100 fault/restart cycles return to baseline frame object and cap counts
Native service activation ok
'
        ;;
    restart)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
flaky-service exits with status 1
flaky-service creates quota-backed endpoint
vertex-init observes failure
restart policy = on-failure
restart backoff sleep elapsed
vertex-init restarts flaky-service once
Krust process restart reload: proc=flaky-service
Krust process restart restores quota baseline: proc=flaky-service
flaky-service restart quota restored
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
proc=vertex-init cap[2] process-control=process-control rights=control|allocate|delegate|revoke|inspect|create|start|kill|wait
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
KrustBoot grants: 61
KrustBoot io port ranges: 3
KrustBoot mmio regions: 0
KrustBoot pci devices: 4
KrustBoot virtio devices: 4
io_port[1] id=cap:io.pci-config base=0x0000000000000cf8 length=0x0000000000000008
io_port[2] id=cap:io.virtio-blk0 base=0x000000000000c000 length=0x0000000000001000
interrupt_line[0] id=cap:irq.virtio-blk0 line=11
dma_region[0] id=cap:dma.virtio-blk0 base=
proc=block-driver cap[6] io-port=cap:io.pci-config rights=read|write
proc=block-driver cap[9] io-port=cap:io.virtio-blk0 rights=read|write
proc=block-driver cap[10] pci-device=device:virtio-blk0 kind=virtio-blk-pci rights=control
proc=block-driver cap[11] virtio-device=device:virtio-blk0 transport=virtio-pci-io rights=control
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
KrustBoot grants: 61
KrustBoot store objects:
proc=block-driver cap[10] store-object=store:block-driver-fault-token rights=read
Object read accepted: proc=block-driver object=store:block-driver-fault-token bytes=25
block-driver fault injection triggers direct invalid load
User page fault: proc=block-driver
User process fault contained: proc=block-driver
vertex-init readiness timeout
activation failed
Native service activation failed
'
        ;;
    m43|vertexdisk)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
KrustBoot endpoints: 12
KrustBoot grants: 61
KrustBoot state volumes: 0
QEMU boots with VertexDisk image attached
VertexDisk superblock accepted
vertex-store reads object index from disk
vertex-state reads state volume from disk
vertex-state writes journal record to disk
vertex-state writes state volume to disk
Native service activation ok
'
        REBOOT_REQUIRED_LINES='
reboot preserves state value
vertex-state reads state volume from disk
Native service activation ok
'
        ;;
    m43-bad-superblock|vertexdisk-bad-superblock)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        VERTEX_DISK_CORRUPT=bad-superblock
        required_lines='
VertexDisk superblock rejected
KrustBoot native store object unavailable for process: process=vertex-init object=store:vertex-init-demo
Native runtime init failed from KrustBoot manifest
Native service activation failed
'
        ;;
    m44|boot-manager)
        MANIFEST="$ROOT_DIR/examples/krust-rollback-bad-generation.vertex.json"
        FALLBACK_MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
Boot generation: gen:bad-0002
activation failed
Native boot manager last_failed_generation=gen:bad-0002
Native boot manager fallback selected_generation=gen:hello-0001
Native boot manager journal: failed generation=gen:bad-0002 fallback=gen:hello-0001
Krust rollback generation accepted: target=gen:hello-0001
Boot generation: gen:hello-0001
Native boot manager known_good_generation=gen:hello-0001
Native service activation ok
'
        ;;
    m45|store-verification)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        VERTEX_DISK_CORRUPT=store-object
        required_lines='
vertex-store hash mismatch security event: object=store:hello-text
vertex-inspect security event: store hash mismatch object=store:hello-text
vertex-init service failed: vertex-store
activation failed
Native service activation failed
'
        ;;
    m46|native-update)
        MANIFEST="$ROOT_DIR/examples/krust-switch-a-generation.vertex.json"
        FALLBACK_MANIFEST="$ROOT_DIR/examples/krust-switch-b-generation.vertex.json"
        BAD_GENERATION_MANIFEST="$ROOT_DIR/examples/krust-switch-c-bad-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
vertex-init validates generation B
Native update transaction verifies manifest hash
Native update transaction verifies store closure
Native update transaction journal commit
Native update transaction selected_generation updated: gen:switch-b-0002
Krust generation switch entering generation: gen:switch-b-0002
Boot generation: gen:switch-b-0002
Native service activation ok
'
        ;;
    m47|store-executables)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
Krust process executable store object: process=logd object=store:logd-demo
store hash verified before process creation: process=logd
Krust process image loaded from native store: process=logd
Krust process executable store object: process=echo object=store:echo-server-demo
store hash verified before process creation: process=echo
Krust process image loaded from native store: process=echo
vertex-store verifies executable store object: logd
vertex-store verifies executable store object: echo
Native service activation ok
'
        ;;
    m47-corrupt-executable|store-executable-corruption)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        VERTEX_DISK_CORRUPT=store-executable
        required_lines='
Krust process executable store object: process=logd object=store:logd-demo
Krust process executable checksum mismatch: process=logd object=store:logd-demo
vertex-inspect security event: store hash mismatch object=store:logd-demo
Native runtime init failed from KrustBoot manifest
Native service activation failed
'
        case_forbidden_lines='
Krust process image loaded from native store: process=logd
Native service activation ok
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
Native boot manager last_failed_generation=gen:switch-c-bad-0003
Native boot manager fallback selected_generation=gen:switch-b-0002
Native boot manager journal: failed generation=gen:switch-c-bad-0003 fallback=gen:switch-b-0002
Krust rollback generation accepted: target=gen:switch-b-0002
Native boot manager previous_generation=gen:switch-c-bad-0003
Krust rollback entering generation: gen:switch-b-0002
Native service activation ok
'
        ;;
    m40|directed-ipc)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
KrustBoot endpoints: 12
KrustBoot grants: 61
IPC FIFO regression: queued sends preserve FIFO order
IPC FIFO regression: queue-full send rejected
IPC FIFO regression: receiver-specific dequeue preserves eligible ordering
IPC FIFO regression: multiple blocked receivers match eligible messages
IPC FIFO regression ok
endpoint[4] name=vertex-store-block-request
endpoint[5] name=vertex-state-block-request
endpoint[6] name=vertex-store-block-reply
endpoint[7] name=vertex-state-block-reply
endpoint[8] name=store-hello-text-request
endpoint[9] name=model-reader-store-reply
endpoint[10] name=state-counter-request
endpoint[11] name=state-reader-state-reply
process=block-driver cap[0] endpoint=vertex-store-block-request rights=receive
process=block-driver cap[3] endpoint=vertex-state-block-request rights=receive
process=vertex-store cap[3] endpoint=vertex-store-block-reply rights=receive
process=vertex-store cap[0] endpoint=store-hello-text-request rights=receive
process=model-reader cap[0] endpoint=model-reader-store-reply rights=receive
process=vertex-state cap[0] endpoint=state-counter-request rights=receive
process=vertex-state cap[3] endpoint=vertex-state-block-reply rights=receive
process=reader-service cap[0] endpoint=state-reader-state-reply rights=receive
vertex-init observed ready: serial-driver
vertex-init observed ready: block-driver
vertex-init observed ready: vertex-store
vertex-init observed ready: vertex-state
vertex-init derives endpoint cap for block-driver from endpoint[6] rights=send
vertex-init derives endpoint cap for block-driver from endpoint[7] rights=send
vertex-init derives endpoint cap for vertex-store from endpoint[4] rights=send
vertex-init derives endpoint cap for vertex-store from endpoint[9] rights=send
vertex-init derives endpoint cap for vertex-state from endpoint[5] rights=send
vertex-init derives endpoint cap for vertex-state from endpoint[11] rights=send
vertex-init derives endpoint cap for model-reader from endpoint[8] rights=send
vertex-init derives endpoint cap for counter-service from endpoint[10] rights=send
vertex-init derives endpoint cap for reader-service from endpoint[10] rights=send
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
KrustBoot boot modules: 14
KrustBoot processes: 14
KrustBoot endpoints: 15
KrustBoot grants: 65
proc=console-driver cap[0] endpoint=console-output rights=receive
proc=console-driver cap[3] endpoint=console-driver-control rights=receive
proc=console-shell cap[0] endpoint=console-shell-request rights=receive
proc=console-driver cap[5] io-port=cap:io.com1 rights=read|write
vertex-init delegates inspect and update authority to console-shell
console-driver ready
vertex-init observed ready: console-driver
console-shell ready
vertex-init observed ready: console-shell
Runtime inspect accepted: proc=console-shell
console-driver wrote console output
Vertex shell ready
console-driver forwarded serial command: help
commands: generation services counter increment install rollback why halt
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
    m48|dynamic-process)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
Process table entries: 1
proc=vertex-init cap[2] process-control=process-control rights=control|allocate|delegate|revoke|inspect|create|start|kill|wait
Krust process create accepted: proc=vertex-init target=logd
vertex-init dynamically created service: logd
initial capability grants supplied explicitly: process=logd
SYS_PROCESS_CREATE rejected: bad capability
unprivileged service calls SYS_PROCESS_CREATE
Krust process wait observed exit: proc=logd
vertex-init waits for service exit status
Native service activation ok
'
        ;;
    m49|config-objects)
        MANIFEST="$ROOT_DIR/examples/krust-inspect-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
Krust native config hash verified: config=config:logd
Config object read accepted: proc=logd config=config:logd
logd reads config object
echo cannot read logd config
vertex-inspect shows config authority without dumping content
Native service activation ok
'
        ;;
    m49-config-corrupt|config-hash-mismatch)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        VERTEX_DISK_CORRUPT=config-object
        required_lines='
Krust native config hash mismatch: config=config:logd
vertex-inspect security event: store hash mismatch object=config:logd
vertex-init service failed: logd
activation failed
Native service activation failed
'
        ;;
    m50|secrets)
        MANIFEST="$ROOT_DIR/examples/krust-inspect-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
Native secret object registered: secret:logd-token storage=in-memory
Native secret grant: process=logd secret=secret:logd-token rights=read|inspect-metadata
Secret read accepted: proc=logd secret=secret:logd-token bytes=<redacted>
service with secret cap reads secret
service without secret cap rejected
vertex-inspect shows which services have secret access
vertex-inspect does not print secret value
Native service activation ok
'
        case_forbidden_lines='
native-secret-value
'
        ;;
    m54|appliance)
        MANIFEST="$ROOT_DIR/examples/krust-console-generation.vertex.json"
        FALLBACK_MANIFEST="$ROOT_DIR/examples/krust-console-new-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        USE_SERIAL_PIPE=1
        SERIAL_INPUT_DELAYED=1
        QEMU_ATTEMPTS=${QEMU_APPLIANCE_ATTEMPTS:-45}
        SERIAL_INPUT='install generation gen:new
counter
increment
rollback to gen:old
why svc:counter state:counter
halt
'
        required_lines='
QEMU boots with VertexDisk image attached
Vertex OS v0 appliance booted
Vertex shell ready
console-driver forwarded serial command: install generation gen:new
install generation gen:new
Native update transaction verifies manifest hash: generation=gen:console-new-0002
Native update transaction verifies store closure: generation=gen:console-new-0002
Krust generation switch accepted: from=gen:console-0001 to=gen:console-new-0002
Krust generation switch entering generation: gen:console-new-0002
console-driver forwarded serial command: counter
counter value: 41
console-driver forwarded serial command: increment
increment -> 42
console-driver forwarded serial command: rollback to gen:old
rollback to gen:old
counter state policy: preserve
counter value: 42
Krust rollback generation accepted: target=gen:console-0001
Krust rollback entering generation: gen:console-0001
console-driver forwarded serial command: why svc:counter state:counter
why svc:counter state:counter
svc:counter has state authority from generation graph
Native console shell ok
Native service activation ok
'
        ;;
    m55|driver-framework)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
KrustBoot pci devices: 4
KrustBoot virtio devices: 4
pci_device[0] id=device:virtio-blk0 kind=virtio-blk-pci
virtio_device[0] id=device:virtio-blk0 transport=virtio-pci-io
virtio_device[1] id=device:virtio-console0 transport=virtio-pci-io
virtio_device[2] id=device:virtio-rng0 transport=virtio-pci-io
virtio_device[3] id=device:virtio-net0 transport=virtio-pci-io
process[1] name=serial-driver module=serial-driver initial=no service=svc:serial-driver restart=0 health=ipc-ping
process[3] name=netstack module=netstack initial=no service=svc:netstack restart=1 health=ipc-ping
process[4] name=block-driver module=block-driver initial=no service=svc:block-driver restart=0 health=ipc-ping
proc=serial-driver cap[3] io-port=cap:io.com1 rights=read|write
proc=serial-driver cap[5] virtio-device=device:virtio-console0 transport=virtio-pci-io rights=control
proc=netstack cap[3] virtio-device=device:virtio-rng0 transport=virtio-pci-io rights=control
proc=netstack cap[5] virtio-device=device:virtio-net0 transport=virtio-pci-io rights=control
proc=netstack cap[6] network-port=cap:net.udp.9000 rights=control
proc=block-driver cap[10] pci-device=device:virtio-blk0 kind=virtio-blk-pci rights=control
proc=block-driver cap[11] virtio-device=device:virtio-blk0 transport=virtio-pci-io rights=control
serial-driver ready
virtio-console replaces raw serial shell transport
netstack ready
virtio-blk driver ready
unauthorized service cannot access PCI I/O, IRQ, or DMA capabilities
Native driver framework ok
Native service activation ok
'
        ;;
    m56|virtio-device-stack)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
KrustBoot pci devices: 4
KrustBoot virtio devices: 4
virtio-console replaces raw serial shell transport
virtio-rng provides random bytes through explicit cap
virtio-net driver can send raw frames
virtio-net driver can receive raw frames
unauthorized service cannot use network device
Native service activation ok
'
        ;;
    m57|networking-v0)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
QEMU user-mode network attached
Virtio net TX completed: proc=netstack virtio-device=device:virtio-net0 frame-bytes=60
Virtio net RX completed: proc=netstack virtio-device=device:virtio-net0 frame-bytes=
QEMU user-mode network delivered a raw frame
Vertex sends ICMP echo
QEMU user-mode network delivered ICMP echo reply
UDP send queued for netstack: proc=echo network-port=cap:net.udp.9000 bytes=13
echo submits UDP request to netstack boundary
Network-port UDP request delivered to netstack: network-port=cap:net.udp.9000 bytes=13
UDP send transmitted: proc=netstack network-port=cap:net.udp.9000 bytes=13
network authority is endpoint/capability mediated
unauthorized service cannot use network device
Native service activation ok
'
        ;;
    m59|namespace-service)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
KrustBoot namespaces: 2
namespace[0] id=cap:namespace.echo entries=1
namespace[1] id=cap:namespace.reader entries=1
proc=echo cap[4] namespace=cap:namespace.echo rights=resolve
proc=reader-service cap[4] namespace=cap:namespace.reader rights=resolve
Namespace resolve accepted: proc=echo namespace=cap:namespace.echo path=/state/a
service A namespace contains /state/a
Namespace resolve rejected: proc=echo namespace=cap:namespace.echo path=/state/b
service A cannot resolve /state/b
Native service activation ok
'
        ;;
    m60|policy-typed)
        MANIFEST="/private/tmp/krust-m60-policy-generation.vertex.json"
        "$ROOT_DIR/target/debug/vertexctl" compile-policy "$ROOT_DIR/examples/policy.vertex" "$MANIFEST"
        "$ROOT_DIR/target/debug/vertexctl" compile-typed "$ROOT_DIR/examples/typed-system.vertex" /private/tmp/krust-m60-typed-generation.vertex.json
        if "$ROOT_DIR/target/debug/vertexctl" compile-typed "$ROOT_DIR/examples/invalid-missing-capability.vertex" /private/tmp/krust-m60-invalid.vertex.json; then
            echo "typed policy unexpectedly accepted missing capability" >&2
            exit 1
        fi
        "$ROOT_DIR/target/debug/vertexctl" compile-boot-manifest "$MANIFEST" /private/tmp/krust-m60-policy.krustboot
        "$ROOT_DIR/target/debug/vertexctl" create-vertex-disk /private/tmp/krust-m60-policy.img "$MANIFEST"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
Boot generation: gen:m60-policy-0001
Native service activation ok
'
        ;;
    m61|abi-authority-hardening)
        MANIFEST="$ROOT_DIR/examples/krust-inspect-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
Boot generation: gen:inspect-0001
M61 malformed boot-read buffer rejected
M61 rights subset checks reject derived and transferred authority
M61 capability move rejects occupied target without dropping source
M61 provider malformed receive/read buffers rejected
M61 syscall negative table: wrong object kind rejected
M61 syscall negative table: missing rights rejected
M61 syscall negative table: malformed buffers rejected
M61 virtio typed device syscalls reject mismatched device IDs
M61 timer syscall rejects wrong object kind
M61 inspect authority rejects wrong kind and missing create right
Capability inspect: proc=echo
parent_cap_id=
generation=gen:inspect-0001
Capability revoke accepted: proc=echo
Native service activation ok
'
        ;;
    m62|storage-durability)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
VertexDisk durability model: ordered journal write, data write, index commit; flush barrier unsupported
virtio-blk request completion status ok
block-driver enforces sector-range and alignment
immutable store endpoint is read-only
immutable store object served read-only
state endpoint write bounds and owner checks ok
vertex-state owner check accepted: state:counter via vertex-state endpoint
vertex-state write bounds enforced
block-driver propagates request completion to client
update commit interrupted before final pointer leaves previous generation bootable
block-driver fault during request fails client request without kernel fault
Native service activation ok
'
        REBOOT_REQUIRED_LINES='
reboot preserves state value
vertex-state reads state volume from disk
Native service activation ok
'
        ;;
    m62-journal-replay|storage-journal-replay)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        VERTEX_DISK_CORRUPT=interrupted-state-journal
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
vertex-state replays journal record
interrupted state journal replays deterministically
Native service activation ok
'
        ;;
    m62-corrupt-journal|storage-corrupt-journal)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        VERTEX_DISK_CORRUPT=corrupt-state-journal
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
vertex-state corrupt journal detected
corrupt state journal reported and rolled back deterministically
Native service activation ok
'
        ;;
    m63|network-boundary)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
netstack owns device:virtio-net0 and raw virtio-net authority
raw virtio-net authority granted only to netstack
ARP cache owned by netstack
IPv4 packet validation ok
QEMU user-mode network attached
echo sends UDP through cap:net.udp.9000 without a raw virtio-device cap
network-port bind/listen rights enforced by netstack boundary
netstack received UDP request through network-port boundary
netstack transmitted UDP packet for network-port client
unauthorized service cannot bind or send on cap:net.udp.9000
unauthorized service cannot use network device
proc=echo cap[3] network-port=cap:net.udp.9000 rights=bind|listen
proc=netstack cap[5] virtio-device=device:virtio-net0 transport=virtio-pci-io rights=control
proc=netstack cap[6] network-port=cap:net.udp.9000 rights=control
Native service activation ok
'
        ;;
    m64|supervisor-lifecycle|m66|memory-lifecycle|m67|address-space-teardown|m68|failure-atomicity|m69|memory-pressure)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
manifest dependency graph defines startup ordering
M66 double-free rejected and accounting unchanged
M66 foreign-free rejected and accounting unchanged
M66 failed contiguous allocation leaves accounting unchanged
M66 restart uses zeroed userspace data
M68 endpoint_create occupied slot rejected before quota charge
M68 cap grant failure leaves source and target unchanged
M68 namespace_resolve occupied slot leaves target unchanged
M69 repeated failed endpoint creates leave quota usable
M69 100 create/start/exit cycles return to baseline frame object and cap counts
M69 100 restart cycles return to baseline frame object and cap counts
M69 endpoint churn reaches quota and returns to baseline after owner exit
M69 inspect shows memory high-water marks and current live counts
service starts only after declared providers are ready
service lifecycle declared: logd
service lifecycle starting: logd
service lifecycle ready: logd
vertex-init observes failure
Krust process address space reaped: proc=flaky-service
service lifecycle restarting: flaky-service
restart budget remaining=0 backoff-ms=10
restart backoff sleep elapsed
restart budget and backoff policy enforced
service lifecycle exited: flaky-service
operator-visible activation log records generation id
runtime inspect lifecycle state verified: declared
runtime inspect lifecycle state verified: starting
runtime inspect lifecycle state verified: ready
runtime inspect lifecycle state verified: failed
runtime inspect lifecycle state verified: restarting
runtime inspect lifecycle state verified: exited
inspect reports frame owner and lifecycle counters
inspect reports zero unreachable kernel objects
inspect reports cap/object leak baseline counters
inspect reports no live mappings for reaped pids
inspect reports declared, starting, ready, failed, restarting, and exited states
M67 kill_process releases sleeping process frames and scheduler state
Native restart policy ok
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
who-can: vertex-state owns state:counter through VertexDisk block service authority
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
    manifest-old-compact-magic)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        KRUSTBOOT_CORRUPT=old-compact-magic
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
        echo "usage: scripts/krust-test.sh <m13|m14|valid-activation|manifest-cycle|bad-cap|readiness|readiness-timeout|rollback|store-state-services|timer|preemption|m30|user-fault|m31|restart|manifest-v1|cap-lifecycle|typed-arenas|quotas|m32|io-substrate|m33|serial-driver|m34|block-driver|m35|store-service|m36|state-service|m37|generation-switch|m38|introspection|m40|directed-ipc|m41|console-shell|m42|virtio-block|m42-driver-fault|block-driver-fault|m43|vertexdisk|m43-bad-superblock|vertexdisk-bad-superblock|m44|boot-manager|m45|store-verification|m46|native-update|m47|store-executables|m47-corrupt-executable|store-executable-corruption|m48|dynamic-process|m49|config-objects|m49-config-corrupt|config-hash-mismatch|m50|secrets|m54|appliance|m55|driver-framework|m56|virtio-device-stack|m57|networking-v0|m59|namespace-service|m60|policy-typed|m61|abi-authority-hardening|m62|storage-durability|m62-journal-replay|storage-journal-replay|m62-corrupt-journal|storage-corrupt-journal|m63|network-boundary|m64|supervisor-lifecycle|m66|memory-lifecycle|m67|address-space-teardown|m68|failure-atomicity|m69|memory-pressure|manifest-truncated|manifest-bad-magic|manifest-raw-compact|manifest-old-compact-magic|manifest-unsupported-version|manifest-oob-record|manifest-missing-provider>" >&2
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
if [ -n "$case_forbidden_lines" ]; then
    forbidden_lines="${forbidden_lines}
$case_forbidden_lines"
fi

(cd "$KRUST_DIR" && make iso VERTEX_MANIFEST="$MANIFEST" FALLBACK_MANIFEST="$FALLBACK_MANIFEST" BAD_GENERATION_MANIFEST="$BAD_GENERATION_MANIFEST" KRUSTBOOT_CORRUPT="$KRUSTBOOT_CORRUPT" VERTEX_DISK_CORRUPT="$VERTEX_DISK_CORRUPT")

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
        if [ "$SERIAL_INPUT_DELAYED" -eq 1 ]; then
            printf '%s' "$SERIAL_INPUT" | while IFS= read -r line; do
                printf '%s\n' "$line" >"$serial_pipe.in" 2>/dev/null || true
                sleep "$SERIAL_INPUT_DELAY_SECONDS"
            done
        else
            printf '%s' "$SERIAL_INPUT" >"$serial_pipe.in" 2>/dev/null || true
        fi
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
            if [ -n "$REBOOT_REQUIRED_LINES" ]; then
                cleanup
                pid=
                rm -f "$SERIAL_LOG"
                required_lines="$REBOOT_REQUIRED_LINES"
                missing_required=
                present_forbidden=

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

                reboot_attempt=1
                while [ "$reboot_attempt" -le "$QEMU_ATTEMPTS" ]; do
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
                    reboot_attempt=$((reboot_attempt + 1))
                done

                cleanup
                pid=
                echo "krust test failed: $CASE reboot after $QEMU_ATTEMPTS checks"
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
            fi
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
