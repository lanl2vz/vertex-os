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
QEMU_ATTEMPTS=${QEMU_ATTEMPTS:-60}
QEMU_POLL_SECONDS=${QEMU_POLL_SECONDS:-1}
QEMU_STABILITY_ATTEMPTS=${QEMU_STABILITY_ATTEMPTS:-1}
QEMU_PREEMPTION_STABILITY_ATTEMPTS=${QEMU_PREEMPTION_STABILITY_ATTEMPTS:-3}
CASE=${1:-m14}
FALLBACK_MANIFEST=
BAD_GENERATION_MANIFEST=
BOOT_FALLBACK_MANIFEST=
BOOT_BAD_GENERATION_MANIFEST=
VERTEX_DISK_GRAPH_ONLY_MANIFESTS=
HOSTLESS_BOOT_GENERATIONS=0
KRUSTBOOT_CORRUPT=
VERTEX_DISK_CORRUPT=
VERTEXFS_CORRUPT=
VERTEXFS_UPDATE_APP_A_PAYLOAD=
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
echo VFS open rejected: permission
unauthorized process cannot open file
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
VFS lock accepted: proc=faulty-service file=a
faulty-service holds VFS lock before fault
vertex-init observes failure
restart policy = on-failure
vertex-init restarts faulty-service once
Krust process restart reload: proc=faulty-service
faulty-service reacquires VFS lock after fault cleanup
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
KrustBoot grants: 65
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
proc=block-driver cap[10] vfs-root=cap:vfs.block-dev-blk0 root=/dev/device:virtio-blk0 rights=read|resolve
proc=block-driver cap[11] pci-device=device:virtio-blk0 kind=virtio-blk-pci rights=control
proc=block-driver cap[12] virtio-device=device:virtio-blk0 transport=virtio-pci-io rights=control
virtio-blk PCI device discovered
direct virtio-device cap is not VFS path authority
device node open requires VFS authority and underlying device authority
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
VFS open accepted: proc=block-driver file=store:block-driver-fault-token
VFS read accepted: proc=block-driver file=store:block-driver-fault-token bytes=25
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
KrustBoot endpoints: 10
KrustBoot grants: 65
KrustBoot state volumes: 2
state_volume[0] id=state:counter
state_volume[1] id=state:scratch
VFS state volume mounted: state=state:counter path=/state/counter source=vertex-state
VFS state volume value file mounted: state=state:counter path=/state/counter/value source=vertex-state
VFS state volume control file mounted: state=state:counter path=/state/counter/control source=vertex-state
VFS state volume mounted: state=state:scratch path=/state/scratch source=vertex-state
VFS state volume value file mounted: state=state:scratch path=/state/scratch/value source=vertex-state
VFS state volume control file mounted: state=state:scratch path=/state/scratch/control source=vertex-state
QEMU boots with VertexDisk image attached
VertexDisk superblock accepted
vertex-store reads object index from disk
vertex-state reads state volume from disk
VFS state transaction request: proc=echo state=state:scratch op=write file=value
VFS state transaction request: proc=echo state=state:scratch op=read file=value
VFS state transaction request: proc=echo state=state:scratch op=stat file=value
generic state volume uses VFS service transaction
VFS state transaction request: proc=echo state=state:counter op=write file=value
VFS state transaction wake: proc=echo file=value op=write result=2
vertex-state serves VFS state write
VFS state transaction request: proc=echo state=state:counter op=read file=value
VFS state transaction wake: proc=echo file=value op=read result=2
vertex-state serves VFS state read
mounted state volume value uses VFS service transaction
VFS state transaction request: proc=echo state=state:counter op=stat file=value
VFS state transaction wake: proc=echo file=value op=stat result=64
vertex-state serves VFS state stat
service-backed state value stat reports durable length
vertex-state writes journal record to disk
vertex-state writes state volume to disk
Native service activation ok
'
REBOOT_REQUIRED_LINES='
reboot preserves state value
reboot preserves state:scratch value
vertex-state reads state volume from disk
Native service activation ok
'
        ;;
    m43-bad-superblock|vertexdisk-bad-superblock)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        VERTEX_DISK_CORRUPT=bad-superblock
        required_lines='
VertexDisk superblock rejected
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
unauthorized process cannot open file
legacy object-read syscall rejected
'
        ;;
    m36|state-service)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
counter-service has VFS state file
counter-service writes state through VFS
reader-service has VFS state file
reader-service reads state
reader-service write rejected
state control requires write-only open
VFS state transaction request: proc=echo state=state:counter op=control file=control
VFS state transaction wake: proc=echo file=control op=control result=1
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
KrustBoot endpoints: 10
KrustBoot grants: 65
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
process=block-driver cap[0] endpoint=vertex-store-block-request rights=receive
process=block-driver cap[3] endpoint=vertex-state-block-request rights=receive
process=vertex-store cap[3] endpoint=vertex-store-block-reply rights=receive
process=vertex-store cap[0] endpoint=store-hello-text-request rights=receive
process=model-reader cap[0] endpoint=model-reader-store-reply rights=receive
process=vertex-state cap[0] endpoint=vertex-state-block-reply rights=receive
process=counter-service cap[0] vfs-root=cap:vfs.counter-state rights=read|write|resolve
process=reader-service cap[0] vfs-root=cap:vfs.state-reader-state rights=read|resolve
process=echo cap[7] vfs-root=cap:vfs.echo-state-control rights=control|resolve
vertex-init observed ready: serial-driver
vertex-init observed ready: block-driver
vertex-init observed ready: vertex-store
vertex-init observed ready: vertex-state
vertex-init derives endpoint cap for block-driver from endpoint[6] rights=send
vertex-init derives endpoint cap for block-driver from endpoint[7] rights=send
vertex-init derives endpoint cap for vertex-store from endpoint[4] rights=send
vertex-init derives endpoint cap for vertex-store from endpoint[9] rights=send
vertex-init derives endpoint cap for vertex-state from endpoint[5] rights=send
vertex-init derives endpoint cap for model-reader from endpoint[8] rights=send
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
	KrustBoot endpoints: 15
	KrustBoot grants: 75
	proc=console-driver cap[0] endpoint=console-output rights=receive
	proc=console-driver cap[3] endpoint=console-driver-control rights=receive
	proc=gen-manager cap[0] endpoint=generation-manager-request rights=receive
	proc=console-shell cap[0] endpoint=console-shell-request rights=receive
	proc=console-shell cap[6] endpoint=generation-manager-request rights=send
	proc=console-shell cap[8] vfs-root=cap:vfs.console-state-control root=/state/counter/control rights=control|resolve
	proc=console-driver cap[5] io-port=cap:io.com1 rights=read|write
	vertex-init delegates inspect authority to console-shell
	vertex-init delegates generation update authority to generation-manager
	console-driver ready
	vertex-init observed ready: console-driver
	generation-manager ready
	vertex-init observed ready: gen-manager
	console-shell ready
	vertex-init observed ready: console-shell
	Runtime inspect accepted: proc=console-shell
