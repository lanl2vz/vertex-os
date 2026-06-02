#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd "$(dirname "$0")/.." && pwd)
KRUST_DIR=${KRUST_DIR:-"$ROOT_DIR/kernel/krust"}
BUILD_DIR=${BUILD_DIR:-"$KRUST_DIR/build"}
ISO_IMAGE=${ISO_IMAGE:-"$BUILD_DIR/krust.iso"}
BLOCK_IMAGE=${BLOCK_IMAGE:-"$BUILD_DIR/krust-block.img"}
SERIAL_LOG=${SERIAL_LOG:-"$BUILD_DIR/serial.log"}
QEMU=${QEMU:-qemu-system-x86_64}
QEMU_EXTRA=${QEMU_EXTRA:-"-object rng-random,filename=/dev/urandom,id=vertexrng -device virtio-rng-pci,rng=vertexrng,disable-modern=on -netdev user,id=vertexnet -device virtio-net-pci,netdev=vertexnet,mac=52:54:00:12:34:56,disable-modern=on"}
QEMU_MACHINE=${QEMU_MACHINE:-}
QEMU_BLOCK=${QEMU_BLOCK:-"-drive if=none,id=vertexblk,file=$BLOCK_IMAGE,format=raw -device virtio-blk-pci,drive=vertexblk,disable-modern=on,queue-size=8"}
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
    $QEMU_MACHINE \
    $QEMU_BLOCK \
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
KrustBoot endpoints: 10
KrustBoot grants: 64
KrustBoot store objects: 14
KrustBoot state volumes: 2
state_volume[0] id=state:counter
state_volume[1] id=state:scratch
KrustBoot network ports: 1
KrustBoot io port ranges: 3
KrustBoot mmio regions: 0
KrustBoot interrupt lines: 1
KrustBoot dma regions: 1
KrustBoot pci devices: 4
KrustBoot virtio devices: 4
KrustBoot namespaces: 2
KrustBoot vfs roots: 7
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
endpoint[4] name=vertex-store-block-request
endpoint[5] name=vertex-state-block-request
endpoint[6] name=vertex-store-block-reply
endpoint[7] name=vertex-state-block-reply
endpoint[8] name=store-hello-text-request
endpoint[9] name=model-reader-store-reply
grant[0] process=vertex-init cap[1] endpoint=serial-log rights=send
grant[1] process=vertex-init cap[3] endpoint=readiness rights=receive
process=serial-driver cap[0] endpoint=serial-console rights=receive
process=logd cap[0] endpoint=log-sink rights=receive
process=block-driver cap[0] endpoint=vertex-store-block-request rights=receive
process=block-driver cap[3] endpoint=vertex-state-block-request rights=receive
process=vertex-store cap[3] endpoint=vertex-store-block-reply rights=receive
process=vertex-store cap[0] endpoint=store-hello-text-request rights=receive
process=model-reader cap[0] endpoint=model-reader-store-reply rights=receive
process=vertex-state cap[0] endpoint=vertex-state-block-reply rights=receive
process=counter-service cap[0] vfs-root=cap:vfs.counter-state rights=read|write|resolve
process=reader-service cap[0] vfs-root=cap:vfs.state-reader-state rights=read|resolve
process=echo cap[7] vfs-root=cap:vfs.echo-state-control rights=control|resolve
process=serial-driver cap[3] io-port=cap:io.com1 rights=read|write
process=block-driver cap[6] io-port=cap:io.pci-config rights=read|write
process=block-driver cap[7] interrupt-line=cap:irq.virtio-blk0 rights=listen
process=block-driver cap[8] dma-region=cap:dma.virtio-blk0 rights=read|write|map
process=block-driver cap[9] io-port=cap:io.virtio-blk0 rights=read|write
process=block-driver cap[10] vfs-root=cap:vfs.block-dev-blk0 rights=read|resolve
process=block-driver cap[11] pci-device=device:virtio-blk0 rights=control
process=block-driver cap[12] virtio-device=device:virtio-blk0 rights=control
process=serial-driver cap[5] virtio-device=device:virtio-console0 rights=control
process=netstack cap[3] virtio-device=device:virtio-rng0 rights=control
process=netstack cap[5] virtio-device=device:virtio-net0 rights=control
process=netstack cap[6] network-port=cap:net.udp.9000 rights=control
process=echo cap[3] network-port=cap:net.udp.9000 rights=bind|listen
process=echo cap[4] namespace=cap:namespace.echo rights=resolve
process=reader-service cap[3] namespace=cap:namespace.reader rights=resolve
process=timer-service cap[0] timer=monotonic-timer rights=control
network_port[0] id=cap:net.udp.9000
io_port[0] id=cap:io.com1 base=0x00000000000003f8 length=0x0000000000000008
io_port[1] id=cap:io.pci-config base=0x0000000000000cf8 length=0x0000000000000008
io_port[2] id=cap:io.virtio-blk0 base=0x000000000000c000 length=0x0000000000001000
interrupt_line[0] id=cap:irq.virtio-blk0 line=11
dma_region[0] id=cap:dma.virtio-blk0 base=
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
Kernel heap arena allocation ok
Typed endpoint arena created 32 endpoints
Typed process arena created 32 processes
Typed arena free and reuse ok
Typed arena allocation failure returned controlled error
Typed object arenas no silent overwrite ok
IPC FIFO regression: queued sends preserve FIFO order
IPC FIFO regression: queue-full send rejected
IPC FIFO regression: receiver-specific dequeue preserves eligible ordering
IPC FIFO regression: multiple blocked receivers match eligible messages
IPC FIFO regression ok
IDT initialized: #UD #GP #PF
Native secret object registered: secret:logd-token storage=in-memory
Process table entries: 1
Endpoint table entries: 12
endpoint[0] id=1 name=serial-log
endpoint[1] id=2 name=readiness
endpoint[2] id=3 name=serial-console
endpoint[3] id=4 name=log-sink
endpoint[4] id=5 name=vertex-store-block-request
endpoint[5] id=6 name=vertex-state-block-request
endpoint[6] id=7 name=vertex-store-block-reply
endpoint[7] id=8 name=vertex-state-block-reply
endpoint[8] id=9 name=store-hello-text-request
endpoint[9] id=10 name=model-reader-store-reply
endpoint[10] id=11 name=state-vfs-request
endpoint[11] id=12 name=state-vfs-reply
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
proc=vertex-init cap[2] process-control=process-control rights=control|allocate|delegate|revoke|inspect|create|start|kill|wait
proc=vertex-init cap[3] endpoint=readiness rights=receive
proc=vertex-init cap[4] endpoint=serial-console rights=send
proc=vertex-init cap[5] endpoint=log-sink rights=send
proc=vertex-init cap[6] endpoint=vertex-store-block-request rights=send
proc=vertex-init cap[7] endpoint=vertex-state-block-request rights=send
proc=vertex-init cap[8] endpoint=vertex-store-block-reply rights=send
proc=vertex-init cap[9] endpoint=vertex-state-block-reply rights=send
proc=vertex-init cap[10] endpoint=store-hello-text-request rights=send
proc=vertex-init cap[11] endpoint=model-reader-store-reply rights=send
proc=serial-driver cap[3] io-port=cap:io.com1 rights=read|write
proc=block-driver cap[0] endpoint=vertex-store-block-request rights=receive
proc=block-driver cap[3] endpoint=vertex-state-block-request rights=receive
proc=block-driver cap[6] io-port=cap:io.pci-config rights=read|write
proc=block-driver cap[7] interrupt-line=cap:irq.virtio-blk0 rights=listen
proc=block-driver cap[8] dma-region=cap:dma.virtio-blk0 base=
proc=block-driver cap[9] io-port=cap:io.virtio-blk0 rights=read|write
proc=block-driver cap[10] vfs-root=cap:vfs.block-dev-blk0 root=/dev/device:virtio-blk0 rights=read|resolve
proc=block-driver cap[11] pci-device=device:virtio-blk0 kind=virtio-blk-pci rights=control
Native VFS state request grant: process=vertex-state endpoint=state-vfs-request rights=receive
Native VFS state reply grant: process=vertex-state endpoint=state-vfs-reply rights=send
proc=vertex-state cap[6] endpoint=state-vfs-reply rights=send
proc=vertex-state cap[7] endpoint=state-vfs-request rights=receive
proc=block-driver cap[12] virtio-device=device:virtio-blk0 transport=virtio-pci-io rights=control
proc=vertex-store cap[0] endpoint=store-hello-text-request rights=receive
proc=vertex-store cap[3] endpoint=vertex-store-block-reply rights=receive
proc=vertex-state cap[0] endpoint=vertex-state-block-reply rights=receive
proc=model-reader cap[0] endpoint=model-reader-store-reply rights=receive
proc=serial-driver cap[5] virtio-device=device:virtio-console0 transport=virtio-pci-io rights=control
proc=netstack cap[3] virtio-device=device:virtio-rng0 transport=virtio-pci-io rights=control
proc=netstack cap[5] virtio-device=device:virtio-net0 transport=virtio-pci-io rights=control
proc=netstack cap[6] network-port=cap:net.udp.9000 rights=control
proc=echo cap[3] network-port=cap:net.udp.9000 rights=bind|listen
proc=echo cap[4] namespace=cap:namespace.echo rights=resolve
proc=counter-service cap[0] vfs-root=cap:vfs.counter-state root=/state/counter rights=read|write|resolve
proc=reader-service cap[0] vfs-root=cap:vfs.state-reader-state root=/state/counter rights=read|resolve
proc=echo cap[7] vfs-root=cap:vfs.echo-state-control root=/state/counter/control rights=control|resolve
proc=reader-service cap[3] namespace=cap:namespace.reader rights=resolve
proc=vertex-init cap[30] timer=monotonic-timer rights=control
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
vertex-init endpoints: 10
vertex-init grants: 64
vertex-init network ports: 1
vertex-init store objects: 14
Krust process executable store object: process=logd object=store:logd-demo
store hash verified before process creation: process=logd
Krust process image loaded from native store: process=logd
Krust process executable store object: process=echo object=store:echo-server-demo
store hash verified before process creation: process=echo
Krust process image loaded from native store: process=echo
vertex-init state volumes: 2
vertex-init io ports: 3
vertex-init mmio regions: 0
vertex-init interrupt lines: 1
vertex-init dma regions: 1
vertex-init pci devices: 4
vertex-init virtio devices: 4
vertex-init namespaces: 2
vertex-init vfs roots: 7
M61 malformed boot-read buffer rejected
M61 rights subset checks reject derived and transferred authority
M61 capability move rejects occupied target without dropping source
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
initial capability grants supplied explicitly: process=serial-driver
Krust process create accepted: proc=vertex-init target=serial-driver
immutable launch object accepted: process=serial-driver
vertex-init dynamically created service: serial-driver
Krust process start accepted: proc=vertex-init target=serial-driver
vertex-init observed ready: serial-driver
vertex-init starting service: logd
Native secret grant: process=logd secret=secret:logd-token rights=read|inspect-metadata
proc=logd cap[4] vfs-root=cap:vfs.logd-log-stream root=/proc/log-stream rights=read|resolve
proc=logd cap[5] config=config:logd rights=read
proc=logd cap[6] secret=secret:logd-token value=<redacted> rights=read|inspect-metadata
initial capability grants supplied explicitly: process=logd
Krust process create accepted: proc=vertex-init target=logd
immutable launch object accepted: process=logd
vertex-init dynamically created service: logd
Krust process start accepted: proc=vertex-init target=logd
VFS open accepted: proc=logd file=log-stream
VFS read blocked: proc=logd
VFS pipe wake reader: proc=logd file=log-stream
VFS pipe read blocks until writer log
logd ready
Krust native config hash verified: config=config:logd
VFS open accepted: proc=logd file=config:logd
VFS stat accepted: proc=logd file=config:logd
VFS read accepted: proc=logd file=config:logd bytes=33
logd reads config through VFS handle
Secret read accepted: proc=logd secret=secret:logd-token bytes=<redacted>
service with secret cap reads secret
vertex-init observed ready: logd
vertex-init starting service: netstack
Krust process create accepted: proc=vertex-init target=netstack
vertex-init dynamically created service: netstack
Krust process start accepted: proc=vertex-init target=netstack
vertex-init starting service: block-driver
Krust process create accepted: proc=vertex-init target=block-driver
vertex-init dynamically created service: block-driver
Krust process start accepted: proc=vertex-init target=block-driver
vertex-init observed ready: block-driver
vertex-init starting service: vertex-store
Krust process create accepted: proc=vertex-init target=vertex-store
vertex-init dynamically created service: vertex-store
Krust process start accepted: proc=vertex-init target=vertex-store
vertex-init observed ready: vertex-store
vertex-init starting service: vertex-state
Krust process create accepted: proc=vertex-init target=vertex-state
vertex-init dynamically created service: vertex-state
Krust process start accepted: proc=vertex-init target=vertex-state
vertex-init observed ready: vertex-state
vertex-init derives endpoint cap for logd from endpoint[2] rights=send
vertex-init derives endpoint cap for block-driver from endpoint[6] rights=send
vertex-init derives endpoint cap for block-driver from endpoint[7] rights=send
vertex-init derives endpoint cap for vertex-store from endpoint[4] rights=send
vertex-init derives endpoint cap for vertex-store from endpoint[9] rights=send
vertex-init derives endpoint cap for vertex-state from endpoint[5] rights=send
vertex-init derives endpoint cap for echo from endpoint[3] rights=send
vertex-init derives endpoint cap for model-reader from endpoint[8] rights=send
Capability inspect: proc=vertex-init
Capability transfer accepted: proc=vertex-init target=echo
vertex-init starting service: echo
Krust process create accepted: proc=vertex-init target=echo
vertex-init dynamically created service: echo
Krust process start accepted: proc=vertex-init target=echo
serial-driver ready
serial-driver has COM1 I/O port capability
serial-driver can write byte
logd sends log message
serial-driver writes message to COM1
echo sent message to logd
syscall entry clears direction flag
service with no allocation authority cannot create endpoint
echo I/O write rejected
echo cannot write COM1 directly
logd cannot write COM1 directly
unauthorized service cannot talk to block-driver
unauthorized service cannot access PCI I/O, IRQ, or DMA capabilities
Capability inspect: proc=echo
cap inspect shows parent chain
Capability copy accepted: proc=echo
cap copy preserves source slot
cap revoke reaches descendants through dropped parents
Capability move accepted: proc=echo
cap move removes source slot
Capability revoke accepted: proc=echo
echo send after revoke rejected
logd received: hello from echo
negative test: echo receive rejected: bad capability
echo VFS open rejected: permission
echo drops cap
echo send after drop rejected
unprivileged service calls SYS_PROCESS_CREATE
negative test: logd process-create rejected: bad capability
echo cannot read logd config
service without secret cap rejected
netstack ready
virtio-rng provides random bytes through explicit cap
virtio-net driver can send raw frames
virtio-net driver can receive raw frames
Virtio net TX completed: proc=netstack virtio-device=device:virtio-net0 frame-bytes=60
Virtio net RX completed: proc=netstack virtio-device=device:virtio-net0 frame-bytes=
QEMU user-mode network delivered a raw frame
Vertex sends ICMP echo
QEMU user-mode network delivered ICMP echo reply
UDP send queued for netstack: proc=echo network-port=cap:net.udp.9000 bytes=13
echo submits UDP request to netstack boundary
Network-port UDP request delivered to netstack: network-port=cap:net.udp.9000 bytes=13
netstack received UDP request through network-port boundary
netstack transmitted UDP packet for network-port client
UDP send transmitted: proc=netstack network-port=cap:net.udp.9000 bytes=13
network authority is endpoint/capability mediated
service A namespace contains /state/a
service A cannot resolve /state/b
VFS state volume mounted: state=state:counter path=/state/counter source=vertex-state
VFS state volume value file mounted: state=state:counter path=/state/counter/value source=vertex-state
VFS state volume control file mounted: state=state:counter path=/state/counter/control source=vertex-state
VFS state volume mounted: state=state:scratch path=/state/scratch source=vertex-state
VFS state volume value file mounted: state=state:scratch path=/state/scratch/value source=vertex-state
VFS state volume control file mounted: state=state:scratch path=/state/scratch/control source=vertex-state
mounted state volume appears at /state/counter
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
vertex-state block cache hit
vertex-state block cache writeback clean
vertex-state cache inspect dirty=0 pinned=0 writeback_errors=0
SYS_VFS_RENAME returned STATUS_VFS_PERMISSION
VFS rename requires explicit rename authority
VFS rename accepted: proc=echo old=/rename-old new=/rename-new canonical_old=/state/rename-old canonical_new=/state/rename-new vnode=
VFS rename moves volatile file and preserves vnode identity
VFS stat reports monotonic metadata version and link count
VFS rmdir rejects non-empty directory
VFS mkdir creates directories and rmdir removes empty directories
VFS unlink of open file keeps existing handle readable until close
VFS hard links share volatile file backing and report link count
VFS hard link metadata version follows shared backing writes
VFS hard link metadata version follows link count changes
VFS hard links cannot cross filesystem boundaries
VFS hard links cannot cross volatile mount instances
VFS rename cannot cross volatile mount instances
long VFS paths and components are rejected before allocation
path traversal cannot escape service namespace root
virtio-blk driver ready
virtio-blk PCI device discovered
IRQ wait accepted: proc=block-driver interrupt-line=cap:irq.virtio-blk0
DMA map accepted: proc=block-driver dma-region=cap:dma.virtio-blk0
block-driver reads sector 0
block-driver writes test sector
readback matches
QEMU boots with VertexDisk image attached
VertexDisk superblock accepted
vertex-store ready
vertex-state ready
model-reader asks for store:hello-text
store-service requests block read
block-driver received block-read request
block-driver returns bytes
vertex-store reads object index from disk
vertex-store verifies hash
modified object fails hash check
model-reader reads bytes
model-reader reads bytes successfully
Native immutable store client ok
counter-service has VFS state file
counter-service writes state through VFS
vertex-state reads state volume from disk
vertex-state writes journal record to disk
vertex-state writes state volume to disk
reader-service has VFS state file
reader-service reads state
reader-service receives state value
reader-service write rejected
state control requires write-only open
VFS state transaction request: proc=echo state=state:counter op=control file=control
VFS state transaction wake: proc=echo file=control op=control result=1
state restored
system generation rollback does not automatically roll back state unless policy says so
Native immutable store service ok
Native VertexDisk state service ok
Native state service client ok
M61 syscall negative table: wrong object kind rejected
M61 syscall negative table: missing rights rejected
M61 syscall negative table: malformed buffers rejected
M61 provider malformed receive/read buffers rejected
M61 virtio typed device syscalls reject mismatched device IDs
M61 timer syscall rejects wrong object kind
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
flaky-service creates quota-backed endpoint
vertex-init observes failure
restart policy = on-failure
restart backoff sleep elapsed
vertex-init restarts flaky-service once
Krust process restart reload: proc=flaky-service
Krust process restart restores quota baseline: proc=flaky-service
flaky-service restart quota restored
flaky-service exits 0
Krust process wait observed exit: proc=logd
vertex-init waits for service exit status
block-driver sleeps on virtio-blk IRQ instead of polling for completion
netstack sleeps on virtio-net IRQ instead of polling for RX completion
driver exit releases DMA buffers and user DMA mappings
inspect reports virtio queue state, last error, reset count, and owner process
release gate checks memory/object/cap/DMA/IRQ leak deltas after fault injection
Native restart policy ok
Native manifest-driven activation ok
Native readiness activation ok
Native service activation ok
'

forbidden_lines='
proc=vertex-init cap[5] store-object=store:hello-text rights=read
proc=vertex-init cap[8] timer=monotonic-timer rights=control
Object read accepted: proc=vertex-init
Object read accepted: proc=logd
Object read accepted: proc=block-driver
Config object read accepted
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
        echo "smoke ok: Krust completed manifest v1, directed IPC, typed arenas, cap lifecycle, quotas, service-local store/state/timer access, restart, verified store execution, update checks, dynamic process creation, config/secret authority, VFS roots, VFS rename, directory metadata, block-cache writeback, blocking VFS pipe reads, service-backed state-volume VFS transactions, and native service activation"
        exit 0
    fi

    sleep "$QEMU_POLL_SECONDS"
    attempt=$((attempt + 1))
done

cleanup
pid=
echo "smoke failed: serial output did not contain the current native activation transcript after $QEMU_ATTEMPTS checks"
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
