# Krust ABI v0

This document describes the current experimental userspace ABI used by the
native Krust QEMU/Limine milestone. It is intentionally small and unstable. Its
current job is to boot native `vertex-init`, start a tiny declared service
graph, and enforce explicit process-local capabilities.

Milestone status: ABI v0 now covers the M14-M36 native activation and substrate
proof. M25 adds the release gate. M26-M29 add Manifest v1 parsing, capability
provenance/revocation, typed arena allocation checks, and resource quotas.
M30-M31 add PIT-backed preemption and user page-fault containment. M32-M36 add
I/O capability objects, user-space serial and block-driver proof paths, and
native store/state services. This is
still experimental and does not freeze syscall, capability, process, or IPC ABI
surface.

## Machine ABI

Architecture: `x86_64`.

Syscall mechanism: `syscall` entry with `iretq` return from the saved userspace
frame.

Register convention:

```text
rax = syscall number
rdi = arg0
rsi = arg1
rdx = arg2
rax = return value
rcx = clobbered by syscall entry
r11 = clobbered by syscall entry
```

The kernel saves a full userspace register return frame on syscall entry and
on user timer interrupts:

```text
r15..rax
user_rip
user_cs
user_rflags
user_rsp
user_ss
```

The scheduler can save that frame into the current process, load a different
process frame, switch CR3, and return into another userspace process through
`iretq`.

## Syscall Numbers

| Number | Name | Arguments | Return |
| --- | --- | --- | --- |
| 1 | reserved | invalid syscall slot | `u64::MAX` |
| 2 | `SYS_EXIT` | `arg0 = status` | does not return in normal use |
| 3 | `SYS_IPC_SEND` | `arg0 = cap_slot`, `arg1 = user_ptr`, `arg2 = len` | status |
| 4 | `SYS_IPC_RECV` | `arg0 = cap_slot`, `arg1 = user_ptr`, `arg2 = max_len` | byte count or error status |
| 5 | `SYS_YIELD` | none | status |
| 6 | `SYS_BOOT_READ` | `arg0 = cap_slot`, `arg1 = user_ptr`, `arg2 = max_len` | byte count or error status |
| 7 | `SYS_LOG_WRITE` | `arg0 = cap_slot`, `arg1 = user_ptr`, `arg2 = len` | status |
| 8 | `SYS_ACTIVATE_GENERATION` | `arg0 = cap_slot`, `arg1 = user_ptr`, `arg2 = len` | status |
| 9 | `SYS_PROCESS_START` | `arg0 = process_control_cap_slot`, `arg1 = process_index`, `arg2 = 0` | status |
| 10 | `SYS_CAP_DERIVE` | `arg0 = parent_cap_slot`, `arg1 = new_cap_slot`, `arg2 = rights_mask` | status |
| 11 | `SYS_CAP_DROP` | `arg0 = cap_slot` | status |
| 12 | `SYS_CAP_TRANSFER` | `arg0 = process_control_cap_slot`, `arg1 = target_process_index`, `arg2 = packed transfer` | status |
| 13 | `SYS_OBJECT_READ` | `arg0 = cap_slot`, `arg1 = user_ptr`, `arg2 = max_len` | byte count or error status |
| 14 | `SYS_STATE_WRITE` | `arg0 = cap_slot`, `arg1 = user_ptr`, `arg2 = len` | status |
| 15 | `SYS_STATE_READ` | `arg0 = cap_slot`, `arg1 = user_ptr`, `arg2 = max_len` | byte count or error status |
| 16 | `SYS_SLEEP_MS` | `arg0 = timer_cap_slot`, `arg1 = milliseconds`, `arg2 = 0` | status |
| 17 | `SYS_PROCESS_STATUS` | `arg0 = process_control_cap_slot`, `arg1 = process_index`, `arg2 = 0` | exit status, running marker, or error status |
| 18 | `SYS_ROLLBACK_GENERATION` | `arg0 = process_control_cap_slot`, `arg1 = generation_ptr`, `arg2 = len` | switches to the prepared fallback generation or returns error |
| 19 | `SYS_IPC_RECV_TIMEOUT` | `arg0 = cap_slot`, `arg1 = user_ptr`, `arg2 = timeout_ms << 32 \| max_len` | byte count, `STATUS_TIMEOUT`, or error status |
| 20 | `SYS_PROCESS_ATTEMPT` | none | current process start attempt count |
| 21 | `SYS_CAP_REVOKE` | `arg0 = cap_slot` | status |
| 22 | `SYS_CAP_INSPECT` | `arg0 = cap_slot` | parent capability id or error status |
| 23 | `SYS_CAP_MOVE` | `arg0 = source_cap_slot`, `arg1 = target_cap_slot` | status |
| 24 | `SYS_CAP_COPY` | `arg0 = source_cap_slot`, `arg1 = target_cap_slot`, `arg2 = rights_mask` | status |
| 25 | `SYS_ENDPOINT_CREATE` | `arg0 = process_control_cap_slot`, `arg1 = target_cap_slot` | status |
| 26 | `SYS_QUOTA_DELEGATE` | `arg0 = process_control_cap_slot`, `arg1 = target_process_index`, `arg2 = max_endpoints` | status |
| 27 | `SYS_IO_READ` | `arg0 = io_port_cap_slot`, `arg1 = port`, `arg2 = 0` | byte value or error status |
| 28 | `SYS_IO_WRITE` | `arg0 = io_port_cap_slot`, `arg1 = port`, `arg2 = byte value` | status |
| 29 | `SYS_IRQ_WAIT` | `arg0 = interrupt_line_cap_slot`, `arg1 = timeout_ms`, `arg2 = 0` | status |
| 30 | `SYS_MMIO_MAP` | `arg0 = mmio_region_cap_slot` | mapped base proof value or error status |