console-driver wrote console output
Vertex shell ready
console-driver forwarded serial command: help
commands: generation services devices counter increment state-health install rollback why halt
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
console-shell observed state clients drained
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
VFS open accepted: proc=logd file=config:logd
VFS read accepted: proc=logd file=config:logd bytes=33
logd reads config through VFS handle
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
        SERIAL_INPUT_DELAY_SECONDS=6
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
	console-shell requests generation-manager install
	generation-manager install candidate from native graph-store: generation=gen:console-new-0002
	generation-manager transaction prepare: generation=gen:console-new-0002
	Native generation manager journal prepare: previous=gen:console-0001 target=gen:console-new-0002
	Native update transaction verifies manifest hash: generation=gen:console-new-0002
	Native update transaction verifies store closure: generation=gen:console-new-0002
	Krust generation switch accepted: from=gen:console-0001 to=gen:console-new-0002
	Native generation manager journal commit: selected_generation=gen:console-new-0002
	Krust generation switch entering generation: gen:console-new-0002
	console-driver forwarded serial command: counter
	counter value: 41
console-driver forwarded serial command: increment
increment -> 42
console-driver forwarded serial command: rollback to gen:old
	rollback to gen:old
	counter state policy: preserve
	counter value: 42
	console-shell requests generation-manager rollback
	generation-manager transaction rollback prepare: target=gen:console-0001
	Native generation manager journal prepare: previous=gen:console-new-0002 target=gen:console-0001
	Krust rollback generation accepted: target=gen:console-0001
	Native generation manager journal rollback: failed=gen:console-new-0002 selected_generation=gen:console-0001 reason=activation-failed
	Krust rollback entering generation: gen:console-0001
	console-driver forwarded serial command: why svc:counter state:counter
	why svc:counter state:counter
