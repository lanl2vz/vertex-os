use core::{
    arch::asm,
    cell::UnsafeCell,
    sync::atomic::{Ordering, compiler_fence},
};

pub use crate::kernel::{
    BOOT_OBJECT_DMA_REGION, BOOT_OBJECT_ENDPOINT, BOOT_OBJECT_FRAMEBUFFER,
    BOOT_OBJECT_INTERRUPT_LINE, BOOT_OBJECT_IO_PORT_RANGE, BOOT_OBJECT_MMIO_REGION,
    BOOT_OBJECT_NAMESPACE, BOOT_OBJECT_NETWORK_PORT, BOOT_OBJECT_PCI_DEVICE, BOOT_OBJECT_SECRET,
    BOOT_OBJECT_STATE, BOOT_OBJECT_STORE, BOOT_OBJECT_TIMER, BOOT_OBJECT_VFS_ROOT,
    BOOT_OBJECT_VIRTIO_DEVICE, BOOT_POLICY_VERSION, BootDmaRegionConfig, BootEndpointConfig,
    BootFramebufferConfig, BootGrantConfig, BootGraphEdgeConfig, BootGraphNodeConfig,
    BootInterruptLineConfig, BootIoPortRangeConfig, BootMmioRegionConfig, BootModuleConfig,
    BootNamespaceConfig, BootNamespaceEntryConfig, BootNetworkPortConfig, BootPciDeviceConfig,
    BootPolicyBootstrapConfig, BootPolicyCapabilityConfig, BootPolicyMountConfig,
    BootPolicyProvideConfig, BootPolicyRequirementConfig, BootPolicyStatePathConfig,
    BootProcessConfig, BootProcessMountConfig, BootRuntimeConfig, BootStateVolumeConfig,
    BootStoreObjectConfig, BootVfsRootConfig, BootVirtioDeviceConfig,
};
use crate::kernel::{
    BOOT_PROCESS_MOUNT_BIND, BOOT_PROCESS_MOUNT_READ_ONLY, BootModuleObject, Capability,
    CapabilitySpace, DmaUserMapping, IpcEndpoint, IpcMessage, KernelObject, MAX_BOOT_NAMESPACES,
    MAX_BOOT_STATE_VOLUMES, MAX_BOOT_VFS_ROOTS, MAX_CAPS, MAX_MESSAGE_BYTES, MAX_OBJECTS,
    MAX_OPEN_FILE_DESCRIPTIONS, MAX_PROCESSES, ObjectTable, Process, ProcessControlObject,
    ProcessState, ProcessTable, SecretObject, StoreObject, known_boot_process_mount_flags,
};
pub use crate::kernel::{
    FRAME_R8, FRAME_R9, FRAME_R10, FRAME_R11, FRAME_R12, FRAME_R13, FRAME_R14, FRAME_R15,
    FRAME_RAX, FRAME_RBP, FRAME_RBX, FRAME_RCX, FRAME_RDI, FRAME_RDX, FRAME_RSI, FRAME_SIZE,
    FRAME_USER_CS, FRAME_USER_RFLAGS, FRAME_USER_RIP, FRAME_USER_RSP, FRAME_USER_SS, InitError,
    IpcError, KernelObjectId, ProcessContext, ProcessId, ScheduleResult, SyscallFrame,
};
use crate::{
    capability,
    device::{
        DmaRegionObject, FramebufferObject, InterruptLineObject, IoPortRangeObject,
        MmioRegionObject, NetworkPortObject, TimerObject, VirtioDeviceObject, VirtioNetState,
        VirtioQueueState, VirtioRngState,
    },
    gdt,
    inspect::InspectReport,
    limine, memory, paging, serial, timer,
    usercopy::{self, UserPtr},
    userspace,
    vfs::{
        FileDescriptionId, FileHandle, MAX_NAMESPACE_ENTRIES, MAX_VERTEXFS_FILE_BYTES,
        MAX_VERTEXFS_FILES, MAX_VFS_MEM_FILE_BYTES, MAX_VFS_NAME_BYTES, MAX_VFS_PATH_BYTES,
        MAX_VFS_PIPE_BYTES, NamespaceEntry, NamespaceObject, OpenFileDescription,
        VERTEXFS_DIRECTORY_SECTOR, VERTEXFS_DIRECTORY_SECTORS, VERTEXFS_DYNAMIC_FILE_CAPACITY,
        VERTEXFS_FREE_MAP_SECTOR, VERTEXFS_IMAGE_BYTES, VERTEXFS_INODE_APP_DIR,
        VERTEXFS_INODE_TABLE_SECTOR, VERTEXFS_INODE_TABLE_SECTORS, VERTEXFS_JOURNAL_PAYLOAD_OFFSET,
        VERTEXFS_JOURNAL_SECTOR, VERTEXFS_MODULE_STRING, VERTEXFS_SECTOR_SIZE,
        VERTEXFS_SYNC_MAX_DEVICE_WRITES, VertexFsDeviceWrite, VertexFsInode, VertexFsSyncResult,
        VfsBacking, VfsEvent, VfsLock, VfsLockMode, VfsMemoryFile, VfsName, VfsNode, VfsNodeId,
        VfsNodeKind, VfsPath, VfsPipeBuffer, VfsStateOperation, VfsVertexFsFile,
        parse_vertexfs_image, state_volume_mount_component, valid_vfs_root_path,
        vertexfs_checksum32, vertexfs_device_absolute_sector, vertexfs_dynamic_data_sector_at,
        vertexfs_dynamic_inode_at, vertexfs_image_has_inode, vertexfs_image_sector,
        vfs_authority_path_covers, write_vertexfs_dynamic_metadata, write_vertexfs_file_extent,
        write_vertexfs_inode_record, write_vertexfs_journal_clean, write_vertexfs_journal_pending,
    },
};
use vertex_abi::graph as graph_abi;