## Return Status Values

| Name | Value | Meaning |
| --- | --- | --- |
| `STATUS_OK` | `0` | Operation accepted. |
| `STATUS_BAD_CAPABILITY` | `u64::MAX - 1` | The process does not hold a suitable capability in the requested slot. |
| `STATUS_BAD_BUFFER` | `u64::MAX - 2` | The user pointer/range failed validation before copying. |
| `STATUS_TOO_LARGE` | `u64::MAX - 3` | IPC message length exceeded the kernel's fixed message buffer. |
| `STATUS_EMPTY` | `u64::MAX - 4` | Endpoint had no message and no process could be scheduled after blocking. |
| `STATUS_RUNNING` | `u64::MAX - 8` | `SYS_PROCESS_STATUS` target has not exited. |
| `STATUS_TIMEOUT` | `u64::MAX - 9` | A timed IPC receive expired before a message arrived. |
| `STATUS_PROCESS_FAULT` | `u64::MAX - 10` | The target exited because of a contained userspace fault. |
| `u64::MAX` | `u64::MAX` | Unknown syscall number. |

For `SYS_IPC_RECV`, any return value less than or equal to the destination
buffer length is a delivered byte count. The current demo treats the high status
values above as errors.

## User Memory Rules

Syscalls must not directly trust userspace pointers.

ABI v0 validation checks:

- The range is low-half canonical.
- The range does not overflow.
- Every page is present in the target user page table.
- Every page has the x86_64 user bit set.
- Write destinations have the writable bit set.

Bad pointers return `STATUS_BAD_BUFFER` for the tested syscall path instead of
becoming uncontrolled kernel faults.

## Capability Slots

Capabilities are process-local. A capability slot number is meaningful only in
the current process's capability space.

Current M14-M36 layout:

