# Krust Kernel

Krust now covers the M14-M24 native graph-activation proof path.

The target is intentionally small:

```text
QEMU boots a Limine ISO
Limine loads krust.elf
Krust enters 64-bit Rust code
Krust writes "Krust Kernel booted" to COM1 serial
Krust reads the Limine memory map response
Krust prints every memory map entry to serial
`vertexctl compile-boot-manifest` derives `hello-generation.krustboot` from the full Vertex IR graph
Limine loads `hello-generation.krustboot` as a boot module
Krust parses the fixed-format KrustBoot manifest and prints its generation ID
Krust prints the KrustBoot boot modules, processes, endpoints, grants, store objects, state volumes, and network ports
Krust builds a physical frame allocator from usable memory map entries
Krust allocates, frees, and reuses 4 KiB physical frames
Krust walks the active x86_64 page tables through Limine's HHDM
Krust maps a small fixed kernel-heap virtual range
Krust writes and reads through the mapped virtual pages
Krust creates fixed kernel objects and boot capabilities
Krust prints the boot capability table
Limine loads native service ELFs for vertex-init, logd, netstack, echo, model-reader, counter, state-reader, timer, and flaky-service
Krust loads each declared process into a fresh low-half address space
Krust creates a runtime process table and endpoint table from the KrustBoot manifest
Krust allocates runtime process IDs and states from KrustBoot process records
Krust grants vertex-init cap[0] read rights to the manifest module
Krust grants vertex-init cap[1] send rights to the serial-log endpoint
Krust grants vertex-init cap[2] process-control authority, cap[3] readiness receive authority, and per-endpoint attenuable endpoint authority starting at cap[4]
Krust grants store/state/timer authority only to services that declare those capabilities
Krust installs a minimal IDT for #UD, #GP, and #PF
Krust enters ring 3 at the initial process entry point
Krust tracks Declared, Ready, Running, BlockedOnEndpoint, Sleeping, and Exited process states
Krust validates syscall user buffers by walking user page tables
vertex-init reads the compact manifest through cap[0]
vertex-init logs through cap[1]
vertex-init computes a manifest-driven activation order and starts declared services through cap[2]
logd reports readiness before echo starts
vertex-init derives and transfers attenuated endpoint authority
echo sends one message to logd through an explicit IPC capability
echo drops its endpoint capability and denied authority stays rejected
model-reader reads an immutable store object through its own store capability
counter-service writes a state volume, and reader-service reads it through read-only state authority
timer-service sleeps through its own timer capability without monopolizing the scheduler
echo proves bounded restart=always with delegated endpoint authority restored, and flaky-service proves restart=on-failure from a fresh restart context
logd receives the message and denial tests reject missing authority
Krust halts after `Native service activation ok`
```

No dynamic heap allocator, APIC-backed timer, preemption, user page-fault
recovery, full interrupt handling, full JSON parsing in the kernel, filesystem,
network, or device drivers are part of this native proof. Timer deadlines use
cooperative sleep states; when no process is ready, the scheduler polls TSC until
the next deadline rather than sleeping on an interrupt. The kernel consumes a
compact KrustBoot manifest compiled by hosted `vertexctl`; graph interpretation
and lifecycle policy remain a userspace responsibility.

## Prerequisites

- Rust stable with the `x86_64-unknown-none` target.
- `qemu-system-x86_64`.
- `limine` v12 or newer.
- `xorriso`.
- Limine boot assets containing:
  - `limine-bios.sys`
  - `limine-bios-cd.bin`
  - `limine-uefi-cd.bin`
  - `BOOTX64.EFI`

Limine installs these files under its configured `${PREFIX}/share` directory.
Typical package-manager locations are `/usr/share/limine`,
`/usr/local/share/limine`, or a Homebrew prefix path.

The Makefile auto-detects these Limine asset directories:

```text
/opt/homebrew/share/limine
/usr/local/share/limine
/usr/share/limine
```

If Limine is installed somewhere else, pass it explicitly:

```sh
LIMINE_DIR=/path/to/limine/assets make smoke
```

On macOS with Homebrew:

```sh
brew install limine xorriso qemu
make doctor
```

On Linux, install the same tools through the host distribution package manager
and run:

```sh
make doctor
```

`make doctor` prints the resolved QEMU, Limine, xorriso, and Limine asset paths.

## Build

```sh
make build
```

This builds `target/x86_64-unknown-none/debug/krust` and the native user
programs under `user/*/target/x86_64-unknown-none/debug/`: `vertex-init`,
`logd`, `echo`, `netstack`, `model-reader`, `counter`, `state-reader`,
`timer`, and `flaky`.

## Build ISO

```sh
make iso
```

This runs `vertexctl compile-boot-manifest`, writes
`build/hello-generation.krustboot` plus `build/fallback-generation.krustboot`,
and creates `build/krust.iso`. By default the fallback manifest is the same
generation; pass `FALLBACK_MANIFEST=/path/to/previous.vertex.json` to package a
real fallback generation.

## Run

```sh
make run
```

Expected terminal output:

```text
Krust Kernel booted
Limine base revision supported
Limine memory map entries: ...
KrustBoot manifest generation: gen:hello-0001
KrustBoot boot modules: 9
KrustBoot processes: 9
KrustBoot endpoints: 3
KrustBoot grants: 18
KrustBoot store objects: 1
KrustBoot state volumes: 1
KrustBoot network ports: 1
  grant[0] process=vertex-init cap[1] endpoint=serial-log rights=send
  grant[11] process=logd cap[0] endpoint=log-sink rights=receive
  grant[12] process=vertex-init cap[4] endpoint=log-sink rights=send|receive
  grant[13] process=echo cap[3] network-port=cap:net.tcp.8080 rights=listen
  grant[14] process=model-reader cap[0] store-object=store:hello-text rights=read
  grant[16] process=reader-service cap[0] state-volume=state:counter rights=read|snapshot|restore
  grant[17] process=timer-service cap[0] timer=monotonic-timer rights=control
Physical allocator demo ok
Virtual memory demo ok
Capability table demo ok
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
proc=echo cap[3] network-port=cap:net.tcp.8080 rights=listen
proc=logd cap[0] endpoint=log-sink rights=receive
proc=model-reader cap[0] store-object=store:hello-text rights=read
proc=reader-service cap[0] state-volume=state:counter rights=read|snapshot|restore
proc=timer-service cap[0] timer=monotonic-timer rights=control
GDT initialized
IDT initialized: #UD #GP #PF
Syscall path initialized
Entering userspace process: vertex-init
vertex-init started
Boot module read accepted: proc=vertex-init module=krustboot-manifest bytes=...
vertex-init received cap[0]=manifest-read
vertex-init received cap[1]=serial-log
vertex-init received cap[2]=process-control
vertex-init manifest generation: gen:hello-0001
vertex-init boot modules: 9
vertex-init processes: 9
vertex-init endpoints: 3
vertex-init grants: 18
vertex-init network ports: 1
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
Capability transfer accepted: proc=vertex-init target=echo slot=0 rights=send
vertex-init starting service: echo
Krust process start accepted: proc=vertex-init target=echo
vertex-init starting service: model-reader
vertex-init starting service: counter-service
vertex-init starting service: reader-service
vertex-init starting service: timer-service
vertex-init starting service: flaky-service
echo sent message to logd
logd received: hello from echo
negative test: echo receive rejected: bad capability
echo read rejected: bad capability
echo send after drop rejected
negative test: logd process-start rejected: bad capability
Object read accepted: proc=model-reader object=store:hello-text bytes=22
Native store-object read ok
State write accepted: proc=counter-service state=state:counter
State read accepted: proc=reader-service state=state:counter
reader-service write rejected
Native state-volume access ok
Timer sleep accepted: proc=timer-service timer=monotonic-timer ms=10
Timer sleep blocked: proc=timer-service
Timer wake: proc=timer-service
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
```

QEMU runs with `-display none`, so all kernel output is written through the
serial console. Interrupt QEMU with `Ctrl-C`.

## Smoke Test

```sh
make smoke
```

The smoke test boots QEMU headlessly, captures serial output to
`build/serial.log`, and passes when it sees the M14-M24 boot transcript. The same
check is available from the repository root:

```sh
scripts/krust-smoke.sh
```

The expected transcript includes:

```text
Krust Kernel booted
Limine memory map entries:
KrustBoot manifest generation: gen:hello-0001
KrustBoot boot modules: 9
KrustBoot processes: 9
KrustBoot endpoints: 3
KrustBoot grants: 18
KrustBoot network ports: 1
grant[0] process=vertex-init cap[1] endpoint=serial-log rights=send
grant[11] process=logd cap[0] endpoint=log-sink rights=receive
grant[12] process=vertex-init cap[4] endpoint=log-sink rights=send|receive
grant[13] process=echo cap[3] network-port=cap:net.tcp.8080 rights=listen
grant[14] process=model-reader cap[0] store-object=store:hello-text rights=read
grant[16] process=reader-service cap[0] state-volume=state:counter rights=read|snapshot|restore
grant[17] process=timer-service cap[0] timer=monotonic-timer rights=control
network_port[0] id=cap:net.tcp.8080
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
proc=echo cap[3] network-port=cap:net.tcp.8080 rights=listen
proc=logd cap[0] endpoint=log-sink rights=receive
proc=model-reader cap[0] store-object=store:hello-text rights=read
proc=counter-service cap[0] state-volume=state:counter rights=write
proc=reader-service cap[0] state-volume=state:counter rights=read|snapshot|restore
proc=timer-service cap[0] timer=monotonic-timer rights=control
vertex-init started
Boot module read accepted: proc=vertex-init module=krustboot-manifest bytes=
vertex-init manifest generation: gen:hello-0001
vertex-init network ports: 1
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
vertex-init observed ready: logd
vertex-init starting service: netstack
vertex-init starting service: echo
Krust process start accepted: proc=vertex-init target=echo
echo sent message to logd
logd received: hello from echo
negative test: echo receive rejected: bad capability
echo send after drop rejected
negative test: logd process-start rejected: bad capability
Object read accepted: proc=model-reader object=store:hello-text bytes=22
State write accepted: proc=counter-service state=state:counter
State read accepted: proc=reader-service state=state:counter
reader-service write rejected
Timer sleep accepted: proc=timer-service timer=monotonic-timer ms=10
Timer sleep blocked: proc=timer-service
Timer wake: proc=timer-service
vertex-init observes exit
restart policy = always
vertex-init restarts echo once
Krust process restart reload: proc=echo
echo restart retained delegated log cap
flaky-service exits with status 1
restart policy = on-failure
vertex-init restarts flaky-service once
Krust process restart reload: proc=flaky-service
flaky-service exits 0
Native manifest-driven activation ok
Native readiness activation ok
Native service activation ok
```

## Machine Notes

Linux x86_64 can add KVM later with QEMU flags such as `-enable-kvm -cpu host`.
That is not enabled by default so the Krust smoke command stays portable:

```sh
QEMU_EXTRA="-enable-kvm -cpu host" make smoke
```

macOS Apple Silicon can run `qemu-system-x86_64` by emulation. It is slower than
native AArch64 virtualization, but it is enough for the Krust serial tests.