mod boot_runtime;
mod capability_syscalls;
mod core_syscalls;
mod device_syscalls;
mod endpoint;
mod generation;
mod inspect_report;
mod memory_mapping;
mod object_lookup;
mod process_syscalls;
mod runtime_access;
mod runtime_state;
mod scheduler;
mod util;
mod vfs_nodes;
mod vfs_paths;
mod vfs_syscalls;
mod vfs_transactions;
mod vfs_wire;
use boot_runtime::{
    RuntimeReapTarget, StagingBuild, commit_staged_boot_config_runtime,
    grant_config_caps_to_process, grant_object_id, install_declared_process_mounts,
    reap_runtime_contexts, stage_boot_config_runtime, validate_boot_config_installable,
    validate_config_caps_for_process,
};
pub use boot_runtime::{init_from_boot_config, initial_process_context, install_frame_allocator};
use capability_syscalls::release_unreferenced_derived_vfs_roots;
pub use capability_syscalls::{
    cap_copy, cap_derive, cap_drop, cap_inspect, cap_move, cap_revoke, cap_transfer,
};
use core_syscalls::record_ready_lifecycle;
pub use core_syscalls::{
    endpoint_create, log_write, namespace_resolve, quota_delegate, read_boot_module, receive,
    receive_timeout, runtime_inspect, secret_read, send,
};
pub use device_syscalls::{
    dma_map, framebuffer_info, framebuffer_map, io_read, io_read16, io_read32, io_write,
    io_write16, io_write32, irq_wait, mmio_map, network_recv_udp, network_send_udp,
    record_hardware_irq, virtio_device_probe, virtio_device_report, virtio_net_rx, virtio_net_tx,
    virtio_rng_read,
};
use device_syscalls::{
    interrupt_owner_name, interrupt_waiter_count, release_all_runtime_dma_mappings,
    release_process_dma_mappings, release_process_virtio_ownership,
};
pub use endpoint::run_fifo_regression;
pub use generation::{
    activate_generation, generation_config_by_id, install_generation_recovery,
    mark_known_good_generation, register_generation_config, rollback_generation,
    set_failed_generation_id,
    set_rollback_boot_config, stage_generation, stage_rollback_generation, verify_generation,
};
use generation::{
    boot_manager, boot_manager_state, registered_generation_config_at, registered_generation_count,
    store_hash_matches, BootManagerState,
};
use inspect_report::{
    build_inspect_report, inspect_report, object_reachable_by_cap, print_boot_tables,
    print_negative, print_process_by_pid, print_rights, process_name_by_pid,
};
use memory_mapping::{
    align_down, align_up, device_user_mapping_base, map_current_process_physical_range,
    ranges_overlap, reap_process_context, unmap_current_process_physical_range,
    write_dma_mapping_info,
};
use object_lookup::{
    boot_module_from_cap, capability_has_revoked_ancestor, current_process_label,
    dma_region_from_cap, endpoint_cap_from_slot, endpoint_from_cap, frame_allocator,
    framebuffer_from_cap, interrupt_line_from_cap, io_port_from_cap, lookup_capability,
    mmio_region_from_cap, namespace_from_cap, network_port_from_cap, port_span_in_range,
    process_control_from_cap, restart_policy_label, runtime, secret_from_cap,
    serial_log_endpoint_from_cap, staging_runtime, timer_from_cap, virtio_device_from_cap,
};
pub use process_syscalls::{
    create_process, exit_current_process, fault_current_process, kill_process,
    preempt_current_process, process_attempt, process_wait, start_process,
    wake_timed_from_interrupt, yield_current_process,
};
use process_syscalls::{
    load_process_context, process_config_for_pid, reclaim_detached_address_space,
};
use runtime_access::current_process_id;
pub use runtime_access::{current_process_name, initial_process_name};
use runtime_state::{
    FRAME_ALLOCATOR, Global, INSPECT_REPORT, INSTALL_STAGING_RUNTIME, MAX_POLICY_DENIAL_RECORDS,
    POLICY_DENIAL_LOG, RUNTIME, RuntimeState, VIRTIO_NET_STATE, VIRTIO_RNG_STATE,
};
pub use scheduler::sleep_ms;
use scheduler::{
    block_current_on_endpoint, cancel_blocked_receivers_for_endpoint_owner,
    cancel_unauthorized_blocked_receivers, deadline_after_ms, read_tsc, schedule_next_ready,
    schedule_next_ready_excluding_current, schedule_next_ready_no_wait_excluding_current,
    wake_blocked_receiver, wake_timed_processes,
};
use util::min;
pub use vfs_syscalls::{
    legacy_object_read, vfs_close, vfs_create, vfs_derive_root, vfs_dup, vfs_link, vfs_lock,
    vfs_mkdir, vfs_mount, vfs_open, vfs_poll, vfs_pread, vfs_pwrite, vfs_read, vfs_readdir,
    vfs_rename, vfs_rmdir, vfs_seek, vfs_stat, vfs_sync, vfs_unlink, vfs_unlock, vfs_unmount,
    vfs_watch, vfs_write,
};
const MAX_BOOT_READ_BYTES: usize = 128 * 1024;
const MAX_VFS_NODES: usize = 96;
const MAX_VFS_MEM_FILES: usize = 8;
const BLOCK_PROTOCOL_V1: u16 = 1;
const BLOCK_OP_WRITE_SECTOR: u16 = 2;
const BLOCK_REQUEST_LEN: usize = 16;
const BLOCK_WRITE_ACK_LEN: usize = 16;
const VFS_STAT_BYTES: usize = 64;
const VFS_DIRENT_BYTES: usize = 96;
const VFS_RENAME_REQUEST_HEADER_BYTES: usize = 16;
const VFS_RENAME_REQUEST_MAX_BYTES: usize =
    VFS_RENAME_REQUEST_HEADER_BYTES + (MAX_VFS_PATH_BYTES * 2);