```text
vertex-init:
  cap[0] = boot module krustboot-manifest, rights=read
  cap[1] = endpoint serial-log, rights=send
  cap[2] = process-control object, rights=control|allocate|delegate|revoke
  cap[3] = endpoint readiness, rights=receive
  cap[4] = endpoint log-sink, rights=send|receive
  cap[5+] = additional delegated endpoint authorities, if the graph needs them

logd:
  cap[0] = endpoint log-sink, rights=receive
  cap[1] = endpoint serial-log, rights=send
  cap[2] = endpoint readiness, rights=send
  cap[3] = endpoint serial-console, rights=send after vertex-init derives and transfers it

serial-driver:
  cap[0] = endpoint serial-console, rights=receive
  cap[1] = endpoint serial-log, rights=send
  cap[3] = io-port cap:io.com1, rights=read|write

block-driver:
  cap[0] = endpoint block-io, rights=send|receive
  cap[1] = endpoint serial-log, rights=send
  cap[3] = mmio-region cap:mmio.virtio-blk0, rights=map
  cap[4] = interrupt-line cap:irq.virtio-blk0, rights=listen
  cap[5] = dma-region cap:dma.virtio-blk0, rights=read|write|map

vertex-store:
  cap[0] = endpoint store-hello-text-api, rights=send|receive
  cap[1] = endpoint serial-log, rights=send
  cap[3] = endpoint block-io, rights=send|receive after vertex-init derives and transfers it

vertex-state:
  cap[0] = endpoint state-counter-api, rights=send|receive
  cap[1] = endpoint serial-log, rights=send
  cap[3] = state-volume state:counter, rights=read|write|snapshot|restore

echo:
  cap[1] = endpoint serial-log, rights=send
  cap[3] = network-port cap:net.tcp.8080, rights=listen
  cap[0] = endpoint log-sink, rights=send after vertex-init derives and transfers it

model-reader:
  cap[0] = endpoint store-hello-text-api, rights=send|receive after vertex-init derives and transfers it
  cap[1] = endpoint serial-log, rights=send

counter-service:
  cap[0] = endpoint state-counter-api, rights=send|receive after vertex-init derives and transfers it
  cap[1] = endpoint serial-log, rights=send

reader-service:
  cap[0] = endpoint state-counter-api, rights=send|receive after vertex-init derives and transfers it
  cap[1] = endpoint serial-log, rights=send

timer-service:
  cap[0] = timer monotonic-timer, rights=control
  cap[1] = endpoint serial-log, rights=send
```

`SYS_IPC_SEND` requires `send` rights on the endpoint capability. `SYS_IPC_RECV`
requires `receive` rights on the endpoint capability. The syscall layer does not
special-case process names; it resolves:

```text
current process -> cap slot -> kernel object -> required rights
```

The native activation path uses the same rule:

```text
SYS_BOOT_READ requires cap[0] read rights to the manifest boot module.
SYS_LOG_WRITE requires cap[1] send rights to the serial-log endpoint.
SYS_ACTIVATE_GENERATION requires cap[2] control rights to process-control.
SYS_PROCESS_START requires cap[2] control rights to process-control.
SYS_PROCESS_STATUS requires cap[2] control rights to process-control.
SYS_ROLLBACK_GENERATION requires cap[2] control rights to process-control.
SYS_CAP_TRANSFER requires a caller-supplied process-control cap slot and applies the packed rights mask.
SYS_ENDPOINT_CREATE requires allocate rights on process-control and available endpoint quota.
SYS_QUOTA_DELEGATE requires delegate rights on process-control and cannot exceed the caller quota.
SYS_OBJECT_READ requires read rights on a store-object cap.
SYS_STATE_WRITE requires write rights on a state-volume cap.
SYS_STATE_READ requires read rights on a state-volume cap.
SYS_SLEEP_MS requires control rights on a timer cap.
SYS_IO_READ requires read rights on an io-port cap and a port inside the granted range.
SYS_IO_WRITE requires write rights on an io-port cap and a port inside the granted range.
SYS_IRQ_WAIT requires listen rights on an interrupt-line cap.
SYS_MMIO_MAP requires map rights on an mmio-region cap.
```

Native network-port objects currently grant bind/listen authority to declared
services; the proof path records and enforces the capability object, but does
not yet include a network driver syscall that consumes it.

Native I/O objects now cover the first hardware authority substrate:
`IoPortRange`, `MmioRegion`, `InterruptLine`, and `DmaRegion`. `DmaRegion`
authority is represented and granted to `block-driver`; real DMA mapping is not
implemented in ABI v0.

Capability records carry kernel-owned metadata:

```text
cap_id
object_id
rights
owner_process
parent_cap_id
generation_id
delegated_by
revoked
```