svc:counter has state authority from generation graph
	        Native console shell ok
		'
		        ;;
		    m83-hostless|generation-manager-hostless)
		        MANIFEST="$ROOT_DIR/examples/krust-console-generation.vertex.json"
		        FALLBACK_MANIFEST="$ROOT_DIR/examples/krust-console-new-generation.vertex.json"
		        HOSTLESS_BOOT_GENERATIONS=1
		        EXPECT_ACTIVATION_SUCCESS=1
		        USE_SERIAL_PIPE=1
		        SERIAL_INPUT_DELAYED=1
		        SERIAL_INPUT_DELAY_SECONDS=6
		        QEMU_ATTEMPTS=${QEMU_M83_ATTEMPTS:-45}
		        SERIAL_INPUT='install generation gen:new
		halt
		'
		        required_lines='
		Boot generation: gen:console-0001
		VertexDisk native generation ready: gen:console-new-0002
		generation-manager ready
		console-shell requests generation-manager install
		generation-manager install candidate from native graph-store: generation=gen:console-new-0002
		generation-manager reads VertexDisk generation metadata
		block-driver writes VertexDisk generation metadata sector
		generation-manager writes VertexDisk generation metadata: transaction=prepare selected=gen:console-0001 previous=gen:console-0001 target=gen:console-new-0002
		Native generation verification accepted: generation=gen:console-new-0002
		Native update transaction verifies manifest hash: generation=gen:console-new-0002
		Native update transaction verifies store closure: generation=gen:console-new-0002
		Krust generation switch staged: from=gen:console-0001 to=gen:console-new-0002
		generation-manager writes VertexDisk generation metadata: transaction=commit selected=gen:console-new-0002 previous=gen:console-0001 target=gen:console-new-0002
		Krust generation switch accepted: from=gen:console-0001 to=gen:console-new-0002
		Krust generation switch entering generation: gen:console-new-0002
		Boot generation: gen:console-new-0002
		console-shell observed state clients drained
		Native console shell ok
		'
		        ;;
		    m83-power-prepare|generation-manager-power-prepare)
		        MANIFEST="$ROOT_DIR/examples/krust-console-generation.vertex.json"
		        FALLBACK_MANIFEST="$ROOT_DIR/examples/krust-console-new-generation.vertex.json"
		        HOSTLESS_BOOT_GENERATIONS=1
		        VERTEX_DISK_CORRUPT=generation-prepare
		        EXPECT_ACTIVATION_SUCCESS=1
		        QEMU_ATTEMPTS=${QEMU_M83_ATTEMPTS:-45}
		        required_lines='
		VertexDisk generation transaction recovery: state=prepare selected=gen:console-0001 target=gen:console-new-0002
		power loss during prepare remounts selected_generation=gen:console-0001
		Native generation manager durable selected_generation=gen:console-0001
		VertexDisk selected generation active: gen:console-0001
		Boot generation: gen:console-0001
		generation-manager ready
		Vertex shell ready
		'
		        ;;
		    m83-power-commit|generation-manager-power-commit)
		        MANIFEST="$ROOT_DIR/examples/krust-console-generation.vertex.json"
		        FALLBACK_MANIFEST="$ROOT_DIR/examples/krust-console-new-generation.vertex.json"
		        HOSTLESS_BOOT_GENERATIONS=1
		        VERTEX_DISK_CORRUPT=generation-commit
		        EXPECT_ACTIVATION_SUCCESS=1
		        QEMU_ATTEMPTS=${QEMU_M83_ATTEMPTS:-45}
		        required_lines='
		VertexDisk generation transaction recovery: state=commit selected=gen:console-new-0002 target=gen:console-new-0002
		power loss during commit remounts selected_generation=gen:console-new-0002
		Native generation manager durable selected_generation=gen:console-new-0002
		VertexDisk selected generation active: gen:console-new-0002
		Boot generation: gen:console-new-0002
		generation-manager ready
		Vertex shell ready
		'
		        ;;
		    m83-power-rollback|generation-manager-power-rollback)
		        MANIFEST="$ROOT_DIR/examples/krust-console-generation.vertex.json"
		        FALLBACK_MANIFEST="$ROOT_DIR/examples/krust-console-new-generation.vertex.json"
		        HOSTLESS_BOOT_GENERATIONS=1
		        VERTEX_DISK_CORRUPT=generation-rollback
		        EXPECT_ACTIVATION_SUCCESS=1
		        QEMU_ATTEMPTS=${QEMU_M83_ATTEMPTS:-45}
		        required_lines='
		VertexDisk generation selection recovered: selected=gen:console-0001 previous=gen:console-new-0002 known_good=gen:console-0001 transaction=rollback target=gen:console-0001 failure_reason=activation-failed
		VertexDisk generation transaction recovery: state=rollback selected=gen:console-0001 target=gen:console-0001
			power loss during rollback remounts selected_generation=gen:console-0001
			Native generation manager durable selected_generation=gen:console-0001
			Native generation manager failure detail: service=gen:console-new-0002 dependency=service-readiness policy=known-good-rollback reason=activation-failed
			VertexDisk selected generation active: gen:console-0001
		Boot generation: gen:console-0001
		generation-manager ready
		Vertex shell ready
		'
		        ;;
		    m83|generation-manager)
		        MANIFEST="$ROOT_DIR/examples/krust-console-generation.vertex.json"
		        FALLBACK_MANIFEST="$ROOT_DIR/examples/krust-console-new-generation.vertex.json"
	        EXPECT_ACTIVATION_SUCCESS=1
	        USE_SERIAL_PIPE=1
	        SERIAL_INPUT_DELAYED=1
	        SERIAL_INPUT_DELAY_SECONDS=6
	        QEMU_ATTEMPTS=${QEMU_M83_ATTEMPTS:-45}
	        SERIAL_INPUT='install generation gen:new
	counter
	increment
	rollback to gen:old
	why svc:counter state:counter
	halt
	'
	        required_lines='
	Boot generation: gen:console-0001
	generation-manager ready
	native generation-manager owns selected-generation state
	vertex-init delegates generation update authority to generation-manager
	vertex-init observed ready: gen-manager
	proc=gen-manager cap[0] endpoint=generation-manager-request rights=receive
	proc=console-shell cap[6] endpoint=generation-manager-request rights=send
	console-shell requests generation-manager install
	generation-manager install candidate from native graph-store: generation=gen:console-new-0002
	generation-manager transaction prepare: generation=gen:console-new-0002
	generation-manager reads VertexDisk generation metadata
	block-driver writes VertexDisk generation metadata sector
	generation-manager writes VertexDisk generation metadata: transaction=prepare selected=gen:console-0001 previous=gen:console-0001 target=gen:console-new-0002
	Native generation verification accepted: generation=gen:console-new-0002
	Native generation manager journal prepare: previous=gen:console-0001 target=gen:console-new-0002
	Native update transaction verifies manifest hash: generation=gen:console-new-0002
	Native update transaction verifies store closure: generation=gen:console-new-0002
	Krust generation switch staged: from=gen:console-0001 to=gen:console-new-0002
	generation-manager writes VertexDisk generation metadata: transaction=commit selected=gen:console-new-0002 previous=gen:console-0001 target=gen:console-new-0002
	Krust generation switch accepted: from=gen:console-0001 to=gen:console-new-0002
	Native generation manager journal commit: selected_generation=gen:console-new-0002
	Krust generation switch entering generation: gen:console-new-0002
	Boot generation: gen:console-new-0002
	counter value: 41
	increment -> 42
	counter state policy: preserve
	counter value: 42
	console-shell requests generation-manager rollback
	generation-manager transaction rollback prepare: target=gen:console-0001
	Native generation verification accepted: generation=gen:console-0001
	Native generation manager journal prepare: previous=gen:console-new-0002 target=gen:console-0001
	Krust rollback generation staged: target=gen:console-0001
	generation-manager writes VertexDisk generation metadata: transaction=rollback selected=gen:console-0001 previous=gen:console-new-0002 target=gen:console-0001
		Krust rollback generation accepted: target=gen:console-0001
		Native generation manager journal rollback: failed=gen:console-new-0002 selected_generation=gen:console-0001 reason=activation-failed
		Native generation manager failure detail: service=gen:console-new-0002 dependency=service-readiness policy=known-good-rollback reason=activation-failed
		Krust rollback entering generation: gen:console-0001
	Boot generation: gen:console-0001
	svc:counter has state authority from generation graph
	console-shell observed state clients drained
	Native console shell ok
	'
	        ;;
	    m84|package-import)
		        MANIFEST="$ROOT_DIR/examples/krust-package-import-generation.vertex.json"
		        BOOT_FALLBACK_MANIFEST="$ROOT_DIR/examples/krust-package-import-new-generation.vertex.json"
		        VERTEX_DISK_GRAPH_ONLY_MANIFESTS="$ROOT_DIR/examples/krust-package-import-new-generation.vertex.json"
		        HOSTLESS_BOOT_GENERATIONS=1
	        EXPECT_ACTIVATION_SUCCESS=1
	        USE_SERIAL_PIPE=1
	        SERIAL_INPUT_DELAYED=1
	        SERIAL_INPUT_DELAY_SECONDS=15
	        QEMU_ATTEMPTS=${QEMU_M84_ATTEMPTS:-100}
	        M84_GRAPH_LINK_DIR="$BUILD_DIR/m84-graph-link"
	        cargo run --locked --offline --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- graph-link "$M84_GRAPH_LINK_DIR" "$ROOT_DIR/examples/packages/serial-driver.vertexpkg" "$ROOT_DIR/examples/packages/logd.vertexpkg"
	        if ! grep -Fq '"closureHash": "9ea0d17ec97f6c5f358d9d77df8bc89ddae6541cfa3d2bb07bae7e81ff099dc6"' "$M84_GRAPH_LINK_DIR/store-closure.json"; then
	            echo "M84 host graph-link closure hash mismatch" >&2
	            exit 1
	        fi
	        SERIAL_INPUT='import package pkg:missing-dependency
	import package pkg:excess-authority
	import package pkg:logd
	rollback imported package
	halt
	'
	        required_lines='
	Boot generation: gen:package-import-0001
	KrustBoot fallback generation ready: gen:package-import-new-0002
	package-import ready
	vertex-init observed ready: package-import
	console-driver forwarded serial command: import package pkg:missing-dependency
	console-shell requests package-import missing-dependency validation
	package-import validates missing-dependency fragment before materialization
	package-import rejected missing dependency: capability=cap:missing.database reason=no-provider no candidate install
	package-import negative missing-dependency import aborted before materialization
	console-driver forwarded serial command: import package pkg:excess-authority
	console-shell requests package-import excess-authority validation
	package-import validates excess-authority fragment before materialization
	package-import rejected excess authority: capability=cap:io.com1/write reason=undeclared no candidate install
	package-import negative excess-authority import aborted before materialization
	console-driver forwarded serial command: import package pkg:logd
	console-shell requests package-import import
	native package-import service reads compact graph fragment
	package-import parsed compact typed graph fragment: package=pkg:logd
	package-import verified store-object hash: object=config:logd size=33
	native package import materializes graph delta: add_service=svc:logd add_capability=cap:log.sink
	native package import activates closure service: svc:echo-server
	package-import authority delta accepted: cap:console.output/send,cap:vfs.logd-log-stream/resolve+read,cap:net.udp.9000/listen+bind,cap:log.sink/send,config:logd/read
	native graph-link closure hash: 95afed4d3a94068eade714c3e8ccc7b7b3dac4ed4e847d74d14e00e1f7d62799
	package-import verified canonical closure hash
	package-import registers native graph generation before activation: gen:package-import-new-0002
	generation-manager registers imported graph generation: generation=gen:package-import-new-0002
	generation-manager writes VertexDisk generation metadata: register generation=gen:package-import-new-0002 count=2
	generation-manager imported graph generation registered: generation=gen:package-import-new-0002
	package-import queues candidate generation for activation
	generation-manager install candidate from native graph-store: generation=gen:package-import-new-0002
	Native update transaction verifies store closure: generation=gen:package-import-new-0002
	Krust generation switch accepted: from=gen:package-import-0001 to=gen:package-import-new-0002
	Krust generation switch entering generation: gen:package-import-new-0002
	Boot generation: gen:package-import-new-0002
	vertex-init observed ready: logd
	logd received: hello from echo
	console-driver forwarded serial command: rollback imported package
	console-shell requests generation-manager rollback
	Native generation verification accepted: generation=gen:package-import-0001
	Krust rollback generation accepted: target=gen:package-import-0001
	Krust rollback entering generation: gen:package-import-0001
	Boot generation: gen:package-import-0001
	console-driver forwarded serial command: halt
	Native console shell ok
	'
	        case_forbidden_lines='
	generation-manager registers imported graph generation: generation=gen:reject-missing-dependency
	generation-manager registers imported graph generation: generation=gen:reject-excess-authority
	generation-manager install candidate from native graph-store: generation=gen:reject-missing-dependency
	generation-manager install candidate from native graph-store: generation=gen:reject-excess-authority
	generation-manager transaction abort: reason=unknown-generation generation=gen:package-import-new-0002
	'
	        ;;
	    m85|state-migration)
		        MANIFEST="$ROOT_DIR/examples/krust-state-migration-generation.vertex.json"
		        FALLBACK_MANIFEST="$ROOT_DIR/examples/krust-state-migration-new-generation.vertex.json"
		        BAD_GENERATION_MANIFEST="$ROOT_DIR/examples/krust-state-migration-bad-generation.vertex.json"
	        EXPECT_ACTIVATION_SUCCESS=1
	        USE_SERIAL_PIPE=1
	        SERIAL_INPUT_DELAYED=1
	        SERIAL_INPUT_DELAY_SECONDS=8
	        QEMU_ATTEMPTS=${QEMU_M85_ATTEMPTS:-120}
	        SERIAL_INPUT='state-health
	install generation gen:state-bad
	state-health
	install generation gen:state-new
	state-health
	rollback state migration
	state-health
	halt
	'
	        required_lines='
	Boot generation: gen:state-migration-0001
	KrustBoot fallback generation ready: gen:state-migration-new-0002
	KrustBoot bad generation ready: gen:state-migration-bad-0003
	console-driver forwarded serial command: state-health
	state-health state:counter owner=svc:echo-server schema=counter.v1 generation=gen:state-migration-0001 migration_status=clean last_error=none
	state-policy state:counter storage=vertexdisk-v1 migration=preserve retention=retain-while-referenced sharing=explicit
	state health reports owner schema generation migration status and last error
	console-driver forwarded serial command: install generation gen:state-bad
	console-shell requests generation-manager bad state migration install
	generation-manager install candidate from native graph-store: generation=gen:state-migration-bad-0003
	generation-manager transaction prepare: generation=gen:state-migration-bad-0003
	Native generation manager journal prepare: previous=gen:state-migration-0001 target=gen:state-migration-bad-0003
	Native update transaction verifies manifest hash: generation=gen:state-migration-bad-0003
	Native update transaction verifies store closure: generation=gen:state-migration-bad-0003
	State migration failed: state=state:counter from=counter.v1 to=counter.v3 reason=missing-migrate-policy
	State migration rollback leaves old state readable: state=state:counter
	Native generation manager journal abort: generation=gen:state-migration-bad-0003 reason=state-migration-failed
	Native generation manager failure detail: service=gen:state-migration-bad-0003 dependency=state-schema policy=state-migration reason=state-migration-failed
	Native update transaction selected_generation unchanged: gen:state-migration-0001
	generation-manager transaction abort: reason=stage-failed generation=gen:state-migration-bad-0003
	state-health state:counter owner=svc:echo-server schema=counter.v1 generation=gen:state-migration-0001 migration_status=failed last_error=missing-migrate-policy
	console-driver forwarded serial command: install generation gen:state-new
	console-shell requests generation-manager state migration install
	generation-manager install candidate from native graph-store: generation=gen:state-migration-new-0002
	generation-manager transaction prepare: generation=gen:state-migration-new-0002
	Native generation manager journal prepare: previous=gen:state-migration-0001 target=gen:state-migration-new-0002
	Native update transaction verifies manifest hash: generation=gen:state-migration-new-0002
	Native update transaction verifies store closure: generation=gen:state-migration-new-0002
	State migration plan accepted: state=state:counter from=counter.v1 to=counter.v2 mode=migrate
	State migration journal record: state=state:counter from=counter.v1 to=counter.v2 status=applied-once
	State garbage collection deferred: state=state:scratch retention=retain-while-referenced
	Krust generation switch staged: from=gen:state-migration-0001 to=gen:state-migration-new-0002
	Krust generation switch accepted: from=gen:state-migration-0001 to=gen:state-migration-new-0002
	Native generation manager journal commit: selected_generation=gen:state-migration-new-0002
	Krust generation switch entering generation: gen:state-migration-new-0002
	Boot generation: gen:state-migration-new-0002
	state-health state:counter owner=svc:echo-server schema=counter.v2 generation=gen:state-migration-new-0002 migration_status=clean last_error=none
	state-policy state:counter storage=vertexdisk-v1 migration=migrate retention=retain-while-referenced sharing=explicit
	console-driver forwarded serial command: rollback state migration
	console-shell requests generation-manager rollback
	generation-manager transaction rollback prepare: target=gen:state-migration-0001
	Native generation manager journal prepare: previous=gen:state-migration-new-0002 target=gen:state-migration-0001
	Krust state rollback policy: state=state:counter mode=preserve action=preserve-current from=counter.v2 to=counter.v1
	State rollback journal record: state=state:counter from=counter.v2 to=counter.v1 status=policy-applied
	Krust rollback generation staged: target=gen:state-migration-0001
	Krust rollback generation accepted: target=gen:state-migration-0001
	Krust rollback entering generation: gen:state-migration-0001
	Boot generation: gen:state-migration-0001
	state-health state:counter owner=svc:echo-server schema=counter.v1 generation=gen:state-migration-0001 migration_status=clean last_error=none
	console-driver forwarded serial command: halt
	Native console shell ok
	console-shell observed state clients drained
	'
	        case_forbidden_lines='
	Krust generation switch entering generation: gen:state-migration-bad-0003
	State migration journal record: state=state:scratch from=scratch.v1 to=scratch.v2 status=applied-once
	State garbage collection removed unreferenced state: state=state:scratch
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
proc=block-driver cap[10] vfs-root=cap:vfs.block-dev-blk0 root=/dev/device:virtio-blk0 rights=read|resolve
proc=block-driver cap[11] pci-device=device:virtio-blk0 kind=virtio-blk-pci rights=control
proc=block-driver cap[12] virtio-device=device:virtio-blk0 transport=virtio-pci-io rights=control
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
KrustBoot vfs roots: 8
VFS state volume mounted: state=state:counter path=/state/counter source=vertex-state
VFS state volume value file mounted: state=state:counter path=/state/counter/value source=vertex-state
VFS state volume mounted: state=state:scratch path=/state/scratch source=vertex-state
VFS state volume value file mounted: state=state:scratch path=/state/scratch/value source=vertex-state
proc=echo cap[4] namespace=cap:namespace.echo rights=resolve
proc=echo cap[5] vfs-root=cap:vfs.echo-state-a root=/state/a rights=read|resolve
proc=echo cap[6] vfs-root=cap:vfs.echo-state-writer root=/state rights=read|write|create|unlink|rename|mount|resolve
proc=echo cap[7] vfs-root=cap:vfs.echo-state-control root=/state/counter/control rights=control|resolve
proc=reader-service cap[3] namespace=cap:namespace.reader rights=resolve
Namespace resolve accepted: proc=echo namespace=cap:namespace.echo path=/state/a
service A namespace contains /state/a
Namespace resolve rejected: proc=echo namespace=cap:namespace.echo path=/state/b
service A cannot resolve /state/b
service-local VFS root opens /state/a
per-process mount namespace maps /a to /state/a
service-local VFS root rejects /state/b
mounted state volume appears at /state/counter
generic state volume uses VFS service transaction
mounted state volume value uses VFS service transaction
VFS state transaction request: proc=echo state=state:counter op=stat file=value
VFS state transaction wake: proc=echo file=value op=stat result=64
vertex-state serves VFS state stat
service-backed state value stat reports durable length
VFS root derive accepted: proc=echo source=6 target=25 root=/state/sub rights=read|write|create|unlink|rename|mount|resolve
directory cap attenuates into read-only subtree authority
service with no lookup authority cannot resolve a child path
VFS namespace root resolved: proc=echo root=/state
VFS readdir accepted: proc=echo dir=state entry=a
VFS directory handle lists child vnode entries
VFS mount requires explicit mount authority
VFS mount accepted: proc=echo path=/mnt canonical=/state/mnt source=volatilefs
SYS_VFS_UNMOUNT returned STATUS_VFS_BUSY
VFS unmount accepted: proc=echo path=/mnt canonical=/state/mnt
VFS mount object creates busy-checks and unmounts volatile root
SYS_VFS_CREATE returned STATUS_VFS_PERMISSION
VFS create accepted: proc=echo path=/new
VFS unlink accepted: proc=echo path=/new canonical=/state/new
manifest-granted VFS writer can create write read and unlink a file
SYS_VFS_RENAME returned STATUS_VFS_PERMISSION
VFS rename requires explicit rename authority
VFS rename accepted: proc=echo old=/rename-old new=/rename-new canonical_old=/state/rename-old canonical_new=/state/rename-new vnode=
VFS rename moves volatile file and preserves vnode identity
VFS open-create accepted: proc=echo path=/opened
VFS unlink accepted: proc=echo path=/opened canonical=/state/opened
VFS open-create creates truncates and appends via native flags
SYS_VFS_LOCK returned STATUS_VFS_BUSY
VFS advisory locks reject conflicts and release on close
VFS open-create quota failure rolls back vnode
Native service activation ok
'
        ;;
    m75|vfs-blocking)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        QEMU_ATTEMPTS=${QEMU_M75_ATTEMPTS:-60}
        required_lines='