const MAX_VFS_LOCKS: usize = MAX_OPEN_FILE_DESCRIPTIONS;
const MAX_VFS_EVENTS: usize = 64;
const MAX_VFS_MOUNTS: usize = 16;
const BUILTIN_VFS_MOUNTS: usize = 6;
const MAX_CAP_LINEAGE: usize = 1024;
const MAX_REVOKED_CAPS: usize = MAX_CAP_LINEAGE;
const MAX_GENERATION_CONFIGS: usize = 4;
const MAX_SERVICE_LIFECYCLE_EVENTS: usize = 128;
const DMA_MAPPING_INFO_BYTES: usize = 24;
const FRAMEBUFFER_INFO_BYTES: usize = 64;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const READY_ENVELOPE_LEN: usize = 16;
const INIT_TIMER_CAP_SLOT: u64 = 30;
const VFS_OPEN_READ: u64 = 1;
const VFS_OPEN_WRITE: u64 = 1 << 1;
const VFS_OPEN_CREATE: u64 = 1 << 2;

pub(crate) fn record_policy_denial(
    generation: &str,
    policy_hash: &[u8],
    source: &str,
    target: &str,
    rule: &str,
    reason: &str,
) {
    policy_denial_log_mut().record(generation, policy_hash, source, target, rule, reason);
}

