# Krust ABI v1

This document describes the current experimental userspace ABI used by the
native Krust QEMU/Limine milestone. It is intentionally small and unstable. Its
current job is to boot native `vertex-init`, create services from verified
process templates, and enforce explicit process-local capabilities.

Milestone status: ABI v1 now covers the M14-M75 native activation and substrate
proof. M25 adds the release gate. M26-M29 add Manifest v1 parsing, capability
provenance/revocation, typed arena allocation checks, and resource quotas.
M30-M31 add PIT-backed preemption and user page-fault containment. M32-M36 add
I/O capability objects, user-space serial, a native block-driver path, and
native store/state services. M37 upgrades generation activation into a real
runtime switch between registered native KrustBoot configs.
M38 adds native runtime introspection through an inspect-only process-control
right. M39 pins the reproducible native build environment and release gate.
M40 freezes ABI v1 with directed request/reply IPC. M41 adds the console shell
path, M42 adds minimal virtio-blk sector I/O over PCI I/O and DMA
capabilities, and M43 adds VertexDisk v0 block-object persistence for store,
state, and journal data. M44-M47 add native boot-manager state, verified store
object identities, native update transactions, and process executable loading
through verified store objects. M48-M55 replace fixed runtime process slots
with PID-based process creation, add native config and secret authority,
and add the first package/link/build/appliance surface. M56-M60 add explicit
virtio-console/rng/net device operations, UDP network-port send authority,
capability namespace resolution, and the policy/typed source layer that compiles
into the same generation manifest contract. M61 makes syscall argument
validation, typed object dispatch, rights-subset checks, namespace target
limits, virtio device identity checks, and generation provenance the standing
security regression baseline. M62-M65 add the storage durability checks,
network boundary assertions, lifecycle reporting, and supported appliance
profile artifact without adding legacy compatibility paths. M74-M75 add the
native VFS object model, service-local mount roots, open-file handle table,
descriptor lifecycle, and volatile create/unlink path. The ABI is still
intentionally small, but this subset is the current native contract.

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
| 8 | `SYS_ACTIVATE_GENERATION` | `arg0 = cap_slot`, `arg1 = user_ptr`, `arg2 = len` | does not return on switch success; status on rejection |
| 9 | `SYS_PROCESS_CREATE` | `arg0 = process_control_cap_slot`, `arg1 = process_template_index`, `arg2 = 0` | new process id or error status |
| 10 | `SYS_CAP_DERIVE` | `arg0 = parent_cap_slot`, `arg1 = new_cap_slot`, `arg2 = rights_mask` | status |
| 11 | `SYS_CAP_DROP` | `arg0 = cap_slot` | status |
| 12 | `SYS_CAP_TRANSFER` | `arg0 = process_control_cap_slot`, `arg1 = target_pid`, `arg2 = packed transfer` | status |
| 13 | `SYS_OBJECT_READ` | legacy object-read slot; always rejected in VFS mode | error status |
| 14 | reserved | removed M43 native state syscall slot | `u64::MAX` |
| 15 | reserved | removed M43 native state syscall slot | `u64::MAX` |
| 16 | `SYS_SLEEP_MS` | `arg0 = timer_cap_slot`, `arg1 = milliseconds`, `arg2 = 0` | status |
| 17 | `SYS_PROCESS_WAIT` | `arg0 = process_control_cap_slot`, `arg1 = pid`, `arg2 = 0` | exit status, running marker, or error status |
| 18 | `SYS_ROLLBACK_GENERATION` | `arg0 = process_control_cap_slot`, `arg1 = generation_ptr`, `arg2 = len` | switches to the prepared fallback generation or returns error |
| 19 | `SYS_IPC_RECV_TIMEOUT` | `arg0 = cap_slot`, `arg1 = user_ptr`, `arg2 = timeout_ms << 32 \| max_len` | byte count, `STATUS_TIMEOUT`, or error status |
| 20 | `SYS_PROCESS_ATTEMPT` | none | current process start attempt count |
| 21 | `SYS_CAP_REVOKE` | `arg0 = cap_slot` | status |
| 22 | `SYS_CAP_INSPECT` | `arg0 = cap_slot` | parent capability id or error status |
| 23 | `SYS_CAP_MOVE` | `arg0 = source_cap_slot`, `arg1 = target_cap_slot` | status |
| 24 | `SYS_CAP_COPY` | `arg0 = source_cap_slot`, `arg1 = target_cap_slot`, `arg2 = rights_mask` | status |
| 25 | `SYS_ENDPOINT_CREATE` | `arg0 = process_control_cap_slot`, `arg1 = target_cap_slot` | status |
| 26 | `SYS_QUOTA_DELEGATE` | `arg0 = process_control_cap_slot`, `arg1 = target_pid`, `arg2 = max_endpoints` | status |
| 27 | `SYS_IO_READ` | `arg0 = io_port_cap_slot`, `arg1 = port`, `arg2 = 0` | byte value or error status |
| 28 | `SYS_IO_WRITE` | `arg0 = io_port_cap_slot`, `arg1 = port`, `arg2 = byte value` | status |
| 29 | `SYS_IRQ_WAIT` | `arg0 = interrupt_line_cap_slot`, `arg1 = timeout_ms`, `arg2 = 0` | status |
| 30 | `SYS_MMIO_MAP` | `arg0 = mmio_region_cap_slot` | mapped base address or error status |
| 31 | `SYS_RUNTIME_INSPECT` | `arg0 = process_control_cap_slot`, `arg1 = user_ptr`, `arg2 = max_len` | byte count or error status |
| 32 | `SYS_DMA_MAP` | `arg0 = dma_region_cap_slot`, `arg1 = mapping_info_ptr`, `arg2 = mapping_info_len` | status |
| 33 | `SYS_IO_READ16` | `arg0 = io_port_cap_slot`, `arg1 = port`, `arg2 = 0` | 16-bit value or error status |
| 34 | `SYS_IO_WRITE16` | `arg0 = io_port_cap_slot`, `arg1 = port`, `arg2 = 16-bit value` | status |
| 35 | `SYS_IO_READ32` | `arg0 = io_port_cap_slot`, `arg1 = port`, `arg2 = 0` | 32-bit value or error status |
| 36 | `SYS_IO_WRITE32` | `arg0 = io_port_cap_slot`, `arg1 = port`, `arg2 = 32-bit value` | status |
| 37 | `SYS_PROCESS_START` | `arg0 = process_control_cap_slot`, `arg1 = pid`, `arg2 = 0` | status |
| 38 | `SYS_PROCESS_KILL` | `arg0 = process_control_cap_slot`, `arg1 = pid`, `arg2 = status` | status |
| 39 | `SYS_SECRET_READ` | `arg0 = secret_cap_slot`, `arg1 = user_ptr`, `arg2 = max_len` | byte count or error status |
| 40 | `SYS_VIRTIO_DEVICE_PROBE` | `arg0 = virtio_device_cap_slot` | status |
| 41 | `SYS_VIRTIO_RNG_READ` | `arg0 = virtio_rng_cap_slot`, `arg1 = user_ptr`, `arg2 = max_len` | byte count or error status |
| 42 | `SYS_VIRTIO_NET_TX` | `arg0 = virtio_net_cap_slot`, `arg1 = frame_ptr`, `arg2 = frame_len` | status |
| 43 | `SYS_VIRTIO_NET_RX` | `arg0 = virtio_net_cap_slot`, `arg1 = frame_ptr`, `arg2 = max_len` | byte count or error status |
| 44 | `SYS_NETWORK_SEND_UDP` | `arg0 = network_port_cap_slot`, `arg1 = payload_ptr`, `arg2 = payload_len` | status |
| 45 | `SYS_NAMESPACE_RESOLVE` | `arg0 = namespace_cap_slot`, `arg1 = path_ptr`, `arg2 = target_slot << 32 \| path_len` | status |
| 46 | `SYS_NETWORK_RECV_UDP` | `arg0 = network_port_cap_slot`, `arg1 = payload_ptr`, `arg2 = max_len` | byte count, `STATUS_EMPTY`, or error status |
| 47 | `SYS_VIRTIO_DEVICE_REPORT` | `arg0 = virtio_device_cap_slot`, `arg1 = report_ptr`, `arg2 = 64` | status |
| 48 | `SYS_VFS_OPEN` | `arg0 = cap_slot`, `arg1 = path_ptr`, `arg2 = open_flags << 32 \| path_len` | file handle or error status |
| 49 | `SYS_VFS_READ` | `arg0 = file_handle`, `arg1 = user_ptr`, `arg2 = max_len` | byte count or error status |
| 50 | `SYS_VFS_CLOSE` | `arg0 = file_handle` | status |
| 51 | `SYS_VFS_STAT` | `arg0 = file_handle`, `arg1 = stat_ptr`, `arg2 = max_len` | byte count or error status |
| 52 | `SYS_VFS_SEEK` | `arg0 = file_handle`, `arg1 = offset`, `arg2 = whence` | new offset or error status |
| 53 | `SYS_VFS_PREAD` | `arg0 = file_handle`, `arg1 = user_ptr`, `arg2 = offset << 32 \| max_len` | byte count or error status |
| 54 | `SYS_VFS_WRITE` | `arg0 = file_handle`, `arg1 = user_ptr`, `arg2 = len` | byte count or error status |
| 55 | `SYS_VFS_PWRITE` | `arg0 = file_handle`, `arg1 = user_ptr`, `arg2 = offset << 32 \| len` | byte count or error status |
| 56 | `SYS_VFS_SYNC` | `arg0 = file_handle` | status |
| 57 | `SYS_VFS_DUP` | `arg0 = file_handle`, `arg1 = dup_flags` | new file handle or error status |
| 58 | `SYS_VFS_CREATE` | `arg0 = vfs_root_cap_slot`, `arg1 = path_ptr`, `arg2 = create_flags << 32 \| path_len` | status |
| 59 | `SYS_VFS_UNLINK` | `arg0 = vfs_root_cap_slot`, `arg1 = path_ptr`, `arg2 = path_len` | status |
| 60 | `SYS_VFS_DERIVE_ROOT` | `arg0 = vfs_root_cap_slot`, `arg1 = path_ptr`, `arg2 = target_slot << 32 \| path_len` | status |
| 61 | `SYS_VFS_LOCK` | `arg0 = file_handle`, `arg1 = lock_flags` | status |
| 62 | `SYS_VFS_UNLOCK` | `arg0 = file_handle` | status |
| 63 | `SYS_VFS_READDIR` | `arg0 = directory_handle`, `arg1 = dirent_ptr`, `arg2 = max_len` | byte count, `0` at end, or error status |
| 64 | `SYS_VFS_MOUNT` | `arg0 = vfs_root_cap_slot`, `arg1 = path_ptr`, `arg2 = mount_flags << 32 \| path_len` | status |
| 65 | `SYS_VFS_UNMOUNT` | `arg0 = vfs_root_cap_slot`, `arg1 = path_ptr`, `arg2 = path_len` | status |
| 66 | `SYS_VFS_RENAME` | `arg0 = vfs_root_cap_slot`, `arg1 = rename_request_ptr`, `arg2 = request_len` | status |