KrustBoot vfs roots: 8
Endpoint table entries: 16
endpoint[10] id=11 name=state-vfs-request
endpoint[11] id=12 name=state-vfs-reply
endpoint[12] id=13 name=vertexfs-device-request
endpoint[13] id=14 name=vertexfs-device-reply
endpoint[14] id=15 name=generation-metadata-block-request
endpoint[15] id=16 name=generation-metadata-block-reply
Native VFS state request grant: process=vertex-state endpoint=state-vfs-request rights=receive
Native VFS state reply grant: process=vertex-state endpoint=state-vfs-reply rights=send
Native VertexFS device request grant: process=block-driver endpoint=vertexfs-device-request rights=receive
Native VertexFS device reply grant: process=block-driver endpoint=vertexfs-device-reply rights=send
proc=vertex-state cap[6] endpoint=state-vfs-reply rights=send
proc=vertex-state cap[7] endpoint=state-vfs-request rights=receive
proc=block-driver cap[13] endpoint=vertexfs-device-request rights=receive
proc=block-driver cap[14] endpoint=vertexfs-device-reply rights=send
proc=block-driver cap[16] endpoint=generation-metadata-block-request rights=receive
proc=block-driver cap[17] endpoint=generation-metadata-block-reply rights=send
proc=logd cap[4] vfs-root=cap:vfs.logd-log-stream root=/proc/log-stream rights=read|resolve
VFS open accepted: proc=logd file=log-stream
VFS read blocked: proc=logd
VFS pipe wake reader: proc=logd file=log-stream
VFS pipe read blocks until writer log
VFS state volume value file mounted: state=state:counter path=/state/counter/value source=vertex-state
VFS state volume control file mounted: state=state:counter path=/state/counter/control source=vertex-state
VFS state volume value file mounted: state=state:scratch path=/state/scratch/value source=vertex-state
VFS state volume control file mounted: state=state:scratch path=/state/scratch/control source=vertex-state
VFS state transaction request: proc=echo state=state:scratch op=write file=value
VFS state transaction request: proc=echo state=state:scratch op=read file=value
VFS state transaction request: proc=echo state=state:scratch op=stat file=value
generic state volume uses VFS service transaction
VFS state transaction request: proc=echo state=state:counter op=write file=value
VFS state transaction wake: proc=echo file=value op=write result=2
vertex-state serves VFS state write
VFS state transaction request: proc=echo state=state:counter op=read file=value
VFS state transaction wake: proc=echo file=value op=read result=2
vertex-state serves VFS state read
mounted state volume value uses VFS service transaction
VFS state transaction request: proc=echo state=state:counter op=stat file=value
VFS state transaction wake: proc=echo file=value op=stat result=64
vertex-state serves VFS state stat
service-backed state value stat reports durable length
VFS state transaction request: proc=echo state=state:counter op=control file=control
VFS state transaction wake: proc=echo file=control op=control result=1
Native service activation ok
'
        ;;
    m76|directory-metadata)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