`SYS_CAP_DERIVE`, `SYS_CAP_TRANSFER`, and `SYS_CAP_COPY` create child
capabilities with attenuated rights and a parent id. `SYS_CAP_MOVE` preserves the
capability id while clearing the source slot. `SYS_CAP_REVOKE` marks a cap id and
its descendants revoked; later lookup rejects revoked caps and caps with revoked
ancestors. `SYS_CAP_INSPECT` prints the current metadata to the serial transcript
and returns the parent capability id.

Process-control authority now distinguishes resource rights:

```text
control
allocate
delegate
revoke
```

The initial process starts with endpoint quota `1`. Services start with endpoint
quota `0` unless delegated a smaller quota through `SYS_QUOTA_DELEGATE`.
`SYS_ENDPOINT_CREATE` consumes endpoint quota and installs a send/receive cap in
the caller's requested slot.

`SYS_ACTIVATE_GENERATION` remains the minimal M12 authority proof. The current
native path uses `SYS_PROCESS_START`, `SYS_PROCESS_STATUS`, and
`SYS_ROLLBACK_GENERATION` for activation, supervision, and fallback.

## Process Model

ABI v0 uses a fixed-size kernel process table.

Current states:

```text
Declared
Ready
Running
BlockedOnEndpoint
Sleeping
Exited
```

Each process has:

```text
pid
name
cr3
entry
stack_top
state
capability space
optional saved userspace frame
resource quota counters
```

Scheduling is round-robin with both cooperative and PIT-backed preemptive
switches. A context switch can happen when a syscall explicitly yields, exits,
or blocks on IPC, and also when PIT IRQ0 interrupts a running userspace process
while another process is ready.

Non-initial processes loaded from the compact manifest start in `Declared`.
They are not scheduler candidates until `SYS_PROCESS_START` changes them to
`Ready`.

`SYS_PROCESS_START` semantics:

```text
requires control rights on the process-control cap
target process index must exist in the compact manifest process table
target process state must be Declared or Exited
on success: Declared -> Ready
restart success: Exited -> Ready with the restart context and initial caps restored
on failure: STATUS_BAD_CAPABILITY
```

Restart uses the same syscall; restarting an exited process resets its saved
frame, exit status, capability table, and user context before making it Ready
again. ABI v0 native supervision is explicitly bounded to one restart per
service; `restart = always` means "restart after the first exit" in this proof,
not an unbounded service-manager loop. `SYS_PROCESS_ATTEMPT` lets a process
distinguish its first start from a kernel-mediated restart without relying on
preserved process memory.

`SYS_SLEEP_MS` moves the caller into `Sleeping` and yields to another ready
process. Deadlines use CPUID-reported TSC/base frequency when available, with a
fixed fallback only when the CPU does not report a usable frequency. If no
process is ready, ABI v0 waits through the PIT interrupt path instead of
accepting a cooperative-only TSC polling fallback.

User page faults are process-contained. A direct userspace page fault identifies
the current process, marks that process `Exited` with
`STATUS_PROCESS_FAULT = u64::MAX - 10`, schedules another ready process, and
keeps the kernel running. Kernel faults still stop the kernel.

## IPC Semantics

Endpoints currently hold one fixed-size message buffer.

Send path:

```text
SYS_IPC_SEND(cap_slot, user_ptr, len)
  validate send capability
  copy message from user
  store message on endpoint
  wake one process blocked on that endpoint, if any
```

Receive path:

```text
SYS_IPC_RECV(cap_slot, user_ptr, max_len)
  validate receive capability
  validate writable user buffer
  if message exists:
      copy to receiver buffer and return byte count
  if no message exists:
      save syscall frame
      set state to BlockedOnEndpoint
      schedule the next Ready process
```

When a sender wakes a blocked receiver, the kernel copies the message into the
receiver's address space and stores the delivered byte count in the receiver's
saved syscall frame. When that process is scheduled again, `iretq` returns to
the original receive call site with `rax = delivered_len`.

## Native vertex-init Semantics

The current hello generation boots one initial userspace process and twelve
declared service processes:

```text
process[0] = vertex-init
process[1] = serial-driver
process[2] = logd
process[3] = netstack
process[4] = block-driver
process[5] = vertex-store
process[6] = vertex-state
process[7] = echo
process[8] = model-reader
process[9] = counter-service
process[10] = reader-service
process[11] = timer-service
process[12] = flaky-service
```

`vertex-init` uses these syscalls:

```text
SYS_BOOT_READ(cap[0], buffer, len)
  copies the compact KrustBoot manifest into userspace

SYS_LOG_WRITE(cap[1], message, len)
  writes a serial-log message if cap[1] grants send rights

SYS_ACTIVATE_GENERATION(cap[2], generation_id, len)
  proves vertex-init holds process-control authority for activation

SYS_PROCESS_START(cap[2], process_index, 0)
  starts a declared process from the compact manifest

SYS_CAP_DERIVE(cap[4], cap[31], send)
  derives attenuated endpoint authority for echo

SYS_CAP_TRANSFER(cap[2], echo_process_index, packed(cap[31], target_cap[0], send))
  transfers the attenuated endpoint authority before echo starts

SYS_CAP_INSPECT(cap[31])
  prints provenance metadata for the derived capability

SYS_ENDPOINT_CREATE(cap[2], cap[29])
  creates one dynamic endpoint when endpoint quota is available

SYS_QUOTA_DELEGATE(cap[2], target_process_index, max_endpoints)
  delegates a bounded endpoint quota to a target process

SYS_PROCESS_STATUS(cap[2], process_index, 0)
  observes exits for supervision and restart policy

SYS_ROLLBACK_GENERATION(cap[2], parent_generation_id, len)
  reinitializes runtime tables from the prepared fallback manifest and enters
  the fallback generation's initial process

SYS_IPC_RECV_TIMEOUT(cap[3], buffer, packed(timeout_ms, len))
  waits for readiness with a scheduler timeout
```

`vertex-init` reads the compact manifest, computes the activation order from
manifest dependencies, waits for logd readiness, derives and transfers endpoint
caps with the rights requested by each consumer, starts the declared services,
and observes their exit status. Negative capability tests prove echo cannot
receive on its send-only cap, logd and echo cannot write COM1 directly, echo
cannot talk to block-driver or device objects without authority, logd cannot
start processes without process-control, and reader-service write attempts are
denied by `vertex-state`.

## Boot ABI

Limine loads:

```text
krust.elf
hello-generation.krustboot
fallback-generation.krustboot
userspace ELF modules
```

Krust consumes a KrustBoot Manifest v1 wrapper around the compact payload rather
than parsing full JSON in kernel space. Hosted `vertexctl compile-boot-manifest`
is responsible for converting source Vertex JSON into the versioned boot
artifact.

The compact manifest describes:

```text
generation_id
parent_generation_id
boot_modules
processes
endpoints
grants
store_objects
state_volumes
network_ports
io_port_ranges
mmio_regions
interrupt_lines
dma_regions
```

Manifest v1 adds a fixed header, record table, checksum, and record bounds
validation. The current record kinds are boot modules, processes, endpoints,
grants, store objects, state volumes, timer, generation, and policy. The kernel
requires the v1 wrapper at the boot-module boundary and rejects an unwrapped
compact payload. After validating the wrapper, the kernel exposes the compact
payload to `vertex-init` through cap[0] so existing userspace parsing stays
small.

Krust also creates fixed boot caps for native `vertex-init`:

```text
cap[0] manifest module read
cap[1] serial-log send
cap[2] process-control control|allocate|delegate|revoke
cap[3] readiness receive
cap[4+] endpoint authority for graph-delegated endpoints, one authority cap per
declared endpoint beyond the fixed serial-log/readiness endpoints
```

Endpoint, hardware, store-object, state-volume, and timer grants for declared services
come from the compact manifest. Endpoint consumers do not receive static boot
send grants for delegated authority; vertex-init derives and transfers the
attenuated cap before starting the consumer. A transfer to a still-declared
process becomes part of that process's restart baseline, so the one ABI v0
restart restores the delegated endpoint cap along with static grants. If a
service both provides an endpoint and consumes delegated endpoint authority, the
provided endpoint keeps cap[0] and delegated endpoint caps start at cap[3] to
avoid the serial-log and readiness slots.