## Return Status Values

| Name | Value | Meaning |
| --- | --- | --- |
| `STATUS_OK` | `0` | Operation accepted. |
| `STATUS_BAD_CAPABILITY` | `u64::MAX - 1` | The process does not hold a suitable capability in the requested slot. |
| `STATUS_BAD_BUFFER` | `u64::MAX - 2` | The user pointer/range failed validation before copying. |
| `STATUS_TOO_LARGE` | `u64::MAX - 3` | IPC message length exceeded the kernel's fixed message buffer. |
| `STATUS_EMPTY` | `u64::MAX - 4` | Endpoint had no message and no process could be scheduled after blocking. |
| `STATUS_RUNNING` | `u64::MAX - 8` | `SYS_PROCESS_WAIT` target has not exited. |
| `STATUS_TIMEOUT` | `u64::MAX - 9` | A timed IPC receive or IRQ wait expired before an event arrived. |
| `STATUS_PROCESS_FAULT` | `u64::MAX - 10` | The target exited because of a contained userspace fault. |
| `STATUS_VFS_PERMISSION` | `u64::MAX - 32` | VFS authority or rights did not cover the requested operation. |
| `STATUS_VFS_BAD_PATH` | `u64::MAX - 33` | VFS path syntax or path length is invalid. |
| `STATUS_VFS_NOT_FOUND` | `u64::MAX - 34` | No VFS node exists at the requested path. |
| `STATUS_VFS_NOT_DIRECTORY` | `u64::MAX - 35` | The operation required a directory node. |
| `STATUS_VFS_NOT_FILE` | `u64::MAX - 36` | The operation required a readable/writable file node. |
| `STATUS_VFS_BUSY` | `u64::MAX - 37` | The VFS node is pinned by live handles, children, or a conflicting advisory lock. |
| `STATUS_VFS_BAD_HANDLE` | `u64::MAX - 38` | The file handle is invalid, stale, or already closed. |
| `STATUS_VFS_UNSUPPORTED` | `u64::MAX - 39` | The VFS node or flag combination is not supported by this ABI generation. |
| `STATUS_VFS_NO_SPACE` | `u64::MAX - 40` | The VFS handle, object, or memory-file quota is exhausted. |
| `STATUS_VFS_EXISTS` | `u64::MAX - 41` | A create operation targeted an existing VFS node. |
| `u64::MAX` | `u64::MAX` | Unknown syscall number. |