VFS rename moves volatile file and preserves vnode identity
VFS stat reports monotonic metadata version and link count
VFS rmdir rejects non-empty directory
VFS mkdir creates directories and rmdir removes empty directories
VFS unlink of open file keeps existing handle readable until close
VFS hard links share volatile file backing and report link count
VFS hard link metadata version follows shared backing writes
VFS hard link metadata version follows link count changes
VFS hard links cannot cross filesystem boundaries
VFS rename cannot cross filesystem boundaries
VFS hard links cannot cross volatile mount instances
VFS rename cannot cross volatile mount instances
long VFS paths and components are rejected before allocation
path traversal cannot escape service namespace root
Native service activation ok
'
        ;;
    m77|cache-writeback)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
vertex-state block cache hit
vertex-state block cache writeback clean
vertex-state cache inspect dirty=0 pinned=0 writeback_errors=0
vertex-state block cache clean eviction under pressure
vertex-state block cache dirty pages are not evicted
vertex-state block cache writeback error leaves dirty page dirty
vertex-state block cache writeback error accounting increments
vertex-state writes state volume to disk
vertex-state serves VFS state write
vertex-state serves VFS state read
Native service activation ok
'
        ;;
    m78|vertexfs-v1)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
VertexFS v1 image module accepted: bytes=32768
block-driver reads VertexFS device image section
block-driver writes VertexFS device image section
block-driver writes VertexFS fsync sector
VertexFS v1 superblock accepted: generation=gen:hello-0001 feature_flags=metadata-v1
VertexFS v1 mounted: path=/fs source=vertexfs
VertexFS v1 directory record verified: path=/fs/app
VertexFS v1 declared file mounted: path=/fs/app/a
proc=model-reader cap[4] vfs-root=cap:vfs.model-reader-vertexfs root=/fs/app rights=read|write|create|resolve
VFS namespace root resolved: proc=model-reader root=/fs/app
mount namespace root exposes declared VertexFS app tree
model-reader VertexFS namespace root maps /a to /fs/app/a
VertexFS v1 declared file read through VFS
VertexFS v1 fsync device transaction committed: proc=model-reader inode=4 sectors=
VertexFS v1 declared file fsync journal readback ok
VFS open-create accepted: proc=model-reader path=/created
VertexFS v1 fsync device transaction committed: proc=model-reader inode=5 sectors=
VertexFS v1 dynamic create write fsync readback ok
VFS open-create accepted: proc=model-reader path=/created2
VertexFS v1 fsync device transaction committed: proc=model-reader inode=6 sectors=
VertexFS v1 second dynamic create write fsync readback ok
VFS open-create accepted: proc=model-reader path=/created3
VertexFS v1 fsync device transaction committed: proc=model-reader inode=7 sectors=
VertexFS v1 third dynamic create write fsync readback ok
VFS open-create accepted: proc=model-reader path=/created4
VertexFS v1 fsync device transaction committed: proc=model-reader inode=8 sectors=
VertexFS v1 expanded metadata create beyond old table capacity ok
VFS open-create accepted: proc=model-reader path=/created11
VertexFS v1 fsync device transaction committed: proc=model-reader inode=15 sectors=
VertexFS v1 expanded metadata fills dynamic inode 15
VertexFS v1 dynamic create returns no space at expanded metadata capacity
Native service activation ok
'
        ;;
    m78-bad-superblock|vertexfs-bad-superblock)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        VERTEXFS_CORRUPT=bad-superblock
        required_lines='