fn policy_denial_log() -> &'static runtime_state::PolicyDenialLog {
    unsafe { &*POLICY_DENIAL_LOG.0.get() }
}

fn policy_denial_log_mut() -> &'static mut runtime_state::PolicyDenialLog {
    unsafe { &mut *POLICY_DENIAL_LOG.0.get() }
}
const VFS_OPEN_TRUNC: u64 = 1 << 3;
const VFS_OPEN_APPEND: u64 = 1 << 4;
const VFS_OPEN_KNOWN_FLAGS: u64 =
    VFS_OPEN_READ | VFS_OPEN_WRITE | VFS_OPEN_CREATE | VFS_OPEN_TRUNC | VFS_OPEN_APPEND;
const VFS_DUP_SHARE_OFFSET: u64 = 1;
const VFS_LOCK_SHARED: u64 = 1;
const VFS_LOCK_EXCLUSIVE: u64 = 2;
const VFS_LOCK_RANGE: u64 = 1 << 8;
const VFS_LOCK_MODE_MASK: u64 = VFS_LOCK_SHARED | VFS_LOCK_EXCLUSIVE;
const VFS_POLL_READABLE: u64 = 1;
const VFS_POLL_WRITABLE: u64 = 1 << 1;
const VFS_POLL_METADATA: u64 = 1 << 3;
const VFS_POLL_KNOWN_EVENTS: u64 = VFS_POLL_READABLE | VFS_POLL_WRITABLE | VFS_POLL_METADATA;
const VFS_EVENT_CREATE: u64 = 1;
const VFS_EVENT_RENAME: u64 = 2;
const VFS_EVENT_UNLINK: u64 = 3;
const VFS_WATCH_EVENT_BYTES: usize = 96;
const VFS_MOUNT_VOLATILE: u64 = 1;
const VFS_MOUNT_BIND: u64 = 1 << 1;
const VFS_MOUNT_READ_ONLY: u64 = 1 << 2;
const VFS_MOUNT_KNOWN_FLAGS: u64 = VFS_MOUNT_VOLATILE | VFS_MOUNT_BIND | VFS_MOUNT_READ_ONLY;
const VFS_SEEK_SET: u64 = 0;
const VFS_SEEK_CURRENT: u64 = 1;
const VFS_SEEK_END: u64 = 2;
const VFS_NODE_KIND_REGULAR: u64 = 1;
const VFS_NODE_KIND_DIRECTORY: u64 = 2;
const VFS_NODE_KIND_DEVICE: u64 = 3;
const VFS_NODE_KIND_PIPE: u64 = 4;
const VFS_NODE_KIND_SYNTHETIC: u64 = 5;
const VFS_SYNTHETIC_INSPECT_BYTES: &[u8] = b"krust synthetic inspect node\n";
const LOG_ENDPOINT_NAME: &str = "serial-log";
const STATE_VFS_REQUEST_ENDPOINT_NAME: &str = "state-vfs-request";
const STATE_VFS_REPLY_ENDPOINT_NAME: &str = "state-vfs-reply";
const VERTEXFS_DEVICE_REQUEST_ENDPOINT_NAME: &str = "vertexfs-device-request";
const VERTEXFS_DEVICE_REPLY_ENDPOINT_NAME: &str = "vertexfs-device-reply";
const GENERATION_METADATA_BLOCK_REQUEST_ENDPOINT_NAME: &str = "generation-metadata-block-request";
const GENERATION_METADATA_BLOCK_REPLY_ENDPOINT_NAME: &str = "generation-metadata-block-reply";
const STATE_VOLUME_VALUE_FILE_NAME: &str = "value";
const STATE_VOLUME_CONTROL_FILE_NAME: &str = "control";
const BLOCK_DRIVER_PROCESS_NAME: &str = "block-driver";
const GENERATION_MANAGER_PROCESS_NAME: &str = "gen-manager";
const VERTEX_STATE_PROCESS_NAME: &str = "vertex-state";
const VERTEX_STATE_VFS_REPLY_CAP_SLOT: u64 = 6;
const VERTEX_STATE_VFS_REQUEST_CAP_SLOT: u64 = 7;
const BLOCK_DRIVER_VERTEXFS_REQUEST_CAP_SLOT: u64 = 13;
const BLOCK_DRIVER_VERTEXFS_REPLY_CAP_SLOT: u64 = 14;
const BLOCK_DRIVER_GENERATION_METADATA_REQUEST_CAP_SLOT: u64 = 16;
const BLOCK_DRIVER_GENERATION_METADATA_REPLY_CAP_SLOT: u64 = 17;
const GENERATION_MANAGER_METADATA_REQUEST_CAP_SLOT: u64 = 4;
const GENERATION_MANAGER_METADATA_REPLY_CAP_SLOT: u64 = 5;
const VFS_STATE_TRANSACTION_ID_BYTES: usize = 8;
const VFS_STATE_REQUEST_HEADER_BYTES: usize = 8;
const VFS_STATE_REQUEST_MAGIC: &[u8; 2] = b"VS";
const VFS_STATE_REQUEST_VERSION: u8 = 1;
const VFS_STATE_OP_READ_VALUE: u8 = b'R';
const VFS_STATE_OP_WRITE_VALUE: u8 = b'W';
const VFS_STATE_OP_STAT_VALUE: u8 = b'S';
const VFS_STATE_OP_CONTROL: u8 = b'C';
const VFS_SERVICE_REQUEST_MAGIC: &[u8; 2] = b"FS";
const VFS_SERVICE_REQUEST_VERSION: u8 = 1;
const VFS_SERVICE_OP_READ_REPORT: u8 = b'R';
const VFS_SERVICE_REQUEST_BYTES: usize = 12;
const VFS_SERVICE_REPORT_BYTES: &[u8] = b"servicefs:vertex-state-report\n";
const MAX_STATE_VOLUME_VALUE_BYTES: usize = 16;
const USER_MMIO_MAPPING_BASE: u64 = 0x0000_5000_0000_0000;
const USER_DMA_MAPPING_BASE: u64 = 0x0000_6000_0000_0000;
const USER_FRAMEBUFFER_MAPPING_BASE: u64 = 0x0000_7000_0000_0000;
const USER_DEVICE_MAPPING_STRIDE: u64 = 1 << 30;
const PCI_CONFIG_ADDRESS: u16 = 0x0cf8;
const PCI_CONFIG_DATA: u16 = 0x0cfc;
const PCI_VENDOR_VIRTIO: u16 = 0x1af4;
const PCI_DEVICE_VIRTIO_NET_IO_TRANSPORT: u16 = 0x1000;
const PCI_DEVICE_VIRTIO_RNG_IO_TRANSPORT: u16 = 0x1005;
const PCI_COMMAND: u8 = 0x04;
const PCI_BAR0: u8 = 0x10;
const PCI_COMMAND_IO: u16 = 1 << 0;
const PCI_COMMAND_BUS_MASTER: u16 = 1 << 2;
const PCI_COMMAND_INTERRUPT_DISABLE: u16 = 1 << 10;
const VIRTIO_PCI_HOST_FEATURES: u16 = 0x00;
const VIRTIO_PCI_GUEST_FEATURES: u16 = 0x04;
const VIRTIO_PCI_QUEUE_PFN: u16 = 0x08;
const VIRTIO_PCI_QUEUE_NUM: u16 = 0x0c;
const VIRTIO_PCI_QUEUE_SEL: u16 = 0x0e;
const VIRTIO_PCI_QUEUE_NOTIFY: u16 = 0x10;
const VIRTIO_PCI_STATUS: u16 = 0x12;
const VIRTIO_PCI_ISR: u16 = 0x13;
const VIRTIO_STATUS_ACKNOWLEDGE: u8 = 1;
const VIRTIO_STATUS_DRIVER: u8 = 2;
const VIRTIO_STATUS_DRIVER_OK: u8 = 4;
const VIRTIO_STATUS_FAILED: u8 = 0x80;
const VIRTIO_QUEUE_MIN_SIZE: u16 = 2;
const VIRTIO_QUEUE_MAX_SIZE: u16 = 256;
const VIRTIO_QUEUE_DESC_OFFSET: usize = 0;
const VIRTIO_QUEUE_RING_ALIGN: usize = 4096;
const VIRTIO_QUEUE_STRIDE: usize = 16 * 1024;
const VIRTIO_RNG_DMA_FRAMES: u64 = 4;
const VIRTIO_NET_DMA_FRAMES: u64 = 8;
const VIRTIO_POLL_SPINS: u64 = 20_000_000;
const VIRTIO_DESC_F_WRITE: u16 = 2;
const VIRTIO_AVAIL_F_NO_INTERRUPT: u16 = 1;
const VIRTIO_NET_HDR_LEN: usize = 10;
const VIRTIO_NET_RX_BUFFER_LEN: usize = 2048;
const ETHERNET_MIN_FRAME_LEN: usize = 60;
const UDP_IPV4_HEADER_LEN: usize = 42;
const VIRTIO_DESC_F_NEXT: u16 = 1;
const VIRTIO_RNG_DEVICE_ID: &str = "device:virtio-rng0";
const VIRTIO_NET_DEVICE_ID: &str = "device:virtio-net0";
const VIRTIO_PCI_IO_TRANSPORT_ID: &str = "virtio-pci-io";
const VIRTIO_DRIVER_REPORT_BYTES: usize = 64;
const VIRTIO_ERROR_NONE: u64 = 0;
const VIRTIO_ERROR_COMPLETION_TIMEOUT: u64 = 1;
const VIRTIO_ERROR_RESET_FAILED: u64 = 2;
const VIRTIO_ERROR_INIT_FAILED: u64 = 3;
const VIRTIO_ERROR_STATUS: u64 = 4;
const NATIVE_SECRET_VALUE: &[u8] = b"native-secret-value";
const STATUS_BAD_CAPABILITY: u64 = u64::MAX - 1;
const STATUS_BAD_BUFFER: u64 = u64::MAX - 2;
const STATUS_OK: u64 = 0;
const STATUS_TIMEOUT: u64 = u64::MAX - 9;
const STATUS_VFS_BAD_HANDLE: u64 = u64::MAX - 38;
const STATUS_VFS_UNSUPPORTED: u64 = u64::MAX - 39;
pub const STATUS_PROCESS_FAULT: u64 = u64::MAX - 10;
const FALLBACK_TSC_TICKS_PER_MS: u64 = 1_000_000;
const GRAPH_NODE_GENERATION: u16 = graph_abi::NODE_GENERATION;
const GRAPH_NODE_SERVICE: u16 = graph_abi::NODE_SERVICE;
const GRAPH_NODE_ENDPOINT: u16 = graph_abi::NODE_ENDPOINT;
const GRAPH_NODE_STORE_OBJECT: u16 = graph_abi::NODE_STORE_OBJECT;
const GRAPH_NODE_CONFIG: u16 = graph_abi::NODE_CONFIG;
const GRAPH_NODE_STATE_VOLUME: u16 = graph_abi::NODE_STATE_VOLUME;
const GRAPH_NODE_DEVICE: u16 = graph_abi::NODE_DEVICE;
const GRAPH_NODE_NAMESPACE: u16 = graph_abi::NODE_NAMESPACE;
const GRAPH_NODE_VFS_ROOT: u16 = graph_abi::NODE_VFS_ROOT;
const GRAPH_NODE_TIMER: u16 = graph_abi::NODE_TIMER;
const GRAPH_NODE_SECRET: u16 = graph_abi::NODE_SECRET;
const GRAPH_EDGE_ACTIVATION: u16 = graph_abi::EDGE_ACTIVATION;
const GRAPH_EDGE_CAPABILITY: u16 = graph_abi::EDGE_CAPABILITY;
const GRAPH_EDGE_MOUNT: u16 = graph_abi::EDGE_MOUNT;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ServiceLifecycleState {
    Declared,
    Starting,
    Ready,
    Failed,
    Restarting,
    Exited,
}

impl ServiceLifecycleState {
    fn label(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Restarting => "restarting",
            Self::Exited => "exited",
        }
    }
}

#[derive(Clone, Copy)]
struct ServiceLifecycleEvent {
    service: &'static str,
    state: ServiceLifecycleState,
    status: u64,
    has_status: bool,
}