For `SYS_IPC_RECV`, any return value less than or equal to the destination
buffer length is a delivered byte count. The current demo treats the high status
values above as errors.

## User Memory Rules

Syscalls must not directly trust userspace pointers.

ABI v1 validation checks:

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

Current M14-M75 layout:

```text
vertex-init:
  cap[0] = boot module krustboot-manifest, rights=read
  cap[1] = endpoint serial-log, rights=send
  cap[2] = process-control object, rights=control|allocate|delegate|revoke|inspect|create|start|kill|wait
  cap[3] = endpoint readiness, rights=receive
  cap[4+] = endpoint authority caps, rights=send, one per declared
           graph endpoint beyond serial-log/readiness
  cap[30] = timer monotonic-timer, rights=control, for supervised restart backoff

logd:
  cap[0] = endpoint log-sink, rights=receive
  cap[1] = endpoint serial-log, rights=send
  cap[2] = endpoint readiness, rights=send
  cap[3] = endpoint serial-console, rights=send after vertex-init derives and transfers it
  cap[4] = vfs-root cap:vfs.logd-log-stream, root=/proc/log-stream, rights=read|resolve
  cap[5] = config config:logd, rights=read
  cap[6] = secret secret:logd-token, rights=read|inspect-metadata

serial-driver:
  cap[0] = endpoint serial-console, rights=receive
  cap[1] = endpoint serial-log, rights=send
  cap[2] = endpoint readiness, rights=send
  cap[3] = io-port cap:io.com1, rights=read|write
  cap[5] = virtio-device device:virtio-console0, rights=control

netstack:
  cap[1] = endpoint serial-log, rights=send
  cap[2] = endpoint readiness, rights=send
  cap[3] = virtio-device device:virtio-rng0, rights=control
  cap[5] = virtio-device device:virtio-net0, rights=control
  cap[6] = network-port cap:net.udp.9000, rights=control

block-driver:
  cap[0] = endpoint vertex-store-block-request, rights=receive
  cap[1] = endpoint serial-log, rights=send
  cap[2] = endpoint readiness, rights=send
  cap[3] = endpoint vertex-state-block-request, rights=receive
  cap[4] = endpoint vertex-store-block-reply, rights=send after vertex-init derives and transfers it
  cap[5] = endpoint vertex-state-block-reply, rights=send after vertex-init derives and transfers it
  cap[6] = io-port cap:io.pci-config, rights=read|write
  cap[7] = interrupt-line cap:irq.virtio-blk0, rights=listen
  cap[8] = dma-region cap:dma.virtio-blk0, rights=read|write|map
  cap[9] = io-port cap:io.virtio-blk0, rights=read|write
  cap[10] = vfs-root cap:vfs.block-dev-blk0, root=/dev/device:virtio-blk0, rights=read|resolve
  cap[11] = pci-device device:virtio-blk0, rights=control
  cap[12] = virtio-device device:virtio-blk0, rights=control

vertex-store:
  cap[0] = endpoint store-hello-text-request, rights=receive
  cap[1] = endpoint serial-log, rights=send
  cap[2] = endpoint readiness, rights=send
  cap[3] = endpoint vertex-store-block-reply, rights=receive
  cap[4] = endpoint vertex-store-block-request, rights=send after vertex-init derives and transfers it
  cap[5] = endpoint model-reader-store-reply, rights=send after vertex-init derives and transfers it
  cap[6] = dynamic init store reply endpoint, rights=send during M37 generation fetch

vertex-state:
  cap[0] = endpoint vertex-state-block-reply, rights=receive
  cap[1] = endpoint serial-log, rights=send
  cap[2] = endpoint readiness, rights=send
  cap[3] = endpoint vertex-state-block-request, rights=send after vertex-init derives and transfers it
  cap[6] = endpoint state-vfs-reply, rights=send, kernel-owned VFS transaction reply endpoint
  cap[7] = endpoint state-vfs-request, rights=receive, kernel-owned VFS transaction request endpoint

vertex-inspect:
  cap[0] = process-control object, rights=inspect after vertex-init transfers it
  cap[1] = endpoint serial-log, rights=send
  cap[3] = boot module krustboot-manifest, rights=read after vertex-init transfers it

echo:
  process mount root = /state
  cap[1] = endpoint serial-log, rights=send
  cap[3] = network-port cap:net.udp.9000, rights=bind|listen
  cap[4] = namespace cap:namespace.echo, rights=resolve
  cap[5] = vfs-root cap:vfs.echo-state-a, root=/state/a, rights=read|resolve
  cap[6] = vfs-root cap:vfs.echo-state-writer, root=/state, rights=read|write|resolve|create|unlink|rename|mount
  cap[0] = endpoint log-sink, rights=send after vertex-init derives and transfers it

model-reader:
  cap[0] = endpoint model-reader-store-reply, rights=receive
  cap[1] = endpoint serial-log, rights=send
  cap[3] = endpoint store-hello-text-request, rights=send after vertex-init derives and transfers it

counter-service:
  cap[0] = vfs-root cap:vfs.counter-state, root=/state/counter, rights=read|write|resolve
  cap[1] = endpoint serial-log, rights=send
  console variant: cap[0] = endpoint cap:counter.request, rights=receive;
                   cap[3] = endpoint cap:console-shell.counter.reply, rights=send;
                   cap[4] = vfs-root cap:vfs.counter-state, root=/state/counter, rights=read|write|resolve;
                   cap[5] = vfs-root cap:vfs.counter-state-control, root=/state/counter/control, rights=write|resolve

reader-service:
  cap[0] = vfs-root cap:vfs.state-reader-state, root=/state/counter, rights=read|resolve
  cap[1] = endpoint serial-log, rights=send
  cap[3] = vfs-root cap:vfs.state-reader-control, root=/state/counter/control, rights=write|resolve
  cap[4] = namespace cap:namespace.reader, rights=resolve

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

M40 uses directed request/reply IPC. A service request endpoint is a one-way
FIFO: clients hold `send`, the provider holds
`receive`, and replies go to a separate reply endpoint where the client holds
`receive` and the provider receives a delegated `send` cap. Native endpoint
requirements are send-only; provider receive authority is derived from
`provides`, and vertex-init's static endpoint authority is send-only.
When vertex-init creates a private dynamic reply endpoint, it transfers `send`
to the provider and attenuates its local cap to `receive` before waiting.

The native activation path uses the same rule:

```text
SYS_BOOT_READ requires cap[0] read rights to the manifest boot module.
SYS_LOG_WRITE requires cap[1] send rights to the serial-log endpoint.
SYS_ACTIVATE_GENERATION requires cap[2] control and revoke rights to process-control.
SYS_PROCESS_CREATE requires create rights on process-control.
SYS_PROCESS_START requires start rights on process-control and a live pid.
SYS_PROCESS_KILL requires kill rights on process-control and a live pid.
SYS_PROCESS_WAIT requires wait rights on process-control and a live pid.
SYS_ROLLBACK_GENERATION requires cap[2] control and revoke rights to process-control.
SYS_RUNTIME_INSPECT requires inspect rights on process-control.
SYS_CAP_TRANSFER requires a caller-supplied process-control cap slot and applies the packed rights mask.
SYS_ENDPOINT_CREATE requires allocate rights on process-control and available endpoint quota.
SYS_QUOTA_DELEGATE requires delegate rights on process-control and cannot exceed the caller quota.
SYS_OBJECT_READ is a removed direct-object-read slot and is rejected. Immutable
store objects and config objects are read through VFS file handles.
SYS_VFS_OPEN resolves either a direct immutable store/config cap with an empty
path or a vfs-root cap plus an absolute process-local path covered by that root.
For VFS-root caps, absolute path arguments are first resolved under the current
process mount root declared by the compact process record. A process with mount
root `/state` sees `/a` as canonical `/state/a`; opening an empty VFS-root path
opens that process mount root. Direct store/config caps keep the empty-path
rule and do not use process mount-root rewriting. Namespace caps and direct
hardware-device caps are not filesystem authority. Device-node opens through a
VFS root additionally require that the current process holds the underlying
virtio-device control cap. The open flags are native Krust bits: read `1`,
write `2`, create `4`, trunc `8`, append `16`. Create, trunc, and append
require write. `SYS_VFS_STAT` returns 32 bytes:
kind, byte length, vnode id, and handle rights as little-endian `u64` values.
For service-backed state-volume value files, stat validates the destination
buffer, blocks in `blocked-vfs-state`, asks the state service for the durable
current length, and then copies the normal 32-byte stat record into userspace.
Node kinds are regular file `1`, directory `2`, device node `3`, pipe `4`, and
synthetic node `5`. VFS syscalls return `STATUS_VFS_*` for filesystem
conditions such as permission denial, not-found paths, busy nodes, stale
handles, unsupported node/flag combinations, and exhausted VFS quotas; they do
not collapse those conditions into `STATUS_BAD_CAPABILITY`. `SYS_VFS_DUP` flag
`1` shares the open-file offset; flag
`0` creates an independent open-file description with a copied offset.
`/proc/log-stream` is a live Krust pipe node. `SYS_VFS_READ` on an empty pipe
validates the destination buffer, saves the current syscall frame, marks the
process `blocked-vfs`, and returns only after the next kernel log write copies
bytes into the reader and wakes it. Writes that occur without a blocked reader
are not retained as compatibility backlog.
`SYS_VFS_OPEN` with the create flag creates a missing volatile regular memory
file below an existing covered directory after validating the path, `create`
authority, and the requested file-handle rights; opening an existing node with
the create flag does not add directory-create rights to the resulting handle.
If handle allocation fails after creating the vnode, the kernel removes the
new vnode and memory-file backing before returning the quota error.
SYS_VFS_CREATE creates a volatile regular memory file below an existing VFS
directory covered by a vfs-root authority that has both `resolve` and `create`.
SYS_VFS_UNLINK removes a volatile memory file covered by `resolve` and
`unlink`; it rejects directories, non-volatile nodes, non-empty subtrees, and
nodes with live open-file descriptions. SYS_VFS_DERIVE_ROOT creates a new
kernel VFS-root object for an existing directory path covered by the source
VFS-root cap; the derived cap keeps the source file-right mask and is linked to
the source cap id, so normal cap-copy or cap-transfer attenuation can delegate
read-only subtree authority without using M59 namespaces as filesystem roots.
SYS_VFS_LOCK creates a nonblocking whole-file advisory lock on a regular-file
open description. Lock flag `1` is shared and requires a read handle; flag `2`
is exclusive and requires a write handle. Shared locks are compatible with
other shared locks, exclusive locks conflict with all other descriptions on
the same vnode, and conflicts return `STATUS_VFS_BUSY`. A shared dup uses the
same open-file description and therefore the same lock ownership; an
independent dup has independent lock ownership. SYS_VFS_UNLOCK drops the lock
held by that open-file description. Closing the final handle for an open-file
description, process exit, process fault, restart reload, kill, and reap all
release its VFS locks.
SYS_VFS_READDIR reads one directory entry from a directory handle and advances
that handle's directory offset by one entry. The user buffer must be at least
96 bytes. The record is little-endian `u64` fields for node kind, vnode id, and
name length, followed by 64 bytes of zero-padded name storage and 8 reserved
zero bytes. End of directory returns `0`. Directory handles require `resolve`
rights and are obtained by opening a directory with the native read flag.
SYS_VFS_MOUNT with mount flag `1` creates an empty volatile mounted directory
at a missing covered path. It requires `resolve` and `mount` authority on the
parent directory and returns `STATUS_VFS_EXISTS` if a node already occupies the
mount path. SYS_VFS_UNMOUNT removes a dynamic mount root when the caller has
`resolve` and `mount` authority over that root; built-in roots are unsupported,
and roots with live handles or children return `STATUS_VFS_BUSY`.
SYS_VFS_RENAME takes a native request buffer:
`old_path_len:u64`, `new_path_len:u64`, old path bytes, then new path bytes. It
requires `resolve` and `rename` authority over both parent directories, rejects
replacement if the destination already exists, and currently supports volatile
memory-file vnodes. The vnode id is preserved across the rename, so live handles
continue to reference the same open file description.
SYS_SECRET_READ requires read rights on a secret cap and logs metadata only.
Native state-volume records are installed as explicit VFS mount roots below
`/state/<state-id suffix>`; direct state-object grants are rejected. Each
manifest-declared state volume exposes `/state/<suffix>/value` as a regular
file whose read/write/stat operations are VFS transactions: the kernel validates
the user buffer, queues a native versioned `VS` request carrying the state id on
the kernel-owned `state-vfs-request` endpoint, blocks the caller in
`blocked-vfs-state`, and wakes it from the kernel-owned `state-vfs-reply`
endpoint when `vertex-state` replies. The old short state commands are not an
accepted compatibility protocol. `/state/<suffix>/control` accepts the native
control command `Q` through the same VFS transaction path and is used for
state-service shutdown. `vertex-state` persists write transactions for every
indexed VertexDisk state volume through the block protocol served by
`block-driver`.
Store and state traffic use separate block request endpoints; the driver treats
the receiving endpoint as the client identity and enforces read-only store
access, state-only writes, and section bounds before
performing sector I/O.
SYS_SLEEP_MS requires control rights on a timer cap.
SYS_IO_READ, SYS_IO_READ16, and SYS_IO_READ32 require read rights on an io-port cap and a fully covered port span inside the granted range.
SYS_IO_WRITE, SYS_IO_WRITE16, and SYS_IO_WRITE32 require write rights on an io-port cap and a fully covered port span inside the granted range.
SYS_IRQ_WAIT requires listen rights on an interrupt-line cap.
SYS_MMIO_MAP requires map rights on an mmio-region cap.
SYS_DMA_MAP requires read, write, and map rights on a dma-region cap and an
8-byte-aligned output buffer for the three-field mapping record.
SYS_VIRTIO_DEVICE_PROBE requires control rights on a virtio-device cap.
SYS_VIRTIO_DEVICE_REPORT requires control rights on a virtio-device cap and a
64-byte driver report containing queue size, avail/used indices, submission,
completion, timeout, reset, and typed last-error counters.
SYS_VIRTIO_RNG_READ requires control rights on a virtio-device cap whose device
ID is the RNG device and whose transport is `virtio-pci-io`.
SYS_VIRTIO_NET_TX and SYS_VIRTIO_NET_RX require control rights on a
virtio-device cap whose device ID is the network device and whose transport is
`virtio-pci-io`.
SYS_NETWORK_SEND_UDP requires bind and listen rights on a network-port cap and
queues the payload for the network provider.
SYS_NETWORK_RECV_UDP requires control rights on a network-port cap and returns
one queued application UDP payload to the provider. When no payload is pending,
the provider blocks on the network-port queue and wakes when a client sends a
payload; `STATUS_EMPTY` is reserved for the no-schedulable-process fallback.
SYS_NAMESPACE_RESOLVE requires resolve rights on a namespace cap and installs only the configured attenuated target capability.
```

Native network-port objects grant bind/listen authority to declared application
services and a control provider cap to `netstack`. M57 consumes that object
through `SYS_NETWORK_SEND_UDP`; `netstack` drains the queue through
`SYS_NETWORK_RECV_UDP` and performs raw virtio-net TX through its separate
driver capability. Raw virtio-net TX/RX remains driver-facing authority and is
not implied by network-port access.

Native I/O objects now cover the first hardware authority substrate:
`IoPortRange`, `MmioRegion`, `InterruptLine`, `DmaRegion`, `PciDevice`, and
`VirtioDevice`. `DmaRegion` authority is represented and granted to
`block-driver`; `SYS_DMA_MAP` maps the region into the calling driver and
returns `{ virtual_base, physical_base, length }`. `PciDevice` and
`VirtioDevice` are ownership objects: they are granted only to the declared
driver service and are intentionally not exposed to unprivileged consumers by
default.

Namespace objects are capability objects, not ambient paths. A namespace maps
absolute names to existing non-namespace capability objects with explicit
attenuated rights. Resolution requires `resolve`, writes the derived capability
into the caller-selected slot, and fails if the path is absent.

VFS root objects are separate capability objects. They carry an absolute root
path and authorize covered canonical filesystem paths for VFS syscalls according
to the capability rights on the grant. Process `mountRoot` values are also
absolute VFS paths and are validated at boot against existing directory nodes.
M59 namespaces remain string-to-capability aliasing; they are not accepted as
VFS root authority.
VFS mount objects are kernel objects rooted at specific vnodes. The initial
runtime installs explicit rootfs, storefs, volatile state, devfs, and procfs
mount objects, and dynamic volatile mounts are visible through runtime inspect
until unmounted.

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
capability id while clearing the source slot, and rejects an occupied or invalid
target slot before clearing the source. `SYS_CAP_REVOKE` marks a cap id and its
descendants revoked; later lookup rejects revoked caps, caps with revoked
ancestors, and caps whose `generation_id` differs from the active runtime
generation. `SYS_CAP_INSPECT` prints the current metadata to the serial
transcript and returns the parent capability id.

Process-control authority now distinguishes resource rights:

```text
control
allocate
delegate
revoke
inspect
```

The initial process starts with endpoint quota `1`. Services start with endpoint
quota `0` unless delegated a smaller quota through `SYS_QUOTA_DELEGATE`.
`SYS_ENDPOINT_CREATE` consumes endpoint quota and installs a send/receive cap in
the caller's requested slot.

`SYS_ACTIVATE_GENERATION` now performs a native generation switch. It requires
process-control and revoke authority, resolves the requested generation ID
against the kernel-registered KrustBoot runtime configs, records the previous
generation as the rollback target, replaces the runtime process/object/capability
tables, and enters the new generation's `vertex-init`.

## Process Model

ABI v1 uses a fixed-size kernel process table.

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

Only the initial process is installed in the runtime process table at boot.
Non-initial compact-manifest process records are process templates. A holder of
process-control `create` authority turns a template into a runtime process with
a fresh pid and an explicit initial capability table.

`SYS_PROCESS_CREATE` semantics:

```text
requires create rights on the process-control cap
target process template index must exist in the compact manifest process table
the template executable must resolve to a verified native store object
the initial capability table is assembled from explicit grants and transfers
on success: returns a fresh pid in Declared state
on failure: STATUS_BAD_CAPABILITY
```

`SYS_PROCESS_START` semantics:

```text
requires start rights on the process-control cap
target pid must name a created runtime process
target process state must be Declared or Exited
on success: Declared -> Ready
restart success: Exited -> Ready with the restart context and initial caps restored
on failure: STATUS_BAD_CAPABILITY
```

`SYS_PROCESS_WAIT` semantics:

```text
requires wait rights on the process-control cap
target pid must name a created runtime process
returns STATUS_RUNNING until the target exits
returns the target exit status once available
```

Restart uses the same syscall; restarting an exited process resets its saved
frame, exit status, capability table, and user context before making it Ready
again. ABI v1 native supervision is explicitly bounded to one restart per
service; `restart = always` means "restart after the first exit" in this proof,
not an unbounded service-manager loop. `SYS_PROCESS_ATTEMPT` lets a process
distinguish its first start from a kernel-mediated restart without relying on
preserved process memory.

`SYS_SLEEP_MS` moves the caller into `Sleeping` and yields to another ready
process. Deadlines use CPUID-reported TSC/base frequency when available, with a
fixed fallback only when the CPU does not report a usable frequency. If no
process is ready, ABI v1 waits through the PIT interrupt path instead of
accepting a cooperative-only TSC polling fallback.

User page faults are process-contained. A direct userspace page fault identifies
the current process, marks that process `Exited` with
`STATUS_PROCESS_FAULT = u64::MAX - 10`, schedules another ready process, and
keeps the kernel running. Kernel faults still stop the kernel.

## IPC Semantics

Endpoints hold a fixed four-message FIFO. Each message is capped at 512 bytes,
which is large enough for M43 VertexDisk fixed-sector block replies. The FIFO
is safe for request endpoints because only providers receive from request
queues and only clients receive from their private reply queues.

Send path:

```text
SYS_IPC_SEND(cap_slot, user_ptr, len)
  validate send capability
  copy message from user
  append message to endpoint FIFO
  wake one process blocked on that endpoint, if any