VertexFS v1 image module accepted: bytes=32768
Krust VertexFS v1 image rejected: bad superblock
Native runtime init failed from KrustBoot manifest
'
        ;;
    m78-journal-replay|vertexfs-journal-replay)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        VERTEXFS_CORRUPT=interrupted-journal
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
VertexFS v1 image module accepted: bytes=32768
VertexFS v1 superblock accepted: generation=gen:hello-0001 feature_flags=metadata-v1
VertexFS v1 mounted: path=/fs source=vertexfs
VertexFS v1 journal replayed: inode=4 outcome=new
VertexFS v1 declared file read through VFS
VertexFS v1 journal replay read returned new file
Native service activation ok
'
        ;;
    m78-journal-checkpoint-after-journal|vertexfs-journal-checkpoint-after-journal)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        VERTEXFS_CORRUPT=journal-checkpoint-after-journal
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
VertexFS v1 image module accepted: bytes=32768
VertexFS v1 superblock accepted: generation=gen:hello-0001 feature_flags=metadata-v1
VertexFS v1 mounted: path=/fs source=vertexfs
VertexFS v1 journal replayed: inode=4 outcome=new
VertexFS v1 declared file read through VFS
VertexFS v1 journal replay read returned new file
Native service activation ok
'
        ;;
    m78-journal-checkpoint-after-data|vertexfs-journal-checkpoint-after-data)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        VERTEXFS_CORRUPT=journal-checkpoint-after-data
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
VertexFS v1 image module accepted: bytes=32768
VertexFS v1 superblock accepted: generation=gen:hello-0001 feature_flags=metadata-v1
VertexFS v1 mounted: path=/fs source=vertexfs
VertexFS v1 journal replayed: inode=4 outcome=new
VertexFS v1 declared file read through VFS
VertexFS v1 journal replay read returned new file
Native service activation ok
'
        ;;
    m78-journal-checkpoint-after-inode|vertexfs-journal-checkpoint-after-inode)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        VERTEXFS_CORRUPT=journal-checkpoint-after-inode
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
VertexFS v1 image module accepted: bytes=32768
VertexFS v1 superblock accepted: generation=gen:hello-0001 feature_flags=metadata-v1
VertexFS v1 mounted: path=/fs source=vertexfs
VertexFS v1 journal replayed: inode=4 outcome=new
VertexFS v1 declared file read through VFS
VertexFS v1 journal replay read returned new file
Native service activation ok
'
        ;;
    m78-post-sync-remount|vertexfs-post-sync-remount)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        VERTEXFS_UPDATE_APP_A_PAYLOAD="$BUILD_DIR/vertexfs-app-a-post-sync.txt"
        mkdir -p "$BUILD_DIR"
        printf 'vertexfs:a=3\n' >"$VERTEXFS_UPDATE_APP_A_PAYLOAD"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
VertexFS v1 image module accepted: bytes=32768
VertexFS v1 superblock accepted: generation=gen:hello-0001 feature_flags=metadata-v1
VertexFS v1 mounted: path=/fs source=vertexfs
VertexFS v1 declared file read through VFS
VertexFS v1 post-sync image read returned committed file
Native service activation ok
'
        ;;
    m78-fsync-fault|vertexfs-fsync-fault)
        MANIFEST="$ROOT_DIR/examples/krust-vertexfs-fsync-fault-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
Boot generation: gen:vertexfs-fsync-fault-0001
proc=block-driver cap[15] store-object=store:vertexfs-fsync-fault-token rights=read
proc=model-reader cap[5] store-object=store:vertexfs-fsync-fault-token rights=read
VFS open accepted: proc=block-driver file=store:vertexfs-fsync-fault-token
block-driver writes VertexFS fsync sector
block-driver fault injection exits during VertexFS fsync
VertexFS v1 fsync device transaction aborted: proc=model-reader
VertexFS v1 fsync block-driver fault returns unsupported
VertexFS v1 fsync fault keeps runtime dirty file readable
restart policy = on-failure
vertex-init restarts block-driver once
Krust process restart reload: proc=block-driver
Native service activation ok
'
        case_forbidden_lines='
VertexFS v1 fsync device transaction committed: proc=model-reader
'
        ;;
    m79|mount-namespaces)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
process[8] id=
name=model-reader state=declared
mount_root=/fs/app
mount[0] path=/declared-ro source=/ flags=bind|read-only
proc=model-reader cap[4] vfs-root=cap:vfs.model-reader-vertexfs root=/fs/app rights=read|write|create|resolve
proc=echo cap[6] vfs-root=cap:vfs.echo-state-writer root=/state rights=read|write|create|unlink|rename|mount|resolve
Krust declared mount snapshot restored: proc=echo path=/declared-ro canonical=/state/declared-ro source=/ canonical_source=/state flags=bind|read-only
VFS namespace root resolved: proc=model-reader root=/fs/app
model-reader VertexFS namespace root maps /a to /fs/app/a
per-process mount namespace maps /a to /state/a
VFS filesystem service file mounted: path=/state/service-report source=servicefs
VFS open accepted: proc=echo file=service-report
VFS filesystem service request: proc=echo file=service-report
vertex-state serves VFS filesystem service report
VFS filesystem service transaction wake: proc=echo file=service-report op=service-read
service-backed filesystem file rejects write opens
service-backed filesystem file read through mount namespace
declared mount snapshot exposes read-only alias
VFS mount requires explicit mount authority
VFS mount object creates busy-checks and unmounts volatile root
VFS bind mount accepted: proc=echo path=/ro canonical=/state/ro source=/state flags=bind|read-only
VFS bind unmount rejects busy mounted subtree
read-only bind mount rejects write through alias
VFS bind mount accepted: proc=echo path=/ro/nested canonical=/state/ro/nested source=/state/ro flags=bind|read-only
nested bind mount inherits read-only source flag
VFS bind mount accepted: proc=echo path=/rw canonical=/state/rw source=/state flags=bind
writable bind mount propagates writes and metadata through alias
VFS bind mount accepted: proc=echo path=/restart-bind canonical=/state/restart-bind source=/state flags=bind
echo leaves dynamic bind mount for restart cleanup
Krust process dynamic bind mounts reaped: proc=echo mounts=1
Krust process declared mount snapshot reaped: proc=echo mounts=1
echo restart restored declared mount namespace root /state
echo restart restored declared mount table alias /declared-ro
echo restart did not inherit previous dynamic bind mount
Native service activation ok
'
        ;;
    m80|vfs-coordination)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        QEMU_ATTEMPTS=40
        required_lines='
VFS poll reports empty pipe not readable
VFS poll on file and pipe handles respects handle authority
directory watcher receives create rename and unlink events in order
VFS lock accepted: proc=echo file=range-lock description=
range=0+4 mode=exclusive
range=4+4 mode=exclusive
byte-range locks reject overlapping writes and allow disjoint ranges
VFS poll reports readiness only for authorized handle events
Native service activation ok
'
        ;;
    m81|vfs-crash-security-soak)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        QEMU_ATTEMPTS=40
        required_lines='
