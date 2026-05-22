# Krust ABI v0

This document describes the current experimental userspace ABI used by the
native Krust QEMU/Limine milestone. It is intentionally small and unstable. Its
current job is to boot native `vertex-init`, start a tiny declared service
graph, and enforce explicit process-local capabilities.

## Machine ABI

Architecture: `x86_64`.

Syscall mechanism: `syscall` / `sysretq`.

Register convention:

```text
rax = syscall number
rdi = arg0
rsi = arg1
rdx = arg2
rax = return value
rcx = clobbered by syscall/sysret
r11 = clobbered by syscall/sysret
```

The kernel saves this minimal return frame on syscall entry:

```text
user_rsp
user_rip
user_rflags
rax
```

The cooperative scheduler can save that frame into the current process, load a
different process frame, switch CR3, and return from the same kernel syscall
entry into another userspace process. There is no timer preemption in ABI v0.

## Syscall Numbers

| Number | Name | Arguments | Return |
| --- | --- | --- | --- |
| 1 | `SYS_WRITE_SERIAL` | legacy, rejected in M12 | `STATUS_BAD_CAPABILITY` |
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

## Return Status Values

| Name | Value | Meaning |
| --- | --- | --- |
| `STATUS_OK` | `0` | Operation accepted. |
| `STATUS_BAD_CAPABILITY` | `u64::MAX - 1` | The process does not hold a suitable capability in the requested slot. |
| `STATUS_BAD_BUFFER` | `u64::MAX - 2` | The user pointer/range failed validation before copying. |
| `STATUS_TOO_LARGE` | `u64::MAX - 3` | IPC message length exceeded the kernel's fixed message buffer. |
| `STATUS_EMPTY` | `u64::MAX - 4` | Endpoint had no message and no process could be scheduled after blocking. |
| `STATUS_RUNNING` | `u64::MAX - 8` | `SYS_PROCESS_STATUS` target has not exited. |
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

Current M14-M24 layout:

```text
vertex-init:
  cap[0] = boot module krustboot-manifest, rights=read
  cap[1] = endpoint serial-log, rights=send
  cap[2] = process-control object, rights=control
  cap[3] = endpoint readiness, rights=receive
  cap[4] = endpoint log-sink, rights=send|receive

logd:
  cap[0] = endpoint log-sink, rights=receive
  cap[1] = endpoint serial-log, rights=send
  cap[2] = endpoint readiness, rights=send

echo:
  cap[1] = endpoint serial-log, rights=send
  cap[0] = endpoint log-sink, rights=send after vertex-init derives and transfers it

model-reader:
  cap[0] = store-object store:hello-text, rights=read
  cap[1] = endpoint serial-log, rights=send

counter-service:
  cap[0] = state-volume state:counter, rights=write
  cap[1] = endpoint serial-log, rights=send

reader-service:
  cap[0] = state-volume state:counter, rights=read|snapshot|restore
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
SYS_CAP_TRANSFER requires a caller-supplied process-control cap slot and applies the packed rights mask.
SYS_OBJECT_READ requires read rights on a store-object cap.
SYS_STATE_WRITE requires write rights on a state-volume cap.
SYS_STATE_READ requires read rights on a state-volume cap.
SYS_SLEEP_MS requires control rights on a timer cap.
```

`SYS_ACTIVATE_GENERATION` remains the minimal M12 authority proof. The current
native path uses `SYS_PROCESS_START` and `SYS_PROCESS_STATUS` for activation
and supervision.

## Process Model

ABI v0 uses a fixed-size kernel process table.

Current states:

```text
Declared
Ready
Running
BlockedOnEndpoint
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
optional saved syscall frame
```

Scheduling is cooperative and round-robin. A context switch currently happens
only when a syscall explicitly yields, exits, or blocks on IPC.

Non-initial processes loaded from the compact manifest start in `Declared`.
They are not scheduler candidates until `SYS_PROCESS_START` changes them to
`Ready`.

`SYS_PROCESS_START` semantics:

```text
requires control rights on the process-control cap
target process index must exist in the compact manifest process table
target process state must be Declared or Exited
on success: Declared -> Ready
on failure: STATUS_BAD_CAPABILITY
```

Restart uses the same syscall; restarting an exited process resets its saved
frame and exit status before making it Ready again.

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
saved syscall frame. When that process is scheduled again, `sysretq` returns to
the original receive call site with `rax = delivered_len`.

## Native vertex-init Semantics

The current hello generation boots one initial userspace process and eight
declared service processes:

```text
process[0] = vertex-init
process[1] = logd
process[2] = netstack
process[3] = echo
process[4] = model-reader
process[5] = counter-service
process[6] = reader-service
process[7] = timer-service
process[8] = flaky-service
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

SYS_CAP_DERIVE(cap[4], cap[9], send)
  derives attenuated endpoint authority for echo

SYS_CAP_TRANSFER(cap[2], echo_process_index, packed(cap[9], target_cap[0], send))
  transfers the attenuated endpoint authority before echo starts

SYS_PROCESS_STATUS(cap[2], process_index, 0)
  observes exits for supervision and restart policy
```

`vertex-init` reads the compact manifest, computes the activation order from
manifest dependencies, waits for logd readiness, derives and transfers echo's
send-only endpoint cap, starts the declared services, and observes their exit
status. Negative capability tests prove echo cannot receive on its send-only
cap, logd cannot start processes without process-control, and reader-service
cannot write through a read-only state cap.

## Boot ABI

Limine loads:

```text
krust.elf
hello-generation.krustboot
userspace ELF modules
```

Krust consumes the compact KrustBoot manifest rather than parsing full JSON in
kernel space. Hosted `vertexctl compile-boot-manifest` is responsible for
converting source Vertex JSON into the compact boot format.

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
```

Krust also creates fixed boot caps for native `vertex-init`:

```text
cap[0] manifest module read
cap[1] serial-log send
cap[2] process-control control
cap[3] readiness receive
cap[4] endpoint authority for graph-delegated endpoints
```

Endpoint, store-object, state-volume, and timer grants for declared services
come from the compact manifest. Endpoint consumers do not receive static boot
send grants for delegated authority; vertex-init derives and transfers the
attenuated cap before starting the consumer.