```

Receive path:

```text
SYS_IPC_RECV(cap_slot, user_ptr, max_len)
  validate receive capability
  validate writable user buffer
  if a queued message from another process exists:
      copy to receiver buffer and return byte count
  if no matching message exists:
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
service templates:

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
  switches to a registered native generation and enters its vertex-init

SYS_PROCESS_CREATE(cap[2], process_template_index, 0)
  creates a runtime process from a verified compact-manifest template and returns a pid

SYS_PROCESS_START(cap[2], pid, 0)
  starts a created runtime process

SYS_CAP_DERIVE(cap[4], cap[31], send)
  derives attenuated endpoint authority for echo

SYS_CAP_TRANSFER(cap[2], echo_pid, packed(cap[31], target_cap[0], send))
  transfers the attenuated endpoint authority before echo starts

SYS_CAP_INSPECT(cap[31])
  prints provenance metadata for the derived capability

SYS_ENDPOINT_CREATE(cap[2], cap[29])
  creates one dynamic endpoint when endpoint quota is available

SYS_QUOTA_DELEGATE(cap[2], target_pid, max_endpoints)
  delegates a bounded endpoint quota to a target process

SYS_PROCESS_WAIT(cap[2], pid, 0)
  observes exits for supervision and restart policy

SYS_ROLLBACK_GENERATION(cap[2], parent_generation_id, len)
  reinitializes runtime tables from the prepared fallback manifest and enters
  the fallback generation's initial process