revoked directory authority prevents new opens but preserves existing handle semantics
revoked file authority prevents handle duplication and new opens
M81 100-cycle file churn returns to baseline handle vnode and lock counts
path traversal integer overflow and bad user buffers are rejected before side effects
service-backed filesystem file read through mount namespace
VertexFS v1 fsync device transaction committed: proc=model-reader
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
state VFS write bounds and owner checks ok
VFS state transaction request: proc=counter-service state=state:counter op=write file=value
vertex-state write bounds enforced
block-driver propagates request completion to client
update commit interrupted before final pointer leaves previous generation bootable
block-driver fault during request fails client request without kernel fault
Native service activation ok
'
REBOOT_REQUIRED_LINES='
reboot preserves state value
reboot preserves state:scratch value
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
    m70|interrupt-routing)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
block-driver sleeps on virtio-blk IRQ instead of polling for completion
netstack sleeps on virtio-net IRQ instead of polling for RX completion
IRQ wait accepted: proc=block-driver interrupt-line=cap:irq.virtio-blk0 line=11
IRQ wait timeout: proc=block-driver
SYS_IRQ_WAIT rejected: bad capability
inspect reports IRQ line, owner, pending count, waiters, and spurious count
Native service activation ok
'
        ;;
    m71|dma-ownership)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
DMA map accepted: proc=block-driver dma-region=cap:dma.virtio-blk0
DMA mapping released: proc=block-driver
driver exit releases DMA buffers and user DMA mappings
DMA map twice for the same object returns the same mapping without leaking frames
unauthorized service cannot map or inspect another driver'"'"'s DMA region
Native service activation ok
'
        ;;
    m72|virtio-recovery)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
Virtio driver report accepted: proc=block-driver virtio-device=device:virtio-blk0
virtio-net RX waits for interrupt-backed completion
Virtio kernel device ownership released: proc=netstack virtio-device=device:virtio-net0
Virtio device ownership released: proc=block-driver virtio-device=device:virtio-blk0
inspect reports virtio queue state, last error, reset count, and owner process
virtio-rng timeout returns a clean syscall error
virtio-net RX timeout does not wedge netstack
M61 virtio typed device syscalls reject mismatched device IDs
Native service activation ok
'
        ;;
    m73|device-fault-gate)
        MANIFEST="$ROOT_DIR/examples/krust-console-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        USE_SERIAL_PIPE=1
        SERIAL_INPUT='devices
halt
'
        required_lines='
Vertex shell ready
console-driver forwarded serial command: devices
last device failure: owner=
appliance shell reports last device failure reason and owner process
block-driver fault during request fails client request without kernel fault
netstack fault releases virtio-net IRQ/DMA ownership and leaves other services running
release gate checks memory/object/cap/DMA/IRQ leak deltas after fault injection
Native service activation ok
'
        ;;
    m38|introspection)
        MANIFEST="$ROOT_DIR/examples/krust-inspect-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
Boot generation: gen:inspect-0001
vertex-init delegates graph inspect authority to vertex-inspect
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
    m82|native-graph-store)
        MANIFEST="$ROOT_DIR/examples/krust-inspect-generation.vertex.json"
        EXPECT_ACTIVATION_SUCCESS=1
        required_lines='
KrustBoot Manifest v1 records: 9
VertexDisk graph-store object accepted: generation=gen:inspect-0001
Native graph store loaded from VertexDisk: generation=gen:inspect-0001
block-driver reads VertexDisk graph-store section
vertex-inspect runtime report captured
vertex-inspect graph-store header parsed
vertex-inspect generation graph: gen:inspect-0001
vertex-inspect native graph-store query ok
native graph query returns generation service store-object state and device nodes
runtime process and capability records point back to native graph nodes
Native introspection service ok
Native service activation ok
'
        ;;
    m82-vertexdisk-graph-corrupt|vertexdisk-graph-store-corrupt)
        MANIFEST="$ROOT_DIR/examples/krust-inspect-generation.vertex.json"
        VERTEX_DISK_CORRUPT=graph-store
        required_lines='
Krust native graph-store checksum mismatch
Native runtime init failed from KrustBoot manifest
Native service activation failed
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
    manifest-graph-store-checksum)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        KRUSTBOOT_CORRUPT=graph-store-checksum
        required_lines='
KrustBoot manifest parse failed: graph-store checksum mismatch
KrustBoot manifest unavailable
'
        ;;
    manifest-graph-store-record)
        MANIFEST="$ROOT_DIR/examples/hello-generation.vertex.json"
        KRUSTBOOT_CORRUPT=graph-store-record
        required_lines='
KrustBoot manifest parse failed: invalid graph record
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
        echo "usage: scripts/krust-test.sh <m13|m14|valid-activation|manifest-cycle|bad-cap|readiness|readiness-timeout|rollback|store-state-services|timer|preemption|m30|user-fault|m31|restart|manifest-v1|cap-lifecycle|typed-arenas|quotas|m32|io-substrate|m33|serial-driver|m34|block-driver|m35|store-service|m36|state-service|m37|generation-switch|m38|introspection|m40|directed-ipc|m41|console-shell|m42|virtio-block|m42-driver-fault|block-driver-fault|m43|vertexdisk|m43-bad-superblock|vertexdisk-bad-superblock|m44|boot-manager|m45|store-verification|m46|native-update|m47|store-executables|m47-corrupt-executable|store-executable-corruption|m48|dynamic-process|m49|config-objects|m49-config-corrupt|config-hash-mismatch|m50|secrets|m54|appliance|m55|driver-framework|m56|virtio-device-stack|m57|networking-v0|m59|namespace-service|m60|policy-typed|m61|abi-authority-hardening|m62|storage-durability|m62-journal-replay|storage-journal-replay|m62-corrupt-journal|storage-corrupt-journal|m63|network-boundary|m64|supervisor-lifecycle|m66|memory-lifecycle|m67|address-space-teardown|m68|failure-atomicity|m69|memory-pressure|m70|interrupt-routing|m71|dma-ownership|m72|virtio-recovery|m73|device-fault-gate|m75|vfs-blocking|m76|directory-metadata|m77|cache-writeback|m78|vertexfs-v1|m78-bad-superblock|vertexfs-bad-superblock|m78-journal-replay|vertexfs-journal-replay|m78-journal-checkpoint-after-journal|vertexfs-journal-checkpoint-after-journal|m78-journal-checkpoint-after-data|vertexfs-journal-checkpoint-after-data|m78-journal-checkpoint-after-inode|vertexfs-journal-checkpoint-after-inode|m78-post-sync-remount|vertexfs-post-sync-remount|m78-fsync-fault|vertexfs-fsync-fault|m79|mount-namespaces|m80|vfs-coordination|m81|vfs-crash-security-soak|m82|native-graph-store|m82-vertexdisk-graph-corrupt|vertexdisk-graph-store-corrupt|m83|generation-manager|m83-hostless|generation-manager-hostless|m83-power-prepare|m83-power-commit|m83-power-rollback|m84|package-import|m85|state-migration|manifest-truncated|manifest-bad-magic|manifest-raw-compact|manifest-old-compact-magic|manifest-graph-store-checksum|manifest-graph-store-record|manifest-unsupported-version|manifest-oob-record|manifest-missing-provider>" >&2
        exit 2
        ;;
esac

if [ "$CASE" = "m78" ] || [ "$CASE" = "vertexfs-v1" ]; then
    VERTEXFS_IMAGE="$BUILD_DIR/krust.vertexfs"
    VERTEXFS_IMAGE_REPRO="$BUILD_DIR/krust-repro.vertexfs"
    VERTEXFS_BAD_SUPERBLOCK="$BUILD_DIR/krust-bad-superblock.vertexfs"
    VERTEXFS_BAD_DIRECTORY="$BUILD_DIR/krust-bad-directory.vertexfs"
    VERTEXFS_OVERLAP="$BUILD_DIR/krust-overlap.vertexfs"
    VERTEXFS_FREE_OVERLAP="$BUILD_DIR/krust-free-overlap.vertexfs"
    VERTEXFS_INTERRUPTED_JOURNAL="$BUILD_DIR/krust-interrupted-journal.vertexfs"
    VERTEXFS_CHECKPOINT_AFTER_JOURNAL="$BUILD_DIR/krust-checkpoint-after-journal.vertexfs"
    VERTEXFS_CHECKPOINT_AFTER_DATA="$BUILD_DIR/krust-checkpoint-after-data.vertexfs"
    VERTEXFS_CHECKPOINT_AFTER_INODE="$BUILD_DIR/krust-checkpoint-after-inode.vertexfs"
    VERTEXFS_UPDATED="$BUILD_DIR/krust-updated.vertexfs"
    VERTEXFS_UPDATE_PAYLOAD="$BUILD_DIR/krust-updated-app-a.txt"
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- create-vertexfs "$VERTEXFS_IMAGE" "$MANIFEST"
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- inspect-vertexfs "$VERTEXFS_IMAGE"
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- verify-vertexfs "$VERTEXFS_IMAGE"
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- create-vertexfs "$VERTEXFS_IMAGE_REPRO" "$MANIFEST"
    cmp "$VERTEXFS_IMAGE" "$VERTEXFS_IMAGE_REPRO"
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- corrupt-vertexfs bad-superblock "$VERTEXFS_IMAGE" "$VERTEXFS_BAD_SUPERBLOCK"
    if cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- verify-vertexfs "$VERTEXFS_BAD_SUPERBLOCK"; then
        echo "VertexFS bad superblock unexpectedly verified" >&2
        exit 1
    fi
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- corrupt-vertexfs bad-directory "$VERTEXFS_IMAGE" "$VERTEXFS_BAD_DIRECTORY"
    if cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- verify-vertexfs "$VERTEXFS_BAD_DIRECTORY"; then
        echo "VertexFS bad directory unexpectedly verified" >&2
        exit 1
    fi
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- corrupt-vertexfs overlapping-extents "$VERTEXFS_IMAGE" "$VERTEXFS_OVERLAP"
    if cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- verify-vertexfs "$VERTEXFS_OVERLAP"; then
        echo "VertexFS overlapping extents unexpectedly verified" >&2
        exit 1
    fi
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- corrupt-vertexfs free-space-overlap "$VERTEXFS_IMAGE" "$VERTEXFS_FREE_OVERLAP"
    if cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- verify-vertexfs "$VERTEXFS_FREE_OVERLAP"; then
        echo "VertexFS corrupt free-space metadata unexpectedly verified" >&2
        exit 1
    fi
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- corrupt-vertexfs interrupted-journal "$VERTEXFS_IMAGE" "$VERTEXFS_INTERRUPTED_JOURNAL"
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- verify-vertexfs "$VERTEXFS_INTERRUPTED_JOURNAL"
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- corrupt-vertexfs journal-checkpoint-after-journal "$VERTEXFS_IMAGE" "$VERTEXFS_CHECKPOINT_AFTER_JOURNAL"
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- verify-vertexfs "$VERTEXFS_CHECKPOINT_AFTER_JOURNAL"
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- corrupt-vertexfs journal-checkpoint-after-data "$VERTEXFS_IMAGE" "$VERTEXFS_CHECKPOINT_AFTER_DATA"
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- verify-vertexfs "$VERTEXFS_CHECKPOINT_AFTER_DATA"
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- corrupt-vertexfs journal-checkpoint-after-inode "$VERTEXFS_IMAGE" "$VERTEXFS_CHECKPOINT_AFTER_INODE"
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- verify-vertexfs "$VERTEXFS_CHECKPOINT_AFTER_INODE"
    printf 'vertexfs:a=3\n' >"$VERTEXFS_UPDATE_PAYLOAD"
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- update-vertexfs-file "$VERTEXFS_IMAGE" "$VERTEXFS_UPDATED" /app/a "$VERTEXFS_UPDATE_PAYLOAD"
    cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- verify-vertexfs "$VERTEXFS_UPDATED"
fi

case "$VERTEXFS_CORRUPT" in
    journal-checkpoint-after-journal|journal-checkpoint-after-data|journal-checkpoint-after-inode)
        VERTEXFS_CHECKPOINT_BASE="$BUILD_DIR/krust-checkpoint-base.vertexfs"
        VERTEXFS_CHECKPOINT_IMAGE="$BUILD_DIR/krust-$VERTEXFS_CORRUPT.vertexfs"
        cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- create-vertexfs "$VERTEXFS_CHECKPOINT_BASE" "$MANIFEST"
        cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- corrupt-vertexfs "$VERTEXFS_CORRUPT" "$VERTEXFS_CHECKPOINT_BASE" "$VERTEXFS_CHECKPOINT_IMAGE"
        cargo run --locked --quiet --manifest-path "$ROOT_DIR/crates/vertexctl/Cargo.toml" -- verify-vertexfs "$VERTEXFS_CHECKPOINT_IMAGE"
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
if [ "$HOSTLESS_BOOT_GENERATIONS" -eq 0 ]; then
    if [ -z "$BOOT_FALLBACK_MANIFEST" ]; then
        BOOT_FALLBACK_MANIFEST="$FALLBACK_MANIFEST"
    fi
    if [ -z "$BOOT_BAD_GENERATION_MANIFEST" ]; then
        BOOT_BAD_GENERATION_MANIFEST="$BAD_GENERATION_MANIFEST"
    fi
fi

(cd "$KRUST_DIR" && make iso VERTEX_MANIFEST="$MANIFEST" FALLBACK_MANIFEST="$FALLBACK_MANIFEST" BAD_GENERATION_MANIFEST="$BAD_GENERATION_MANIFEST" BOOT_FALLBACK_MANIFEST="$BOOT_FALLBACK_MANIFEST" BOOT_BAD_GENERATION_MANIFEST="$BOOT_BAD_GENERATION_MANIFEST" VERTEX_DISK_GRAPH_ONLY_MANIFESTS="$VERTEX_DISK_GRAPH_ONLY_MANIFESTS" KRUSTBOOT_CORRUPT="$KRUSTBOOT_CORRUPT" VERTEX_DISK_CORRUPT="$VERTEX_DISK_CORRUPT" VERTEXFS_CORRUPT="$VERTEXFS_CORRUPT" VERTEXFS_UPDATE_APP_A_PAYLOAD="$VERTEXFS_UPDATE_APP_A_PAYLOAD")

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
        line=$(printf '%s\n' "$line" | sed 's/^[[:space:]]*//')
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
        line=$(printf '%s\n' "$line" | sed 's/^[[:space:]]*//')
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