SYS_IPC_RECV_TIMEOUT(cap[3], buffer, packed(timeout_ms, len))
  waits for readiness with a scheduler timeout
```

`vertex-init` reads the compact manifest, computes the activation order from
manifest dependencies, creates each service dynamically, waits for logd
readiness, derives and transfers endpoint caps with the rights requested by
each consumer, starts the created services, and observes their exit status.
Negative capability tests prove echo cannot receive on its send-only cap, logd
and echo cannot write COM1 directly, echo cannot talk to block-driver or
device objects without authority, logd cannot create processes without
process-control, services cannot read ungranted config or secret objects, and
reader-service write attempts are denied by `vertex-state`.

## Boot ABI

Limine loads:

```text
krust.elf
hello-generation.krustboot
fallback-generation.krustboot
krust-block.img
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
state_volumes (installed as explicit VFS state-volume mount roots; direct state grants are rejected; native Krust routes manifest-declared state volumes through service-backed VFS transactions)
network_ports
io_port_ranges
mmio_regions
interrupt_lines
dma_regions
pci_devices
virtio_devices
namespaces
vfs_roots
```

The compact payload identity is `KRUSTBOOTM75` version 11. Older compact
payload identities, including the previous M61 identity, are rejected instead of
being retained as compatibility formats.

Manifest v1 adds a fixed header, record table, checksum, and record bounds
validation. The current record kinds are boot modules, process templates,
endpoints, grants, store objects, state volumes, timer, generation, and policy. The kernel
requires the v1 wrapper at the boot-module boundary and rejects an unwrapped
compact payload. After validating the wrapper, the kernel exposes the compact
payload to `vertex-init` through cap[0] so native userspace parsing stays small.
Each compact process record carries `name`, `module`, `service`, restart and
health policy, and `mount_root`; hosted `vertexctl` derives `mount_root` from
the service `mountRoot` field and defaults it to `/` when absent. Older compact
process records without `mount_root` are rejected by the version check rather
than accepted through a compatibility parser.

Krust also creates fixed boot caps for native `vertex-init`:

```text
cap[0] manifest module read
cap[1] serial-log send
cap[2] process-control control|allocate|delegate|revoke|inspect|create|start|kill|wait
cap[3] readiness receive
cap[4+] endpoint authority for graph-delegated endpoints, one authority cap per
declared endpoint beyond the fixed serial-log/readiness endpoints
cap[30] monotonic timer control for restart backoff
```

Endpoint, hardware, store-object, config, secret, network, device, and timer
grants for service templates come from the compact manifest and native object
registry. Endpoint consumers do not receive static boot send grants for
delegated authority; vertex-init derives and transfers the attenuated cap after
creating the target process and before starting it. A transfer to a Declared
process becomes part of that process's restart baseline, so the bounded ABI v1
restart restores the delegated endpoint cap along with static grants. If a
service both provides an endpoint and consumes delegated endpoint authority, the
provided endpoint keeps cap[0] and delegated endpoint caps start at cap[3] to
avoid the serial-log and readiness slots. Quota delegated before first start is
also part of the restart baseline, so restarted services receive the same
endpoint allocation budget they had at initial launch.
