use core::{
    arch::{asm, x86_64::__cpuid_count},
    cell::UnsafeCell,
    sync::atomic::{Ordering, compiler_fence},
};

use crate::{
    capability, gdt, limine, memory, paging, serial, timer,
    usercopy::{self, UserPtr},
    userspace,
};
use vertex_abi::graph as graph_abi;

pub const BOOT_ENDPOINT_ID: u64 = 1;

const MAX_MESSAGE_BYTES: usize = 512;
const ENDPOINT_QUEUE_CAPACITY: usize = 4;
const MAX_BOOT_READ_BYTES: usize = 64 * 1024;
const MAX_OBJECTS: usize = 128;
const MAX_PROCESSES: usize = 16;
const MAX_CAPS: usize = 32;
const MAX_FILE_HANDLES: usize = 16;
const FILE_HANDLE_SLOT_BITS: u64 = 8;
const FILE_HANDLE_SLOT_MASK: u64 = (1 << FILE_HANDLE_SLOT_BITS) - 1;
const MAX_OPEN_FILE_DESCRIPTIONS: usize = MAX_PROCESSES * MAX_FILE_HANDLES;
const MAX_VFS_NODES: usize = 96;
const MAX_VFS_MEM_FILES: usize = 8;
const MAX_VFS_MEM_FILE_BYTES: usize = 512;
const MAX_VERTEXFS_FILES: usize = 16;
const MAX_VERTEXFS_FILE_BYTES: usize = 512;
const VERTEXFS_MODULE_STRING: &[u8] = b"vertexfs-v1";
const VERTEXFS_SECTOR_SIZE: usize = 512;
const VERTEXFS_SECTORS: usize = 64;
const VERTEXFS_IMAGE_BYTES: usize = VERTEXFS_SECTOR_SIZE * VERTEXFS_SECTORS;
const VERTEXFS_SUPERBLOCK_MAGIC: &[u8; 16] = b"VERTEXFSV1\0\0\0\0\0\0";
const VERTEXFS_INODE_TABLE_MAGIC: &[u8; 16] = b"VFSINODEV1\0\0\0\0\0\0";
const VERTEXFS_DIRECTORY_MAGIC: &[u8; 16] = b"VFSDIRV1\0\0\0\0\0\0\0\0";
const VERTEXFS_FREE_MAP_MAGIC: &[u8; 16] = b"VFSFREEV1\0\0\0\0\0\0\0";
const VERTEXFS_JOURNAL_MAGIC: &[u8; 16] = b"VFSJOURNALV1\0\0\0\0";
const VERTEXFS_VERSION: u16 = 1;
const VERTEXFS_CHECKSUM_OFFSET: usize = 20;
const VERTEXFS_FEATURE_METADATA_V1: u32 = 1;
const VERTEXFS_FEATURE_DIRECTORY_CHECKSUMS: u32 = 1 << 1;
const VERTEXFS_FEATURE_FREE_SPACE_CHECKSUMS: u32 = 1 << 2;
const VERTEXFS_FEATURE_JOURNAL_V1: u32 = 1 << 3;
const VERTEXFS_FEATURE_FLAGS: u32 = VERTEXFS_FEATURE_METADATA_V1
    | VERTEXFS_FEATURE_DIRECTORY_CHECKSUMS
    | VERTEXFS_FEATURE_FREE_SPACE_CHECKSUMS
    | VERTEXFS_FEATURE_JOURNAL_V1;
const VERTEXFS_GENERATION_OFFSET: usize = 32;
const VERTEXFS_SECTION_TABLE_OFFSET: usize = 128;
const VERTEXFS_SECTION_RECORD_LEN: usize = 16;
const VERTEXFS_INODE_TABLE_SECTOR: u64 = 1;
const VERTEXFS_INODE_TABLE_SECTORS: u64 = 2;
const VERTEXFS_DIRECTORY_SECTOR: u64 = VERTEXFS_INODE_TABLE_SECTOR + VERTEXFS_INODE_TABLE_SECTORS;
const VERTEXFS_DIRECTORY_SECTORS: u64 = 2;
const VERTEXFS_FREE_MAP_SECTOR: u64 = VERTEXFS_DIRECTORY_SECTOR + VERTEXFS_DIRECTORY_SECTORS;
const VERTEXFS_JOURNAL_SECTOR: u64 = VERTEXFS_FREE_MAP_SECTOR + 1;
const VERTEXFS_DATA_SECTOR: u64 = VERTEXFS_JOURNAL_SECTOR + 1;
const VERTEXFS_DATA_SECTORS: u64 = (VERTEXFS_SECTORS as u64) - VERTEXFS_DATA_SECTOR;
const VERTEXFS_INODE_ENTRY_OFFSET: usize = 32;
const VERTEXFS_INODE_ENTRY_LEN: usize = 64;
const VERTEXFS_INODE_TABLE_BYTES: usize =
    VERTEXFS_SECTOR_SIZE * VERTEXFS_INODE_TABLE_SECTORS as usize;
const VERTEXFS_DIRECTORY_ENTRY_OFFSET: usize = 32;
const VERTEXFS_DIRECTORY_ENTRY_LEN: usize = 64;
const VERTEXFS_DIRECTORY_NAME_BYTES: usize = VERTEXFS_DIRECTORY_ENTRY_LEN - 12;
const VERTEXFS_DIRECTORY_BYTES: usize = VERTEXFS_SECTOR_SIZE * VERTEXFS_DIRECTORY_SECTORS as usize;
const VERTEXFS_INODE_ROOT: u32 = 1;
const VERTEXFS_INODE_README: u32 = 2;
const VERTEXFS_INODE_APP_DIR: u32 = 3;
const VERTEXFS_INODE_APP_A: u32 = 4;
const VERTEXFS_BASE_INODE_COUNT: usize = 4;
const VERTEXFS_BASE_DIRECTORY_COUNT: usize = 3;
const VERTEXFS_INODE_ENTRY_CAPACITY: usize =
    (VERTEXFS_INODE_TABLE_BYTES - VERTEXFS_INODE_ENTRY_OFFSET) / VERTEXFS_INODE_ENTRY_LEN;
const VERTEXFS_DIRECTORY_ENTRY_CAPACITY: usize =
    (VERTEXFS_DIRECTORY_BYTES - VERTEXFS_DIRECTORY_ENTRY_OFFSET) / VERTEXFS_DIRECTORY_ENTRY_LEN;
const VERTEXFS_DYNAMIC_INODE_FIRST: u32 = 5;
const VERTEXFS_DYNAMIC_DATA_SECTOR_FIRST: u64 = VERTEXFS_DATA_SECTOR + 2;
const VERTEXFS_DYNAMIC_FILE_CAPACITY: usize =
    VERTEXFS_INODE_ENTRY_CAPACITY - VERTEXFS_BASE_INODE_COUNT;
const VERTEXFS_DIRECTORY_DYNAMIC_FILE_CAPACITY: usize =
    VERTEXFS_DIRECTORY_ENTRY_CAPACITY - VERTEXFS_BASE_DIRECTORY_COUNT;
const VERTEXFS_KIND_DIR: u16 = 1;
const VERTEXFS_KIND_FILE: u16 = 2;
const VERTEXFS_JOURNAL_STATE_CLEAN: u16 = 0;
const VERTEXFS_JOURNAL_STATE_PENDING: u16 = 1;
const VERTEXFS_JOURNAL_PAYLOAD_OFFSET: usize = 64;
const VERTEXFS_SYNC_MAX_DEVICE_WRITES: usize = 8;
const VERTEXDISK_VERTEXFS_IMAGE_SECTOR: u64 = 49_209;
const BLOCK_PROTOCOL_V1: u16 = 1;
const BLOCK_OP_WRITE_SECTOR: u16 = 2;
const BLOCK_REQUEST_LEN: usize = 16;
const BLOCK_WRITE_ACK_LEN: usize = 16;
const MAX_VFS_PATH_BYTES: usize = 128;
const MAX_VFS_NAME_BYTES: usize = 64;
const VFS_STAT_BYTES: usize = 64;
const VFS_DIRENT_BYTES: usize = 96;
const VFS_RENAME_REQUEST_HEADER_BYTES: usize = 16;
const VFS_RENAME_REQUEST_MAX_BYTES: usize =
    VFS_RENAME_REQUEST_HEADER_BYTES + (MAX_VFS_PATH_BYTES * 2);
const MAX_VFS_LOCKS: usize = MAX_OPEN_FILE_DESCRIPTIONS;
const MAX_VFS_EVENTS: usize = 64;
const MAX_VFS_PIPE_BYTES: usize = 128;
const MAX_BOOT_GRANTS: usize = 128;
const MAX_BOOT_NAMESPACES: usize = 4;
const MAX_NAMESPACE_ENTRIES: usize = 4;
const MAX_BOOT_VFS_ROOTS: usize = 8;
const MAX_BOOT_STATE_VOLUMES: usize = 4;
const MAX_BOOT_PROCESS_MOUNTS: usize = 4;
const MAX_BOOT_GRAPH_NODES: usize = 128;
const MAX_BOOT_GRAPH_EDGES: usize = 224;
const MAX_VFS_MOUNTS: usize = 16;
const BUILTIN_VFS_MOUNTS: usize = 6;
const MAX_CAP_LINEAGE: usize = 1024;
const MAX_REVOKED_CAPS: usize = MAX_CAP_LINEAGE;
const MAX_GENERATION_CONFIGS: usize = 4;
const MAX_INSPECT_REPORT_BYTES: usize = 128 * 1024;
const MAX_SERVICE_LIFECYCLE_EVENTS: usize = 128;
const DMA_MAPPING_INFO_BYTES: usize = 24;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const READY_ENVELOPE_LEN: usize = 16;
const INIT_TIMER_CAP_SLOT: u64 = 30;
const VFS_OPEN_READ: u64 = 1;
const VFS_OPEN_WRITE: u64 = 1 << 1;
const VFS_OPEN_CREATE: u64 = 1 << 2;
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
const BOOT_PROCESS_MOUNT_BIND: u16 = 1;
const BOOT_PROCESS_MOUNT_READ_ONLY: u16 = 1 << 1;
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
const INITIAL_USER_RFLAGS: u64 = 0x202;
const STATUS_BAD_CAPABILITY: u64 = u64::MAX - 1;
const STATUS_BAD_BUFFER: u64 = u64::MAX - 2;
const STATUS_OK: u64 = 0;
const STATUS_TIMEOUT: u64 = u64::MAX - 9;
const STATUS_VFS_BAD_HANDLE: u64 = u64::MAX - 38;
const STATUS_VFS_UNSUPPORTED: u64 = u64::MAX - 39;
pub const STATUS_PROCESS_FAULT: u64 = u64::MAX - 10;
const FALLBACK_TSC_TICKS_PER_MS: u64 = 1_000_000;
pub const BOOT_OBJECT_ENDPOINT: u16 = 1;
pub const BOOT_OBJECT_STORE: u16 = 2;
pub const BOOT_OBJECT_STATE: u16 = 3;
pub const BOOT_OBJECT_TIMER: u16 = 4;
pub const BOOT_OBJECT_NETWORK_PORT: u16 = 5;
pub const BOOT_OBJECT_IO_PORT_RANGE: u16 = 6;
pub const BOOT_OBJECT_MMIO_REGION: u16 = 7;
pub const BOOT_OBJECT_INTERRUPT_LINE: u16 = 8;
pub const BOOT_OBJECT_DMA_REGION: u16 = 9;
pub const BOOT_OBJECT_PCI_DEVICE: u16 = 10;
pub const BOOT_OBJECT_VIRTIO_DEVICE: u16 = 11;
pub const BOOT_OBJECT_NAMESPACE: u16 = 12;
pub const BOOT_OBJECT_VFS_ROOT: u16 = 13;
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

pub const FRAME_R15: usize = 0;
pub const FRAME_R14: usize = 8;
pub const FRAME_R13: usize = 16;
pub const FRAME_R12: usize = 24;
pub const FRAME_R11: usize = 32;
pub const FRAME_R10: usize = 40;
pub const FRAME_R9: usize = 48;
pub const FRAME_R8: usize = 56;
pub const FRAME_RSI: usize = 64;
pub const FRAME_RDI: usize = 72;
pub const FRAME_RBP: usize = 80;
pub const FRAME_RDX: usize = 88;
pub const FRAME_RCX: usize = 96;
pub const FRAME_RBX: usize = 104;
pub const FRAME_RAX: usize = 112;
pub const FRAME_USER_RIP: usize = 120;
pub const FRAME_USER_CS: usize = 128;
pub const FRAME_USER_RFLAGS: usize = 136;
pub const FRAME_USER_RSP: usize = 144;
pub const FRAME_USER_SS: usize = 152;
pub const FRAME_SIZE: usize = 160;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SyscallFrame {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    pub rax: u64,
    pub user_rip: u64,
    pub user_cs: u64,
    pub user_rflags: u64,
    pub user_rsp: u64,
    pub user_ss: u64,
}

impl SyscallFrame {
    const fn empty() -> Self {
        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            rdx: 0,
            rcx: 0,
            rbx: 0,
            rax: 0,
            user_rip: 0,
            user_cs: 0,
            user_rflags: 0,
            user_rsp: 0,
            user_ss: 0,
        }
    }

    fn from_context(context: ProcessContext) -> Self {
        Self {
            user_rip: context.entry,
            user_cs: gdt::USER_CODE_SELECTOR as u64,
            user_rflags: INITIAL_USER_RFLAGS,
            user_rsp: context.stack_top,
            user_ss: gdt::USER_DATA_SELECTOR as u64,
            ..Self::empty()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessId(u64);

impl ProcessId {
    const fn empty() -> Self {
        Self(0)
    }

    fn new(raw: u64) -> Self {
        Self(raw)
    }

    fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct KernelObjectId(u64);

impl KernelObjectId {
    fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy)]
pub struct ProcessContext {
    pub cr3: u64,
    pub entry: u64,
    pub stack_top: u64,
}

#[derive(Clone, Copy)]
pub struct BootProcessConfig {
    pub name: &'static str,
    pub graph_node: &'static str,
    pub image_base: u64,
    pub image_length: u64,
    pub initial: bool,
    pub mount_root: &'static str,
    pub mounts: [Option<BootProcessMountConfig>; MAX_BOOT_PROCESS_MOUNTS],
    pub mount_count: usize,
}

#[derive(Clone, Copy)]
pub struct BootProcessMountConfig {
    pub path: &'static str,
    pub source: &'static str,
    pub flags: u16,
}

#[derive(Clone, Copy)]
struct RuntimeReapTarget {
    pid: ProcessId,
    name: &'static str,
    cr3: u64,
}

#[derive(Clone, Copy)]
struct DmaUserMapping {
    region: KernelObjectId,
    virtual_base: u64,
    physical_base: u64,
    length: u64,
}

#[derive(Clone, Copy)]
pub struct BootEndpointConfig {
    pub name: &'static str,
}

#[derive(Clone, Copy)]
pub struct BootModuleConfig {
    pub name: &'static str,
    pub base: u64,
    pub length: u64,
}

#[derive(Clone, Copy)]
pub struct BootStoreObjectConfig {
    pub id: &'static str,
    pub base: u64,
    pub length: u64,
    pub hash: &'static str,
}

#[derive(Clone, Copy)]
pub struct BootStateVolumeConfig {
    pub id: &'static str,
}

#[derive(Clone, Copy)]
pub struct BootNetworkPortConfig {
    pub id: &'static str,
}

#[derive(Clone, Copy)]
pub struct BootIoPortRangeConfig {
    pub id: &'static str,
    pub base: u64,
    pub length: u64,
}

#[derive(Clone, Copy)]
pub struct BootMmioRegionConfig {
    pub id: &'static str,
    pub base: u64,
    pub length: u64,
}

#[derive(Clone, Copy)]
pub struct BootInterruptLineConfig {
    pub id: &'static str,
    pub line: u64,
}

#[derive(Clone, Copy)]
pub struct BootDmaRegionConfig {
    pub id: &'static str,
    pub base: u64,
    pub length: u64,
}

#[derive(Clone, Copy)]
pub struct BootPciDeviceConfig {
    pub id: &'static str,
    pub kind: &'static str,
}

#[derive(Clone, Copy)]
pub struct BootVirtioDeviceConfig {
    pub id: &'static str,
    pub transport: &'static str,
}

#[derive(Clone, Copy)]
pub struct BootNamespaceEntryConfig {
    pub path: &'static str,
    pub object_kind: u16,
    pub object_index: usize,
    pub rights: u64,
}

#[derive(Clone, Copy)]
pub struct BootNamespaceConfig {
    pub id: &'static str,
    pub entries: [Option<BootNamespaceEntryConfig>; MAX_NAMESPACE_ENTRIES],
    pub entry_count: usize,
}

#[derive(Clone, Copy)]
pub struct BootVfsRootConfig {
    pub id: &'static str,
    pub root_path: &'static str,
}

#[derive(Clone, Copy)]
pub struct BootGraphNodeConfig {
    pub kind: u16,
    pub object_kind: u16,
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Clone, Copy)]
pub struct BootGraphEdgeConfig {
    pub kind: u16,
    pub from_index: usize,
    pub to_index: usize,
    pub rights: u64,
    pub id: &'static str,
}

#[derive(Clone, Copy)]
pub struct BootGrantConfig {
    pub process_index: usize,
    pub cap_slot: u64,
    pub object_kind: u16,
    pub object_index: usize,
    pub rights: u64,
}

#[derive(Clone, Copy)]
pub struct BootRuntimeConfig {
    generation_id: &'static str,
    manifest_hash: [u8; 64],
    graph_store_hash: [u8; 64],
    graph_store_checksum: u32,
    graph_store_source: &'static str,
    processes: [Option<BootProcessConfig>; MAX_PROCESSES],
    process_count: usize,
    endpoints: [Option<BootEndpointConfig>; MAX_OBJECTS],
    endpoint_count: usize,
    manifest_module: Option<BootModuleConfig>,
    store_objects: [Option<BootStoreObjectConfig>; MAX_OBJECTS],
    store_object_count: usize,
    state_volumes: [Option<BootStateVolumeConfig>; MAX_BOOT_STATE_VOLUMES],
    state_volume_count: usize,
    network_ports: [Option<BootNetworkPortConfig>; MAX_OBJECTS],
    network_port_count: usize,
    io_ports: [Option<BootIoPortRangeConfig>; MAX_OBJECTS],
    io_port_count: usize,
    mmio_regions: [Option<BootMmioRegionConfig>; MAX_OBJECTS],
    mmio_region_count: usize,
    interrupt_lines: [Option<BootInterruptLineConfig>; MAX_OBJECTS],
    interrupt_line_count: usize,
    dma_regions: [Option<BootDmaRegionConfig>; MAX_OBJECTS],
    dma_region_count: usize,
    pci_devices: [Option<BootPciDeviceConfig>; MAX_OBJECTS],
    pci_device_count: usize,
    virtio_devices: [Option<BootVirtioDeviceConfig>; MAX_OBJECTS],
    virtio_device_count: usize,
    namespaces: [Option<BootNamespaceConfig>; MAX_BOOT_NAMESPACES],
    namespace_count: usize,
    vfs_roots: [Option<BootVfsRootConfig>; MAX_BOOT_VFS_ROOTS],
    vfs_root_count: usize,
    graph_nodes: [Option<BootGraphNodeConfig>; MAX_BOOT_GRAPH_NODES],
    graph_node_count: usize,
    graph_edges: [Option<BootGraphEdgeConfig>; MAX_BOOT_GRAPH_EDGES],
    graph_edge_count: usize,
    grants: [Option<BootGrantConfig>; MAX_BOOT_GRANTS],
    grant_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProcessState {
    Empty,
    Declared,
    Ready,
    Running,
    BlockedOnEndpoint {
        endpoint: KernelObjectId,
        cap_id: u64,
        destination: u64,
        max_len: usize,
        timeout_tsc: Option<u64>,
    },
    BlockedOnInterrupt {
        interrupt: KernelObjectId,
        timeout_tsc: Option<u64>,
    },
    BlockedOnVfsRead {
        node: VfsNodeId,
        description: FileDescriptionId,
        destination: u64,
        max_len: usize,
    },
    BlockedOnVfsState {
        reply_endpoint: KernelObjectId,
        node: VfsNodeId,
        description: FileDescriptionId,
        operation: VfsStateOperation,
        transaction_id: u64,
        offset: u64,
        destination: u64,
        max_len: usize,
        write_len: usize,
        update_offset: bool,
    },
    BlockedOnVertexFsSync {
        request_endpoint: KernelObjectId,
        reply_endpoint: KernelObjectId,
        backing: usize,
        inode_id: u32,
        checksum: u32,
        write_count: usize,
        next_write: usize,
        expected_sector: u64,
    },
    BlockedOnNetworkPort {
        port: KernelObjectId,
        destination: u64,
        max_len: usize,
    },
    Sleeping {
        wake_tsc: u64,
    },
    Exited,
}

impl ProcessState {
    fn label(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Declared => "declared",
            Self::Ready => "ready",
            Self::Running => "running",
            Self::BlockedOnEndpoint { .. } => "blocked",
            Self::BlockedOnInterrupt { .. } => "blocked-irq",
            Self::BlockedOnVfsRead { .. } => "blocked-vfs",
            Self::BlockedOnVfsState { .. } => "blocked-vfs-state",
            Self::BlockedOnVertexFsSync { .. } => "blocked-vertexfs-sync",
            Self::BlockedOnNetworkPort { .. } => "blocked-net",
            Self::Sleeping { .. } => "sleeping",
            Self::Exited => "exited",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VfsStateOperation {
    Read,
    Stat,
    Write,
    Control,
    ServiceRead,
}

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

#[derive(Clone, Copy)]
pub enum ScheduleResult {
    Continue,
    Switched,
    Halt { ok: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitError {
    ObjectTableFull,
    ProcessTableFull,
    CapabilityTableFull,
    InvalidBootManifest,
}

#[derive(Clone, Copy)]
struct VertexFsBootFiles<'a> {
    generation: &'a [u8],
    readme: VertexFsBootFile<'a>,
    app_a: VertexFsBootFile<'a>,
    journal_replayed: bool,
}

#[derive(Clone, Copy)]
struct VertexFsBootFile<'a> {
    inode: VertexFsInode,
    payload: &'a [u8],
}

#[derive(Clone, Copy)]
struct VertexFsParsedInodes {
    readme: VertexFsInode,
    app_a: VertexFsInode,
    dynamic: [Option<VertexFsInode>; VERTEXFS_DYNAMIC_FILE_CAPACITY],
}

#[derive(Clone, Copy)]
struct VertexFsInode {
    id: u32,
    kind: u16,
    size: u64,
    first_sector: u64,
    sector_count: u32,
    checksum: u32,
    parent: u32,
}

#[derive(Clone, Copy)]
struct VertexFsJournalRecord<'a> {
    target_inode: u32,
    payload: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    BadCapability,
    InvalidUserBuffer,
    MessageTooLarge,
    Empty,
    VfsPermission,
    VfsBadPath,
    VfsNotFound,
    VfsNotDirectory,
    VfsNotFile,
    VfsBusy,
    VfsBadHandle,
    VfsUnsupported,
    VfsNoSpace,
    VfsExists,
}

#[derive(Clone, Copy)]
struct Capability {
    id: u64,
    object: KernelObjectId,
    rights: u64,
    owner_process: ProcessId,
    parent_cap_id: u64,
    generation_id: &'static str,
    delegated_by: ProcessId,
    revoked: bool,
}

#[derive(Clone, Copy)]
struct CapabilitySpace {
    caps: [Option<Capability>; MAX_CAPS],
}

#[derive(Clone, Copy)]
struct Process {
    pid: ProcessId,
    name: &'static str,
    context: ProcessContext,
    image_base: u64,
    image_length: u64,
    context_reaped: bool,
    state: ProcessState,
    caps: CapabilitySpace,
    initial_caps: CapabilitySpace,
    saved_frame: SyscallFrame,
    has_saved_frame: bool,
    exit_status: u64,
    has_exited: bool,
    start_count: u64,
    quota: ProcessQuota,
    initial_quota: ProcessQuota,
    mount_root: VfsPath,
    dma_mappings: [Option<DmaUserMapping>; MAX_OBJECTS],
    file_handles: [FileHandleSlot; MAX_FILE_HANDLES],
}

#[derive(Clone, Copy)]
struct ProcessQuota {
    max_caps: u64,
    max_endpoints: u64,
    max_memory_pages: u64,
    max_child_processes: u64,
    max_ipc_bytes: u64,
    used_endpoints: u64,
}

#[derive(Clone, Copy)]
struct IpcMessage {
    sender: ProcessId,
    len: usize,
    bytes: [u8; MAX_MESSAGE_BYTES],
}

#[derive(Clone, Copy)]
struct IpcEndpoint {
    id: KernelObjectId,
    name: &'static str,
    owner: ProcessId,
    queue: [IpcMessage; ENDPOINT_QUEUE_CAPACITY],
    queue_len: usize,
}

#[derive(Clone, Copy)]
struct BootModuleObject {
    id: KernelObjectId,
    name: &'static str,
    base: u64,
    length: u64,
}

#[derive(Clone, Copy)]
struct StoreObject {
    id: KernelObjectId,
    name: &'static str,
    base: u64,
    length: u64,
    hash: &'static str,
}

#[derive(Clone, Copy)]
struct StateVolumeObject {
    id: KernelObjectId,
    name: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct VfsNodeId(u64);

impl VfsNodeId {
    fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy)]
enum VfsNodeKind {
    RegularFile,
    Directory,
    DeviceNode,
    Pipe,
    SyntheticNode,
}

#[derive(Clone, Copy)]
enum VfsBacking {
    None,
    StoreObject(KernelObjectId),
    StateVolume(KernelObjectId),
    StateVolumeValue(KernelObjectId),
    StateVolumeControl(KernelObjectId),
    MemoryFile(usize),
    VertexFsFile(usize),
    Device(KernelObjectId),
    Synthetic(&'static [u8]),
    FsServiceReport,
    Pipe,
}

#[derive(Clone, Copy)]
struct VfsName {
    bytes: [u8; MAX_VFS_NAME_BYTES],
    len: usize,
}

#[derive(Clone, Copy)]
struct VfsPath {
    bytes: [u8; MAX_VFS_PATH_BYTES],
    len: usize,
}

#[derive(Clone, Copy)]
struct VfsNode {
    id: VfsNodeId,
    name: VfsName,
    parent: Option<VfsNodeId>,
    kind: VfsNodeKind,
    backing: VfsBacking,
    mount_source: &'static str,
    metadata_version: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct FileDescriptionId(u64);

impl FileDescriptionId {
    fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy)]
struct OpenFileDescription {
    id: FileDescriptionId,
    node: VfsNodeId,
    rights: u64,
    flags: u64,
    offset: u64,
    ref_count: u64,
    owner: ProcessId,
    authority_cap_id: u64,
    watch_cursor: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VfsLockMode {
    Shared,
    Exclusive,
}

#[derive(Clone, Copy)]
struct VfsLock {
    node: VfsNodeId,
    owner: ProcessId,
    description: FileDescriptionId,
    mode: VfsLockMode,
    start: u64,
    len: u64,
}

#[derive(Clone, Copy)]
struct VfsEvent {
    parent: VfsNodeId,
    kind: u64,
    name: VfsName,
    metadata_version: u64,
}

#[derive(Clone, Copy)]
struct VfsPipeBuffer {
    bytes: [u8; MAX_VFS_PIPE_BYTES],
    len: usize,
}

#[derive(Clone, Copy)]
struct FileHandle {
    description: FileDescriptionId,
}

#[derive(Clone, Copy)]
struct FileHandleSlot {
    generation: u64,
    handle: Option<FileHandle>,
}

#[derive(Clone, Copy)]
struct VfsMemoryFile {
    name: &'static str,
    bytes: [u8; MAX_VFS_MEM_FILE_BYTES],
    len: usize,
}

#[derive(Clone, Copy)]
struct VfsVertexFsFile {
    name: &'static str,
    vfs_name: VfsName,
    inode_id: u32,
    parent_inode_id: u32,
    first_sector: u64,
    sector_count: u32,
    bytes: [u8; MAX_VERTEXFS_FILE_BYTES],
    len: usize,
    dirty: bool,
    checksum: u32,
}

#[derive(Clone, Copy)]
enum VertexFsSyncResult {
    Journaled {
        inode_id: u32,
        checksum: u32,
        write_count: usize,
    },
    Cached {
        checksum: u32,
    },
}

#[derive(Clone, Copy)]
struct VertexFsDeviceWrite {
    sector: u64,
    bytes: [u8; VERTEXFS_SECTOR_SIZE],
}

impl VertexFsDeviceWrite {
    const fn empty() -> Self {
        Self {
            sector: 0,
            bytes: [0; VERTEXFS_SECTOR_SIZE],
        }
    }
}

#[derive(Clone, Copy)]
struct TimerObject {
    id: KernelObjectId,
    name: &'static str,
}

#[derive(Clone, Copy)]
struct NetworkPortObject {
    id: KernelObjectId,
    name: &'static str,
    queue: [IpcMessage; ENDPOINT_QUEUE_CAPACITY],
    queue_len: usize,
}

#[derive(Clone, Copy)]
struct IoPortRangeObject {
    id: KernelObjectId,
    name: &'static str,
    base: u64,
    length: u64,
}

#[derive(Clone, Copy)]
struct MmioRegionObject {
    id: KernelObjectId,
    name: &'static str,
    base: u64,
    length: u64,
}

#[derive(Clone, Copy)]
struct InterruptLineObject {
    id: KernelObjectId,
    name: &'static str,
    line: u64,
    pending_count: u64,
    delivered_count: u64,
    spurious_count: u64,
}

#[derive(Clone, Copy)]
struct DmaRegionObject {
    id: KernelObjectId,
    name: &'static str,
    base: u64,
    length: u64,
    mapped_by: ProcessId,
    map_count: u64,
    release_count: u64,
}

#[derive(Clone, Copy)]
struct PciDeviceObject {
    id: KernelObjectId,
    name: &'static str,
    kind: &'static str,
}

#[derive(Clone, Copy)]
struct VirtioDeviceObject {
    id: KernelObjectId,
    name: &'static str,
    transport: &'static str,
    owner: ProcessId,
    queue_size: u16,
    avail_idx: u16,
    used_idx: u16,
    submissions: u64,
    completions: u64,
    timeouts: u64,
    reset_count: u64,
    last_error: &'static str,
}

#[derive(Clone, Copy)]
struct VirtioQueueState {
    dma_physical: u64,
    dma_virtual: u64,
    queue_size: u16,
    avail_offset: usize,
    used_offset: usize,
    data_offset: usize,
    avail_idx: u16,
    used_idx: u16,
    submissions: u64,
    completions: u64,
    interrupt_waits: u64,
    timeouts: u64,
    last_error: &'static str,
}

impl VirtioQueueState {
    const fn empty() -> Self {
        Self {
            dma_physical: 0,
            dma_virtual: 0,
            queue_size: 0,
            avail_offset: 0,
            used_offset: 0,
            data_offset: 0,
            avail_idx: 0,
            used_idx: 0,
            submissions: 0,
            completions: 0,
            interrupt_waits: 0,
            timeouts: 0,
            last_error: "none",
        }
    }

    const fn new(dma_physical: u64, dma_virtual: u64) -> Self {
        Self {
            dma_physical,
            dma_virtual,
            queue_size: 0,
            avail_offset: 0,
            used_offset: 0,
            data_offset: 0,
            avail_idx: 0,
            used_idx: 0,
            submissions: 0,
            completions: 0,
            interrupt_waits: 0,
            timeouts: 0,
            last_error: "none",
        }
    }
}

#[derive(Clone, Copy)]
struct VirtioRngState {
    initialized: bool,
    io_base: u16,
    queue: VirtioQueueState,
    owner: ProcessId,
    reset_count: u64,
    last_error: &'static str,
}

impl VirtioRngState {
    const fn new() -> Self {
        Self {
            initialized: false,
            io_base: 0,
            queue: VirtioQueueState::empty(),
            owner: ProcessId::empty(),
            reset_count: 0,
            last_error: "none",
        }
    }
}

#[derive(Clone, Copy)]
struct VirtioNetState {
    initialized: bool,
    io_base: u16,
    rx: VirtioQueueState,
    tx: VirtioQueueState,
    rx_posted: bool,
    owner: ProcessId,
    reset_count: u64,
    last_error: &'static str,
}

impl VirtioNetState {
    const fn new() -> Self {
        Self {
            initialized: false,
            io_base: 0,
            rx: VirtioQueueState::empty(),
            tx: VirtioQueueState::empty(),
            rx_posted: false,
            owner: ProcessId::empty(),
            reset_count: 0,
            last_error: "none",
        }
    }
}

#[derive(Clone, Copy)]
struct NamespaceEntry {
    path: &'static str,
    object: KernelObjectId,
    rights: u64,
}

#[derive(Clone, Copy)]
struct NamespaceObject {
    id: KernelObjectId,
    name: &'static str,
    entries: [Option<NamespaceEntry>; MAX_NAMESPACE_ENTRIES],
    entry_count: usize,
}

#[derive(Clone, Copy)]
struct VfsRootObject {
    id: KernelObjectId,
    name: &'static str,
    root_path: VfsPath,
    derived: bool,
}

#[derive(Clone, Copy)]
struct VfsMountObject {
    id: KernelObjectId,
    name: &'static str,
    root_node: VfsNodeId,
    root_path: VfsPath,
    source: &'static str,
    flags: u64,
    dynamic: bool,
    owner: ProcessId,
}

#[derive(Clone, Copy)]
struct ProcessControlObject {
    id: KernelObjectId,
    name: &'static str,
}

#[derive(Clone, Copy)]
struct SecretObject {
    id: KernelObjectId,
    name: &'static str,
    value: &'static [u8],
}

struct InspectReport {
    bytes: [u8; MAX_INSPECT_REPORT_BYTES],
    len: usize,
    truncated: bool,
}

#[derive(Clone, Copy)]
enum KernelObject {
    IpcEndpoint(IpcEndpoint),
    BootModule(BootModuleObject),
    StoreObject(StoreObject),
    StateVolume(StateVolumeObject),
    Timer(TimerObject),
    NetworkPort(NetworkPortObject),
    IoPortRange(IoPortRangeObject),
    MmioRegion(MmioRegionObject),
    InterruptLine(InterruptLineObject),
    DmaRegion(DmaRegionObject),
    PciDevice(PciDeviceObject),
    VirtioDevice(VirtioDeviceObject),
    Namespace(NamespaceObject),
    VfsRoot(VfsRootObject),
    VfsMount(VfsMountObject),
    ProcessControl(ProcessControlObject),
    Secret(SecretObject),
}

impl KernelObject {
    fn id(self) -> KernelObjectId {
        match self {
            Self::IpcEndpoint(object) => object.id,
            Self::BootModule(object) => object.id,
            Self::StoreObject(object) => object.id,
            Self::StateVolume(object) => object.id,
            Self::Timer(object) => object.id,
            Self::NetworkPort(object) => object.id,
            Self::IoPortRange(object) => object.id,
            Self::MmioRegion(object) => object.id,
            Self::InterruptLine(object) => object.id,
            Self::DmaRegion(object) => object.id,
            Self::PciDevice(object) => object.id,
            Self::VirtioDevice(object) => object.id,
            Self::Namespace(object) => object.id,
            Self::VfsRoot(object) => object.id,
            Self::VfsMount(object) => object.id,
            Self::ProcessControl(object) => object.id,
            Self::Secret(object) => object.id,
        }
    }
}

struct ObjectTable {
    objects: [Option<KernelObject>; MAX_OBJECTS],
    count: usize,
    next_id: u64,
}

struct ProcessTable {
    processes: [Option<Process>; MAX_PROCESSES],
    count: usize,
    current: Option<ProcessId>,
    next_id: u64,
}

#[derive(Clone, Copy)]
struct CapabilityLineage {
    cap_id: u64,
    parent_cap_id: u64,
}

struct RuntimeState {
    objects: ObjectTable,
    processes: ProcessTable,
    generation_id: &'static str,
    active_config: Option<&'static BootRuntimeConfig>,
    next_cap_id: u64,
    revoked_caps: [u64; MAX_REVOKED_CAPS],
    revoked_cap_count: usize,
    cap_lineage: [Option<CapabilityLineage>; MAX_CAP_LINEAGE],
    cap_lineage_count: usize,
    vfs_nodes: [Option<VfsNode>; MAX_VFS_NODES],
    vfs_node_count: usize,
    next_vfs_node_id: u64,
    next_vfs_metadata_version: u64,
    vfs_mem_files: [VfsMemoryFile; MAX_VFS_MEM_FILES],
    vfs_mem_file_count: usize,
    vertexfs_image: [u8; VERTEXFS_IMAGE_BYTES],
    vertexfs_image_loaded: bool,
    vertexfs_files: [VfsVertexFsFile; MAX_VERTEXFS_FILES],
    vertexfs_file_count: usize,
    open_file_descriptions: [Option<OpenFileDescription>; MAX_OPEN_FILE_DESCRIPTIONS],
    next_file_description_id: u64,
    vfs_locks: [Option<VfsLock>; MAX_VFS_LOCKS],
    vfs_events: [Option<VfsEvent>; MAX_VFS_EVENTS],
    vfs_event_count: usize,
    vfs_pipe: VfsPipeBuffer,
    endpoint_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    store_object_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    state_volume_ids: [Option<KernelObjectId>; MAX_BOOT_STATE_VOLUMES],
    network_port_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    io_port_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    mmio_region_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    interrupt_line_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    dma_region_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    pci_device_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    virtio_device_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    namespace_ids: [Option<KernelObjectId>; MAX_BOOT_NAMESPACES],
    vfs_root_ids: [Option<KernelObjectId>; MAX_BOOT_VFS_ROOTS],
    vfs_mount_ids: [Option<KernelObjectId>; MAX_VFS_MOUNTS],
    vfs_mount_count: usize,
    timer_id: Option<KernelObjectId>,
    process_control_id: Option<KernelObjectId>,
    secret_id: Option<KernelObjectId>,
    state_vfs_request_endpoint: Option<KernelObjectId>,
    state_vfs_reply_endpoint: Option<KernelObjectId>,
    vertexfs_device_request_endpoint: Option<KernelObjectId>,
    vertexfs_device_reply_endpoint: Option<KernelObjectId>,
    generation_metadata_block_request_endpoint: Option<KernelObjectId>,
    generation_metadata_block_reply_endpoint: Option<KernelObjectId>,
    next_vfs_state_transaction_id: u64,
    vertexfs_sync_writes: [VertexFsDeviceWrite; VERTEXFS_SYNC_MAX_DEVICE_WRITES],
    vertexfs_sync_write_count: usize,
    process_template_pids: [Option<ProcessId>; MAX_PROCESSES],
    service_lifecycle_events: [Option<ServiceLifecycleEvent>; MAX_SERVICE_LIFECYCLE_EVENTS],
    service_lifecycle_event_count: usize,
}

#[derive(Clone, Copy)]
struct GenerationRuntime {
    generation_id: &'static str,
    config: &'static BootRuntimeConfig,
}

struct GenerationRuntimeTable {
    entries: [Option<GenerationRuntime>; MAX_GENERATION_CONFIGS],
    count: usize,
}

struct BootManagerState {
    selected_generation: &'static str,
    previous_generation: &'static str,
    known_good_generation: &'static str,
    last_failed_generation: &'static str,
    last_failure_reason: &'static str,
    last_failure_service: &'static str,
    last_failure_dependency: &'static str,
    last_failure_policy: &'static str,
    last_transaction_state: &'static str,
    last_transaction_target: &'static str,
    transaction_counter: u64,
    boot_attempt_counter: u64,
}

struct Global<T>(UnsafeCell<T>);

unsafe impl<T> Sync for Global<T> {}

static RUNTIME: Global<RuntimeState> = Global(UnsafeCell::new(RuntimeState::new()));
static INSTALL_STAGING_RUNTIME: Global<RuntimeState> = Global(UnsafeCell::new(RuntimeState::new()));
static GENERATION_RUNTIMES: Global<GenerationRuntimeTable> =
    Global(UnsafeCell::new(GenerationRuntimeTable::new()));
static ROLLBACK_RUNTIME: Global<Option<GenerationRuntime>> = Global(UnsafeCell::new(None));
static FAILED_GENERATION: Global<Option<&'static str>> = Global(UnsafeCell::new(None));
static BOOT_MANAGER: Global<BootManagerState> = Global(UnsafeCell::new(BootManagerState::new()));
static FRAME_ALLOCATOR: Global<Option<*mut memory::FrameAllocator>> = Global(UnsafeCell::new(None));
static VIRTIO_RNG_STATE: Global<VirtioRngState> = Global(UnsafeCell::new(VirtioRngState::new()));
static VIRTIO_NET_STATE: Global<VirtioNetState> = Global(UnsafeCell::new(VirtioNetState::new()));
static INSPECT_REPORT: Global<InspectReport> = Global(UnsafeCell::new(InspectReport::new()));

impl CapabilitySpace {
    const fn new() -> Self {
        Self {
            caps: [None; MAX_CAPS],
        }
    }

    fn grant(&mut self, slot: u64, cap: Capability) -> Result<(), InitError> {
        let Ok(slot) = usize::try_from(slot) else {
            return Err(InitError::CapabilityTableFull);
        };
        if slot >= self.caps.len() {
            return Err(InitError::CapabilityTableFull);
        }
        if self.caps[slot].is_some() {
            return Err(InitError::InvalidBootManifest);
        }

        self.caps[slot] = Some(cap);
        Ok(())
    }

    fn lookup(&self, slot: u64) -> Option<Capability> {
        let slot = usize::try_from(slot).ok()?;
        self.caps.get(slot).copied().flatten()
    }

    fn clear(&mut self, slot: u64) -> Result<Capability, IpcError> {
        let Ok(slot) = usize::try_from(slot) else {
            return Err(IpcError::BadCapability);
        };
        if slot >= self.caps.len() {
            return Err(IpcError::BadCapability);
        }
        let Some(cap) = self.caps[slot] else {
            return Err(IpcError::BadCapability);
        };
        self.caps[slot] = None;
        Ok(cap)
    }

    fn can_grant(&self, slot: u64) -> bool {
        let Ok(slot) = usize::try_from(slot) else {
            return false;
        };
        slot < self.caps.len() && self.caps[slot].is_none()
    }

    fn mark_revoked(&mut self, cap_id: u64) {
        let mut index = 0;
        while index < self.caps.len() {
            if let Some(mut cap) = self.caps[index]
                && cap.id == cap_id
            {
                cap.revoked = true;
                self.caps[index] = Some(cap);
            }
            index += 1;
        }
    }
}

impl ProcessQuota {
    const fn initial() -> Self {
        Self {
            max_caps: MAX_CAPS as u64,
            max_endpoints: 1,
            max_memory_pages: 0,
            max_child_processes: MAX_PROCESSES as u64,
            max_ipc_bytes: MAX_MESSAGE_BYTES as u64,
            used_endpoints: 0,
        }
    }

    const fn service() -> Self {
        Self {
            max_caps: MAX_CAPS as u64,
            max_endpoints: 0,
            max_memory_pages: 0,
            max_child_processes: 0,
            max_ipc_bytes: MAX_MESSAGE_BYTES as u64,
            used_endpoints: 0,
        }
    }
}

impl Process {
    const fn empty() -> Self {
        Self {
            pid: ProcessId::empty(),
            name: "",
            context: ProcessContext {
                cr3: 0,
                entry: 0,
                stack_top: 0,
            },
            image_base: 0,
            image_length: 0,
            context_reaped: true,
            state: ProcessState::Empty,
            caps: CapabilitySpace::new(),
            initial_caps: CapabilitySpace::new(),
            saved_frame: SyscallFrame::empty(),
            has_saved_frame: false,
            exit_status: 0,
            has_exited: false,
            start_count: 0,
            quota: ProcessQuota::service(),
            initial_quota: ProcessQuota::service(),
            mount_root: VfsPath::empty(),
            dma_mappings: [None; MAX_OBJECTS],
            file_handles: [FileHandleSlot::empty(); MAX_FILE_HANDLES],
        }
    }

    fn new(
        pid: ProcessId,
        name: &'static str,
        context: ProcessContext,
        image_base: u64,
        image_length: u64,
        state: ProcessState,
        caps: CapabilitySpace,
        mount_root: VfsPath,
    ) -> Self {
        let initial = state == ProcessState::Running;
        let start_count = if initial { 1 } else { 0 };
        let quota = if initial {
            ProcessQuota::initial()
        } else {
            ProcessQuota::service()
        };
        Self {
            pid,
            name,
            context,
            image_base,
            image_length,
            context_reaped: false,
            state,
            caps,
            initial_caps: caps,
            saved_frame: SyscallFrame::empty(),
            has_saved_frame: false,
            exit_status: 0,
            has_exited: false,
            start_count,
            quota,
            initial_quota: quota,
            mount_root,
            dma_mappings: [None; MAX_OBJECTS],
            file_handles: [FileHandleSlot::empty(); MAX_FILE_HANDLES],
        }
    }

    fn dma_mapping(&self, region: KernelObjectId) -> Option<DmaUserMapping> {
        let mut index = 0;
        while index < self.dma_mappings.len() {
            if let Some(mapping) = self.dma_mappings[index]
                && mapping.region == region
            {
                return Some(mapping);
            }
            index += 1;
        }
        None
    }

    fn add_dma_mapping(&mut self, mapping: DmaUserMapping) -> Result<(), IpcError> {
        if self.dma_mapping(mapping.region).is_some() {
            return Ok(());
        }
        let mut index = 0;
        while index < self.dma_mappings.len() {
            if self.dma_mappings[index].is_none() {
                self.dma_mappings[index] = Some(mapping);
                return Ok(());
            }
            index += 1;
        }
        Err(IpcError::VfsNoSpace)
    }

    fn clear_dma_mappings(&mut self) {
        let mut index = 0;
        while index < self.dma_mappings.len() {
            self.dma_mappings[index] = None;
            index += 1;
        }
    }

    fn take_dma_mapping(&mut self, index: usize) -> Option<DmaUserMapping> {
        if index >= self.dma_mappings.len() {
            return None;
        }
        let mapping = self.dma_mappings[index];
        self.dma_mappings[index] = None;
        mapping
    }

    fn open_file_handle(&mut self, handle: FileHandle) -> Result<u64, IpcError> {
        let mut index = 0;
        while index < self.file_handles.len() {
            if self.file_handles[index].handle.is_none() {
                let mut generation = self.file_handles[index].generation.saturating_add(1);
                if generation == 0 {
                    generation = 1;
                }
                self.file_handles[index] = FileHandleSlot {
                    generation,
                    handle: Some(handle),
                };
                return Ok((generation << FILE_HANDLE_SLOT_BITS) | ((index as u64) + 1));
            }
            index += 1;
        }
        Err(IpcError::VfsNoSpace)
    }

    fn file_handle(&self, raw: u64) -> Result<(usize, FileHandle), IpcError> {
        let (index, generation) = decode_file_handle(raw)?;
        if index >= self.file_handles.len() {
            return Err(IpcError::VfsBadHandle);
        }
        let slot = self.file_handles[index];
        if slot.generation != generation {
            return Err(IpcError::VfsBadHandle);
        }
        let Some(handle) = slot.handle else {
            return Err(IpcError::VfsBadHandle);
        };
        Ok((index, handle))
    }

    fn close_file_handle(&mut self, raw: u64) -> Result<FileHandle, IpcError> {
        let (index, handle) = self.file_handle(raw)?;
        self.file_handles[index].handle = None;
        Ok(handle)
    }

    fn clear_file_handles(&mut self) {
        let mut index = 0;
        while index < self.file_handles.len() {
            self.file_handles[index].handle = None;
            index += 1;
        }
    }
}

impl FileHandleSlot {
    const fn empty() -> Self {
        Self {
            generation: 0,
            handle: None,
        }
    }
}

impl OpenFileDescription {
    const fn new(
        id: FileDescriptionId,
        node: VfsNodeId,
        rights: u64,
        flags: u64,
        owner: ProcessId,
        authority_cap_id: u64,
        watch_cursor: usize,
    ) -> Self {
        Self {
            id,
            node,
            rights,
            flags,
            offset: 0,
            ref_count: 1,
            owner,
            authority_cap_id,
            watch_cursor,
        }
    }
}

impl VfsPipeBuffer {
    const fn empty() -> Self {
        Self {
            bytes: [0; MAX_VFS_PIPE_BYTES],
            len: 0,
        }
    }

    fn enqueue(&mut self, bytes: &[u8]) -> Result<usize, IpcError> {
        if !self.is_empty() {
            return Err(IpcError::VfsBusy);
        }
        if bytes.len() > self.bytes.len() {
            return Err(IpcError::VfsNoSpace);
        }
        let mut index = 0;
        while index < bytes.len() {
            self.bytes[index] = bytes[index];
            index += 1;
        }
        self.len = bytes.len();
        Ok(bytes.len())
    }

    fn is_empty(self) -> bool {
        self.len == 0
    }
}

impl VfsMemoryFile {
    const fn empty() -> Self {
        Self {
            name: "",
            bytes: [0; MAX_VFS_MEM_FILE_BYTES],
            len: 0,
        }
    }

    fn new(name: &'static str, initial: &[u8]) -> Result<Self, InitError> {
        if initial.len() > MAX_VFS_MEM_FILE_BYTES {
            return Err(InitError::InvalidBootManifest);
        }
        let mut file = Self::empty();
        file.name = name;
        let mut index = 0;
        while index < initial.len() {
            file.bytes[index] = initial[index];
            index += 1;
        }
        file.len = initial.len();
        Ok(file)
    }
}

impl VfsVertexFsFile {
    const fn empty() -> Self {
        Self {
            name: "",
            vfs_name: VfsName::empty(),
            inode_id: 0,
            parent_inode_id: 0,
            first_sector: 0,
            sector_count: 0,
            bytes: [0; MAX_VERTEXFS_FILE_BYTES],
            len: 0,
            dirty: false,
            checksum: 0,
        }
    }

    fn new(
        name: &'static str,
        initial: &[u8],
        inode: Option<VertexFsInode>,
    ) -> Result<Self, InitError> {
        if initial.len() > MAX_VERTEXFS_FILE_BYTES {
            return Err(InitError::InvalidBootManifest);
        }
        let mut file = Self::empty();
        file.name = name;
        file.vfs_name = VfsName::from_static(name)?;
        if let Some(inode) = inode {
            file.inode_id = inode.id;
            file.parent_inode_id = inode.parent;
            file.first_sector = inode.first_sector;
            file.sector_count = inode.sector_count;
        }
        let mut index = 0;
        while index < initial.len() {
            file.bytes[index] = initial[index];
            index += 1;
        }
        file.len = initial.len();
        file.checksum = vertexfs_checksum32(initial);
        Ok(file)
    }
}

fn decode_file_handle(raw: u64) -> Result<(usize, u64), IpcError> {
    let slot = raw & FILE_HANDLE_SLOT_MASK;
    let generation = raw >> FILE_HANDLE_SLOT_BITS;
    if raw == 0 || slot == 0 || generation == 0 {
        return Err(IpcError::VfsBadHandle);
    }
    Ok(((slot - 1) as usize, generation))
}

impl IpcEndpoint {
    const fn new(id: KernelObjectId, name: &'static str, owner: ProcessId) -> Self {
        Self {
            id,
            name,
            owner,
            queue: [IpcMessage::empty(); ENDPOINT_QUEUE_CAPACITY],
            queue_len: 0,
        }
    }

    fn enqueue(
        &mut self,
        sender: ProcessId,
        bytes: &[u8; MAX_MESSAGE_BYTES],
        len: usize,
    ) -> Result<(), IpcError> {
        if self.queue_len == ENDPOINT_QUEUE_CAPACITY {
            return Err(IpcError::MessageTooLarge);
        }

        let mut message = IpcMessage::empty();
        message.sender = sender;
        message.len = len;
        message.bytes[..len].copy_from_slice(&bytes[..len]);
        self.queue[self.queue_len] = message;
        self.queue_len += 1;
        Ok(())
    }

    fn has_message_for(&self, receiver: ProcessId) -> bool {
        let mut index = 0;
        while index < self.queue_len {
            if self.queue[index].sender != receiver {
                return true;
            }
            index += 1;
        }
        false
    }

    fn dequeue_for(&mut self, receiver: ProcessId) -> Option<IpcMessage> {
        let mut index = 0;
        while index < self.queue_len {
            if self.queue[index].sender != receiver {
                let message = self.queue[index];
                while index + 1 < self.queue_len {
                    self.queue[index] = self.queue[index + 1];
                    index += 1;
                }
                self.queue_len -= 1;
                self.queue[self.queue_len] = IpcMessage::empty();
                return Some(message);
            }
            index += 1;
        }
        None
    }

    fn has_vfs_state_reply_for(&self, receiver: ProcessId, transaction_id: u64) -> bool {
        let mut index = 0;
        while index < self.queue_len {
            if self.queue[index].sender != receiver
                && ipc_message_transaction_id(self.queue[index]) == Some(transaction_id)
            {
                return true;
            }
            index += 1;
        }
        false
    }

    fn dequeue_vfs_state_reply_for(
        &mut self,
        receiver: ProcessId,
        transaction_id: u64,
    ) -> Option<IpcMessage> {
        let mut index = 0;
        while index < self.queue_len {
            if self.queue[index].sender != receiver
                && ipc_message_transaction_id(self.queue[index]) == Some(transaction_id)
            {
                let message = self.queue[index];
                while index + 1 < self.queue_len {
                    self.queue[index] = self.queue[index + 1];
                    index += 1;
                }
                self.queue_len -= 1;
                self.queue[self.queue_len] = IpcMessage::empty();
                return Some(message);
            }
            index += 1;
        }

        None
    }

    fn remove_vfs_state_request(&mut self, sender: ProcessId, transaction_id: u64) -> bool {
        let mut index = 0;
        while index < self.queue_len {
            if self.queue[index].sender == sender
                && ipc_message_transaction_id(self.queue[index]) == Some(transaction_id)
            {
                while index + 1 < self.queue_len {
                    self.queue[index] = self.queue[index + 1];
                    index += 1;
                }
                self.queue_len -= 1;
                self.queue[self.queue_len] = IpcMessage::empty();
                return true;
            }
            index += 1;
        }
        false
    }

    fn remove_all_from_sender(&mut self, sender: ProcessId) -> usize {
        let mut removed = 0;
        let mut index = 0;
        while index < self.queue_len {
            if self.queue[index].sender == sender {
                let mut shift = index;
                while shift + 1 < self.queue_len {
                    self.queue[shift] = self.queue[shift + 1];
                    shift += 1;
                }
                self.queue_len -= 1;
                self.queue[self.queue_len] = IpcMessage::empty();
                removed += 1;
            } else {
                index += 1;
            }
        }
        removed
    }
}

impl IpcMessage {
    const fn empty() -> Self {
        Self {
            sender: ProcessId::empty(),
            len: 0,
            bytes: [0; MAX_MESSAGE_BYTES],
        }
    }
}

pub fn run_fifo_regression() {
    let provider = ProcessId::new(1);
    let client_a = ProcessId::new(2);
    let client_b = ProcessId::new(3);
    let mut endpoint = IpcEndpoint::new(
        KernelObjectId(0xf100),
        "fifo-regression",
        ProcessId::empty(),
    );
    let mut message = [0u8; MAX_MESSAGE_BYTES];

    message[0] = b'a';
    if endpoint.enqueue(client_a, &message, 1).is_err() {
        fifo_regression_failed("enqueue a");
        return;
    }
    message[0] = b'b';
    if endpoint.enqueue(client_b, &message, 1).is_err() {
        fifo_regression_failed("enqueue b");
        return;
    }
    if endpoint
        .dequeue_for(provider)
        .map(|queued| queued.len == 1 && queued.bytes[0] == b'a')
        != Some(true)
    {
        fifo_regression_failed("fifo first");
        return;
    }
    if endpoint
        .dequeue_for(provider)
        .map(|queued| queued.len == 1 && queued.bytes[0] == b'b')
        != Some(true)
    {
        fifo_regression_failed("fifo second");
        return;
    }
    serial::write_str("IPC FIFO regression: queued sends preserve FIFO order\n");

    let mut full_endpoint = IpcEndpoint::new(
        KernelObjectId(0xf101),
        "fifo-full-regression",
        ProcessId::empty(),
    );
    let mut index = 0;
    while index < ENDPOINT_QUEUE_CAPACITY {
        message[0] = b'0' + index as u8;
        if full_endpoint.enqueue(client_a, &message, 1).is_err() {
            fifo_regression_failed("fill queue");
            return;
        }
        index += 1;
    }
    if !matches!(
        full_endpoint.enqueue(client_b, &message, 1),
        Err(IpcError::MessageTooLarge)
    ) {
        fifo_regression_failed("queue full");
        return;
    }
    serial::write_str("IPC FIFO regression: queue-full send rejected\n");

    let mut receiver_endpoint = IpcEndpoint::new(
        KernelObjectId(0xf102),
        "fifo-receiver-regression",
        ProcessId::empty(),
    );
    message[0] = b'a';
    if receiver_endpoint.enqueue(client_a, &message, 1).is_err() {
        fifo_regression_failed("receiver enqueue a");
        return;
    }
    if receiver_endpoint.has_message_for(client_a) {
        fifo_regression_failed("self message visible");
        return;
    }
    if !receiver_endpoint.has_message_for(client_b) {
        fifo_regression_failed("other receiver hidden");
        return;
    }
    message[0] = b'b';
    if receiver_endpoint.enqueue(client_b, &message, 1).is_err() {
        fifo_regression_failed("receiver enqueue b");
        return;
    }
    if !receiver_endpoint.has_message_for(client_a) || !receiver_endpoint.has_message_for(client_b)
    {
        fifo_regression_failed("blocked receiver eligibility");
        return;
    }
    if receiver_endpoint
        .dequeue_for(client_a)
        .map(|queued| queued.len == 1 && queued.bytes[0] == b'b')
        != Some(true)
    {
        fifo_regression_failed("receiver a eligible message");
        return;
    }
    if receiver_endpoint
        .dequeue_for(client_b)
        .map(|queued| queued.len == 1 && queued.bytes[0] == b'a')
        != Some(true)
    {
        fifo_regression_failed("receiver b eligible message");
        return;
    }
    serial::write_str(
        "IPC FIFO regression: receiver-specific dequeue preserves eligible ordering\n",
    );
    serial::write_str("IPC FIFO regression: multiple blocked receivers match eligible messages\n");
    serial::write_str("IPC FIFO regression ok\n");
}

fn fifo_regression_failed(reason: &str) {
    serial::write_str("IPC FIFO regression failed: ");
    serial::write_str(reason);
    serial::write_str("\n");
}

impl BootModuleObject {
    const fn new(id: KernelObjectId, name: &'static str, base: u64, length: u64) -> Self {
        Self {
            id,
            name,
            base,
            length,
        }
    }
}

impl StoreObject {
    const fn new(
        id: KernelObjectId,
        name: &'static str,
        base: u64,
        length: u64,
        hash: &'static str,
    ) -> Self {
        Self {
            id,
            name,
            base,
            length,
            hash,
        }
    }
}

impl StateVolumeObject {
    const fn new(id: KernelObjectId, name: &'static str) -> Self {
        Self { id, name }
    }
}

impl VfsName {
    const fn empty() -> Self {
        Self {
            bytes: [0; MAX_VFS_NAME_BYTES],
            len: 0,
        }
    }

    fn from_static(value: &'static str) -> Result<Self, InitError> {
        Self::from_bytes(value.as_bytes()).map_err(|_| InitError::InvalidBootManifest)
    }

    fn from_user_component(value: &[u8]) -> Result<Self, IpcError> {
        if value == b"." || value == b".." {
            return Err(IpcError::VfsBadPath);
        }
        Self::from_bytes(value).map_err(|_| IpcError::BadCapability)
    }

    fn from_bytes(value: &[u8]) -> Result<Self, ()> {
        if value.is_empty() || value.len() > MAX_VFS_NAME_BYTES {
            return Err(());
        }
        let mut name = Self::empty();
        if value == b"/" {
            name.bytes[0] = b'/';
            name.len = 1;
            return Ok(name);
        }
        let mut index = 0;
        while index < value.len() {
            let byte = value[index];
            if byte == b'/' || byte == 0 {
                return Err(());
            }
            name.bytes[index] = byte;
            index += 1;
        }
        name.len = value.len();
        Ok(name)
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

impl VfsPath {
    const fn empty() -> Self {
        Self {
            bytes: [0; MAX_VFS_PATH_BYTES],
            len: 0,
        }
    }

    fn from_root_path(path: &[u8]) -> Result<Self, IpcError> {
        if path.len() > MAX_VFS_PATH_BYTES || !valid_vfs_root_path(path) {
            return Err(IpcError::VfsBadPath);
        }
        let mut value = Self {
            bytes: [0; MAX_VFS_PATH_BYTES],
            len: path.len(),
        };
        let mut index = 0;
        while index < path.len() {
            value.bytes[index] = path[index];
            index += 1;
        }
        Ok(value)
    }

    fn from_boot_root_path(path: &'static str) -> Result<Self, InitError> {
        if path.len() > MAX_VFS_PATH_BYTES || !valid_vfs_root_path(path.as_bytes()) {
            return Err(InitError::InvalidBootManifest);
        }
        let mut value = Self {
            bytes: [0; MAX_VFS_PATH_BYTES],
            len: path.len(),
        };
        let mut index = 0;
        while index < path.len() {
            value.bytes[index] = path.as_bytes()[index];
            index += 1;
        }
        Ok(value)
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

fn state_volume_mount_component(id: &'static str) -> Result<&'static str, InitError> {
    let Some(component) = id.strip_prefix("state:") else {
        return Err(InitError::InvalidBootManifest);
    };
    if component.is_empty() || component.len() > MAX_VFS_NAME_BYTES {
        return Err(InitError::InvalidBootManifest);
    }
    let mut index = 0;
    while index < component.len() {
        let byte = component.as_bytes()[index];
        if byte == b'/' || byte == 0 {
            return Err(InitError::InvalidBootManifest);
        }
        index += 1;
    }
    Ok(component)
}

fn state_volume_vfs_name(id: &'static str) -> Result<VfsName, InitError> {
    VfsName::from_static(state_volume_mount_component(id)?)
}

fn state_volume_vfs_path(id: &'static str) -> Result<VfsPath, InitError> {
    let component = state_volume_mount_component(id)?.as_bytes();
    const PREFIX: &[u8] = b"/state/";
    let len = PREFIX
        .len()
        .checked_add(component.len())
        .ok_or(InitError::InvalidBootManifest)?;
    if len > MAX_VFS_PATH_BYTES {
        return Err(InitError::InvalidBootManifest);
    }
    let mut bytes = [0u8; MAX_VFS_PATH_BYTES];
    let mut index = 0;
    while index < PREFIX.len() {
        bytes[index] = PREFIX[index];
        index += 1;
    }
    let mut component_index = 0;
    while component_index < component.len() {
        bytes[index] = component[component_index];
        index += 1;
        component_index += 1;
    }
    VfsPath::from_root_path(&bytes[..len]).map_err(|_| InitError::InvalidBootManifest)
}

impl VfsNode {
    fn with_name(
        id: VfsNodeId,
        name: VfsName,
        parent: Option<VfsNodeId>,
        kind: VfsNodeKind,
        backing: VfsBacking,
        mount_source: &'static str,
        metadata_version: u64,
    ) -> Self {
        Self {
            id,
            name,
            parent,
            kind,
            backing,
            mount_source,
            metadata_version,
        }
    }
}

impl TimerObject {
    const fn new(id: KernelObjectId, name: &'static str) -> Self {
        Self { id, name }
    }
}

impl NetworkPortObject {
    const fn new(id: KernelObjectId, name: &'static str) -> Self {
        Self {
            id,
            name,
            queue: [IpcMessage::empty(); ENDPOINT_QUEUE_CAPACITY],
            queue_len: 0,
        }
    }

    fn enqueue_udp(
        &mut self,
        sender: ProcessId,
        bytes: &[u8; MAX_MESSAGE_BYTES],
        len: usize,
    ) -> Result<(), IpcError> {
        if self.queue_len == ENDPOINT_QUEUE_CAPACITY {
            return Err(IpcError::MessageTooLarge);
        }

        let mut message = IpcMessage::empty();
        message.sender = sender;
        message.len = len;
        message.bytes[..len].copy_from_slice(&bytes[..len]);
        self.queue[self.queue_len] = message;
        self.queue_len += 1;
        Ok(())
    }

    fn dequeue_udp(&mut self) -> Option<IpcMessage> {
        if self.queue_len == 0 {
            return None;
        }

        let message = self.queue[0];
        let mut index = 1;
        while index < self.queue_len {
            self.queue[index - 1] = self.queue[index];
            index += 1;
        }
        self.queue_len -= 1;
        self.queue[self.queue_len] = IpcMessage::empty();
        Some(message)
    }
}

impl IoPortRangeObject {
    const fn new(id: KernelObjectId, name: &'static str, base: u64, length: u64) -> Self {
        Self {
            id,
            name,
            base,
            length,
        }
    }
}

impl MmioRegionObject {
    const fn new(id: KernelObjectId, name: &'static str, base: u64, length: u64) -> Self {
        Self {
            id,
            name,
            base,
            length,
        }
    }
}

impl InterruptLineObject {
    const fn new(id: KernelObjectId, name: &'static str, line: u64) -> Self {
        Self {
            id,
            name,
            line,
            pending_count: 0,
            delivered_count: 0,
            spurious_count: 0,
        }
    }
}

impl DmaRegionObject {
    const fn new(id: KernelObjectId, name: &'static str, base: u64, length: u64) -> Self {
        Self {
            id,
            name,
            base,
            length,
            mapped_by: ProcessId::empty(),
            map_count: 0,
            release_count: 0,
        }
    }
}

impl PciDeviceObject {
    const fn new(id: KernelObjectId, name: &'static str, kind: &'static str) -> Self {
        Self { id, name, kind }
    }
}

impl VirtioDeviceObject {
    const fn new(id: KernelObjectId, name: &'static str, transport: &'static str) -> Self {
        Self {
            id,
            name,
            transport,
            owner: ProcessId::empty(),
            queue_size: 0,
            avail_idx: 0,
            used_idx: 0,
            submissions: 0,
            completions: 0,
            timeouts: 0,
            reset_count: 0,
            last_error: "none",
        }
    }
}

impl NamespaceObject {
    const fn new(
        id: KernelObjectId,
        name: &'static str,
        entries: [Option<NamespaceEntry>; MAX_NAMESPACE_ENTRIES],
        entry_count: usize,
    ) -> Self {
        Self {
            id,
            name,
            entries,
            entry_count,
        }
    }

    fn resolve(&self, path: &[u8]) -> Option<NamespaceEntry> {
        let mut index = 0;
        while index < self.entry_count {
            if let Some(entry) = self.entries[index]
                && entry.path.as_bytes() == path
            {
                return Some(entry);
            }
            index += 1;
        }
        None
    }
}

impl VfsRootObject {
    const fn new(
        id: KernelObjectId,
        name: &'static str,
        root_path: VfsPath,
        derived: bool,
    ) -> Self {
        Self {
            id,
            name,
            root_path,
            derived,
        }
    }
}

impl VfsMountObject {
    const fn new(
        id: KernelObjectId,
        name: &'static str,
        root_node: VfsNodeId,
        root_path: VfsPath,
        source: &'static str,
        flags: u64,
        dynamic: bool,
        owner: ProcessId,
    ) -> Self {
        Self {
            id,
            name,
            root_node,
            root_path,
            source,
            flags,
            dynamic,
            owner,
        }
    }
}

fn vfs_authority_path_covers(authority: &[u8], path: &[u8]) -> bool {
    if authority == b"/" {
        return !path.is_empty() && path[0] == b'/';
    }
    if authority == path {
        return true;
    }
    path.len() > authority.len() && path.starts_with(authority) && path[authority.len()] == b'/'
}

impl ProcessControlObject {
    const fn new(id: KernelObjectId, name: &'static str) -> Self {
        Self { id, name }
    }
}

impl SecretObject {
    const fn new(id: KernelObjectId, name: &'static str, value: &'static [u8]) -> Self {
        Self { id, name, value }
    }
}

impl InspectReport {
    const fn new() -> Self {
        Self {
            bytes: [0; MAX_INSPECT_REPORT_BYTES],
            len: 0,
            truncated: false,
        }
    }

    fn clear(&mut self) {
        self.len = 0;
        self.truncated = false;
    }

    fn push_byte(&mut self, byte: u8) {
        if self.len == self.bytes.len() {
            self.truncated = true;
            return;
        }
        self.bytes[self.len] = byte;
        self.len += 1;
    }

    fn push_str(&mut self, value: &str) {
        self.push_bytes(value.as_bytes());
    }

    fn push_bytes(&mut self, value: &[u8]) {
        let mut index = 0;
        while index < value.len() {
            self.push_byte(value[index]);
            index += 1;
        }
    }

    fn push_u64_dec(&mut self, mut value: u64) {
        if value == 0 {
            self.push_byte(b'0');
            return;
        }

        let mut digits = [0u8; 20];
        let mut len = 0;
        while value > 0 {
            digits[len] = b'0' + (value % 10) as u8;
            value /= 10;
            len += 1;
        }
        while len > 0 {
            len -= 1;
            self.push_byte(digits[len]);
        }
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

impl ObjectTable {
    const fn new() -> Self {
        Self {
            objects: [None; MAX_OBJECTS],
            count: 0,
            next_id: BOOT_ENDPOINT_ID,
        }
    }

    fn reset(&mut self) {
        self.count = 0;
        self.next_id = BOOT_ENDPOINT_ID;
    }

    fn add_endpoint(&mut self, name: &'static str) -> Result<KernelObjectId, InitError> {
        self.add_endpoint_owned(name, ProcessId::empty())
    }

    fn add_endpoint_owned(
        &mut self,
        name: &'static str,
        owner: ProcessId,
    ) -> Result<KernelObjectId, InitError> {
        let id = KernelObjectId(self.next_id);
        self.insert_object(KernelObject::IpcEndpoint(IpcEndpoint::new(id, name, owner)))?;
        self.next_id += 1;
        Ok(id)
    }

    fn add_boot_module(
        &mut self,
        name: &'static str,
        base: u64,
        length: u64,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::BootModule(BootModuleObject::new(
            id, name, base, length,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_store_object(
        &mut self,
        name: &'static str,
        base: u64,
        length: u64,
        hash: &'static str,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::StoreObject(StoreObject::new(
            id, name, base, length, hash,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_state_volume(&mut self, name: &'static str) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] =
            Some(KernelObject::StateVolume(StateVolumeObject::new(id, name)));
        self.count += 1;
        Ok(id)
    }

    fn add_timer(&mut self, name: &'static str) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::Timer(TimerObject::new(id, name)));
        self.count += 1;
        Ok(id)
    }

    fn add_network_port(&mut self, name: &'static str) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] =
            Some(KernelObject::NetworkPort(NetworkPortObject::new(id, name)));
        self.count += 1;
        Ok(id)
    }

    fn add_io_port(
        &mut self,
        name: &'static str,
        base: u64,
        length: u64,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::IoPortRange(IoPortRangeObject::new(
            id, name, base, length,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_mmio_region(
        &mut self,
        name: &'static str,
        base: u64,
        length: u64,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::MmioRegion(MmioRegionObject::new(
            id, name, base, length,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_interrupt_line(
        &mut self,
        name: &'static str,
        line: u64,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::InterruptLine(InterruptLineObject::new(
            id, name, line,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_dma_region(
        &mut self,
        name: &'static str,
        base: u64,
        length: u64,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::DmaRegion(DmaRegionObject::new(
            id, name, base, length,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_pci_device(
        &mut self,
        name: &'static str,
        kind: &'static str,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::PciDevice(PciDeviceObject::new(
            id, name, kind,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_virtio_device(
        &mut self,
        name: &'static str,
        transport: &'static str,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::VirtioDevice(VirtioDeviceObject::new(
            id, name, transport,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_namespace(
        &mut self,
        name: &'static str,
        entries: [Option<NamespaceEntry>; MAX_NAMESPACE_ENTRIES],
        entry_count: usize,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::Namespace(NamespaceObject::new(
            id,
            name,
            entries,
            entry_count,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_vfs_root(
        &mut self,
        name: &'static str,
        root_path: &'static str,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }
        let root_path = VfsPath::from_boot_root_path(root_path)?;

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::VfsRoot(VfsRootObject::new(
            id, name, root_path, false,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_derived_vfs_root(&mut self, root_path: VfsPath) -> Result<KernelObjectId, IpcError> {
        if self.count == self.objects.len() {
            return Err(IpcError::BadCapability);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::VfsRoot(VfsRootObject::new(
            id,
            "vfs-root:derived",
            root_path,
            true,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_vfs_mount(
        &mut self,
        name: &'static str,
        root_node: VfsNodeId,
        root_path: VfsPath,
        source: &'static str,
        flags: u64,
        dynamic: bool,
        owner: ProcessId,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.insert_object(KernelObject::VfsMount(VfsMountObject::new(
            id, name, root_node, root_path, source, flags, dynamic, owner,
        )))?;
        Ok(id)
    }

    fn add_process_control(&mut self, name: &'static str) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::ProcessControl(ProcessControlObject::new(
            id, name,
        )));
        self.count += 1;
        Ok(id)
    }

    fn add_secret(
        &mut self,
        name: &'static str,
        value: &'static [u8],
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::Secret(SecretObject::new(id, name, value)));
        self.count += 1;
        Ok(id)
    }

    fn endpoint_count(&self) -> usize {
        let mut count = 0;
        let mut index = 0;
        while index < self.count {
            if matches!(self.objects[index], Some(KernelObject::IpcEndpoint(_))) {
                count += 1;
            }
            index += 1;
        }
        count
    }

    fn remove_owned_endpoints(&mut self, owner: ProcessId) -> u64 {
        let mut removed = 0;
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::IpcEndpoint(endpoint)) = self.objects[index]
                && endpoint.owner == owner
            {
                self.objects[index] = None;
                removed += 1;
            }
            index += 1;
        }
        self.trim_empty_tail();
        removed
    }

    fn remove_owned_endpoint(&mut self, id: KernelObjectId, owner: ProcessId) -> bool {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::IpcEndpoint(endpoint)) = self.objects[index]
                && endpoint.id == id
                && endpoint.owner == owner
            {
                self.objects[index] = None;
                self.trim_empty_tail();
                return true;
            }
            index += 1;
        }
        false
    }

    fn remove_derived_vfs_root(&mut self, id: KernelObjectId) -> bool {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::VfsRoot(root)) = self.objects[index]
                && root.id == id
                && root.derived
            {
                self.objects[index] = None;
                self.trim_empty_tail();
                return true;
            }
            index += 1;
        }
        false
    }

    fn remove_dynamic_vfs_mount(&mut self, root_node: VfsNodeId) -> Option<KernelObjectId> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::VfsMount(mount)) = self.objects[index]
                && mount.root_node == root_node
                && mount.dynamic
            {
                self.objects[index] = None;
                self.trim_empty_tail();
                return Some(mount.id);
            }
            index += 1;
        }
        None
    }

    fn remove_dynamic_vfs_mount_by_path(&mut self, root_path: &[u8]) -> Option<KernelObjectId> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::VfsMount(mount)) = self.objects[index]
                && mount.root_path.as_bytes() == root_path
                && mount.dynamic
            {
                self.objects[index] = None;
                self.trim_empty_tail();
                return Some(mount.id);
            }
            index += 1;
        }
        None
    }

    fn live_count(&self) -> usize {
        let mut live = 0;
        let mut index = 0;
        while index < self.count {
            if self.objects[index].is_some() {
                live += 1;
            }
            index += 1;
        }
        live
    }

    fn insert_object(&mut self, object: KernelObject) -> Result<(), InitError> {
        let mut index = 0;
        while index < self.count {
            if self.objects[index].is_none() {
                self.objects[index] = Some(object);
                return Ok(());
            }
            index += 1;
        }

        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        self.objects[self.count] = Some(object);
        self.count += 1;
        Ok(())
    }

    fn trim_empty_tail(&mut self) {
        while self.count > 0 && self.objects[self.count - 1].is_none() {
            self.count -= 1;
        }
    }

    fn get_endpoint_mut(&mut self, id: KernelObjectId) -> Option<&mut IpcEndpoint> {
        let mut found = None;
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::IpcEndpoint(endpoint)) = self.objects[index]
                && endpoint.id == id
            {
                found = Some(index);
                break;
            }
            index += 1;
        }

        match &mut self.objects[found?] {
            Some(KernelObject::IpcEndpoint(endpoint)) => Some(endpoint),
            _ => None,
        }
    }

    fn get_endpoint(&self, id: KernelObjectId) -> Option<IpcEndpoint> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::IpcEndpoint(endpoint)) = self.objects[index]
                && endpoint.id == id
            {
                return Some(endpoint);
            }
            index += 1;
        }
        None
    }

    fn get_boot_module(&self, id: KernelObjectId) -> Option<BootModuleObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::BootModule(module)) = self.objects[index]
                && module.id == id
            {
                return Some(module);
            }
            index += 1;
        }

        None
    }

    fn get_store_object(&self, id: KernelObjectId) -> Option<StoreObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::StoreObject(object)) = self.objects[index]
                && object.id == id
            {
                return Some(object);
            }
            index += 1;
        }

        None
    }

    fn get_state_volume(&self, id: KernelObjectId) -> Option<StateVolumeObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::StateVolume(object)) = self.objects[index]
                && object.id == id
            {
                return Some(object);
            }
            index += 1;
        }

        None
    }

    fn get_network_port(&self, id: KernelObjectId) -> Option<NetworkPortObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::NetworkPort(port)) = self.objects[index]
                && port.id == id
            {
                return Some(port);
            }
            index += 1;
        }

        None
    }

    fn get_network_port_mut(&mut self, id: KernelObjectId) -> Option<&mut NetworkPortObject> {
        let mut found = None;
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::NetworkPort(port)) = self.objects[index]
                && port.id == id
            {
                found = Some(index);
                break;
            }
            index += 1;
        }

        match &mut self.objects[found?] {
            Some(KernelObject::NetworkPort(port)) => Some(port),
            _ => None,
        }
    }

    fn get_timer(&self, id: KernelObjectId) -> Option<TimerObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::Timer(timer)) = self.objects[index]
                && timer.id == id
            {
                return Some(timer);
            }
            index += 1;
        }

        None
    }

    fn get_io_port(&self, id: KernelObjectId) -> Option<IoPortRangeObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::IoPortRange(port)) = self.objects[index]
                && port.id == id
            {
                return Some(port);
            }
            index += 1;
        }

        None
    }

    fn get_mmio_region(&self, id: KernelObjectId) -> Option<MmioRegionObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::MmioRegion(region)) = self.objects[index]
                && region.id == id
            {
                return Some(region);
            }
            index += 1;
        }

        None
    }

    fn get_interrupt_line(&self, id: KernelObjectId) -> Option<InterruptLineObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::InterruptLine(line)) = self.objects[index]
                && line.id == id
            {
                return Some(line);
            }
            index += 1;
        }

        None
    }

    fn get_interrupt_line_mut(&mut self, id: KernelObjectId) -> Option<&mut InterruptLineObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::InterruptLine(line)) = self.objects[index]
                && line.id == id
            {
                break;
            }
            index += 1;
        }

        if index == self.count {
            return None;
        }

        match self.objects[index].as_mut() {
            Some(KernelObject::InterruptLine(line)) => Some(line),
            _ => None,
        }
    }

    fn get_interrupt_line_by_number(&self, irq_line: u64) -> Option<InterruptLineObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::InterruptLine(line)) = self.objects[index]
                && line.line == irq_line
            {
                return Some(line);
            }
            index += 1;
        }

        None
    }

    fn get_dma_region(&self, id: KernelObjectId) -> Option<DmaRegionObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::DmaRegion(region)) = self.objects[index]
                && region.id == id
            {
                return Some(region);
            }
            index += 1;
        }

        None
    }

    fn get_dma_region_mut(&mut self, id: KernelObjectId) -> Option<&mut DmaRegionObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::DmaRegion(region)) = self.objects[index]
                && region.id == id
            {
                break;
            }
            index += 1;
        }

        if index == self.count {
            return None;
        }

        match self.objects[index].as_mut() {
            Some(KernelObject::DmaRegion(region)) => Some(region),
            _ => None,
        }
    }

    fn get_virtio_device(&self, id: KernelObjectId) -> Option<VirtioDeviceObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::VirtioDevice(device)) = self.objects[index]
                && device.id == id
            {
                return Some(device);
            }
            index += 1;
        }

        None
    }

    fn get_virtio_device_mut(&mut self, id: KernelObjectId) -> Option<&mut VirtioDeviceObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::VirtioDevice(device)) = self.objects[index]
                && device.id == id
            {
                break;
            }
            index += 1;
        }

        if index == self.count {
            return None;
        }

        match self.objects[index].as_mut() {
            Some(KernelObject::VirtioDevice(device)) => Some(device),
            _ => None,
        }
    }

    fn get_namespace(&self, id: KernelObjectId) -> Option<NamespaceObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::Namespace(namespace)) = self.objects[index]
                && namespace.id == id
            {
                return Some(namespace);
            }
            index += 1;
        }

        None
    }

    fn get_vfs_root(&self, id: KernelObjectId) -> Option<VfsRootObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::VfsRoot(root)) = self.objects[index]
                && root.id == id
            {
                return Some(root);
            }
            index += 1;
        }

        None
    }

    fn get_vfs_mount_by_root_node(&self, root_node: VfsNodeId) -> Option<VfsMountObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::VfsMount(mount)) = self.objects[index]
                && mount.root_node == root_node
            {
                return Some(mount);
            }
            index += 1;
        }

        None
    }

    fn get_vfs_mount_by_path(&self, path: &[u8]) -> Option<VfsMountObject> {
        let mut best = None;
        let mut best_len = 0;
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::VfsMount(mount)) = self.objects[index] {
                let root_path = mount.root_path.as_bytes();
                if vfs_authority_path_covers(root_path, path) && root_path.len() >= best_len {
                    best = Some(mount);
                    best_len = root_path.len();
                }
            }
            index += 1;
        }
        best
    }

    fn get_vfs_mount_by_exact_path(&self, path: &[u8]) -> Option<VfsMountObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::VfsMount(mount)) = self.objects[index]
                && mount.root_path.as_bytes() == path
            {
                return Some(mount);
            }
            index += 1;
        }
        None
    }

    fn get_process_control(&self, id: KernelObjectId) -> Option<ProcessControlObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::ProcessControl(process_control)) = self.objects[index]
                && process_control.id == id
            {
                return Some(process_control);
            }
            index += 1;
        }

        None
    }

    fn get_secret(&self, id: KernelObjectId) -> Option<SecretObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::Secret(secret)) = self.objects[index]
                && secret.id == id
            {
                return Some(secret);
            }
            index += 1;
        }

        None
    }
}

fn ipc_message_transaction_id(message: IpcMessage) -> Option<u64> {
    if message.len < VFS_STATE_TRANSACTION_ID_BYTES {
        return None;
    }
    Some(read_u64_le(&message.bytes, 0))
}

impl ProcessTable {
    const fn new() -> Self {
        Self {
            processes: [Some(Process::empty()); MAX_PROCESSES],
            count: 0,
            current: None,
            next_id: 1,
        }
    }

    fn reset(&mut self) {
        self.count = 0;
        self.current = None;
        self.next_id = 1;
    }

    fn add_process(
        &mut self,
        name: &'static str,
        context: ProcessContext,
        image_base: u64,
        image_length: u64,
        state: ProcessState,
        caps: CapabilitySpace,
        mount_root: VfsPath,
    ) -> Result<ProcessId, InitError> {
        if self.count == self.processes.len() {
            return Err(InitError::ProcessTableFull);
        }

        let pid = ProcessId::new(self.next_id);
        self.next_id += 1;
        self.processes[self.count] = Some(Process::new(
            pid,
            name,
            context,
            image_base,
            image_length,
            state,
            caps,
            mount_root,
        ));
        self.count += 1;
        Ok(pid)
    }

    fn remove_last_process(&mut self, pid: ProcessId) -> Result<(), InitError> {
        if self.count == 0 {
            return Err(InitError::InvalidBootManifest);
        }

        let index = self.count - 1;
        let Some(process) = self.processes[index] else {
            return Err(InitError::InvalidBootManifest);
        };
        if process.pid != pid {
            return Err(InitError::InvalidBootManifest);
        }

        self.processes[index] = Some(Process::empty());
        self.count -= 1;
        if self.next_id == pid.raw() + 1 {
            self.next_id = pid.raw();
        }
        Ok(())
    }

    fn remove_process(&mut self, pid: ProcessId) -> Result<(), InitError> {
        let mut found = None;
        let mut index = 0;
        while index < self.count {
            if let Some(process) = self.processes[index]
                && process.pid == pid
            {
                found = Some(index);
                break;
            }
            index += 1;
        }

        let Some(mut index) = found else {
            return Err(InitError::InvalidBootManifest);
        };
        if self.current == Some(pid) {
            return Err(InitError::InvalidBootManifest);
        }

        while index + 1 < self.count {
            self.processes[index] = self.processes[index + 1];
            index += 1;
        }
        self.count -= 1;
        self.processes[self.count] = Some(Process::empty());
        Ok(())
    }

    fn set_current(&mut self, pid: ProcessId) {
        self.current = Some(pid);
    }

    fn current_process(&self) -> Option<Process> {
        let pid = self.current?;
        self.process(pid).copied()
    }

    fn current_process_mut(&mut self) -> Option<&mut Process> {
        let pid = self.current?;
        self.process_mut(pid)
    }

    fn process(&self, pid: ProcessId) -> Option<&Process> {
        let mut index = 0;
        while index < self.count {
            if let Some(process) = &self.processes[index]
                && process.pid == pid
            {
                return Some(process);
            }
            index += 1;
        }

        None
    }

    fn process_mut(&mut self, pid: ProcessId) -> Option<&mut Process> {
        let mut found = None;
        let mut index = 0;
        while index < self.count {
            if let Some(process) = self.processes[index]
                && process.pid == pid
            {
                found = Some(index);
                break;
            }
            index += 1;
        }

        self.processes[found?].as_mut()
    }

    fn current_index(&self) -> Option<usize> {
        let pid = self.current?;
        let mut index = 0;
        while index < self.count {
            if let Some(process) = self.processes[index]
                && process.pid == pid
            {
                return Some(index);
            }
            index += 1;
        }

        None
    }

    fn next_ready_index_round_robin(&self, include_current: bool) -> Option<usize> {
        if self.count == 0 {
            return None;
        }

        let current = self.current_index();
        let start = self
            .current_index()
            .map(|index| (index + 1) % self.count)
            .unwrap_or(0);
        let mut offset = 0;

        while offset < self.count {
            let index = (start + offset) % self.count;
            if !include_current && current == Some(index) {
                offset += 1;
                continue;
            }
            if let Some(process) = self.processes[index]
                && process.state == ProcessState::Ready
            {
                return Some(index);
            }
            offset += 1;
        }

        None
    }

    fn all_exited_successfully(&self) -> bool {
        let mut index = 0;
        while index < self.count {
            if let Some(process) = self.processes[index]
                && (process.state != ProcessState::Exited || process.exit_status != 0)
            {
                return false;
            }
            index += 1;
        }

        true
    }
}

impl RuntimeState {
    const fn new() -> Self {
        Self {
            objects: ObjectTable::new(),
            processes: ProcessTable::new(),
            generation_id: "",
            active_config: None,
            next_cap_id: 1,
            revoked_caps: [0; MAX_REVOKED_CAPS],
            revoked_cap_count: 0,
            cap_lineage: [None; MAX_CAP_LINEAGE],
            cap_lineage_count: 0,
            vfs_nodes: [None; MAX_VFS_NODES],
            vfs_node_count: 0,
            next_vfs_node_id: 1,
            next_vfs_metadata_version: 1,
            vfs_mem_files: [VfsMemoryFile::empty(); MAX_VFS_MEM_FILES],
            vfs_mem_file_count: 0,
            vertexfs_image: [0; VERTEXFS_IMAGE_BYTES],
            vertexfs_image_loaded: false,
            vertexfs_files: [VfsVertexFsFile::empty(); MAX_VERTEXFS_FILES],
            vertexfs_file_count: 0,
            open_file_descriptions: [None; MAX_OPEN_FILE_DESCRIPTIONS],
            next_file_description_id: 1,
            vfs_locks: [None; MAX_VFS_LOCKS],
            vfs_events: [None; MAX_VFS_EVENTS],
            vfs_event_count: 0,
            vfs_pipe: VfsPipeBuffer::empty(),
            endpoint_ids: [None; MAX_OBJECTS],
            store_object_ids: [None; MAX_OBJECTS],
            state_volume_ids: [None; MAX_BOOT_STATE_VOLUMES],
            network_port_ids: [None; MAX_OBJECTS],
            io_port_ids: [None; MAX_OBJECTS],
            mmio_region_ids: [None; MAX_OBJECTS],
            interrupt_line_ids: [None; MAX_OBJECTS],
            dma_region_ids: [None; MAX_OBJECTS],
            pci_device_ids: [None; MAX_OBJECTS],
            virtio_device_ids: [None; MAX_OBJECTS],
            namespace_ids: [None; MAX_BOOT_NAMESPACES],
            vfs_root_ids: [None; MAX_BOOT_VFS_ROOTS],
            vfs_mount_ids: [None; MAX_VFS_MOUNTS],
            vfs_mount_count: 0,
            timer_id: None,
            process_control_id: None,
            secret_id: None,
            state_vfs_request_endpoint: None,
            state_vfs_reply_endpoint: None,
            vertexfs_device_request_endpoint: None,
            vertexfs_device_reply_endpoint: None,
            generation_metadata_block_request_endpoint: None,
            generation_metadata_block_reply_endpoint: None,
            next_vfs_state_transaction_id: 1,
            vertexfs_sync_writes: [VertexFsDeviceWrite::empty(); VERTEXFS_SYNC_MAX_DEVICE_WRITES],
            vertexfs_sync_write_count: 0,
            process_template_pids: [None; MAX_PROCESSES],
            service_lifecycle_events: [None; MAX_SERVICE_LIFECYCLE_EVENTS],
            service_lifecycle_event_count: 0,
        }
    }

    fn reset_capability_lifecycle(&mut self, config: &'static BootRuntimeConfig) {
        self.generation_id = config.generation_id;
        self.active_config = Some(config);
        self.next_cap_id = 1;
        self.revoked_cap_count = 0;
        self.cap_lineage_count = 0;
        self.vfs_node_count = 0;
        self.next_vfs_node_id = 1;
        self.next_vfs_metadata_version = 1;
        self.vfs_mem_file_count = 0;
        self.vertexfs_file_count = 0;
        self.next_file_description_id = 1;
        self.vfs_event_count = 0;
        self.vfs_pipe = VfsPipeBuffer::empty();
        self.timer_id = None;
        self.process_control_id = None;
        self.secret_id = None;
        self.state_vfs_request_endpoint = None;
        self.state_vfs_reply_endpoint = None;
        self.vertexfs_device_request_endpoint = None;
        self.vertexfs_device_reply_endpoint = None;
        self.generation_metadata_block_request_endpoint = None;
        self.generation_metadata_block_reply_endpoint = None;
        self.next_vfs_state_transaction_id = 1;
        self.vertexfs_sync_write_count = 0;
        self.vfs_mount_count = 0;
        self.service_lifecycle_event_count = 0;
        let mut index = 0;
        while index < self.revoked_caps.len() {
            self.revoked_caps[index] = 0;
            index += 1;
        }
        index = 0;
        while index < self.cap_lineage.len() {
            self.cap_lineage[index] = None;
            index += 1;
        }
        index = 0;
        while index < self.vfs_nodes.len() {
            self.vfs_nodes[index] = None;
            index += 1;
        }
        index = 0;
        while index < self.vfs_mem_files.len() {
            self.vfs_mem_files[index] = VfsMemoryFile::empty();
            index += 1;
        }
        index = 0;
        while index < self.vertexfs_image.len() {
            self.vertexfs_image[index] = 0;
            index += 1;
        }
        self.vertexfs_image_loaded = false;
        index = 0;
        while index < self.vertexfs_files.len() {
            self.vertexfs_files[index] = VfsVertexFsFile::empty();
            index += 1;
        }
        index = 0;
        while index < self.vertexfs_sync_writes.len() {
            self.vertexfs_sync_writes[index] = VertexFsDeviceWrite::empty();
            index += 1;
        }
        index = 0;
        while index < self.open_file_descriptions.len() {
            self.open_file_descriptions[index] = None;
            index += 1;
        }
        index = 0;
        while index < self.vfs_locks.len() {
            self.vfs_locks[index] = None;
            index += 1;
        }
        index = 0;
        while index < self.vfs_events.len() {
            self.vfs_events[index] = None;
            index += 1;
        }
        index = 0;
        while index < self.endpoint_ids.len() {
            self.endpoint_ids[index] = None;
            self.store_object_ids[index] = None;
            self.network_port_ids[index] = None;
            self.io_port_ids[index] = None;
            self.mmio_region_ids[index] = None;
            self.interrupt_line_ids[index] = None;
            self.dma_region_ids[index] = None;
            self.pci_device_ids[index] = None;
            self.virtio_device_ids[index] = None;
            index += 1;
        }
        index = 0;
        while index < self.state_volume_ids.len() {
            self.state_volume_ids[index] = None;
            index += 1;
        }
        index = 0;
        while index < self.namespace_ids.len() {
            self.namespace_ids[index] = None;
            index += 1;
        }
        index = 0;
        while index < self.vfs_root_ids.len() {
            self.vfs_root_ids[index] = None;
            index += 1;
        }
        index = 0;
        while index < self.vfs_mount_ids.len() {
            self.vfs_mount_ids[index] = None;
            index += 1;
        }
        index = 0;
        while index < self.process_template_pids.len() {
            self.process_template_pids[index] = None;
            index += 1;
        }
        index = 0;
        while index < self.service_lifecycle_events.len() {
            self.service_lifecycle_events[index] = None;
            index += 1;
        }
    }

    fn record_service_lifecycle(
        &mut self,
        service: &'static str,
        state: ServiceLifecycleState,
        status: Option<u64>,
    ) {
        let event = Some(ServiceLifecycleEvent {
            service,
            state,
            status: status.unwrap_or(0),
            has_status: status.is_some(),
        });
        if self.service_lifecycle_event_count < MAX_SERVICE_LIFECYCLE_EVENTS {
            self.service_lifecycle_events[self.service_lifecycle_event_count] = event;
            self.service_lifecycle_event_count += 1;
            return;
        }

        let mut index = 1;
        while index < MAX_SERVICE_LIFECYCLE_EVENTS {
            self.service_lifecycle_events[index - 1] = self.service_lifecycle_events[index];
            index += 1;
        }
        self.service_lifecycle_events[MAX_SERVICE_LIFECYCLE_EVENTS - 1] = event;
    }

    fn add_vfs_node(
        &mut self,
        name: &'static str,
        parent: Option<VfsNodeId>,
        kind: VfsNodeKind,
        backing: VfsBacking,
        mount_source: &'static str,
    ) -> Result<VfsNodeId, InitError> {
        let name = VfsName::from_static(name)?;
        self.add_vfs_node_with_name(name, parent, kind, backing, mount_source)
    }

    fn add_vfs_node_with_name(
        &mut self,
        name: VfsName,
        parent: Option<VfsNodeId>,
        kind: VfsNodeKind,
        backing: VfsBacking,
        mount_source: &'static str,
    ) -> Result<VfsNodeId, InitError> {
        let mut slot = 0;
        while slot < self.vfs_nodes.len() && self.vfs_nodes[slot].is_some() {
            slot += 1;
        }
        if slot == self.vfs_nodes.len() {
            return Err(InitError::ObjectTableFull);
        }
        let id = VfsNodeId(self.next_vfs_node_id);
        self.next_vfs_node_id = self.next_vfs_node_id.saturating_add(1);
        let metadata_version = self.allocate_vfs_metadata_version();
        self.vfs_nodes[slot] = Some(VfsNode::with_name(
            id,
            name,
            parent,
            kind,
            backing,
            mount_source,
            metadata_version,
        ));
        if slot >= self.vfs_node_count {
            self.vfs_node_count = slot + 1;
        }
        Ok(id)
    }

    fn allocate_vfs_metadata_version(&mut self) -> u64 {
        let version = self.next_vfs_metadata_version;
        self.next_vfs_metadata_version = self.next_vfs_metadata_version.saturating_add(1);
        version
    }

    fn touch_vfs_memory_file_nodes(&mut self, backing: usize) -> Result<u64, IpcError> {
        let mut found = false;
        let mut index = 0;
        while index < self.vfs_node_count {
            if let Some(node) = self.vfs_nodes[index]
                && let VfsBacking::MemoryFile(node_backing) = node.backing
                && node_backing == backing
            {
                found = true;
                break;
            }
            index += 1;
        }
        if !found {
            return Err(IpcError::VfsBadHandle);
        }

        let version = self.allocate_vfs_metadata_version();
        index = 0;
        while index < self.vfs_node_count {
            if let Some(node) = self.vfs_nodes[index].as_mut()
                && let VfsBacking::MemoryFile(node_backing) = node.backing
                && node_backing == backing
            {
                node.metadata_version = version;
            }
            index += 1;
        }
        Ok(version)
    }

    fn add_vfs_memory_file(
        &mut self,
        name: &'static str,
        initial: &[u8],
    ) -> Result<usize, InitError> {
        if self.vfs_mem_file_count == self.vfs_mem_files.len() {
            return Err(InitError::ObjectTableFull);
        }
        let index = self.vfs_mem_file_count;
        self.vfs_mem_files[index] = VfsMemoryFile::new(name, initial)?;
        self.vfs_mem_file_count += 1;
        Ok(index)
    }

    fn add_vfs_empty_memory_file(&mut self) -> Result<usize, InitError> {
        let mut index = 0;
        while index < self.vfs_mem_files.len() {
            if !self.vfs_memory_file_in_use(index) {
                self.vfs_mem_files[index] = VfsMemoryFile::empty();
                if index >= self.vfs_mem_file_count {
                    self.vfs_mem_file_count = index + 1;
                }
                return Ok(index);
            }
            index += 1;
        }
        Err(InitError::ObjectTableFull)
    }

    fn vfs_memory_file_in_use(&self, file_index: usize) -> bool {
        let mut index = 0;
        while index < self.vfs_node_count {
            if let Some(node) = self.vfs_nodes[index]
                && let VfsBacking::MemoryFile(backing_index) = node.backing
                && backing_index == file_index
            {
                return true;
            }
            index += 1;
        }
        false
    }

    fn release_vfs_memory_file(&mut self, file_index: usize) -> Result<(), IpcError> {
        if file_index >= self.vfs_mem_files.len() || self.vfs_memory_file_in_use(file_index) {
            return Err(IpcError::BadCapability);
        }
        self.vfs_mem_files[file_index] = VfsMemoryFile::empty();
        while self.vfs_mem_file_count > 0
            && !self.vfs_memory_file_in_use(self.vfs_mem_file_count - 1)
        {
            self.vfs_mem_file_count -= 1;
        }
        Ok(())
    }

    fn vfs_memory_file_link_count(&self, backing: usize) -> u64 {
        let mut count = 0;
        let mut index = 0;
        while index < self.vfs_node_count {
            if let Some(node) = self.vfs_nodes[index]
                && let VfsBacking::MemoryFile(node_backing) = node.backing
                && node_backing == backing
                && node.parent.is_some()
            {
                count += 1;
            }
            index += 1;
        }
        count
    }

    fn touch_vertexfs_file_nodes(&mut self, backing: usize) -> Result<u64, IpcError> {
        let mut found = false;
        let mut index = 0;
        while index < self.vfs_node_count {
            if let Some(node) = self.vfs_nodes[index]
                && let VfsBacking::VertexFsFile(node_backing) = node.backing
                && node_backing == backing
            {
                found = true;
                break;
            }
            index += 1;
        }
        if !found {
            return Err(IpcError::VfsBadHandle);
        }

        let version = self.allocate_vfs_metadata_version();
        index = 0;
        while index < self.vfs_node_count {
            if let Some(node) = self.vfs_nodes[index].as_mut()
                && let VfsBacking::VertexFsFile(node_backing) = node.backing
                && node_backing == backing
            {
                node.metadata_version = version;
            }
            index += 1;
        }
        Ok(version)
    }

    fn add_vertexfs_file(
        &mut self,
        name: &'static str,
        initial: &[u8],
        inode: Option<VertexFsInode>,
    ) -> Result<usize, InitError> {
        if self.vertexfs_file_count == self.vertexfs_files.len() {
            return Err(InitError::ObjectTableFull);
        }
        let index = self.vertexfs_file_count;
        self.vertexfs_files[index] = VfsVertexFsFile::new(name, initial, inode)?;
        self.vertexfs_file_count += 1;
        Ok(index)
    }

    fn add_empty_vertexfs_file(
        &mut self,
        name: VfsName,
        parent_inode_id: u32,
    ) -> Result<usize, IpcError> {
        if name.len > 28 || parent_inode_id != VERTEXFS_INODE_APP_DIR {
            return Err(IpcError::VfsUnsupported);
        }
        if !self.vertexfs_image_loaded {
            return Err(IpcError::VfsUnsupported);
        }
        let mut dynamic_index = 0;
        let mut inode_id = 0;
        let mut first_sector = 0;
        while dynamic_index < VERTEXFS_DYNAMIC_FILE_CAPACITY {
            let candidate_inode = vertexfs_dynamic_inode_at(dynamic_index)?;
            if !self.vertexfs_dynamic_inode_in_use(candidate_inode)
                && !self.vertexfs_image_has_inode(candidate_inode)?
            {
                inode_id = candidate_inode;
                first_sector = vertexfs_dynamic_data_sector_at(dynamic_index)?;
                break;
            }
            dynamic_index += 1;
        }
        if dynamic_index == VERTEXFS_DYNAMIC_FILE_CAPACITY {
            return Err(IpcError::VfsNoSpace);
        }

        let mut index = 0;
        while index < self.vertexfs_files.len() {
            if !self.vertexfs_file_in_use(index) {
                self.vertexfs_files[index] = VfsVertexFsFile::empty();
                self.vertexfs_files[index].vfs_name = name;
                self.vertexfs_files[index].inode_id = inode_id;
                self.vertexfs_files[index].parent_inode_id = parent_inode_id;
                self.vertexfs_files[index].first_sector = first_sector;
                self.vertexfs_files[index].sector_count = 1;
                if index >= self.vertexfs_file_count {
                    self.vertexfs_file_count = index + 1;
                }
                return Ok(index);
            }
            index += 1;
        }
        Err(IpcError::VfsNoSpace)
    }

    fn vertexfs_dynamic_inode_in_use(&self, inode_id: u32) -> bool {
        let mut index = 0;
        while index < self.vertexfs_file_count {
            if self.vertexfs_files[index].inode_id == inode_id {
                return true;
            }
            index += 1;
        }
        false
    }

    fn vertexfs_file_in_use(&self, file_index: usize) -> bool {
        let mut index = 0;
        while index < self.vfs_node_count {
            if let Some(node) = self.vfs_nodes[index]
                && let VfsBacking::VertexFsFile(backing_index) = node.backing
                && backing_index == file_index
            {
                return true;
            }
            index += 1;
        }
        false
    }

    fn release_vertexfs_file(&mut self, file_index: usize) -> Result<(), IpcError> {
        if file_index >= self.vertexfs_files.len() || self.vertexfs_file_in_use(file_index) {
            return Err(IpcError::BadCapability);
        }
        self.vertexfs_files[file_index] = VfsVertexFsFile::empty();
        while self.vertexfs_file_count > 0
            && !self.vertexfs_file_in_use(self.vertexfs_file_count - 1)
        {
            self.vertexfs_file_count -= 1;
        }
        Ok(())
    }

    fn load_vertexfs_image(&mut self, image: &[u8]) -> Result<(), InitError> {
        if image.len() != self.vertexfs_image.len() {
            return Err(InitError::InvalidBootManifest);
        }
        let mut index = 0;
        while index < image.len() {
            self.vertexfs_image[index] = image[index];
            index += 1;
        }
        self.vertexfs_image_loaded = true;
        Ok(())
    }

    fn prepare_vertexfs_sync_file(
        &mut self,
        backing: usize,
    ) -> Result<VertexFsSyncResult, IpcError> {
        if backing >= self.vertexfs_file_count {
            return Err(IpcError::VfsBadHandle);
        }
        let file = self.vertexfs_files[backing];
        let checksum = vertexfs_checksum32(&file.bytes[..file.len]);
        if file.inode_id == 0 {
            let file = &mut self.vertexfs_files[backing];
            file.checksum = checksum;
            file.dirty = false;
            return Ok(VertexFsSyncResult::Cached { checksum });
        }
        if !self.vertexfs_image_loaded {
            return Err(IpcError::VfsUnsupported);
        }
        let write_count = self.commit_vertexfs_file_to_image(file, checksum)?;
        Ok(VertexFsSyncResult::Journaled {
            inode_id: file.inode_id,
            checksum,
            write_count,
        })
    }

    fn finish_vertexfs_sync_file(&mut self, backing: usize, checksum: u32) -> Result<(), IpcError> {
        if backing >= self.vertexfs_file_count {
            return Err(IpcError::VfsBadHandle);
        }
        let file = &mut self.vertexfs_files[backing];
        file.checksum = checksum;
        file.dirty = false;
        Ok(())
    }

    fn commit_vertexfs_file_to_image(
        &mut self,
        file: VfsVertexFsFile,
        checksum: u32,
    ) -> Result<usize, IpcError> {
        let extent_len = file
            .sector_count
            .checked_mul(VERTEXFS_SECTOR_SIZE as u32)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(IpcError::VfsNoSpace)?;
        if file.len > extent_len
            || file.len > VERTEXFS_SECTOR_SIZE - VERTEXFS_JOURNAL_PAYLOAD_OFFSET
        {
            return Err(IpcError::VfsNoSpace);
        }
        self.vertexfs_sync_write_count = 0;
        self.write_vertexfs_journal_pending(file.inode_id, &file.bytes[..file.len])?;
        self.record_vertexfs_sync_sector(VERTEXFS_JOURNAL_SECTOR)?;
        self.write_vertexfs_file_extent(file)?;
        let mut sector = 0;
        while sector < file.sector_count {
            self.record_vertexfs_sync_sector(file.first_sector + sector as u64)?;
            sector += 1;
        }
        if self.vertexfs_image_has_inode(file.inode_id)? {
            self.write_vertexfs_inode_record(file, checksum)?;
            self.record_vertexfs_sync_section(
                VERTEXFS_INODE_TABLE_SECTOR,
                VERTEXFS_INODE_TABLE_SECTORS,
            )?;
        } else {
            self.write_vertexfs_dynamic_metadata(file, checksum)?;
            self.record_vertexfs_sync_section(
                VERTEXFS_INODE_TABLE_SECTOR,
                VERTEXFS_INODE_TABLE_SECTORS,
            )?;
            self.record_vertexfs_sync_section(
                VERTEXFS_DIRECTORY_SECTOR,
                VERTEXFS_DIRECTORY_SECTORS,
            )?;
            self.record_vertexfs_sync_sector(VERTEXFS_FREE_MAP_SECTOR)?;
        }
        self.write_vertexfs_journal_clean()?;
        self.record_vertexfs_sync_sector(VERTEXFS_JOURNAL_SECTOR)?;
        parse_vertexfs_image(&self.vertexfs_image).map_err(|_| IpcError::VfsUnsupported)?;
        Ok(self.vertexfs_sync_write_count)
    }

    fn record_vertexfs_sync_section(
        &mut self,
        first_sector: u64,
        sector_count: u64,
    ) -> Result<(), IpcError> {
        let mut index = 0;
        while index < sector_count {
            self.record_vertexfs_sync_sector(first_sector + index)?;
            index += 1;
        }
        Ok(())
    }

    fn record_vertexfs_sync_sector(&mut self, sector: u64) -> Result<(), IpcError> {
        if self.vertexfs_sync_write_count == self.vertexfs_sync_writes.len() {
            return Err(IpcError::VfsNoSpace);
        }
        let mut bytes = [0u8; VERTEXFS_SECTOR_SIZE];
        bytes.copy_from_slice(self.vertexfs_image_sector(sector)?);
        let index = self.vertexfs_sync_write_count;
        self.vertexfs_sync_writes[index].sector = sector;
        self.vertexfs_sync_writes[index].bytes = bytes;
        self.vertexfs_sync_write_count += 1;
        Ok(())
    }

    fn write_vertexfs_journal_pending(
        &mut self,
        inode_id: u32,
        payload: &[u8],
    ) -> Result<(), IpcError> {
        let sector = self.vertexfs_image_sector_mut(VERTEXFS_JOURNAL_SECTOR)?;
        sector.fill(0);
        copy_bytes(
            &mut sector[..VERTEXFS_JOURNAL_MAGIC.len()],
            VERTEXFS_JOURNAL_MAGIC,
        );
        write_u16_le(sector, 16, VERTEXFS_VERSION);
        write_u16_le(sector, 18, VERTEXFS_JOURNAL_STATE_PENDING);
        write_u32_le(sector, 24, inode_id);
        write_u32_le(sector, 28, payload.len() as u32);
        write_u32_le(sector, 32, vertexfs_checksum32(payload));
        copy_bytes(
            &mut sector
                [VERTEXFS_JOURNAL_PAYLOAD_OFFSET..VERTEXFS_JOURNAL_PAYLOAD_OFFSET + payload.len()],
            payload,
        );
        write_vertexfs_sector_checksum(sector);
        Ok(())
    }

    fn write_vertexfs_file_extent(&mut self, file: VfsVertexFsFile) -> Result<(), IpcError> {
        let start = file
            .first_sector
            .checked_mul(VERTEXFS_SECTOR_SIZE as u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(IpcError::VfsNoSpace)?;
        let extent_len = file
            .sector_count
            .checked_mul(VERTEXFS_SECTOR_SIZE as u32)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(IpcError::VfsNoSpace)?;
        let end = start.checked_add(extent_len).ok_or(IpcError::VfsNoSpace)?;
        let Some(extent) = self.vertexfs_image.get_mut(start..end) else {
            return Err(IpcError::VfsNoSpace);
        };
        extent.fill(0);
        copy_bytes(&mut extent[..file.len], &file.bytes[..file.len]);
        Ok(())
    }

    fn write_vertexfs_inode_record(
        &mut self,
        file: VfsVertexFsFile,
        checksum: u32,
    ) -> Result<(), IpcError> {
        let sector = self.vertexfs_image_section_mut(
            VERTEXFS_INODE_TABLE_SECTOR,
            VERTEXFS_INODE_TABLE_SECTORS,
        )?;
        let offset = vertexfs_inode_offset_by_id(sector, file.inode_id)?;
        write_u64_le(sector, offset + 8, file.len as u64);
        write_u32_le(sector, offset + 28, checksum);
        write_vertexfs_sector_checksum(sector);
        Ok(())
    }

    fn write_vertexfs_dynamic_metadata(
        &mut self,
        file: VfsVertexFsFile,
        checksum: u32,
    ) -> Result<(), IpcError> {
        let dynamic_index = vertexfs_dynamic_index_for_inode(file.inode_id)?;
        if file.parent_inode_id != VERTEXFS_INODE_APP_DIR
            || file.first_sector != vertexfs_dynamic_data_sector_at(dynamic_index)?
            || file.sector_count != 1
        {
            return Err(IpcError::VfsUnsupported);
        }
        let inode_entry_index = VERTEXFS_BASE_INODE_COUNT + dynamic_index;
        let directory_entry_index = VERTEXFS_BASE_DIRECTORY_COUNT + dynamic_index;
        let expected_inode_count = VERTEXFS_BASE_INODE_COUNT + dynamic_index;
        let expected_directory_count = VERTEXFS_BASE_DIRECTORY_COUNT + dynamic_index;
        {
            let sector = self.vertexfs_image_section_mut(
                VERTEXFS_INODE_TABLE_SECTOR,
                VERTEXFS_INODE_TABLE_SECTORS,
            )?;
            if read_u16_le(sector, 18) as usize != expected_inode_count {
                return Err(IpcError::VfsUnsupported);
            }
            write_u16_le(sector, 18, (expected_inode_count + 1) as u16);
            let offset = vertexfs_inode_offset(inode_entry_index)?;
            write_u32_le(sector, offset, file.inode_id);
            write_u16_le(sector, offset + 4, VERTEXFS_KIND_FILE);
            write_u16_le(sector, offset + 6, 0);
            write_u64_le(sector, offset + 8, file.len as u64);
            write_u64_le(sector, offset + 16, file.first_sector);
            write_u32_le(sector, offset + 24, file.sector_count);
            write_u32_le(sector, offset + 28, checksum);
            write_u32_le(sector, offset + 32, file.parent_inode_id);
            write_vertexfs_fixed_vfs_name(sector, offset + 36, 28, file.vfs_name)?;
            write_vertexfs_sector_checksum(sector);
        }
        {
            let sector = self.vertexfs_image_section_mut(
                VERTEXFS_DIRECTORY_SECTOR,
                VERTEXFS_DIRECTORY_SECTORS,
            )?;
            if read_u16_le(sector, 18) as usize != expected_directory_count {
                return Err(IpcError::VfsUnsupported);
            }
            write_u16_le(sector, 18, (expected_directory_count + 1) as u16);
            let offset = vertexfs_directory_offset(directory_entry_index)?;
            write_u32_le(sector, offset, file.parent_inode_id);
            write_u32_le(sector, offset + 4, file.inode_id);
            write_u16_le(sector, offset + 8, VERTEXFS_KIND_FILE);
            write_u16_le(sector, offset + 10, 0);
            write_vertexfs_fixed_vfs_name(
                sector,
                offset + 12,
                VERTEXFS_DIRECTORY_NAME_BYTES,
                file.vfs_name,
            )?;
            write_vertexfs_sector_checksum(sector);
        }
        {
            let sector = self.vertexfs_image_sector_mut(VERTEXFS_FREE_MAP_SECTOR)?;
            let sector_index =
                usize::try_from(file.first_sector).map_err(|_| IpcError::VfsUnsupported)?;
            let Some(byte) = sector.get_mut(32 + sector_index) else {
                return Err(IpcError::VfsUnsupported);
            };
            if *byte != 0 {
                return Err(IpcError::VfsNoSpace);
            }
            *byte = 1;
            write_vertexfs_sector_checksum(sector);
        }
        Ok(())
    }

    fn vertexfs_image_has_inode(&self, inode_id: u32) -> Result<bool, IpcError> {
        let sector =
            self.vertexfs_image_section(VERTEXFS_INODE_TABLE_SECTOR, VERTEXFS_INODE_TABLE_SECTORS)?;
        let count = read_u16_le(sector, 18) as usize;
        let mut index = 0;
        while index < count {
            let offset = vertexfs_inode_offset(index)?;
            if read_u32_le(sector, offset) == inode_id {
                return Ok(true);
            }
            index += 1;
        }
        Ok(false)
    }

    fn write_vertexfs_journal_clean(&mut self) -> Result<(), IpcError> {
        let sector = self.vertexfs_image_sector_mut(VERTEXFS_JOURNAL_SECTOR)?;
        sector.fill(0);
        copy_bytes(
            &mut sector[..VERTEXFS_JOURNAL_MAGIC.len()],
            VERTEXFS_JOURNAL_MAGIC,
        );
        write_u16_le(sector, 16, VERTEXFS_VERSION);
        write_u16_le(sector, 18, VERTEXFS_JOURNAL_STATE_CLEAN);
        write_vertexfs_sector_checksum(sector);
        Ok(())
    }

    fn vertexfs_image_sector_mut(&mut self, sector: u64) -> Result<&mut [u8], IpcError> {
        let sector_index = usize::try_from(sector).map_err(|_| IpcError::VfsNoSpace)?;
        let start = sector_index
            .checked_mul(VERTEXFS_SECTOR_SIZE)
            .ok_or(IpcError::VfsNoSpace)?;
        let end = start
            .checked_add(VERTEXFS_SECTOR_SIZE)
            .ok_or(IpcError::VfsNoSpace)?;
        self.vertexfs_image
            .get_mut(start..end)
            .ok_or(IpcError::VfsNoSpace)
    }

    fn vertexfs_image_sector(&self, sector: u64) -> Result<&[u8], IpcError> {
        let sector_index = usize::try_from(sector).map_err(|_| IpcError::VfsNoSpace)?;
        let start = sector_index
            .checked_mul(VERTEXFS_SECTOR_SIZE)
            .ok_or(IpcError::VfsNoSpace)?;
        let end = start
            .checked_add(VERTEXFS_SECTOR_SIZE)
            .ok_or(IpcError::VfsNoSpace)?;
        self.vertexfs_image
            .get(start..end)
            .ok_or(IpcError::VfsNoSpace)
    }

    fn vertexfs_image_section_mut(
        &mut self,
        first_sector: u64,
        sector_count: u64,
    ) -> Result<&mut [u8], IpcError> {
        let start_sector = usize::try_from(first_sector).map_err(|_| IpcError::VfsNoSpace)?;
        let sector_count = usize::try_from(sector_count).map_err(|_| IpcError::VfsNoSpace)?;
        let start = start_sector
            .checked_mul(VERTEXFS_SECTOR_SIZE)
            .ok_or(IpcError::VfsNoSpace)?;
        let len = sector_count
            .checked_mul(VERTEXFS_SECTOR_SIZE)
            .ok_or(IpcError::VfsNoSpace)?;
        let end = start.checked_add(len).ok_or(IpcError::VfsNoSpace)?;
        self.vertexfs_image
            .get_mut(start..end)
            .ok_or(IpcError::VfsNoSpace)
    }

    fn vertexfs_image_section(
        &self,
        first_sector: u64,
        sector_count: u64,
    ) -> Result<&[u8], IpcError> {
        let start_sector = usize::try_from(first_sector).map_err(|_| IpcError::VfsNoSpace)?;
        let sector_count = usize::try_from(sector_count).map_err(|_| IpcError::VfsNoSpace)?;
        let start = start_sector
            .checked_mul(VERTEXFS_SECTOR_SIZE)
            .ok_or(IpcError::VfsNoSpace)?;
        let len = sector_count
            .checked_mul(VERTEXFS_SECTOR_SIZE)
            .ok_or(IpcError::VfsNoSpace)?;
        let end = start.checked_add(len).ok_or(IpcError::VfsNoSpace)?;
        self.vertexfs_image
            .get(start..end)
            .ok_or(IpcError::VfsNoSpace)
    }

    fn vfs_node_link_count(&self, node: VfsNode) -> u64 {
        match node.backing {
            VfsBacking::MemoryFile(backing) => self.vfs_memory_file_link_count(backing),
            _ => 1,
        }
    }

    fn add_vfs_mount(
        &mut self,
        name: &'static str,
        root_node: VfsNodeId,
        root_path: VfsPath,
        source: &'static str,
        flags: u64,
        dynamic: bool,
        owner: ProcessId,
    ) -> Result<KernelObjectId, InitError> {
        let mut free_slot = None;
        let mut index = 0;
        while index < self.vfs_mount_ids.len() {
            if self.vfs_mount_ids[index].is_none() {
                free_slot = Some(index);
                break;
            }
            index += 1;
        }
        let Some(free_slot) = free_slot else {
            return Err(InitError::ObjectTableFull);
        };
        let id = self
            .objects
            .add_vfs_mount(name, root_node, root_path, source, flags, dynamic, owner)?;
        self.vfs_mount_ids[free_slot] = Some(id);
        if free_slot >= self.vfs_mount_count {
            self.vfs_mount_count = free_slot + 1;
        }
        Ok(id)
    }

    fn remove_vfs_mount_id(&mut self, id: KernelObjectId) {
        let mut index = 0;
        while index < self.vfs_mount_count {
            if self.vfs_mount_ids[index] == Some(id) {
                self.vfs_mount_ids[index] = None;
            }
            index += 1;
        }
        while self.vfs_mount_count > 0 && self.vfs_mount_ids[self.vfs_mount_count - 1].is_none() {
            self.vfs_mount_count -= 1;
        }
    }

    fn remove_owned_dynamic_bind_mounts(&mut self, owner: ProcessId) -> u64 {
        let mut removed = 0;
        let mut index = 0;
        while index < self.objects.count {
            if let Some(KernelObject::VfsMount(mount)) = self.objects.objects[index]
                && mount.dynamic
                && mount.owner == owner
                && mount.flags & VFS_MOUNT_BIND != 0
            {
                self.objects.objects[index] = None;
                self.remove_vfs_mount_id(mount.id);
                removed += 1;
            }
            index += 1;
        }
        self.objects.trim_empty_tail();
        removed
    }

    fn remove_owned_declared_bind_mounts(&mut self, owner: ProcessId) -> u64 {
        let mut removed = 0;
        let mut index = 0;
        while index < self.objects.count {
            if let Some(KernelObject::VfsMount(mount)) = self.objects.objects[index]
                && !mount.dynamic
                && mount.owner == owner
                && mount.flags & VFS_MOUNT_BIND != 0
            {
                self.objects.objects[index] = None;
                self.remove_vfs_mount_id(mount.id);
                removed += 1;
            }
            index += 1;
        }
        self.objects.trim_empty_tail();
        removed
    }

    fn vfs_node(&self, id: VfsNodeId) -> Option<VfsNode> {
        let mut index = 0;
        while index < self.vfs_node_count {
            if let Some(node) = self.vfs_nodes[index]
                && node.id == id
            {
                return Some(node);
            }
            index += 1;
        }
        None
    }

    fn vfs_node_by_parent_name(&self, parent: VfsNodeId, name: &[u8]) -> Option<VfsNode> {
        let mut index = 0;
        while index < self.vfs_node_count {
            if let Some(node) = self.vfs_nodes[index]
                && node.parent == Some(parent)
                && node.name.as_bytes() == name
            {
                return Some(node);
            }
            index += 1;
        }
        None
    }

    fn vfs_node_by_path_from(&self, mut node: VfsNode, path: &[u8]) -> Option<VfsNode> {
        if path.is_empty() {
            return Some(node);
        }
        if path[0] != b'/' {
            return None;
        }
        let mut start = 1;
        while start <= path.len() {
            let mut end = start;
            while end < path.len() && path[end] != b'/' {
                end += 1;
            }
            if end == start {
                return None;
            }
            node = self.vfs_node_by_parent_name(node.id, &path[start..end])?;
            if end == path.len() {
                return Some(node);
            }
            start = end + 1;
        }
        None
    }

    fn vfs_node_by_bind_mount_path(&self, path: &[u8]) -> Option<VfsNode> {
        let mount = self.objects.get_vfs_mount_by_path(path)?;
        if mount.flags & VFS_MOUNT_BIND == 0 {
            return None;
        }
        let root_path = mount.root_path.as_bytes();
        let root = self.vfs_node(mount.root_node)?;
        if path.len() == root_path.len() {
            return Some(root);
        }
        self.vfs_node_by_path_from(root, &path[root_path.len()..])
    }

    fn vfs_node_by_path(&self, path: &[u8]) -> Option<VfsNode> {
        if let Some(node) = self.vfs_node_by_bind_mount_path(path) {
            return Some(node);
        }
        if path == b"/" {
            return self.vfs_nodes[0];
        }
        if path.is_empty() || path[0] != b'/' {
            return None;
        }

        self.vfs_node_by_path_from(self.vfs_nodes[0]?, path)
    }

    fn vfs_node_index(&self, id: VfsNodeId) -> Option<usize> {
        let mut index = 0;
        while index < self.vfs_node_count {
            if let Some(node) = self.vfs_nodes[index]
                && node.id == id
            {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn vfs_node_has_children(&self, id: VfsNodeId) -> bool {
        let mut index = 0;
        while index < self.vfs_node_count {
            if let Some(node) = self.vfs_nodes[index]
                && node.parent == Some(id)
            {
                return true;
            }
            index += 1;
        }
        false
    }

    fn vfs_child_by_entry_index(&self, parent: VfsNodeId, entry_index: usize) -> Option<VfsNode> {
        let mut seen = 0;
        let mut index = 0;
        while index < self.vfs_node_count {
            if let Some(node) = self.vfs_nodes[index]
                && node.parent == Some(parent)
            {
                if seen == entry_index {
                    return Some(node);
                }
                seen += 1;
            }
            index += 1;
        }
        None
    }

    fn vfs_node_has_open_description(&self, id: VfsNodeId) -> bool {
        let mut index = 0;
        while index < self.open_file_descriptions.len() {
            if let Some(description) = self.open_file_descriptions[index]
                && description.node == id
            {
                return true;
            }
            index += 1;
        }
        false
    }

    fn vfs_subtree_has_open_description(&self, root: VfsNodeId) -> bool {
        let mut index = 0;
        while index < self.open_file_descriptions.len() {
            if let Some(description) = self.open_file_descriptions[index]
                && self.vfs_node_is_descendant_or_self(description.node, root)
            {
                return true;
            }
            index += 1;
        }
        false
    }

    fn vfs_node_is_descendant_or_self(&self, mut node: VfsNodeId, root: VfsNodeId) -> bool {
        loop {
            if node == root {
                return true;
            }
            let Some(current) = self.vfs_node(node) else {
                return false;
            };
            let Some(parent) = current.parent else {
                return false;
            };
            node = parent;
        }
    }

    fn remove_vfs_node(&mut self, id: VfsNodeId) -> Result<(), IpcError> {
        let index = self.vfs_node_index(id).ok_or(IpcError::BadCapability)?;
        self.vfs_nodes[index] = None;
        while self.vfs_node_count > 0 && self.vfs_nodes[self.vfs_node_count - 1].is_none() {
            self.vfs_node_count -= 1;
        }
        Ok(())
    }

    fn detach_vfs_node(&mut self, id: VfsNodeId) -> Result<VfsNode, IpcError> {
        let index = self.vfs_node_index(id).ok_or(IpcError::VfsBadHandle)?;
        let version = self.allocate_vfs_metadata_version();
        let Some(node) = self.vfs_nodes[index].as_mut() else {
            return Err(IpcError::VfsBadHandle);
        };
        node.parent = None;
        node.metadata_version = version;
        Ok(*node)
    }

    fn rename_vfs_node(
        &mut self,
        id: VfsNodeId,
        new_parent: VfsNodeId,
        new_name: VfsName,
    ) -> Result<VfsNode, IpcError> {
        let index = self.vfs_node_index(id).ok_or(IpcError::VfsBadHandle)?;
        let version = self.allocate_vfs_metadata_version();
        let Some(node) = self.vfs_nodes[index].as_mut() else {
            return Err(IpcError::VfsBadHandle);
        };
        node.parent = Some(new_parent);
        node.name = new_name;
        node.metadata_version = version;
        Ok(*node)
    }

    fn vfs_node_for_store_object(&self, object: KernelObjectId) -> Option<VfsNode> {
        let mut index = 0;
        while index < self.vfs_node_count {
            if let Some(node) = self.vfs_nodes[index]
                && let VfsBacking::StoreObject(store_object) = node.backing
                && store_object == object
            {
                return Some(node);
            }
            index += 1;
        }
        None
    }

    fn open_file_description(
        &mut self,
        node: VfsNodeId,
        rights: u64,
        flags: u64,
        owner: ProcessId,
        authority_cap_id: u64,
    ) -> Result<FileDescriptionId, IpcError> {
        let mut index = 0;
        while index < self.open_file_descriptions.len() {
            if self.open_file_descriptions[index].is_none() {
                if self.next_file_description_id == 0 {
                    self.next_file_description_id = 1;
                }
                let id = FileDescriptionId(self.next_file_description_id);
                self.next_file_description_id = self.next_file_description_id.saturating_add(1);
                self.open_file_descriptions[index] = Some(OpenFileDescription::new(
                    id,
                    node,
                    rights,
                    flags,
                    owner,
                    authority_cap_id,
                    self.vfs_event_count,
                ));
                return Ok(id);
            }
            index += 1;
        }
        Err(IpcError::BadCapability)
    }

    fn file_description(&self, id: FileDescriptionId) -> Option<OpenFileDescription> {
        let mut index = 0;
        while index < self.open_file_descriptions.len() {
            if let Some(description) = self.open_file_descriptions[index]
                && description.id == id
            {
                return Some(description);
            }
            index += 1;
        }
        None
    }

    fn file_description_mut(&mut self, id: FileDescriptionId) -> Option<&mut OpenFileDescription> {
        let mut index = 0;
        while index < self.open_file_descriptions.len() {
            if let Some(description) = self.open_file_descriptions[index]
                && description.id == id
            {
                break;
            }
            index += 1;
        }
        if index == self.open_file_descriptions.len() {
            return None;
        }
        self.open_file_descriptions[index].as_mut()
    }

    fn retain_file_description(&mut self, id: FileDescriptionId) -> Result<(), IpcError> {
        let description = self
            .file_description_mut(id)
            .ok_or(IpcError::VfsBadHandle)?;
        description.ref_count = description
            .ref_count
            .checked_add(1)
            .ok_or(IpcError::VfsNoSpace)?;
        Ok(())
    }

    fn release_file_description(&mut self, id: FileDescriptionId) -> Result<(), IpcError> {
        let mut index = 0;
        while index < self.open_file_descriptions.len() {
            if let Some(mut description) = self.open_file_descriptions[index]
                && description.id == id
            {
                if description.ref_count <= 1 {
                    self.release_vfs_locks_for_description(id);
                    let node = description.node;
                    self.open_file_descriptions[index] = None;
                    self.reap_unlinked_vfs_node_if_idle(node);
                } else {
                    description.ref_count -= 1;
                    self.open_file_descriptions[index] = Some(description);
                }
                return Ok(());
            }
            index += 1;
        }
        Err(IpcError::VfsBadHandle)
    }

    fn reap_unlinked_vfs_node_if_idle(&mut self, id: VfsNodeId) {
        if self.vfs_node_has_open_description(id) {
            return;
        }
        let Some(node) = self.vfs_node(id) else {
            return;
        };
        if node.parent.is_some() {
            return;
        }
        if let VfsBacking::MemoryFile(backing) = node.backing {
            let _ = self.remove_vfs_node(node.id);
            let _ = self.release_vfs_memory_file(backing);
        }
    }

    fn release_process_file_descriptions(&mut self, pid: ProcessId) {
        self.release_vfs_locks_for_process(pid);
        let mut index = 0;
        while index < self.open_file_descriptions.len() {
            if let Some(description) = self.open_file_descriptions[index]
                && description.owner == pid
            {
                let node = description.node;
                self.open_file_descriptions[index] = None;
                self.reap_unlinked_vfs_node_if_idle(node);
            }
            index += 1;
        }
    }

    fn acquire_vfs_lock(
        &mut self,
        description: OpenFileDescription,
        mode: VfsLockMode,
        start: u64,
        len: u64,
    ) -> Result<(), IpcError> {
        let mut own_lock = None;
        let mut free_lock = None;
        let mut index = 0;
        while index < self.vfs_locks.len() {
            match self.vfs_locks[index] {
                Some(lock) if lock.description == description.id => own_lock = Some(index),
                Some(lock) if lock.node == description.node => {
                    if ranges_overlap(start, len, lock.start, lock.len)
                        && (mode == VfsLockMode::Exclusive || lock.mode == VfsLockMode::Exclusive)
                    {
                        return Err(IpcError::VfsBusy);
                    }
                }
                None if free_lock.is_none() => free_lock = Some(index),
                _ => {}
            }
            index += 1;
        }

        let lock = VfsLock {
            node: description.node,
            owner: description.owner,
            description: description.id,
            mode,
            start,
            len,
        };
        if let Some(index) = own_lock {
            self.vfs_locks[index] = Some(lock);
            return Ok(());
        }
        let Some(index) = free_lock else {
            return Err(IpcError::VfsNoSpace);
        };
        self.vfs_locks[index] = Some(lock);
        Ok(())
    }

    fn record_vfs_event(&mut self, parent: VfsNodeId, kind: u64, name: VfsName) {
        let event = Some(VfsEvent {
            parent,
            kind,
            name,
            metadata_version: self.allocate_vfs_metadata_version(),
        });
        if self.vfs_event_count < self.vfs_events.len() {
            self.vfs_events[self.vfs_event_count] = event;
            self.vfs_event_count += 1;
            return;
        }
        let mut index = 1;
        while index < self.vfs_events.len() {
            self.vfs_events[index - 1] = self.vfs_events[index];
            index += 1;
        }
        self.vfs_events[self.vfs_events.len() - 1] = event;
    }

    fn cap_id_revoked_or_has_revoked_ancestor(&self, cap_id: u64) -> bool {
        let mut current = cap_id;
        while current != 0 {
            if self.cap_id_revoked(current) {
                return true;
            }
            let Some(parent) = self.cap_parent_id(current) else {
                return false;
            };
            current = parent;
        }
        false
    }

    fn cap_parent_id(&self, cap_id: u64) -> Option<u64> {
        let mut index = 0;
        while index < self.cap_lineage_count {
            if let Some(lineage) = self.cap_lineage[index]
                && lineage.cap_id == cap_id
            {
                return Some(lineage.parent_cap_id);
            }
            index += 1;
        }
        None
    }

    fn release_vfs_lock(&mut self, description: FileDescriptionId) -> bool {
        let mut released = false;
        let mut index = 0;
        while index < self.vfs_locks.len() {
            if let Some(lock) = self.vfs_locks[index]
                && lock.description == description
            {
                self.vfs_locks[index] = None;
                released = true;
            }
            index += 1;
        }
        released
    }

    fn release_vfs_locks_for_description(&mut self, description: FileDescriptionId) {
        let _ = self.release_vfs_lock(description);
    }

    fn release_vfs_locks_for_process(&mut self, pid: ProcessId) {
        let mut index = 0;
        while index < self.vfs_locks.len() {
            if let Some(lock) = self.vfs_locks[index]
                && lock.owner == pid
            {
                self.vfs_locks[index] = None;
            }
            index += 1;
        }
    }

    fn generation_cap_count(&self, generation_id: &'static str) -> u64 {
        let mut count = 0;
        let mut process_index = 0;
        while process_index < self.processes.count {
            if let Some(process) = self.processes.processes[process_index] {
                count += generation_cap_count_in_space(process.caps, generation_id);
                count += generation_cap_count_in_space(process.initial_caps, generation_id);
            }
            process_index += 1;
        }
        count
    }

    fn can_allocate_capability(&self) -> bool {
        self.next_cap_id != 0
            && self.next_cap_id != u64::MAX
            && self.cap_lineage_count < self.cap_lineage.len()
    }

    fn new_capability(
        &mut self,
        object: KernelObjectId,
        rights: u64,
        owner_process: ProcessId,
        parent_cap_id: u64,
        delegated_by: ProcessId,
    ) -> Result<Capability, IpcError> {
        if self.next_cap_id == 0 || self.next_cap_id == u64::MAX {
            return Err(IpcError::BadCapability);
        }
        let cap_id = self.next_cap_id;
        self.record_cap_lineage(cap_id, parent_cap_id)?;
        let cap = Capability {
            id: cap_id,
            object,
            rights,
            owner_process,
            parent_cap_id,
            generation_id: self.generation_id,
            delegated_by,
            revoked: false,
        };
        self.next_cap_id += 1;
        Ok(cap)
    }

    fn rollback_last_capability(&mut self, cap: Capability) {
        if self.next_cap_id == cap.id.saturating_add(1) {
            self.next_cap_id = cap.id;
        }
        if self.cap_lineage_count > 0
            && self.cap_lineage[self.cap_lineage_count - 1]
                .map(|lineage| lineage.cap_id == cap.id)
                .unwrap_or(false)
        {
            self.cap_lineage_count -= 1;
            self.cap_lineage[self.cap_lineage_count] = None;
        }
    }

    fn record_cap_lineage(&mut self, cap_id: u64, parent_cap_id: u64) -> Result<(), IpcError> {
        if self.cap_lineage_count == self.cap_lineage.len() {
            return Err(IpcError::BadCapability);
        }
        self.cap_lineage[self.cap_lineage_count] = Some(CapabilityLineage {
            cap_id,
            parent_cap_id,
        });
        self.cap_lineage_count += 1;
        Ok(())
    }

    fn cap_parent_from_lineage(&self, cap_id: u64) -> Option<u64> {
        let mut index = 0;
        while index < self.cap_lineage_count {
            if let Some(lineage) = self.cap_lineage[index]
                && lineage.cap_id == cap_id
            {
                return Some(lineage.parent_cap_id);
            }
            index += 1;
        }
        None
    }

    fn cap_id_revoked(&self, cap_id: u64) -> bool {
        let mut index = 0;
        while index < self.revoked_cap_count {
            if self.revoked_caps[index] == cap_id {
                return true;
            }
            index += 1;
        }
        false
    }

    fn revoke_cap_id(&mut self, cap_id: u64) -> Result<(), IpcError> {
        if cap_id == 0 {
            return Err(IpcError::BadCapability);
        }
        self.add_revoked_cap(cap_id)?;

        let mut changed = true;
        while changed {
            changed = false;
            let mut index = 0;
            while index < self.cap_lineage_count {
                if let Some(lineage) = self.cap_lineage[index]
                    && lineage.parent_cap_id != 0
                    && self.cap_id_revoked(lineage.parent_cap_id)
                    && self.add_revoked_cap(lineage.cap_id)?
                {
                    changed = true;
                }
                index += 1;
            }
        }
        self.mark_all_revoked_caps();
        Ok(())
    }

    fn add_revoked_cap(&mut self, cap_id: u64) -> Result<bool, IpcError> {
        if cap_id == 0 || self.cap_id_revoked(cap_id) {
            return Ok(false);
        }
        if self.revoked_cap_count == self.revoked_caps.len() {
            return Err(IpcError::BadCapability);
        }
        self.revoked_caps[self.revoked_cap_count] = cap_id;
        self.revoked_cap_count += 1;
        Ok(true)
    }

    fn mark_all_revoked_caps(&mut self) {
        let mut index = 0;
        while index < self.revoked_cap_count {
            self.mark_cap_revoked(self.revoked_caps[index]);
            index += 1;
        }
    }

    fn mark_cap_revoked(&mut self, cap_id: u64) {
        let mut index = 0;
        while index < self.processes.count {
            if let Some(process) = self.processes.processes[index].as_mut() {
                process.caps.mark_revoked(cap_id);
                process.initial_caps.mark_revoked(cap_id);
            }
            index += 1;
        }
    }
}

impl BootRuntimeConfig {
    pub const fn new() -> Self {
        Self {
            generation_id: "",
            manifest_hash: [0; 64],
            graph_store_hash: [0; 64],
            graph_store_checksum: 0,
            graph_store_source: "",
            processes: [None; MAX_PROCESSES],
            process_count: 0,
            endpoints: [None; MAX_OBJECTS],
            endpoint_count: 0,
            manifest_module: None,
            store_objects: [None; MAX_OBJECTS],
            store_object_count: 0,
            state_volumes: [None; MAX_BOOT_STATE_VOLUMES],
            state_volume_count: 0,
            network_ports: [None; MAX_OBJECTS],
            network_port_count: 0,
            io_ports: [None; MAX_OBJECTS],
            io_port_count: 0,
            mmio_regions: [None; MAX_OBJECTS],
            mmio_region_count: 0,
            interrupt_lines: [None; MAX_OBJECTS],
            interrupt_line_count: 0,
            dma_regions: [None; MAX_OBJECTS],
            dma_region_count: 0,
            pci_devices: [None; MAX_OBJECTS],
            pci_device_count: 0,
            virtio_devices: [None; MAX_OBJECTS],
            virtio_device_count: 0,
            namespaces: [None; MAX_BOOT_NAMESPACES],
            namespace_count: 0,
            vfs_roots: [None; MAX_BOOT_VFS_ROOTS],
            vfs_root_count: 0,
            graph_nodes: [None; MAX_BOOT_GRAPH_NODES],
            graph_node_count: 0,
            graph_edges: [None; MAX_BOOT_GRAPH_EDGES],
            graph_edge_count: 0,
            grants: [None; MAX_BOOT_GRANTS],
            grant_count: 0,
        }
    }

    pub fn reset(&mut self) {
        self.generation_id = "";
        self.manifest_hash = [0; 64];
        self.graph_store_hash = [0; 64];
        self.graph_store_checksum = 0;
        self.graph_store_source = "";
        self.process_count = 0;
        self.endpoint_count = 0;
        self.manifest_module = None;
        self.store_object_count = 0;
        self.state_volume_count = 0;
        self.network_port_count = 0;
        self.io_port_count = 0;
        self.mmio_region_count = 0;
        self.interrupt_line_count = 0;
        self.dma_region_count = 0;
        self.pci_device_count = 0;
        self.virtio_device_count = 0;
        self.namespace_count = 0;
        self.vfs_root_count = 0;
        self.graph_node_count = 0;
        self.graph_edge_count = 0;
        self.grant_count = 0;
    }

    pub fn set_generation_id(&mut self, generation_id: &'static str) {
        self.generation_id = generation_id;
    }

    pub fn set_manifest_hash(&mut self, hash: [u8; 64]) {
        self.manifest_hash = hash;
    }

    pub fn set_graph_store_hash(&mut self, hash: [u8; 64]) {
        self.graph_store_hash = hash;
    }

    pub fn set_graph_store_checksum(&mut self, checksum: u32) {
        self.graph_store_checksum = checksum;
    }

    pub fn set_graph_store_source(&mut self, source: &'static str) {
        self.graph_store_source = source;
    }

    pub fn add_process(&mut self, process: BootProcessConfig) -> Result<(), InitError> {
        if self.process_count == self.processes.len() {
            return Err(InitError::ProcessTableFull);
        }
        if !valid_vfs_root_path(process.mount_root.as_bytes()) {
            return Err(InitError::InvalidBootManifest);
        }
        if !valid_boot_process_mounts(process) {
            return Err(InitError::InvalidBootManifest);
        }
        self.processes[self.process_count] = Some(process);
        self.process_count += 1;
        Ok(())
    }

    pub fn add_endpoint(&mut self, endpoint: BootEndpointConfig) -> Result<(), InitError> {
        if self.endpoint_count == self.endpoints.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.endpoints[self.endpoint_count] = Some(endpoint);
        self.endpoint_count += 1;
        Ok(())
    }

    pub fn set_manifest_module(&mut self, module: BootModuleConfig) {
        self.manifest_module = Some(module);
    }

    pub fn add_store_object(&mut self, object: BootStoreObjectConfig) -> Result<(), InitError> {
        if self.store_object_count == self.store_objects.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.store_objects[self.store_object_count] = Some(object);
        self.store_object_count += 1;
        Ok(())
    }

    pub fn add_state_volume(&mut self, state: BootStateVolumeConfig) -> Result<(), InitError> {
        if self.state_volume_count == self.state_volumes.len()
            || state_volume_mount_component(state.id).is_err()
        {
            return Err(InitError::InvalidBootManifest);
        }
        self.state_volumes[self.state_volume_count] = Some(state);
        self.state_volume_count += 1;
        Ok(())
    }

    pub fn add_network_port(&mut self, port: BootNetworkPortConfig) -> Result<(), InitError> {
        if self.network_port_count == self.network_ports.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.network_ports[self.network_port_count] = Some(port);
        self.network_port_count += 1;
        Ok(())
    }

    pub fn add_io_port(&mut self, port: BootIoPortRangeConfig) -> Result<(), InitError> {
        if self.io_port_count == self.io_ports.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.io_ports[self.io_port_count] = Some(port);
        self.io_port_count += 1;
        Ok(())
    }

    pub fn add_mmio_region(&mut self, region: BootMmioRegionConfig) -> Result<(), InitError> {
        if self.mmio_region_count == self.mmio_regions.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.mmio_regions[self.mmio_region_count] = Some(region);
        self.mmio_region_count += 1;
        Ok(())
    }

    pub fn add_interrupt_line(&mut self, line: BootInterruptLineConfig) -> Result<(), InitError> {
        if self.interrupt_line_count == self.interrupt_lines.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.interrupt_lines[self.interrupt_line_count] = Some(line);
        self.interrupt_line_count += 1;
        Ok(())
    }

    pub fn add_dma_region(&mut self, region: BootDmaRegionConfig) -> Result<(), InitError> {
        if self.dma_region_count == self.dma_regions.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.dma_regions[self.dma_region_count] = Some(region);
        self.dma_region_count += 1;
        Ok(())
    }

    pub fn add_pci_device(&mut self, device: BootPciDeviceConfig) -> Result<(), InitError> {
        if self.pci_device_count == self.pci_devices.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.pci_devices[self.pci_device_count] = Some(device);
        self.pci_device_count += 1;
        Ok(())
    }

    pub fn add_virtio_device(&mut self, device: BootVirtioDeviceConfig) -> Result<(), InitError> {
        if self.virtio_device_count == self.virtio_devices.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.virtio_devices[self.virtio_device_count] = Some(device);
        self.virtio_device_count += 1;
        Ok(())
    }

    pub fn add_namespace(&mut self, namespace: BootNamespaceConfig) -> Result<(), InitError> {
        if self.namespace_count == self.namespaces.len() {
            return Err(InitError::ObjectTableFull);
        }
        if namespace.entry_count > MAX_NAMESPACE_ENTRIES {
            return Err(InitError::InvalidBootManifest);
        }
        self.namespaces[self.namespace_count] = Some(namespace);
        self.namespace_count += 1;
        Ok(())
    }

    pub fn add_vfs_root(&mut self, root: BootVfsRootConfig) -> Result<(), InitError> {
        if self.vfs_root_count == self.vfs_roots.len() {
            return Err(InitError::ObjectTableFull);
        }
        if !valid_vfs_root_path(root.root_path.as_bytes()) {
            return Err(InitError::InvalidBootManifest);
        }
        self.vfs_roots[self.vfs_root_count] = Some(root);
        self.vfs_root_count += 1;
        Ok(())
    }

    pub fn add_graph_node(&mut self, node: BootGraphNodeConfig) -> Result<(), InitError> {
        if self.graph_node_count == self.graph_nodes.len() || node.kind == 0 || node.id.is_empty() {
            return Err(InitError::InvalidBootManifest);
        }
        let mut index = 0;
        while index < self.graph_node_count {
            if let Some(existing) = self.graph_nodes[index]
                && existing.id == node.id
            {
                return Err(InitError::InvalidBootManifest);
            }
            index += 1;
        }
        self.graph_nodes[self.graph_node_count] = Some(node);
        self.graph_node_count += 1;
        Ok(())
    }

    pub fn add_graph_edge(&mut self, edge: BootGraphEdgeConfig) -> Result<(), InitError> {
        if self.graph_edge_count == self.graph_edges.len()
            || edge.kind == 0
            || edge.id.is_empty()
            || edge.from_index >= self.graph_node_count
            || edge.to_index >= self.graph_node_count
            || (edge.kind == GRAPH_EDGE_CAPABILITY && edge.rights == 0)
        {
            return Err(InitError::InvalidBootManifest);
        }
        let mut index = 0;
        while index < self.graph_edge_count {
            if let Some(existing) = self.graph_edges[index]
                && existing.id == edge.id
            {
                return Err(InitError::InvalidBootManifest);
            }
            index += 1;
        }
        self.graph_edges[self.graph_edge_count] = Some(edge);
        self.graph_edge_count += 1;
        Ok(())
    }

    pub fn add_grant(&mut self, grant: BootGrantConfig) -> Result<(), InitError> {
        if self.grant_count == self.grants.len() {
            return Err(InitError::CapabilityTableFull);
        }
        if grant.process_index >= self.process_count {
            return Err(InitError::InvalidBootManifest);
        }
        let mut index = 0;
        while index < self.grant_count {
            if let Some(existing) = self.grants[index]
                && existing.process_index == grant.process_index
                && existing.cap_slot == grant.cap_slot
            {
                return Err(InitError::InvalidBootManifest);
            }
            index += 1;
        }
        match grant.object_kind {
            BOOT_OBJECT_ENDPOINT if grant.object_index < self.endpoint_count => {}
            BOOT_OBJECT_STORE if grant.object_index < self.store_object_count => {}
            BOOT_OBJECT_TIMER if grant.object_index == 0 => {}
            BOOT_OBJECT_NETWORK_PORT if grant.object_index < self.network_port_count => {}
            BOOT_OBJECT_IO_PORT_RANGE if grant.object_index < self.io_port_count => {}
            BOOT_OBJECT_MMIO_REGION if grant.object_index < self.mmio_region_count => {}
            BOOT_OBJECT_INTERRUPT_LINE if grant.object_index < self.interrupt_line_count => {}
            BOOT_OBJECT_DMA_REGION if grant.object_index < self.dma_region_count => {}
            BOOT_OBJECT_PCI_DEVICE if grant.object_index < self.pci_device_count => {}
            BOOT_OBJECT_VIRTIO_DEVICE if grant.object_index < self.virtio_device_count => {}
            BOOT_OBJECT_NAMESPACE if grant.object_index < self.namespace_count => {}
            BOOT_OBJECT_VFS_ROOT if grant.object_index < self.vfs_root_count => {}
            BOOT_OBJECT_ENDPOINT
            | BOOT_OBJECT_STORE
            | BOOT_OBJECT_STATE
            | BOOT_OBJECT_TIMER
            | BOOT_OBJECT_NETWORK_PORT
            | BOOT_OBJECT_IO_PORT_RANGE
            | BOOT_OBJECT_MMIO_REGION
            | BOOT_OBJECT_INTERRUPT_LINE
            | BOOT_OBJECT_DMA_REGION
            | BOOT_OBJECT_PCI_DEVICE
            | BOOT_OBJECT_VIRTIO_DEVICE
            | BOOT_OBJECT_NAMESPACE
            | BOOT_OBJECT_VFS_ROOT => return Err(InitError::InvalidBootManifest),
            _ => return Err(InitError::InvalidBootManifest),
        }
        self.grants[self.grant_count] = Some(grant);
        self.grant_count += 1;
        Ok(())
    }
}

impl GenerationRuntimeTable {
    const fn new() -> Self {
        Self {
            entries: [None; MAX_GENERATION_CONFIGS],
            count: 0,
        }
    }

    fn register(&mut self, runtime: GenerationRuntime) -> Result<(), InitError> {
        let mut index = 0;
        while index < self.count {
            if let Some(existing) = self.entries[index]
                && existing.generation_id == runtime.generation_id
            {
                self.entries[index] = Some(runtime);
                return Ok(());
            }
            index += 1;
        }

        if self.count == self.entries.len() {
            return Err(InitError::ObjectTableFull);
        }

        self.entries[self.count] = Some(runtime);
        self.count += 1;
        Ok(())
    }

    fn find(&self, generation_id: &[u8]) -> Option<GenerationRuntime> {
        let mut index = 0;
        while index < self.count {
            if let Some(runtime) = self.entries[index]
                && runtime.generation_id.as_bytes() == generation_id
            {
                return Some(runtime);
            }
            index += 1;
        }
        None
    }
}

impl BootManagerState {
    const fn new() -> Self {
        Self {
            selected_generation: "",
            previous_generation: "",
            known_good_generation: "",
            last_failed_generation: "",
            last_failure_reason: "",
            last_failure_service: "",
            last_failure_dependency: "",
            last_failure_policy: "",
            last_transaction_state: "idle",
            last_transaction_target: "",
            transaction_counter: 0,
            boot_attempt_counter: 0,
        }
    }

    fn start_boot(&mut self, generation_id: &'static str) {
        if self.selected_generation.is_empty() {
            self.selected_generation = generation_id;
        }
        self.boot_attempt_counter = self.boot_attempt_counter.saturating_add(1);
        serial::write_str("Native boot manager selected_generation=");
        serial::write_str(self.selected_generation);
        serial::write_str("\n");
        serial::write_str("Native boot manager previous_generation=");
        serial::write_str(if self.previous_generation.is_empty() {
            "<none>"
        } else {
            self.previous_generation
        });
        serial::write_str("\n");
        serial::write_str("Native boot manager known_good_generation=");
        serial::write_str(if self.known_good_generation.is_empty() {
            "<none>"
        } else {
            self.known_good_generation
        });
        serial::write_str("\n");
        serial::write_str("Native boot manager boot_attempt_counter=");
        serial::write_u64_dec(self.boot_attempt_counter);
        serial::write_str("\n");
    }

    fn install_selected(&mut self, previous: &'static str, selected: &'static str) {
        self.previous_generation = previous;
        self.selected_generation = selected;
        self.last_failure_reason = "";
        self.last_failure_service = "";
        self.last_failure_dependency = "";
        self.last_failure_policy = "";
        self.last_transaction_state = "commit";
        self.last_transaction_target = selected;
        self.transaction_counter = self.transaction_counter.saturating_add(1);
        self.boot_attempt_counter = self.boot_attempt_counter.saturating_add(1);
        serial::write_str("Native generation manager journal commit: selected_generation=");
        serial::write_str(selected);
        serial::write_str("\n");
        serial::write_str("Native update transaction selected_generation updated: ");
        serial::write_str(selected);
        serial::write_str("\n");
    }

    fn install_prepare(&mut self, previous: &'static str, target: &'static str) {
        self.previous_generation = previous;
        self.last_transaction_state = "prepare";
        self.last_transaction_target = target;
        self.transaction_counter = self.transaction_counter.saturating_add(1);
        serial::write_str("Native generation manager journal prepare: previous=");
        serial::write_str(previous);
        serial::write_str(" target=");
        serial::write_str(target);
        serial::write_str("\n");
    }

    fn install_abort(&mut self, target: &'static str, reason: &'static str) {
        self.last_failed_generation = target;
        self.last_failure_reason = reason;
        self.record_failure_detail(target, reason);
        self.last_transaction_state = "abort";
        self.last_transaction_target = target;
        self.transaction_counter = self.transaction_counter.saturating_add(1);
        serial::write_str("Native generation manager journal abort: generation=");
        serial::write_str(target);
        serial::write_str(" reason=");
        serial::write_str(reason);
        serial::write_str("\n");
    }

    fn mark_known_good(&mut self, generation_id: &'static str) {
        self.known_good_generation = generation_id;
        self.selected_generation = generation_id;
        self.last_failure_reason = "";
        self.last_failure_service = "";
        self.last_failure_dependency = "";
        self.last_failure_policy = "";
        serial::write_str("Native boot manager known_good_generation=");
        serial::write_str(generation_id);
        serial::write_str("\n");
        serial::write_str("Native boot manager journal: activation-ok generation=");
        serial::write_str(generation_id);
        serial::write_str("\n");
    }

    fn mark_failed_and_fallback(&mut self, failed: &'static str, fallback: &'static str) {
        self.last_failed_generation = failed;
        self.last_failure_reason = "activation-failed";
        self.last_failure_service = failed;
        self.last_failure_dependency = "service-readiness";
        self.last_failure_policy = "known-good-rollback";
        self.previous_generation = failed;
        self.selected_generation = fallback;
        self.last_transaction_state = "rollback";
        self.last_transaction_target = fallback;
        self.transaction_counter = self.transaction_counter.saturating_add(1);
        serial::write_str("Native generation manager journal rollback: failed=");
        serial::write_str(failed);
        serial::write_str(" selected_generation=");
        serial::write_str(fallback);
        serial::write_str(" reason=activation-failed\n");
        serial::write_str("Native boot manager last_failed_generation=");
        serial::write_str(failed);
        serial::write_str("\n");
        serial::write_str("Native boot manager fallback selected_generation=");
        serial::write_str(fallback);
        serial::write_str("\n");
        serial::write_str("Native boot manager previous_generation=");
        serial::write_str(failed);
        serial::write_str("\n");
        serial::write_str("Native boot manager journal: failed generation=");
        serial::write_str(failed);
        serial::write_str(" fallback=");
        serial::write_str(fallback);
        serial::write_str("\n");
        self.log_failure_detail();
    }

    fn recover_from_disk(
        &mut self,
        selected: &'static str,
        previous: &'static str,
        known_good: &'static str,
        transaction: &'static str,
        target: &'static str,
        failure_reason: &'static str,
    ) {
        self.selected_generation = selected;
        self.previous_generation = previous;
        self.known_good_generation = known_good;
        self.last_failure_reason = failure_reason;
        if !failure_reason.is_empty() {
            self.last_failure_service = if transaction == "rollback" && !previous.is_empty() {
                previous
            } else {
                target
            };
            self.last_failure_dependency = if transaction == "rollback" {
                "service-readiness"
            } else {
                "store-closure"
            };
            self.last_failure_policy = if transaction == "rollback" {
                "known-good-rollback"
            } else {
                "activation"
            };
        }
        self.last_transaction_state = transaction;
        self.last_transaction_target = target;
        if !failure_reason.is_empty() {
            self.last_failed_generation = if transaction == "rollback" && !previous.is_empty() {
                previous
            } else {
                target
            };
        }
        serial::write_str("Native generation manager recovered durable state from VertexDisk\n");
        serial::write_str("Native generation manager durable selected_generation=");
        serial::write_str(selected);
        serial::write_str("\n");
        self.log_failure_detail();
    }

    fn record_failure_detail(&mut self, generation: &'static str, reason: &'static str) {
        self.last_failure_service = generation;
        match reason {
            "verification-failed" => {
                self.last_failure_dependency = "store-closure";
                self.last_failure_policy = "installable-generation";
            }
            "runtime-build-failed" => {
                self.last_failure_dependency = "service-readiness";
                self.last_failure_policy = "activation";
            }
            "rollback-build-failed" => {
                self.last_failure_dependency = "rollback-runtime";
                self.last_failure_policy = "known-good-rollback";
            }
            _ => {
                self.last_failure_dependency = "unknown";
                self.last_failure_policy = "activation";
            }
        }
        self.log_failure_detail();
    }

    fn log_failure_detail(&self) {
        if self.last_failure_reason.is_empty() {
            return;
        }
        serial::write_str("Native generation manager failure detail: service=");
        serial::write_str(self.last_failure_service);
        serial::write_str(" dependency=");
        serial::write_str(self.last_failure_dependency);
        serial::write_str(" policy=");
        serial::write_str(self.last_failure_policy);
        serial::write_str(" reason=");
        serial::write_str(self.last_failure_reason);
        serial::write_str("\n");
    }
}

fn generation_cap_count_in_space(space: CapabilitySpace, generation_id: &'static str) -> u64 {
    let mut count = 0;
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = space.caps[slot]
            && cap.generation_id == generation_id
            && !cap.revoked
        {
            count += 1;
        }
        slot += 1;
    }
    count
}

pub fn init_from_boot_config(config: &'static BootRuntimeConfig) -> Result<(), InitError> {
    validate_boot_config_installable(config)?;
    let initial_index = initial_process_index(config)?;
    let initial_process = config.processes[initial_index].ok_or(InitError::InvalidBootManifest)?;
    let initial_context = load_boot_initial_context(initial_process)?;
    let (old_contexts, old_context_count) = snapshot_runtime_reap_targets();

    let result = {
        let staging = staging_runtime();
        build_boot_config_runtime(
            staging,
            config,
            initial_index,
            initial_process,
            initial_context,
        )
    };
    if result.is_err() {
        reclaim_detached_address_space(initial_process.name, initial_context.cr3);
        return result;
    }

    boot_manager().start_boot(config.generation_id);
    release_all_runtime_dma_mappings();
    commit_staging_runtime();
    install_runtime_interrupt_masks(config);
    print_boot_tables(runtime());

    if old_context_count > 0 {
        unsafe {
            gdt::switch_address_space(initial_context.cr3);
        }
        if reap_runtime_contexts(&old_contexts, old_context_count).is_err() {
            serial::write_str("Krust old runtime address-space reap incomplete\n");
        }
    }

    Ok(())
}

fn build_boot_config_runtime(
    runtime: &mut RuntimeState,
    config: &'static BootRuntimeConfig,
    initial_index: usize,
    initial_process: BootProcessConfig,
    initial_context: ProcessContext,
) -> Result<(), InitError> {
    runtime.objects.reset();
    runtime.processes.reset();
    runtime.reset_capability_lifecycle(config);

    let mut endpoint_index = 0;
    while endpoint_index < config.endpoint_count {
        let endpoint = config.endpoints[endpoint_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.endpoint_ids[endpoint_index] = Some(runtime.objects.add_endpoint(endpoint.name)?);
        endpoint_index += 1;
    }
    if config.state_volume_count > 0 {
        runtime.state_vfs_request_endpoint = Some(
            runtime
                .objects
                .add_endpoint(STATE_VFS_REQUEST_ENDPOINT_NAME)?,
        );
        runtime.state_vfs_reply_endpoint = Some(
            runtime
                .objects
                .add_endpoint(STATE_VFS_REPLY_ENDPOINT_NAME)?,
        );
    }
    runtime.vertexfs_device_request_endpoint = Some(
        runtime
            .objects
            .add_endpoint(VERTEXFS_DEVICE_REQUEST_ENDPOINT_NAME)?,
    );
    runtime.vertexfs_device_reply_endpoint = Some(
        runtime
            .objects
            .add_endpoint(VERTEXFS_DEVICE_REPLY_ENDPOINT_NAME)?,
    );
    runtime.generation_metadata_block_request_endpoint = Some(
        runtime
            .objects
            .add_endpoint(GENERATION_METADATA_BLOCK_REQUEST_ENDPOINT_NAME)?,
    );
    runtime.generation_metadata_block_reply_endpoint = Some(
        runtime
            .objects
            .add_endpoint(GENERATION_METADATA_BLOCK_REPLY_ENDPOINT_NAME)?,
    );

    let mut store_index = 0;
    while store_index < config.store_object_count {
        let object = config.store_objects[store_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.store_object_ids[store_index] = Some(runtime.objects.add_store_object(
            object.id,
            object.base,
            object.length,
            object.hash,
        )?);
        store_index += 1;
    }
    let mut state_index = 0;
    while state_index < config.state_volume_count {
        let state = config.state_volumes[state_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.state_volume_ids[state_index] = Some(runtime.objects.add_state_volume(state.id)?);
        state_index += 1;
    }
    let mut network_index = 0;
    while network_index < config.network_port_count {
        let port = config.network_ports[network_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.network_port_ids[network_index] = Some(runtime.objects.add_network_port(port.id)?);
        network_index += 1;
    }

    let mut io_index = 0;
    while io_index < config.io_port_count {
        let port = config.io_ports[io_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.io_port_ids[io_index] = Some(runtime.objects.add_io_port(
            port.id,
            port.base,
            port.length,
        )?);
        io_index += 1;
    }

    let mut mmio_index = 0;
    while mmio_index < config.mmio_region_count {
        let region = config.mmio_regions[mmio_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.mmio_region_ids[mmio_index] = Some(runtime.objects.add_mmio_region(
            region.id,
            region.base,
            region.length,
        )?);
        mmio_index += 1;
    }

    let mut irq_index = 0;
    while irq_index < config.interrupt_line_count {
        let line = config.interrupt_lines[irq_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.interrupt_line_ids[irq_index] =
            Some(runtime.objects.add_interrupt_line(line.id, line.line)?);
        irq_index += 1;
    }

    let mut dma_index = 0;
    while dma_index < config.dma_region_count {
        let region = config.dma_regions[dma_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.dma_region_ids[dma_index] = Some(runtime.objects.add_dma_region(
            region.id,
            region.base,
            region.length,
        )?);
        dma_index += 1;
    }

    let mut pci_index = 0;
    while pci_index < config.pci_device_count {
        let device = config.pci_devices[pci_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.pci_device_ids[pci_index] =
            Some(runtime.objects.add_pci_device(device.id, device.kind)?);
        pci_index += 1;
    }

    let mut virtio_index = 0;
    while virtio_index < config.virtio_device_count {
        let device = config.virtio_devices[virtio_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.virtio_device_ids[virtio_index] = Some(
            runtime
                .objects
                .add_virtio_device(device.id, device.transport)?,
        );
        virtio_index += 1;
    }

    runtime.timer_id = Some(runtime.objects.add_timer("monotonic-timer")?);
    install_vfs_nodes(runtime)?;
    validate_process_mount_roots(runtime, config)?;

    let mut namespace_index = 0;
    while namespace_index < config.namespace_count {
        let namespace = config.namespaces[namespace_index].ok_or(InitError::InvalidBootManifest)?;
        let mut entries = [None; MAX_NAMESPACE_ENTRIES];
        let mut entry_index = 0;
        while entry_index < namespace.entry_count {
            let entry = namespace.entries[entry_index].ok_or(InitError::InvalidBootManifest)?;
            let object = namespace_entry_object_id(runtime, entry)?;
            entries[entry_index] = Some(NamespaceEntry {
                path: entry.path,
                object,
                rights: entry.rights,
            });
            entry_index += 1;
        }
        runtime.namespace_ids[namespace_index] = Some(runtime.objects.add_namespace(
            namespace.id,
            entries,
            namespace.entry_count,
        )?);
        namespace_index += 1;
    }

    let mut vfs_root_index = 0;
    while vfs_root_index < config.vfs_root_count {
        let root = config.vfs_roots[vfs_root_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.vfs_root_ids[vfs_root_index] =
            Some(runtime.objects.add_vfs_root(root.id, root.root_path)?);
        vfs_root_index += 1;
    }

    runtime.secret_id = Some(
        runtime
            .objects
            .add_secret("secret:logd-token", NATIVE_SECRET_VALUE)?,
    );
    serial::write_str("Native secret object registered: secret:logd-token storage=in-memory\n");

    let initial_mount_root = VfsPath::from_boot_root_path(initial_process.mount_root)?;
    let initial_pid = runtime.processes.add_process(
        initial_process.name,
        initial_context,
        initial_process.image_base,
        initial_process.image_length,
        ProcessState::Running,
        CapabilitySpace::new(),
        initial_mount_root,
    )?;
    install_declared_process_mounts(runtime, initial_process, initial_pid, initial_mount_root)?;
    runtime.process_template_pids[initial_index] = Some(initial_pid);
    runtime.processes.set_current(initial_pid);

    grant_config_caps_to_process(runtime, config, initial_index, initial_pid)?;

    if let Some(module) = config.manifest_module {
        let module_id = runtime
            .objects
            .add_boot_module(module.name, module.base, module.length)?;
        let cap = runtime
            .new_capability(
                module_id,
                capability::RIGHT_READ,
                initial_pid,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(runtime, initial_pid, 0, cap, true)?;
    }

    let process_control_id = runtime.objects.add_process_control("process-control")?;
    runtime.process_control_id = Some(process_control_id);
    let process_control_rights = capability::RIGHT_CONTROL
        | capability::RIGHT_ALLOCATE
        | capability::RIGHT_DELEGATE
        | capability::RIGHT_REVOKE
        | capability::RIGHT_INSPECT
        | capability::RIGHT_CREATE
        | capability::RIGHT_START
        | capability::RIGHT_KILL
        | capability::RIGHT_WAIT;
    let cap = runtime
        .new_capability(
            process_control_id,
            process_control_rights,
            initial_pid,
            0,
            ProcessId::empty(),
        )
        .map_err(|_| InitError::CapabilityTableFull)?;
    grant_process_cap_by_pid(runtime, initial_pid, 2, cap, true)?;

    let timer_id = runtime.timer_id.ok_or(InitError::InvalidBootManifest)?;
    let cap = runtime
        .new_capability(
            timer_id,
            capability::RIGHT_CONTROL,
            initial_pid,
            0,
            ProcessId::empty(),
        )
        .map_err(|_| InitError::CapabilityTableFull)?;
    grant_process_cap_by_pid(runtime, initial_pid, INIT_TIMER_CAP_SLOT, cap, true)?;

    Ok(())
}

fn commit_staging_runtime() {
    unsafe {
        core::ptr::copy_nonoverlapping(
            INSTALL_STAGING_RUNTIME.0.get() as *const RuntimeState,
            RUNTIME.0.get(),
            1,
        );
    }
}

fn install_runtime_interrupt_masks(config: &BootRuntimeConfig) {
    timer::reset_legacy_irq_masks();
    let mut irq_index = 0;
    while irq_index < config.interrupt_line_count {
        let Some(line) = config.interrupt_lines[irq_index] else {
            return;
        };
        timer::enable_legacy_irq(line.line as u8);
        serial::write_str("Legacy IRQ unmasked: interrupt-line=");
        serial::write_str(line.id);
        serial::write_str(" line=");
        serial::write_u64_dec(line.line);
        serial::write_str("\n");
        irq_index += 1;
    }
}

fn validate_boot_config_installable(config: &BootRuntimeConfig) -> Result<(), InitError> {
    validate_counted_config_entries(&config.processes, config.process_count)?;
    validate_counted_config_entries(&config.endpoints, config.endpoint_count)?;
    validate_counted_config_entries(&config.store_objects, config.store_object_count)?;
    validate_counted_config_entries(&config.state_volumes, config.state_volume_count)?;
    validate_counted_config_entries(&config.network_ports, config.network_port_count)?;
    validate_counted_config_entries(&config.io_ports, config.io_port_count)?;
    validate_counted_config_entries(&config.mmio_regions, config.mmio_region_count)?;
    validate_counted_config_entries(&config.interrupt_lines, config.interrupt_line_count)?;
    validate_counted_config_entries(&config.dma_regions, config.dma_region_count)?;
    validate_counted_config_entries(&config.pci_devices, config.pci_device_count)?;
    validate_counted_config_entries(&config.virtio_devices, config.virtio_device_count)?;
    validate_counted_config_entries(&config.namespaces, config.namespace_count)?;
    validate_counted_config_entries(&config.vfs_roots, config.vfs_root_count)?;
    validate_counted_config_entries(&config.graph_nodes, config.graph_node_count)?;
    validate_counted_config_entries(&config.graph_edges, config.graph_edge_count)?;
    validate_counted_config_entries(&config.grants, config.grant_count)?;

    if config.endpoint_count == 0 {
        return Err(InitError::InvalidBootManifest);
    }
    let log_endpoint = config.endpoints[0].ok_or(InitError::InvalidBootManifest)?;
    if log_endpoint.name != LOG_ENDPOINT_NAME {
        return Err(InitError::InvalidBootManifest);
    }
    let mut endpoint_index = 1;
    while endpoint_index < config.endpoint_count {
        let endpoint = config.endpoints[endpoint_index].ok_or(InitError::InvalidBootManifest)?;
        if endpoint.name == LOG_ENDPOINT_NAME {
            return Err(InitError::InvalidBootManifest);
        }
        endpoint_index += 1;
    }
    let initial_index = initial_process_index(config)?;
    validate_boot_config_state_volumes(config)?;

    let object_count = boot_config_object_count(config).ok_or(InitError::ObjectTableFull)?;
    if object_count > MAX_OBJECTS {
        serial::write_str("Krust boot config rejected: object budget exceeded objects=");
        serial::write_u64_dec(object_count as u64);
        serial::write_str(" max=");
        serial::write_u64_dec(MAX_OBJECTS as u64);
        serial::write_str("\n");
        return Err(InitError::ObjectTableFull);
    }
    validate_boot_config_hardware_authority(config)?;
    validate_boot_config_graph_store(config)?;

    let mut namespace_index = 0;
    while namespace_index < config.namespace_count {
        let namespace = config.namespaces[namespace_index].ok_or(InitError::InvalidBootManifest)?;
        if namespace.entry_count > MAX_NAMESPACE_ENTRIES {
            return Err(InitError::InvalidBootManifest);
        }
        validate_counted_config_entries(&namespace.entries, namespace.entry_count)?;
        let mut entry_index = 0;
        while entry_index < namespace.entry_count {
            let entry = namespace.entries[entry_index].ok_or(InitError::InvalidBootManifest)?;
            if !namespace_entry_object_kind_allowed(entry.object_kind)
                || !boot_object_config_ref_valid(config, entry.object_kind, entry.object_index)
            {
                return Err(InitError::InvalidBootManifest);
            }
            entry_index += 1;
        }
        namespace_index += 1;
    }

    let mut vfs_root_index = 0;
    while vfs_root_index < config.vfs_root_count {
        let root = config.vfs_roots[vfs_root_index].ok_or(InitError::InvalidBootManifest)?;
        if !valid_vfs_root_path(root.root_path.as_bytes()) {
            return Err(InitError::InvalidBootManifest);
        }
        vfs_root_index += 1;
    }

    let mut grant_index = 0;
    while grant_index < config.grant_count {
        let grant = config.grants[grant_index].ok_or(InitError::InvalidBootManifest)?;
        if grant.process_index >= config.process_count {
            return Err(InitError::InvalidBootManifest);
        }
        let Ok(slot) = usize::try_from(grant.cap_slot) else {
            return Err(InitError::CapabilityTableFull);
        };
        if slot >= MAX_CAPS
            || !boot_object_config_ref_valid(config, grant.object_kind, grant.object_index)
        {
            return Err(InitError::InvalidBootManifest);
        }
        if grant.process_index == initial_index
            && initial_process_reserved_cap_slot(config, grant.cap_slot)
        {
            return Err(InitError::InvalidBootManifest);
        }

        let mut previous = 0;
        while previous < grant_index {
            let previous_grant = config.grants[previous].ok_or(InitError::InvalidBootManifest)?;
            if previous_grant.process_index == grant.process_index
                && previous_grant.cap_slot == grant.cap_slot
            {
                return Err(InitError::InvalidBootManifest);
            }
            previous += 1;
        }
        grant_index += 1;
    }

    Ok(())
}

fn initial_process_reserved_cap_slot(config: &BootRuntimeConfig, slot: u64) -> bool {
    slot == 2 || slot == INIT_TIMER_CAP_SLOT || (slot == 0 && config.manifest_module.is_some())
}

fn validate_boot_config_graph_store(config: &BootRuntimeConfig) -> Result<(), InitError> {
    if config.graph_node_count == 0
        || config.graph_store_hash[0] == 0
        || config.graph_store_source.is_empty()
    {
        return Err(InitError::InvalidBootManifest);
    }
    let mut generation_nodes = 0;
    let mut index = 0;
    while index < config.graph_node_count {
        let node = config.graph_nodes[index].ok_or(InitError::InvalidBootManifest)?;
        if node.kind == 0 || node.id.is_empty() {
            return Err(InitError::InvalidBootManifest);
        }
        if node.kind == GRAPH_NODE_GENERATION {
            generation_nodes += 1;
            if node.id != config.generation_id {
                return Err(InitError::InvalidBootManifest);
            }
        }
        let mut previous = 0;
        while previous < index {
            let prior = config.graph_nodes[previous].ok_or(InitError::InvalidBootManifest)?;
            if prior.id == node.id {
                return Err(InitError::InvalidBootManifest);
            }
            previous += 1;
        }
        index += 1;
    }
    if generation_nodes != 1 {
        return Err(InitError::InvalidBootManifest);
    }

    index = 0;
    while index < config.graph_edge_count {
        let edge = config.graph_edges[index].ok_or(InitError::InvalidBootManifest)?;
        if edge.kind == 0
            || edge.id.is_empty()
            || edge.from_index >= config.graph_node_count
            || edge.to_index >= config.graph_node_count
            || (edge.kind == GRAPH_EDGE_CAPABILITY && edge.rights == 0)
        {
            return Err(InitError::InvalidBootManifest);
        }
        let mut previous = 0;
        while previous < index {
            let prior = config.graph_edges[previous].ok_or(InitError::InvalidBootManifest)?;
            if prior.id == edge.id {
                return Err(InitError::InvalidBootManifest);
            }
            previous += 1;
        }
        index += 1;
    }

    index = 0;
    while index < config.process_count {
        let process = config.processes[index].ok_or(InitError::InvalidBootManifest)?;
        if !boot_graph_has_node(config, GRAPH_NODE_SERVICE, process.graph_node) {
            return Err(InitError::InvalidBootManifest);
        }
        index += 1;
    }

    Ok(())
}

fn boot_graph_has_node(config: &BootRuntimeConfig, kind: u16, id: &str) -> bool {
    let mut index = 0;
    while index < config.graph_node_count {
        if let Some(node) = config.graph_nodes[index]
            && node.kind == kind
            && node.id == id
        {
            return true;
        }
        index += 1;
    }
    false
}

fn validate_counted_config_entries<T: Copy, const N: usize>(
    entries: &[Option<T>; N],
    count: usize,
) -> Result<(), InitError> {
    if count > N {
        return Err(InitError::InvalidBootManifest);
    }
    let mut index = 0;
    while index < count {
        if entries[index].is_none() {
            return Err(InitError::InvalidBootManifest);
        }
        index += 1;
    }
    Ok(())
}

fn validate_boot_config_state_volumes(config: &BootRuntimeConfig) -> Result<(), InitError> {
    if BUILTIN_VFS_MOUNTS
        .checked_add(config.state_volume_count)
        .is_none_or(|count| count > MAX_VFS_MOUNTS)
    {
        return Err(InitError::ObjectTableFull);
    }
    let mut index = 0;
    while index < config.state_volume_count {
        let state = config.state_volumes[index].ok_or(InitError::InvalidBootManifest)?;
        let component = state_volume_mount_component(state.id)?;
        let mut previous = 0;
        while previous < index {
            let prior = config.state_volumes[previous].ok_or(InitError::InvalidBootManifest)?;
            if prior.id == state.id || state_volume_mount_component(prior.id)? == component {
                return Err(InitError::InvalidBootManifest);
            }
            previous += 1;
        }
        index += 1;
    }
    Ok(())
}

fn boot_config_object_count(config: &BootRuntimeConfig) -> Option<usize> {
    let mut count = 0usize;
    count = count.checked_add(config.endpoint_count)?;
    if config.state_volume_count > 0 {
        count = count.checked_add(2)?; // kernel-owned state VFS request/reply endpoints
    }
    count = count.checked_add(config.store_object_count)?;
    count = count.checked_add(config.state_volume_count)?;
    count = count.checked_add(config.network_port_count)?;
    count = count.checked_add(config.io_port_count)?;
    count = count.checked_add(config.mmio_region_count)?;
    count = count.checked_add(config.interrupt_line_count)?;
    count = count.checked_add(config.dma_region_count)?;
    count = count.checked_add(config.pci_device_count)?;
    count = count.checked_add(config.virtio_device_count)?;
    count = count.checked_add(config.namespace_count)?;
    count = count.checked_add(config.vfs_root_count)?;
    count = count.checked_add(BUILTIN_VFS_MOUNTS)?;
    count = count.checked_add(config.state_volume_count)?;
    count = count.checked_add(1)?; // monotonic timer
    count = count.checked_add(1)?; // logd secret
    count = count.checked_add(1)?; // process-control
    if config.manifest_module.is_some() {
        count = count.checked_add(1)?;
    }
    Some(count)
}

fn boot_object_config_ref_valid(
    config: &BootRuntimeConfig,
    object_kind: u16,
    object_index: usize,
) -> bool {
    match object_kind {
        BOOT_OBJECT_ENDPOINT => object_index < config.endpoint_count,
        BOOT_OBJECT_STORE => object_index < config.store_object_count,
        BOOT_OBJECT_STATE => false,
        BOOT_OBJECT_TIMER => object_index == 0,
        BOOT_OBJECT_NETWORK_PORT => object_index < config.network_port_count,
        BOOT_OBJECT_IO_PORT_RANGE => object_index < config.io_port_count,
        BOOT_OBJECT_MMIO_REGION => object_index < config.mmio_region_count,
        BOOT_OBJECT_INTERRUPT_LINE => object_index < config.interrupt_line_count,
        BOOT_OBJECT_DMA_REGION => object_index < config.dma_region_count,
        BOOT_OBJECT_PCI_DEVICE => object_index < config.pci_device_count,
        BOOT_OBJECT_VIRTIO_DEVICE => object_index < config.virtio_device_count,
        BOOT_OBJECT_NAMESPACE => object_index < config.namespace_count,
        BOOT_OBJECT_VFS_ROOT => object_index < config.vfs_root_count,
        _ => false,
    }
}

fn validate_boot_config_hardware_authority(config: &BootRuntimeConfig) -> Result<(), InitError> {
    let mut index = 0;
    while index < config.io_port_count {
        let range = config.io_ports[index].ok_or(InitError::InvalidBootManifest)?;
        validate_io_boot_range(range.base, range.length)?;
        let mut previous = 0;
        while previous < index {
            let prior = config.io_ports[previous].ok_or(InitError::InvalidBootManifest)?;
            if boot_ranges_overlap(range.base, range.length, prior.base, prior.length)? {
                return Err(InitError::InvalidBootManifest);
            }
            previous += 1;
        }
        index += 1;
    }

    index = 0;
    while index < config.mmio_region_count {
        let region = config.mmio_regions[index].ok_or(InitError::InvalidBootManifest)?;
        validate_device_boot_range(region.base, region.length, false)?;
        let mut previous = 0;
        while previous < index {
            let prior = config.mmio_regions[previous].ok_or(InitError::InvalidBootManifest)?;
            if boot_ranges_overlap(region.base, region.length, prior.base, prior.length)? {
                return Err(InitError::InvalidBootManifest);
            }
            previous += 1;
        }
        index += 1;
    }

    index = 0;
    while index < config.interrupt_line_count {
        let line = config.interrupt_lines[index].ok_or(InitError::InvalidBootManifest)?;
        if line.line > 15 {
            return Err(InitError::InvalidBootManifest);
        }
        let mut previous = 0;
        while previous < index {
            let prior = config.interrupt_lines[previous].ok_or(InitError::InvalidBootManifest)?;
            if prior.line == line.line {
                return Err(InitError::InvalidBootManifest);
            }
            previous += 1;
        }
        index += 1;
    }

    index = 0;
    while index < config.dma_region_count {
        let region = config.dma_regions[index].ok_or(InitError::InvalidBootManifest)?;
        validate_device_boot_range(region.base, region.length, true)?;
        if region.length % memory::FRAME_SIZE != 0 || region.length > USER_DEVICE_MAPPING_STRIDE {
            return Err(InitError::InvalidBootManifest);
        }
        let mut previous = 0;
        while previous < index {
            let prior = config.dma_regions[previous].ok_or(InitError::InvalidBootManifest)?;
            if boot_ranges_overlap(region.base, region.length, prior.base, prior.length)? {
                return Err(InitError::InvalidBootManifest);
            }
            previous += 1;
        }
        index += 1;
    }

    Ok(())
}

fn validate_io_boot_range(base: u64, length: u64) -> Result<(), InitError> {
    if length == 0 {
        return Err(InitError::InvalidBootManifest);
    }
    let Some(last) = base.checked_add(length - 1) else {
        return Err(InitError::InvalidBootManifest);
    };
    if last > u16::MAX as u64 {
        return Err(InitError::InvalidBootManifest);
    }
    Ok(())
}

fn validate_device_boot_range(
    base: u64,
    length: u64,
    page_aligned_base: bool,
) -> Result<(), InitError> {
    if length == 0 || length > USER_DEVICE_MAPPING_STRIDE {
        return Err(InitError::InvalidBootManifest);
    }
    base.checked_add(length - 1)
        .ok_or(InitError::InvalidBootManifest)?;
    if page_aligned_base && base % memory::FRAME_SIZE != 0 {
        return Err(InitError::InvalidBootManifest);
    }
    Ok(())
}

fn boot_ranges_overlap(
    base: u64,
    length: u64,
    other_base: u64,
    other_length: u64,
) -> Result<bool, InitError> {
    if length == 0 || other_length == 0 {
        return Ok(false);
    }
    let end = base
        .checked_add(length)
        .ok_or(InitError::InvalidBootManifest)?;
    let other_end = other_base
        .checked_add(other_length)
        .ok_or(InitError::InvalidBootManifest)?;
    Ok(base < other_end && other_base < end)
}

fn load_boot_initial_context(process: BootProcessConfig) -> Result<ProcessContext, InitError> {
    load_process_context(process.name, process.image_base, process.image_length)
        .map_err(|_| InitError::InvalidBootManifest)
}

fn snapshot_runtime_reap_targets() -> ([Option<RuntimeReapTarget>; MAX_PROCESSES], usize) {
    let mut targets = [None; MAX_PROCESSES];
    let mut count = 0;
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && !process.context_reaped
            && process.context.cr3 != 0
        {
            targets[count] = Some(RuntimeReapTarget {
                pid: process.pid,
                name: process.name,
                cr3: process.context.cr3,
            });
            count += 1;
        }
        index += 1;
    }
    (targets, count)
}

fn reap_runtime_contexts(
    targets: &[Option<RuntimeReapTarget>; MAX_PROCESSES],
    count: usize,
) -> Result<(), IpcError> {
    let hhdm_offset = limine::hhdm_offset().ok_or(IpcError::BadCapability)?;
    let allocator = frame_allocator()?;
    let mut index = 0;
    while index < count {
        let target = targets[index].ok_or(IpcError::BadCapability)?;
        let stats = paging::reclaim_user_address_space(hhdm_offset, target.cr3, allocator)
            .map_err(|_| IpcError::BadCapability)?;
        serial::write_str("Krust old runtime address space reaped: proc=");
        serial::write_str(target.name);
        serial::write_str(" pid=");
        serial::write_u64_dec(target.pid.raw());
        serial::write_str(" user_frames=");
        serial::write_u64_dec(stats.user_leaf_frames);
        serial::write_str(" page_tables=");
        serial::write_u64_dec(stats.page_table_frames);
        serial::write_str(" device_mappings=");
        serial::write_u64_dec(stats.device_mappings);
        serial::write_str("\n");
        index += 1;
    }
    Ok(())
}

pub fn register_generation_config(config: &'static BootRuntimeConfig) -> Result<(), InitError> {
    let table = generation_runtimes();
    table.register(GenerationRuntime {
        generation_id: config.generation_id,
        config,
    })
}

pub fn generation_config_by_id(generation_id: &[u8]) -> Option<&'static BootRuntimeConfig> {
    generation_runtimes()
        .find(generation_id)
        .map(|runtime| runtime.config)
}

pub fn set_rollback_boot_config(config: &'static BootRuntimeConfig) {
    set_rollback_runtime(GenerationRuntime {
        generation_id: config.generation_id,
        config,
    });
}

pub fn set_failed_generation_id(generation_id: &'static str) {
    set_failed_generation(generation_id);
}

pub fn install_generation_recovery(
    selected: &'static str,
    previous: &'static str,
    known_good: &'static str,
    transaction: &'static str,
    target: &'static str,
    failure_reason: &'static str,
) {
    boot_manager().recover_from_disk(
        selected,
        previous,
        known_good,
        transaction,
        target,
        failure_reason,
    );
}

pub fn install_frame_allocator(allocator: *mut memory::FrameAllocator) {
    unsafe {
        *FRAME_ALLOCATOR.0.get() = Some(allocator);
    }
}

fn validate_config_caps_for_process(
    runtime: &RuntimeState,
    config: &BootRuntimeConfig,
    config_process_index: usize,
) -> Result<(), InitError> {
    let process = config.processes[config_process_index].ok_or(InitError::InvalidBootManifest)?;
    let mut occupied_slots = [false; MAX_CAPS];
    let mut grant_index = 0;
    while grant_index < config.grant_count {
        let grant = config.grants[grant_index].ok_or(InitError::InvalidBootManifest)?;
        if grant.process_index != config_process_index {
            grant_index += 1;
            continue;
        }

        grant_object_id(runtime, grant)?;
        let Ok(slot) = usize::try_from(grant.cap_slot) else {
            return Err(InitError::CapabilityTableFull);
        };
        if slot >= MAX_CAPS {
            return Err(InitError::CapabilityTableFull);
        }
        if occupied_slots[slot] {
            return Err(InitError::InvalidBootManifest);
        }
        occupied_slots[slot] = true;
        grant_index += 1;
    }

    if process.name == "logd" {
        runtime.secret_id.ok_or(InitError::InvalidBootManifest)?;
        let secret_slot = 6usize;
        if secret_slot >= MAX_CAPS || occupied_slots[secret_slot] {
            return Err(InitError::InvalidBootManifest);
        }
    }
    if process.name == VERTEX_STATE_PROCESS_NAME && runtime.state_vfs_reply_endpoint.is_some() {
        let Ok(request_slot) = usize::try_from(VERTEX_STATE_VFS_REQUEST_CAP_SLOT) else {
            return Err(InitError::CapabilityTableFull);
        };
        let Ok(reply_slot) = usize::try_from(VERTEX_STATE_VFS_REPLY_CAP_SLOT) else {
            return Err(InitError::CapabilityTableFull);
        };
        if request_slot >= MAX_CAPS || occupied_slots[request_slot] {
            return Err(InitError::InvalidBootManifest);
        }
        if reply_slot >= MAX_CAPS || occupied_slots[reply_slot] {
            return Err(InitError::InvalidBootManifest);
        }
    }
    if process.name == BLOCK_DRIVER_PROCESS_NAME && runtime.vertexfs_device_reply_endpoint.is_some()
    {
        let Ok(request_slot) = usize::try_from(BLOCK_DRIVER_VERTEXFS_REQUEST_CAP_SLOT) else {
            return Err(InitError::CapabilityTableFull);
        };
        let Ok(reply_slot) = usize::try_from(BLOCK_DRIVER_VERTEXFS_REPLY_CAP_SLOT) else {
            return Err(InitError::CapabilityTableFull);
        };
        if request_slot >= MAX_CAPS || occupied_slots[request_slot] {
            return Err(InitError::InvalidBootManifest);
        }
        if reply_slot >= MAX_CAPS || occupied_slots[reply_slot] {
            return Err(InitError::InvalidBootManifest);
        }
    }

    Ok(())
}

fn grant_config_caps_to_process(
    runtime: &mut RuntimeState,
    config: &BootRuntimeConfig,
    config_process_index: usize,
    owner: ProcessId,
) -> Result<(), InitError> {
    let mut grant_index = 0;
    while grant_index < config.grant_count {
        let grant = config.grants[grant_index].ok_or(InitError::InvalidBootManifest)?;
        if grant.process_index != config_process_index {
            grant_index += 1;
            continue;
        }
        let object = grant_object_id(runtime, grant)?;
        let cap = runtime
            .new_capability(object, grant.rights, owner, 0, ProcessId::empty())
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(runtime, owner, grant.cap_slot, cap, true)?;
        grant_index += 1;
    }

    let Some(process) = config.processes[config_process_index] else {
        return Err(InitError::InvalidBootManifest);
    };
    if process.name == "logd" {
        let secret_id = runtime.secret_id.ok_or(InitError::InvalidBootManifest)?;
        let cap = runtime
            .new_capability(
                secret_id,
                capability::RIGHT_READ | capability::RIGHT_INSPECT_METADATA,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(runtime, owner, 6, cap, true)?;
        serial::write_str(
            "Native secret grant: process=logd secret=secret:logd-token rights=read|inspect-metadata\n",
        );
    }
    if process.name == VERTEX_STATE_PROCESS_NAME
        && let Some(request_endpoint) = runtime.state_vfs_request_endpoint
    {
        let cap = runtime
            .new_capability(
                request_endpoint,
                capability::RIGHT_RECEIVE,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(runtime, owner, VERTEX_STATE_VFS_REQUEST_CAP_SLOT, cap, true)?;
        serial::write_str(
            "Native VFS state request grant: process=vertex-state endpoint=state-vfs-request rights=receive\n",
        );
    }
    if process.name == VERTEX_STATE_PROCESS_NAME
        && let Some(reply_endpoint) = runtime.state_vfs_reply_endpoint
    {
        let cap = runtime
            .new_capability(
                reply_endpoint,
                capability::RIGHT_SEND,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(runtime, owner, VERTEX_STATE_VFS_REPLY_CAP_SLOT, cap, true)?;
        serial::write_str(
            "Native VFS state reply grant: process=vertex-state endpoint=state-vfs-reply rights=send\n",
        );
    }
    if process.name == BLOCK_DRIVER_PROCESS_NAME
        && let Some(request_endpoint) = runtime.vertexfs_device_request_endpoint
    {
        let cap = runtime
            .new_capability(
                request_endpoint,
                capability::RIGHT_RECEIVE,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(
            runtime,
            owner,
            BLOCK_DRIVER_VERTEXFS_REQUEST_CAP_SLOT,
            cap,
            true,
        )?;
        serial::write_str(
            "Native VertexFS device request grant: process=block-driver endpoint=vertexfs-device-request rights=receive\n",
        );
    }
    if process.name == BLOCK_DRIVER_PROCESS_NAME
        && let Some(reply_endpoint) = runtime.vertexfs_device_reply_endpoint
    {
        let cap = runtime
            .new_capability(
                reply_endpoint,
                capability::RIGHT_SEND,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(
            runtime,
            owner,
            BLOCK_DRIVER_VERTEXFS_REPLY_CAP_SLOT,
            cap,
            true,
        )?;
        serial::write_str(
            "Native VertexFS device reply grant: process=block-driver endpoint=vertexfs-device-reply rights=send\n",
        );
    }
    if process.name == BLOCK_DRIVER_PROCESS_NAME
        && let Some(request_endpoint) = runtime.generation_metadata_block_request_endpoint
    {
        let cap = runtime
            .new_capability(
                request_endpoint,
                capability::RIGHT_RECEIVE,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(
            runtime,
            owner,
            BLOCK_DRIVER_GENERATION_METADATA_REQUEST_CAP_SLOT,
            cap,
            true,
        )?;
        serial::write_str(
            "Native generation metadata block request grant: process=block-driver endpoint=generation-metadata-block-request rights=receive\n",
        );
    }
    if process.name == BLOCK_DRIVER_PROCESS_NAME
        && let Some(reply_endpoint) = runtime.generation_metadata_block_reply_endpoint
    {
        let cap = runtime
            .new_capability(
                reply_endpoint,
                capability::RIGHT_SEND,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(
            runtime,
            owner,
            BLOCK_DRIVER_GENERATION_METADATA_REPLY_CAP_SLOT,
            cap,
            true,
        )?;
        serial::write_str(
            "Native generation metadata block reply grant: process=block-driver endpoint=generation-metadata-block-reply rights=send\n",
        );
    }
    if process.name == GENERATION_MANAGER_PROCESS_NAME
        && let Some(request_endpoint) = runtime.generation_metadata_block_request_endpoint
    {
        let cap = runtime
            .new_capability(
                request_endpoint,
                capability::RIGHT_SEND,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(
            runtime,
            owner,
            GENERATION_MANAGER_METADATA_REQUEST_CAP_SLOT,
            cap,
            true,
        )?;
        serial::write_str(
            "Native generation metadata block request grant: process=gen-manager endpoint=generation-metadata-block-request rights=send\n",
        );
    }
    if process.name == GENERATION_MANAGER_PROCESS_NAME
        && let Some(reply_endpoint) = runtime.generation_metadata_block_reply_endpoint
    {
        let cap = runtime
            .new_capability(
                reply_endpoint,
                capability::RIGHT_RECEIVE,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(
            runtime,
            owner,
            GENERATION_MANAGER_METADATA_REPLY_CAP_SLOT,
            cap,
            true,
        )?;
        serial::write_str(
            "Native generation metadata block reply grant: process=gen-manager endpoint=generation-metadata-block-reply rights=receive\n",
        );
    }

    Ok(())
}

fn install_vfs_nodes(runtime: &mut RuntimeState) -> Result<(), InitError> {
    let vertexfs_image = vertexfs_boot_image()?;
    let vertexfs = parse_vertexfs_image(vertexfs_image)?;
    runtime.load_vertexfs_image(vertexfs_image)?;
    let root = runtime.add_vfs_node(
        "/",
        None,
        VfsNodeKind::Directory,
        VfsBacking::None,
        "rootfs",
    )?;
    runtime.add_vfs_mount(
        "mount:rootfs",
        root,
        VfsPath::from_boot_root_path("/")?,
        "rootfs",
        0,
        false,
        ProcessId::empty(),
    )?;
    let store_root = runtime.add_vfs_node(
        "store",
        Some(root),
        VfsNodeKind::Directory,
        VfsBacking::None,
        "storefs",
    )?;
    runtime.add_vfs_mount(
        "mount:storefs",
        store_root,
        VfsPath::from_boot_root_path("/store")?,
        "storefs",
        0,
        false,
        ProcessId::empty(),
    )?;
    let state_root = runtime.add_vfs_node(
        "state",
        Some(root),
        VfsNodeKind::Directory,
        VfsBacking::None,
        "state:volatile",
    )?;
    runtime.add_vfs_mount(
        "mount:state-volatile",
        state_root,
        VfsPath::from_boot_root_path("/state")?,
        "state:volatile",
        VFS_MOUNT_VOLATILE,
        false,
        ProcessId::empty(),
    )?;
    let dev_root = runtime.add_vfs_node(
        "dev",
        Some(root),
        VfsNodeKind::Directory,
        VfsBacking::None,
        "devfs",
    )?;
    runtime.add_vfs_mount(
        "mount:devfs",
        dev_root,
        VfsPath::from_boot_root_path("/dev")?,
        "devfs",
        0,
        false,
        ProcessId::empty(),
    )?;
    let proc_root = runtime.add_vfs_node(
        "proc",
        Some(root),
        VfsNodeKind::Directory,
        VfsBacking::None,
        "procfs",
    )?;
    runtime.add_vfs_mount(
        "mount:procfs",
        proc_root,
        VfsPath::from_boot_root_path("/proc")?,
        "procfs",
        0,
        false,
        ProcessId::empty(),
    )?;
    let fs_root = runtime.add_vfs_node(
        "fs",
        Some(root),
        VfsNodeKind::Directory,
        VfsBacking::None,
        "vertexfs",
    )?;
    runtime.add_vfs_mount(
        "mount:vertexfs-v1",
        fs_root,
        VfsPath::from_boot_root_path("/fs")?,
        "vertexfs",
        0,
        false,
        ProcessId::empty(),
    )?;
    let vertexfs_readme = runtime.add_vertexfs_file(
        "readme",
        vertexfs.readme.payload,
        Some(vertexfs.readme.inode),
    )?;
    runtime.add_vfs_node(
        "readme",
        Some(fs_root),
        VfsNodeKind::RegularFile,
        VfsBacking::VertexFsFile(vertexfs_readme),
        "vertexfs",
    )?;
    let fs_app = runtime.add_vfs_node(
        "app",
        Some(fs_root),
        VfsNodeKind::Directory,
        VfsBacking::None,
        "vertexfs",
    )?;
    let vertexfs_app_a =
        runtime.add_vertexfs_file("a", vertexfs.app_a.payload, Some(vertexfs.app_a.inode))?;
    runtime.add_vfs_node(
        "a",
        Some(fs_app),
        VfsNodeKind::RegularFile,
        VfsBacking::VertexFsFile(vertexfs_app_a),
        "vertexfs",
    )?;
    serial::write_str("VertexFS v1 superblock accepted: generation=");
    serial::write_ascii_bytes(vertexfs.generation);
    serial::write_str(" feature_flags=metadata-v1\n");
    serial::write_str("VertexFS v1 mounted: path=/fs source=vertexfs\n");
    serial::write_str("VertexFS v1 directory record verified: path=/fs/app\n");
    serial::write_str("VertexFS v1 declared file mounted: path=/fs/app/a\n");
    if vertexfs.journal_replayed {
        serial::write_str("VertexFS v1 journal replayed: inode=4 outcome=new\n");
    }
    let state_a = runtime.add_vfs_memory_file("a", b"state:a=0\n")?;
    runtime.add_vfs_node(
        "a",
        Some(state_root),
        VfsNodeKind::RegularFile,
        VfsBacking::MemoryFile(state_a),
        "state:volatile",
    )?;
    let state_sub = runtime.add_vfs_node(
        "sub",
        Some(state_root),
        VfsNodeKind::Directory,
        VfsBacking::None,
        "state:volatile",
    )?;
    let state_sub_a = runtime.add_vfs_memory_file("a", b"state:sub:a=0\n")?;
    runtime.add_vfs_node(
        "a",
        Some(state_sub),
        VfsNodeKind::RegularFile,
        VfsBacking::MemoryFile(state_sub_a),
        "state:volatile",
    )?;
    runtime.add_vfs_node(
        "service-report",
        Some(state_root),
        VfsNodeKind::RegularFile,
        VfsBacking::FsServiceReport,
        "servicefs",
    )?;
    serial::write_str(
        "VFS filesystem service file mounted: path=/state/service-report source=servicefs\n",
    );
    let mut state_index = 0;
    while state_index < runtime.state_volume_ids.len() {
        if let Some(object_id) = runtime.state_volume_ids[state_index] {
            let state = runtime
                .objects
                .get_state_volume(object_id)
                .ok_or(InitError::InvalidBootManifest)?;
            let node_name = state_volume_vfs_name(state.name)?;
            let root_path = state_volume_vfs_path(state.name)?;
            if runtime.vfs_node_by_path(root_path.as_bytes()).is_some() {
                return Err(InitError::InvalidBootManifest);
            }
            let root_node = runtime.add_vfs_node_with_name(
                node_name,
                Some(state_root),
                VfsNodeKind::Directory,
                VfsBacking::StateVolume(object_id),
                state.name,
            )?;
            runtime.add_vfs_node(
                STATE_VOLUME_VALUE_FILE_NAME,
                Some(root_node),
                VfsNodeKind::RegularFile,
                VfsBacking::StateVolumeValue(object_id),
                state.name,
            )?;
            runtime.add_vfs_node(
                STATE_VOLUME_CONTROL_FILE_NAME,
                Some(root_node),
                VfsNodeKind::RegularFile,
                VfsBacking::StateVolumeControl(object_id),
                state.name,
            )?;
            runtime.add_vfs_mount(
                state.name,
                root_node,
                root_path,
                state.name,
                0,
                false,
                ProcessId::empty(),
            )?;
            serial::write_str("VFS state volume mounted: state=");
            serial::write_str(state.name);
            serial::write_str(" path=");
            serial::write_ascii_bytes(root_path.as_bytes());
            serial::write_str(" source=vertex-state\n");
            serial::write_str("VFS state volume value file mounted: state=");
            serial::write_str(state.name);
            serial::write_str(" path=");
            serial::write_ascii_bytes(root_path.as_bytes());
            serial::write_str("/value source=vertex-state\n");
            serial::write_str("VFS state volume control file mounted: state=");
            serial::write_str(state.name);
            serial::write_str(" path=");
            serial::write_ascii_bytes(root_path.as_bytes());
            serial::write_str("/control source=vertex-state\n");
        }
        state_index += 1;
    }
    runtime.add_vfs_node(
        "inspect",
        Some(proc_root),
        VfsNodeKind::SyntheticNode,
        VfsBacking::Synthetic(VFS_SYNTHETIC_INSPECT_BYTES),
        "procfs",
    )?;
    runtime.add_vfs_node(
        "log-stream",
        Some(proc_root),
        VfsNodeKind::Pipe,
        VfsBacking::Pipe,
        "pipefs",
    )?;

    let mut index = 0;
    while index < runtime.store_object_ids.len() {
        if let Some(object_id) = runtime.store_object_ids[index] {
            let store = runtime
                .objects
                .get_store_object(object_id)
                .ok_or(InitError::InvalidBootManifest)?;
            runtime.add_vfs_node(
                store.name,
                Some(store_root),
                VfsNodeKind::RegularFile,
                VfsBacking::StoreObject(object_id),
                "storefs",
            )?;
            serial::write_str("VFS node registered: file=");
            serial::write_str(store.name);
            serial::write_str(" backing=store-object\n");
        }
        index += 1;
    }

    index = 0;
    while index < runtime.virtio_device_ids.len() {
        if let Some(object_id) = runtime.virtio_device_ids[index] {
            let device = runtime
                .objects
                .get_virtio_device(object_id)
                .ok_or(InitError::InvalidBootManifest)?;
            runtime.add_vfs_node(
                device.name,
                Some(dev_root),
                VfsNodeKind::DeviceNode,
                VfsBacking::Device(object_id),
                "devfs",
            )?;
            serial::write_str("VFS node registered: device=");
            serial::write_str(device.name);
            serial::write_str(" backing=virtio-device\n");
        }
        index += 1;
    }
    Ok(())
}

fn vertexfs_boot_image() -> Result<&'static [u8], InitError> {
    let Some(modules) = limine::modules() else {
        serial::write_str("Krust VertexFS v1 image missing: limine modules unavailable\n");
        return Err(InitError::InvalidBootManifest);
    };

    let mut found = None;
    let mut index = 0;
    while index < modules.module_count() {
        if let Some(module) = modules.module(index)
            && c_string_eq_bytes(module.string, VERTEXFS_MODULE_STRING)
        {
            if found.is_some() {
                return reject_vertexfs_image("duplicate module");
            }
            found = Some(module);
        }
        index += 1;
    }

    let Some(module) = found else {
        serial::write_str("Krust VertexFS v1 image missing\n");
        return Err(InitError::InvalidBootManifest);
    };
    if module.address.is_null() {
        return reject_vertexfs_image("null module");
    }
    let Ok(size) = usize::try_from(module.size) else {
        return reject_vertexfs_image("size overflow");
    };
    if size != VERTEXFS_IMAGE_BYTES {
        serial::write_str("Krust VertexFS v1 image rejected: size=");
        serial::write_u64_dec(module.size);
        serial::write_str(" expected=");
        serial::write_u64_dec(VERTEXFS_IMAGE_BYTES as u64);
        serial::write_str("\n");
        return Err(InitError::InvalidBootManifest);
    }

    serial::write_str("VertexFS v1 image module accepted: bytes=");
    serial::write_u64_dec(module.size);
    serial::write_str("\n");
    Ok(unsafe { core::slice::from_raw_parts(module.address, size) })
}

fn parse_vertexfs_image(image: &[u8]) -> Result<VertexFsBootFiles<'_>, InitError> {
    let superblock = vertexfs_sector(image, 0)?;
    if !vertexfs_magic_matches(superblock, VERTEXFS_SUPERBLOCK_MAGIC) {
        return reject_vertexfs_image("bad superblock");
    }
    if read_u16_le(superblock, 16) != VERTEXFS_VERSION {
        return reject_vertexfs_image("unsupported superblock version");
    }
    if read_u16_le(superblock, 18) as usize != VERTEXFS_SECTOR_SIZE {
        return reject_vertexfs_image("unsupported sector size");
    }
    if !vertexfs_checksum_valid(superblock) {
        return reject_vertexfs_image("superblock checksum mismatch");
    }
    if read_u32_le(superblock, 24) as usize != VERTEXFS_SECTORS {
        return reject_vertexfs_image("unsupported sector count");
    }
    if read_u32_le(superblock, 28) != VERTEXFS_FEATURE_FLAGS {
        return reject_vertexfs_image("unsupported feature flags");
    }
    if !vertexfs_section_matches(
        superblock,
        0,
        VERTEXFS_INODE_TABLE_SECTOR,
        VERTEXFS_INODE_TABLE_SECTORS,
    ) || !vertexfs_section_matches(
        superblock,
        1,
        VERTEXFS_DIRECTORY_SECTOR,
        VERTEXFS_DIRECTORY_SECTORS,
    ) || !vertexfs_section_matches(superblock, 2, VERTEXFS_FREE_MAP_SECTOR, 1)
        || !vertexfs_section_matches(superblock, 3, VERTEXFS_JOURNAL_SECTOR, 1)
        || !vertexfs_section_matches(superblock, 4, VERTEXFS_DATA_SECTOR, VERTEXFS_DATA_SECTORS)
    {
        return reject_vertexfs_image("unsupported section layout");
    }
    let generation = vertexfs_fixed_string(superblock, VERTEXFS_GENERATION_OFFSET, 64)?;

    let journal = vertexfs_parse_journal(image)?;
    let inodes = vertexfs_parse_inode_table(image)?;
    vertexfs_validate_directory(image, &inodes.dynamic)?;
    let (readme, readme_first, readme_end) = vertexfs_file_payload(image, inodes.readme)?;
    let (base_app_a, app_a_first, app_a_end) =
        vertexfs_recoverable_file_payload(image, inodes.app_a, journal)?;
    if readme_first < app_a_end && app_a_first < readme_end {
        return reject_vertexfs_image("overlapping file extents");
    }
    let mut dynamic_extents = [(0u64, 0u64); VERTEXFS_DYNAMIC_FILE_CAPACITY];
    let mut dynamic_extent_count = 0;
    let mut dynamic_index = 0;
    while dynamic_index < VERTEXFS_DYNAMIC_FILE_CAPACITY {
        let Some(dynamic) = inodes.dynamic[dynamic_index] else {
            dynamic_index += 1;
            continue;
        };
        let (_, dynamic_first, dynamic_end) = vertexfs_file_payload(image, dynamic)?;
        if (dynamic_first < readme_end && readme_first < dynamic_end)
            || (dynamic_first < app_a_end && app_a_first < dynamic_end)
        {
            return reject_vertexfs_image("overlapping file extents");
        }
        let mut prior = 0;
        while prior < dynamic_extent_count {
            let (prior_first, prior_end) = dynamic_extents[prior];
            if dynamic_first < prior_end && prior_first < dynamic_end {
                return reject_vertexfs_image("overlapping file extents");
            }
            prior += 1;
        }
        dynamic_extents[dynamic_extent_count] = (dynamic_first, dynamic_end);
        dynamic_extent_count += 1;
        dynamic_index += 1;
    }
    vertexfs_validate_free_map(image, inodes.readme, inodes.app_a, &inodes.dynamic)?;
    let (app_a, journal_replayed) = vertexfs_replay_journal(inodes.app_a, base_app_a, journal)?;

    Ok(VertexFsBootFiles {
        generation,
        readme: VertexFsBootFile {
            inode: inodes.readme,
            payload: readme,
        },
        app_a: VertexFsBootFile {
            inode: inodes.app_a,
            payload: app_a,
        },
        journal_replayed,
    })
}

fn vertexfs_parse_inode_table(image: &[u8]) -> Result<VertexFsParsedInodes, InitError> {
    let sector = vertexfs_section(
        image,
        VERTEXFS_INODE_TABLE_SECTOR,
        VERTEXFS_INODE_TABLE_SECTORS,
    )?;
    if !vertexfs_magic_matches(sector, VERTEXFS_INODE_TABLE_MAGIC) {
        return reject_vertexfs_image("bad inode table");
    }
    if read_u16_le(sector, 16) != VERTEXFS_VERSION || !vertexfs_checksum_valid(sector) {
        return reject_vertexfs_image("inode table metadata invalid");
    }
    let count = read_u16_le(sector, 18) as usize;
    if count < VERTEXFS_BASE_INODE_COUNT
        || count > VERTEXFS_BASE_INODE_COUNT + VERTEXFS_DYNAMIC_FILE_CAPACITY
    {
        return reject_vertexfs_image("inode table count mismatch");
    }

    let root = vertexfs_read_inode(sector, 0);
    if root.id != VERTEXFS_INODE_ROOT
        || root.kind != VERTEXFS_KIND_DIR
        || root.size != 0
        || root.first_sector != 0
        || root.sector_count != 0
        || root.checksum != 0
        || root.parent != 0
        || !vertexfs_inode_reserved_zero(sector, 0)
        || !vertexfs_inode_name_eq(sector, 0, b"/")
    {
        return reject_vertexfs_image("root inode mismatch");
    }

    let readme = vertexfs_read_inode(sector, 1);
    if readme.id != VERTEXFS_INODE_README
        || readme.kind != VERTEXFS_KIND_FILE
        || readme.parent != VERTEXFS_INODE_ROOT
        || !vertexfs_inode_reserved_zero(sector, 1)
        || !vertexfs_inode_name_eq(sector, 1, b"readme")
    {
        return reject_vertexfs_image("readme inode mismatch");
    }

    let app_dir = vertexfs_read_inode(sector, 2);
    if app_dir.id != VERTEXFS_INODE_APP_DIR
        || app_dir.kind != VERTEXFS_KIND_DIR
        || app_dir.size != 0
        || app_dir.first_sector != 0
        || app_dir.sector_count != 0
        || app_dir.checksum != 0
        || app_dir.parent != VERTEXFS_INODE_ROOT
        || !vertexfs_inode_reserved_zero(sector, 2)
        || !vertexfs_inode_name_eq(sector, 2, b"app")
    {
        return reject_vertexfs_image("app directory inode mismatch");
    }

    let app_a = vertexfs_read_inode(sector, 3);
    if app_a.id != VERTEXFS_INODE_APP_A
        || app_a.kind != VERTEXFS_KIND_FILE
        || app_a.parent != VERTEXFS_INODE_APP_DIR
        || !vertexfs_inode_reserved_zero(sector, 3)
        || !vertexfs_inode_name_eq(sector, 3, b"a")
    {
        return reject_vertexfs_image("app/a inode mismatch");
    }

    let mut dynamic = [None; VERTEXFS_DYNAMIC_FILE_CAPACITY];
    let dynamic_count = count - VERTEXFS_BASE_INODE_COUNT;
    let mut dynamic_index = 0;
    while dynamic_index < dynamic_count {
        let inode_index = VERTEXFS_BASE_INODE_COUNT + dynamic_index;
        let inode = vertexfs_read_inode(sector, inode_index);
        if inode.id != VERTEXFS_DYNAMIC_INODE_FIRST + dynamic_index as u32
            || inode.kind != VERTEXFS_KIND_FILE
            || inode.parent != VERTEXFS_INODE_APP_DIR
            || inode.first_sector != VERTEXFS_DYNAMIC_DATA_SECTOR_FIRST + dynamic_index as u64
            || inode.sector_count != 1
            || !vertexfs_inode_reserved_zero(sector, inode_index)
            || vertexfs_fixed_string(sector, vertexfs_inode_name_offset(inode_index), 28).is_err()
        {
            return reject_vertexfs_image("dynamic inode mismatch");
        }
        dynamic[dynamic_index] = Some(inode);
        dynamic_index += 1;
    }

    Ok(VertexFsParsedInodes {
        readme,
        app_a,
        dynamic,
    })
}

fn vertexfs_validate_directory(
    image: &[u8],
    dynamic: &[Option<VertexFsInode>; VERTEXFS_DYNAMIC_FILE_CAPACITY],
) -> Result<(), InitError> {
    let sector = vertexfs_section(image, VERTEXFS_DIRECTORY_SECTOR, VERTEXFS_DIRECTORY_SECTORS)?;
    if !vertexfs_magic_matches(sector, VERTEXFS_DIRECTORY_MAGIC) {
        return reject_vertexfs_image("bad directory");
    }
    if read_u16_le(sector, 16) != VERTEXFS_VERSION || !vertexfs_checksum_valid(sector) {
        return reject_vertexfs_image("directory metadata invalid");
    }
    let count = read_u16_le(sector, 18) as usize;
    if count < VERTEXFS_BASE_DIRECTORY_COUNT
        || count > VERTEXFS_BASE_DIRECTORY_COUNT + VERTEXFS_DIRECTORY_DYNAMIC_FILE_CAPACITY
    {
        return reject_vertexfs_image("directory count mismatch");
    }
    let mut dynamic_count = 0;
    while dynamic_count < VERTEXFS_DYNAMIC_FILE_CAPACITY && dynamic[dynamic_count].is_some() {
        dynamic_count += 1;
    }
    if count != VERTEXFS_BASE_DIRECTORY_COUNT + dynamic_count {
        return reject_vertexfs_image("directory count mismatch");
    }
    if !vertexfs_directory_entry_eq(
        sector,
        0,
        VERTEXFS_INODE_ROOT,
        VERTEXFS_INODE_README,
        VERTEXFS_KIND_FILE,
        b"readme",
    ) || !vertexfs_directory_entry_eq(
        sector,
        1,
        VERTEXFS_INODE_ROOT,
        VERTEXFS_INODE_APP_DIR,
        VERTEXFS_KIND_DIR,
        b"app",
    ) || !vertexfs_directory_entry_eq(
        sector,
        2,
        VERTEXFS_INODE_APP_DIR,
        VERTEXFS_INODE_APP_A,
        VERTEXFS_KIND_FILE,
        b"a",
    ) {
        return reject_vertexfs_image("directory entry mismatch");
    }
    let inode_sector = vertexfs_section(
        image,
        VERTEXFS_INODE_TABLE_SECTOR,
        VERTEXFS_INODE_TABLE_SECTORS,
    )?;
    let mut dynamic_index = 0;
    while dynamic_index < dynamic_count {
        let Some(dynamic_inode) = dynamic[dynamic_index] else {
            return reject_vertexfs_image("dynamic directory entry mismatch");
        };
        let inode_index = VERTEXFS_BASE_INODE_COUNT + dynamic_index;
        let directory_index = VERTEXFS_BASE_DIRECTORY_COUNT + dynamic_index;
        let inode_name =
            vertexfs_fixed_string(inode_sector, vertexfs_inode_name_offset(inode_index), 28)?;
        if !vertexfs_directory_entry_eq(
            sector,
            directory_index,
            VERTEXFS_INODE_APP_DIR,
            dynamic_inode.id,
            VERTEXFS_KIND_FILE,
            inode_name,
        ) {
            return reject_vertexfs_image("dynamic directory entry mismatch");
        }
        dynamic_index += 1;
    }
    Ok(())
}

fn vertexfs_replay_journal<'a>(
    app_a_inode: VertexFsInode,
    base_app_a: &'a [u8],
    journal: Option<VertexFsJournalRecord<'a>>,
) -> Result<(&'a [u8], bool), InitError> {
    let Some(record) = journal else {
        return Ok((base_app_a, false));
    };
    if record.target_inode != VERTEXFS_INODE_APP_A || app_a_inode.id != VERTEXFS_INODE_APP_A {
        return reject_vertexfs_image("journal target unsupported");
    }
    if record.payload.len() > MAX_VERTEXFS_FILE_BYTES {
        return reject_vertexfs_image("journal payload too large");
    }
    if record.payload.len() as u64 > app_a_inode.sector_count as u64 * VERTEXFS_SECTOR_SIZE as u64 {
        return reject_vertexfs_image("journal payload exceeds target extent");
    }
    Ok((record.payload, true))
}

fn vertexfs_parse_journal(image: &[u8]) -> Result<Option<VertexFsJournalRecord<'_>>, InitError> {
    let sector = vertexfs_sector(image, VERTEXFS_JOURNAL_SECTOR)?;
    if !vertexfs_magic_matches(sector, VERTEXFS_JOURNAL_MAGIC) {
        return reject_vertexfs_image("bad journal");
    }
    if read_u16_le(sector, 16) != VERTEXFS_VERSION || !vertexfs_checksum_valid(sector) {
        return reject_vertexfs_image("journal metadata invalid");
    }
    let state = read_u16_le(sector, 18);
    if state == VERTEXFS_JOURNAL_STATE_CLEAN {
        return Ok(None);
    }
    if state != VERTEXFS_JOURNAL_STATE_PENDING {
        return reject_vertexfs_image("journal state unsupported");
    }
    let target_inode = read_u32_le(sector, 24);
    let payload_len = read_u32_le(sector, 28) as usize;
    let payload_checksum = read_u32_le(sector, 32);
    let Some(end) = VERTEXFS_JOURNAL_PAYLOAD_OFFSET.checked_add(payload_len) else {
        return reject_vertexfs_image("journal payload overflow");
    };
    let Some(payload) = sector.get(VERTEXFS_JOURNAL_PAYLOAD_OFFSET..end) else {
        return reject_vertexfs_image("journal payload out of bounds");
    };
    if vertexfs_checksum32(payload) != payload_checksum {
        return reject_vertexfs_image("journal payload checksum mismatch");
    }
    Ok(Some(VertexFsJournalRecord {
        target_inode,
        payload,
    }))
}

fn vertexfs_file_payload(
    image: &[u8],
    inode: VertexFsInode,
) -> Result<(&[u8], u64, u64), InitError> {
    let (data, first_sector, end_sector) = vertexfs_file_extent_payload(image, inode)?;
    if vertexfs_checksum32(data) != inode.checksum {
        return reject_vertexfs_image("file checksum mismatch");
    }
    Ok((data, first_sector, end_sector))
}

fn vertexfs_recoverable_file_payload<'a>(
    image: &'a [u8],
    inode: VertexFsInode,
    journal: Option<VertexFsJournalRecord<'a>>,
) -> Result<(&'a [u8], u64, u64), InitError> {
    let (data, first_sector, end_sector) = vertexfs_file_extent_payload(image, inode)?;
    if vertexfs_checksum32(data) == inode.checksum {
        return Ok((data, first_sector, end_sector));
    }
    if let Some(record) = journal
        && record.target_inode == inode.id
        && vertexfs_extent_starts_with(image, inode, record.payload)
    {
        return Ok((record.payload, first_sector, end_sector));
    }
    reject_vertexfs_image("file checksum mismatch")
}

fn vertexfs_file_extent_payload(
    image: &[u8],
    inode: VertexFsInode,
) -> Result<(&[u8], u64, u64), InitError> {
    if inode.sector_count == 0 {
        return reject_vertexfs_image("file has no extent");
    }
    let Some(end_sector) = inode.first_sector.checked_add(inode.sector_count as u64) else {
        return reject_vertexfs_image("file extent overflow");
    };
    if inode.first_sector < VERTEXFS_DATA_SECTOR
        || end_sector > VERTEXFS_DATA_SECTOR + VERTEXFS_DATA_SECTORS
    {
        return reject_vertexfs_image("file extent outside data section");
    }
    let max_len = inode.sector_count as u64 * VERTEXFS_SECTOR_SIZE as u64;
    if inode.size > max_len || inode.size as usize > MAX_VERTEXFS_FILE_BYTES {
        return reject_vertexfs_image("file length invalid");
    }
    let Ok(start_sector) = usize::try_from(inode.first_sector) else {
        return reject_vertexfs_image("file sector overflow");
    };
    let Some(start) = start_sector.checked_mul(VERTEXFS_SECTOR_SIZE) else {
        return reject_vertexfs_image("file offset overflow");
    };
    let Ok(len) = usize::try_from(inode.size) else {
        return reject_vertexfs_image("file length overflow");
    };
    let Some(end) = start.checked_add(len) else {
        return reject_vertexfs_image("file data overflow");
    };
    let Some(data) = image.get(start..end) else {
        return reject_vertexfs_image("file data out of bounds");
    };
    Ok((data, inode.first_sector, end_sector))
}

fn vertexfs_extent_starts_with(image: &[u8], inode: VertexFsInode, payload: &[u8]) -> bool {
    let Some(start_sector) = usize::try_from(inode.first_sector).ok() else {
        return false;
    };
    let Some(start) = start_sector.checked_mul(VERTEXFS_SECTOR_SIZE) else {
        return false;
    };
    let Some(end) = start.checked_add(payload.len()) else {
        return false;
    };
    image
        .get(start..end)
        .map(|stored| stored == payload)
        .unwrap_or(false)
}

fn vertexfs_validate_free_map(
    image: &[u8],
    readme: VertexFsInode,
    app_a: VertexFsInode,
    dynamic: &[Option<VertexFsInode>; VERTEXFS_DYNAMIC_FILE_CAPACITY],
) -> Result<(), InitError> {
    let sector = vertexfs_sector(image, VERTEXFS_FREE_MAP_SECTOR)?;
    if !vertexfs_magic_matches(sector, VERTEXFS_FREE_MAP_MAGIC) {
        return reject_vertexfs_image("bad free-space map");
    }
    if read_u16_le(sector, 16) != VERTEXFS_VERSION || !vertexfs_checksum_valid(sector) {
        return reject_vertexfs_image("free-space metadata invalid");
    }
    if read_u16_le(sector, 18) as usize != VERTEXFS_SECTORS {
        return reject_vertexfs_image("free-space sector count mismatch");
    }

    let mut index = 0;
    while index < VERTEXFS_SECTORS {
        let allocated = sector[32 + index];
        if allocated != 0 && allocated != 1 {
            return reject_vertexfs_image("free-space allocation byte invalid");
        }
        let sector_number = index as u64;
        let mut dynamic_allocated = false;
        let mut dynamic_index = 0;
        while dynamic_index < VERTEXFS_DYNAMIC_FILE_CAPACITY {
            if let Some(inode) = dynamic[dynamic_index]
                && vertexfs_inode_covers_sector(inode, sector_number)
            {
                dynamic_allocated = true;
                break;
            }
            dynamic_index += 1;
        }
        let expected = sector_number == 0
            || vertexfs_sector_in_section(
                sector_number,
                VERTEXFS_INODE_TABLE_SECTOR,
                VERTEXFS_INODE_TABLE_SECTORS,
            )
            || vertexfs_sector_in_section(
                sector_number,
                VERTEXFS_DIRECTORY_SECTOR,
                VERTEXFS_DIRECTORY_SECTORS,
            )
            || sector_number == VERTEXFS_FREE_MAP_SECTOR
            || sector_number == VERTEXFS_JOURNAL_SECTOR
            || vertexfs_inode_covers_sector(readme, sector_number)
            || vertexfs_inode_covers_sector(app_a, sector_number)
            || dynamic_allocated;
        if (allocated == 1) != expected {
            return reject_vertexfs_image("free-space metadata mismatch");
        }
        index += 1;
    }
    Ok(())
}

fn vertexfs_sector(image: &[u8], sector: u64) -> Result<&[u8], InitError> {
    let Ok(sector_index) = usize::try_from(sector) else {
        return reject_vertexfs_image("sector overflow");
    };
    let Some(offset) = sector_index.checked_mul(VERTEXFS_SECTOR_SIZE) else {
        return reject_vertexfs_image("sector offset overflow");
    };
    let Some(end) = offset.checked_add(VERTEXFS_SECTOR_SIZE) else {
        return reject_vertexfs_image("sector end overflow");
    };
    let Some(bytes) = image.get(offset..end) else {
        return reject_vertexfs_image("sector out of bounds");
    };
    Ok(bytes)
}

fn vertexfs_section(
    image: &[u8],
    first_sector: u64,
    sector_count: u64,
) -> Result<&[u8], InitError> {
    let Ok(start_sector) = usize::try_from(first_sector) else {
        return reject_vertexfs_image("section overflow");
    };
    let Ok(sector_count) = usize::try_from(sector_count) else {
        return reject_vertexfs_image("section overflow");
    };
    let Some(start) = start_sector.checked_mul(VERTEXFS_SECTOR_SIZE) else {
        return reject_vertexfs_image("section offset overflow");
    };
    let Some(len) = sector_count.checked_mul(VERTEXFS_SECTOR_SIZE) else {
        return reject_vertexfs_image("section length overflow");
    };
    let Some(end) = start.checked_add(len) else {
        return reject_vertexfs_image("section end overflow");
    };
    let Some(bytes) = image.get(start..end) else {
        return reject_vertexfs_image("section out of bounds");
    };
    Ok(bytes)
}

fn vertexfs_read_inode(sector: &[u8], index: usize) -> VertexFsInode {
    let offset = VERTEXFS_INODE_ENTRY_OFFSET + index * VERTEXFS_INODE_ENTRY_LEN;
    VertexFsInode {
        id: read_u32_le(sector, offset),
        kind: read_u16_le(sector, offset + 4),
        size: read_u64_le(sector, offset + 8),
        first_sector: read_u64_le(sector, offset + 16),
        sector_count: read_u32_le(sector, offset + 24),
        checksum: read_u32_le(sector, offset + 28),
        parent: read_u32_le(sector, offset + 32),
    }
}

fn vertexfs_inode_name_eq(sector: &[u8], index: usize, expected: &[u8]) -> bool {
    let offset = vertexfs_inode_name_offset(index);
    vertexfs_fixed_string_eq(sector, offset, 28, expected)
}

fn vertexfs_inode_reserved_zero(sector: &[u8], index: usize) -> bool {
    let offset = VERTEXFS_INODE_ENTRY_OFFSET + index * VERTEXFS_INODE_ENTRY_LEN + 6;
    read_u16_le(sector, offset) == 0
}

fn vertexfs_inode_name_offset(index: usize) -> usize {
    VERTEXFS_INODE_ENTRY_OFFSET + index * VERTEXFS_INODE_ENTRY_LEN + 36
}

fn vertexfs_directory_entry_eq(
    sector: &[u8],
    index: usize,
    parent: u32,
    child: u32,
    kind: u16,
    name: &[u8],
) -> bool {
    let offset = VERTEXFS_DIRECTORY_ENTRY_OFFSET + index * VERTEXFS_DIRECTORY_ENTRY_LEN;
    read_u32_le(sector, offset) == parent
        && read_u32_le(sector, offset + 4) == child
        && read_u16_le(sector, offset + 8) == kind
        && read_u16_le(sector, offset + 10) == 0
        && vertexfs_fixed_string_eq(sector, offset + 12, VERTEXFS_DIRECTORY_NAME_BYTES, name)
}

fn vertexfs_inode_covers_sector(inode: VertexFsInode, sector: u64) -> bool {
    inode
        .first_sector
        .checked_add(inode.sector_count as u64)
        .is_some_and(|end| sector >= inode.first_sector && sector < end)
}

fn vertexfs_sector_in_section(sector: u64, first_sector: u64, sector_count: u64) -> bool {
    first_sector
        .checked_add(sector_count)
        .is_some_and(|end| sector >= first_sector && sector < end)
}

fn vertexfs_section_matches(
    superblock: &[u8],
    index: usize,
    first_sector: u64,
    sector_count: u64,
) -> bool {
    let offset = VERTEXFS_SECTION_TABLE_OFFSET + index * VERTEXFS_SECTION_RECORD_LEN;
    read_u64_le(superblock, offset) == first_sector
        && read_u64_le(superblock, offset + 8) == sector_count
}

fn vertexfs_magic_matches(sector: &[u8], magic: &[u8; 16]) -> bool {
    let mut index = 0;
    while index < magic.len() {
        if sector[index] != magic[index] {
            return false;
        }
        index += 1;
    }
    true
}

fn vertexfs_fixed_string(buffer: &[u8], offset: usize, max_len: usize) -> Result<&[u8], InitError> {
    if offset
        .checked_add(max_len)
        .is_none_or(|end| end > buffer.len())
    {
        return reject_vertexfs_image("fixed string bounds invalid");
    }
    let mut len = 0;
    while len < max_len && buffer[offset + len] != 0 {
        let byte = buffer[offset + len];
        if !byte.is_ascii_graphic() && byte != b' ' {
            return reject_vertexfs_image("fixed string byte invalid");
        }
        len += 1;
    }
    if len == 0 {
        return reject_vertexfs_image("fixed string empty");
    }
    let mut index = len;
    while index < max_len {
        if buffer[offset + index] != 0 {
            return reject_vertexfs_image("fixed string not zero padded");
        }
        index += 1;
    }
    Ok(&buffer[offset..offset + len])
}

fn vertexfs_fixed_string_eq(buffer: &[u8], offset: usize, max_len: usize, expected: &[u8]) -> bool {
    if expected.is_empty()
        || expected.len() > max_len
        || offset
            .checked_add(max_len)
            .is_none_or(|end| end > buffer.len())
    {
        return false;
    }
    let mut index = 0;
    while index < expected.len() {
        if buffer[offset + index] != expected[index] {
            return false;
        }
        index += 1;
    }
    while index < max_len {
        if buffer[offset + index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

fn vertexfs_checksum_valid(bytes: &[u8]) -> bool {
    let stored = read_u32_le(bytes, VERTEXFS_CHECKSUM_OFFSET);
    let mut checksum = 0u32;
    let mut index = 0;
    while index < bytes.len() {
        let byte = if index >= VERTEXFS_CHECKSUM_OFFSET && index < VERTEXFS_CHECKSUM_OFFSET + 4 {
            0
        } else {
            bytes[index]
        };
        checksum = checksum.wrapping_add((byte as u32).wrapping_mul(index as u32 + 1));
        index += 1;
    }
    checksum == stored
}

fn vertexfs_checksum32(bytes: &[u8]) -> u32 {
    let mut checksum = 0u32;
    let mut index = 0;
    while index < bytes.len() {
        checksum = checksum.wrapping_add((bytes[index] as u32).wrapping_mul(index as u32 + 1));
        index += 1;
    }
    checksum
}

fn write_vertexfs_sector_checksum(sector: &mut [u8]) {
    write_u32_le(sector, VERTEXFS_CHECKSUM_OFFSET, 0);
    let checksum = vertexfs_checksum32(sector);
    write_u32_le(sector, VERTEXFS_CHECKSUM_OFFSET, checksum);
}

fn vertexfs_inode_offset_by_id(sector: &[u8], inode_id: u32) -> Result<usize, IpcError> {
    let count = read_u16_le(sector, 18) as usize;
    let mut index = 0;
    while index < count {
        let offset = vertexfs_inode_offset(index)?;
        if read_u32_le(sector, offset) == inode_id {
            return Ok(offset);
        }
        index += 1;
    }
    Err(IpcError::VfsUnsupported)
}

fn vertexfs_dynamic_index_for_inode(inode_id: u32) -> Result<usize, IpcError> {
    if inode_id < VERTEXFS_DYNAMIC_INODE_FIRST {
        return Err(IpcError::VfsUnsupported);
    }
    let index = usize::try_from(inode_id - VERTEXFS_DYNAMIC_INODE_FIRST)
        .map_err(|_| IpcError::VfsUnsupported)?;
    if index >= VERTEXFS_DYNAMIC_FILE_CAPACITY {
        return Err(IpcError::VfsUnsupported);
    }
    Ok(index)
}

fn vertexfs_dynamic_inode_at(index: usize) -> Result<u32, IpcError> {
    if index >= VERTEXFS_DYNAMIC_FILE_CAPACITY {
        return Err(IpcError::VfsUnsupported);
    }
    Ok(VERTEXFS_DYNAMIC_INODE_FIRST + index as u32)
}

fn vertexfs_dynamic_data_sector_at(index: usize) -> Result<u64, IpcError> {
    if index >= VERTEXFS_DYNAMIC_FILE_CAPACITY {
        return Err(IpcError::VfsUnsupported);
    }
    Ok(VERTEXFS_DYNAMIC_DATA_SECTOR_FIRST + index as u64)
}

fn vertexfs_inode_offset(index: usize) -> Result<usize, IpcError> {
    let offset = VERTEXFS_INODE_ENTRY_OFFSET
        .checked_add(
            index
                .checked_mul(VERTEXFS_INODE_ENTRY_LEN)
                .ok_or(IpcError::VfsUnsupported)?,
        )
        .ok_or(IpcError::VfsUnsupported)?;
    if offset
        .checked_add(VERTEXFS_INODE_ENTRY_LEN)
        .is_none_or(|end| end > VERTEXFS_INODE_TABLE_BYTES)
    {
        return Err(IpcError::VfsUnsupported);
    }
    Ok(offset)
}

fn vertexfs_directory_offset(index: usize) -> Result<usize, IpcError> {
    let offset = VERTEXFS_DIRECTORY_ENTRY_OFFSET
        .checked_add(
            index
                .checked_mul(VERTEXFS_DIRECTORY_ENTRY_LEN)
                .ok_or(IpcError::VfsUnsupported)?,
        )
        .ok_or(IpcError::VfsUnsupported)?;
    if offset
        .checked_add(VERTEXFS_DIRECTORY_ENTRY_LEN)
        .is_none_or(|end| end > VERTEXFS_DIRECTORY_BYTES)
    {
        return Err(IpcError::VfsUnsupported);
    }
    Ok(offset)
}

fn write_vertexfs_fixed_vfs_name(
    sector: &mut [u8],
    offset: usize,
    max_len: usize,
    name: VfsName,
) -> Result<(), IpcError> {
    if name.len == 0
        || name.len > max_len
        || offset
            .checked_add(max_len)
            .is_none_or(|end| end > sector.len())
    {
        return Err(IpcError::VfsUnsupported);
    }
    let mut index = 0;
    while index < max_len {
        sector[offset + index] = 0;
        index += 1;
    }
    copy_bytes(&mut sector[offset..offset + name.len], name.as_bytes());
    Ok(())
}

fn copy_bytes(destination: &mut [u8], source: &[u8]) {
    let mut index = 0;
    while index < source.len() {
        destination[index] = source[index];
        index += 1;
    }
}

fn c_string_eq_bytes(value: *const u8, expected: &[u8]) -> bool {
    if value.is_null() {
        return false;
    }
    let mut index = 0;
    while index < expected.len() {
        if unsafe { value.add(index).read() } != expected[index] {
            return false;
        }
        index += 1;
    }
    unsafe { value.add(expected.len()).read() == 0 }
}

fn reject_vertexfs_image<T>(reason: &str) -> Result<T, InitError> {
    serial::write_str("Krust VertexFS v1 image rejected: ");
    serial::write_str(reason);
    serial::write_str("\n");
    Err(InitError::InvalidBootManifest)
}

fn validate_process_mount_roots(
    runtime: &RuntimeState,
    config: &BootRuntimeConfig,
) -> Result<(), InitError> {
    let mut index = 0;
    while index < config.process_count {
        let process = config.processes[index].ok_or(InitError::InvalidBootManifest)?;
        let node = runtime
            .vfs_node_by_path(process.mount_root.as_bytes())
            .ok_or(InitError::InvalidBootManifest)?;
        if !matches!(node.kind, VfsNodeKind::Directory) {
            return Err(InitError::InvalidBootManifest);
        }
        index += 1;
    }
    Ok(())
}

fn install_declared_process_mounts(
    runtime: &mut RuntimeState,
    process: BootProcessConfig,
    pid: ProcessId,
    mount_root: VfsPath,
) -> Result<u64, InitError> {
    let mut installed = 0;
    let mut index = 0;
    while index < process.mount_count {
        let mount = process.mounts[index].ok_or(InitError::InvalidBootManifest)?;
        let destination = resolve_vfs_path_under_root(mount_root, mount.path.as_bytes())
            .map_err(|_| InitError::InvalidBootManifest)?;
        let source = resolve_vfs_path_under_root(mount_root, mount.source.as_bytes())
            .map_err(|_| InitError::InvalidBootManifest)?;
        let (parent_path, _) = split_vfs_parent_child(destination.as_bytes())
            .map_err(|_| InitError::InvalidBootManifest)?;
        let parent = runtime
            .vfs_node_by_path(parent_path)
            .ok_or(InitError::InvalidBootManifest)?;
        if !matches!(parent.kind, VfsNodeKind::Directory)
            || runtime.vfs_node_by_path(destination.as_bytes()).is_some()
            || runtime
                .objects
                .get_vfs_mount_by_exact_path(destination.as_bytes())
                .is_some()
        {
            return Err(InitError::InvalidBootManifest);
        }
        let source_node = runtime
            .vfs_node_by_path(source.as_bytes())
            .ok_or(InitError::InvalidBootManifest)?;
        if !matches!(source_node.kind, VfsNodeKind::Directory) {
            return Err(InitError::InvalidBootManifest);
        }
        let source_mount_flags = runtime
            .objects
            .get_vfs_mount_by_path(source.as_bytes())
            .ok_or(InitError::InvalidBootManifest)?
            .flags;
        let flags = boot_process_mount_flags_to_vfs(mount.flags)?
            | (source_mount_flags & VFS_MOUNT_READ_ONLY);
        runtime.add_vfs_mount(
            "mount:declared-bind",
            source_node.id,
            destination,
            source_node.mount_source,
            flags,
            false,
            pid,
        )?;
        serial::write_str("Krust declared mount snapshot restored: proc=");
        serial::write_str(process.name);
        serial::write_str(" path=");
        serial::write_ascii_bytes(mount.path.as_bytes());
        serial::write_str(" canonical=");
        serial::write_ascii_bytes(destination.as_bytes());
        serial::write_str(" source=");
        serial::write_ascii_bytes(mount.source.as_bytes());
        serial::write_str(" canonical_source=");
        serial::write_ascii_bytes(source.as_bytes());
        serial::write_str(" flags=");
        serial_write_vfs_mount_flags(flags);
        serial::write_str("\n");
        installed += 1;
        index += 1;
    }
    Ok(installed)
}

fn boot_process_mount_flags_to_vfs(flags: u16) -> Result<u64, InitError> {
    if flags & !known_boot_process_mount_flags() != 0 || flags & BOOT_PROCESS_MOUNT_BIND == 0 {
        return Err(InitError::InvalidBootManifest);
    }
    let mut vfs_flags = VFS_MOUNT_BIND;
    if flags & BOOT_PROCESS_MOUNT_READ_ONLY != 0 {
        vfs_flags |= VFS_MOUNT_READ_ONLY;
    }
    Ok(vfs_flags)
}

fn grant_object_id(
    runtime: &RuntimeState,
    grant: BootGrantConfig,
) -> Result<KernelObjectId, InitError> {
    match grant.object_kind {
        BOOT_OBJECT_ENDPOINT => {
            runtime.endpoint_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_STORE => {
            runtime.store_object_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_STATE => Err(InitError::InvalidBootManifest),
        BOOT_OBJECT_TIMER if grant.object_index == 0 => {
            runtime.timer_id.ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_NETWORK_PORT => {
            runtime.network_port_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_IO_PORT_RANGE => {
            runtime.io_port_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_MMIO_REGION => {
            runtime.mmio_region_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_INTERRUPT_LINE => {
            runtime.interrupt_line_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_DMA_REGION => {
            runtime.dma_region_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_PCI_DEVICE => {
            runtime.pci_device_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_VIRTIO_DEVICE => {
            runtime.virtio_device_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_NAMESPACE => {
            runtime.namespace_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_VFS_ROOT => {
            runtime.vfs_root_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        _ => Err(InitError::InvalidBootManifest),
    }
}

fn namespace_entry_object_id(
    runtime: &RuntimeState,
    entry: BootNamespaceEntryConfig,
) -> Result<KernelObjectId, InitError> {
    if !namespace_entry_object_kind_allowed(entry.object_kind) {
        return Err(InitError::InvalidBootManifest);
    }
    grant_object_id(
        runtime,
        BootGrantConfig {
            process_index: 0,
            cap_slot: 0,
            object_kind: entry.object_kind,
            object_index: entry.object_index,
            rights: entry.rights,
        },
    )
}

fn namespace_entry_object_kind_allowed(object_kind: u16) -> bool {
    matches!(
        object_kind,
        BOOT_OBJECT_ENDPOINT | BOOT_OBJECT_STORE | BOOT_OBJECT_TIMER | BOOT_OBJECT_NETWORK_PORT
    )
}

fn grant_process_cap_by_pid(
    runtime: &mut RuntimeState,
    pid: ProcessId,
    slot: u64,
    cap: Capability,
    persist_for_restart: bool,
) -> Result<(), InitError> {
    let Some(process) = runtime.processes.process_mut(pid) else {
        return Err(InitError::InvalidBootManifest);
    };
    let mut caps = process.caps;
    let mut initial_caps = process.initial_caps;
    caps.grant(slot, cap)?;
    if persist_for_restart {
        initial_caps.grant(slot, cap)?;
    }
    process.caps = caps;
    if persist_for_restart {
        process.initial_caps = initial_caps;
    }
    Ok(())
}

fn initial_process_index(config: &BootRuntimeConfig) -> Result<usize, InitError> {
    let mut found = None;
    let mut index = 0;

    while index < config.process_count {
        let process = config.processes[index].ok_or(InitError::InvalidBootManifest)?;
        if process.initial {
            if found.is_some() {
                return Err(InitError::InvalidBootManifest);
            }
            found = Some(index);
        }
        index += 1;
    }

    found.ok_or(InitError::InvalidBootManifest)
}

pub fn initial_process_context() -> Option<ProcessContext> {
    runtime()
        .processes
        .current_process()
        .map(|process| process.context)
}

pub fn initial_process_name() -> &'static str {
    current_process_name()
}

pub fn current_process_name() -> &'static str {
    runtime()
        .processes
        .current_process()
        .map(|process| process.name)
        .unwrap_or("<none>")
}

fn current_process_id() -> ProcessId {
    runtime()
        .processes
        .current_process()
        .map(|process| process.pid)
        .unwrap_or_else(ProcessId::empty)
}

pub fn exit_current_process(status: u64, frame: &mut SyscallFrame) -> ScheduleResult {
    let initial_exited = {
        let runtime = runtime();
        runtime
            .processes
            .current_process()
            .map(|process| process.pid.raw() == 1)
            .unwrap_or(true)
    };

    let (lifecycle_event, exiting_pid, exiting_name) = {
        let runtime = runtime();

        if let Some(process) = runtime.processes.current_process_mut() {
            let pid = process.pid;
            let name = process.name;
            let event = if process.pid.raw() == 1 {
                None
            } else {
                let lifecycle_state = if status == 0 {
                    ServiceLifecycleState::Exited
                } else {
                    ServiceLifecycleState::Failed
                };
                Some((process.name, lifecycle_state))
            };
            process.state = ProcessState::Exited;
            process.has_saved_frame = false;
            process.exit_status = status;
            process.has_exited = true;
            process.clear_file_handles();
            runtime.release_process_file_descriptions(pid);
            (event, Some(pid), Some(name))
        } else {
            (None, None, None)
        }
    };
    if let Some(pid) = exiting_pid {
        let _ = cancel_blocked_receivers_for_endpoint_owner(pid, STATUS_BAD_CAPABILITY);
    }
    release_unreferenced_derived_vfs_roots(runtime());
    if let Some((service, lifecycle_state)) = lifecycle_event {
        runtime().record_service_lifecycle(service, lifecycle_state, Some(status));
    }
    if exiting_name == Some(VERTEX_STATE_PROCESS_NAME) {
        abort_vfs_state_transactions(STATUS_VFS_UNSUPPORTED);
    }
    if exiting_name == Some(BLOCK_DRIVER_PROCESS_NAME) {
        abort_vertexfs_sync_transactions(STATUS_VFS_UNSUPPORTED);
    }

    if initial_exited && status != 0 {
        return ScheduleResult::Halt { ok: false };
    }

    if schedule_next_ready(frame) {
        if let Some(pid) = exiting_pid {
            let _ = reap_process_context(pid);
        }
        ScheduleResult::Switched
    } else {
        let ok = runtime().processes.all_exited_successfully();
        if ok {
            let generation_id = runtime().generation_id;
            boot_manager().mark_known_good(generation_id);
        }
        ScheduleResult::Halt { ok }
    }
}

pub fn yield_current_process(frame: &mut SyscallFrame) -> ScheduleResult {
    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return ScheduleResult::Halt { ok: false };
        };

        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::Ready;
        process.name
    };

    if schedule_next_ready_excluding_current(frame) {
        ScheduleResult::Switched
    } else {
        let runtime = runtime();
        if let Some(process) = runtime.processes.current_process_mut() {
            process.state = ProcessState::Running;
        }

        serial::write_str("Scheduler yield: proc=");
        serial::write_str(current);
        serial::write_str(" no other ready process\n");
        ScheduleResult::Continue
    }
}

pub fn preempt_current_process(frame: &mut SyscallFrame) -> ScheduleResult {
    wake_timed_processes(read_tsc());
    let current = {
        let runtime = runtime();
        if runtime
            .processes
            .next_ready_index_round_robin(false)
            .is_none()
        {
            return ScheduleResult::Continue;
        }
        let Some(process) = runtime.processes.current_process_mut() else {
            return ScheduleResult::Halt { ok: false };
        };
        if process.state != ProcessState::Running {
            return ScheduleResult::Continue;
        }

        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::Ready;
        process.name
    };

    if schedule_next_ready_no_wait_excluding_current(frame) {
        serial::write_str("Scheduler preempted process without explicit yield: from=");
        serial::write_str(current);
        serial::write_str(" to=");
        serial::write_str(current_process_name());
        serial::write_str("\n");
        ScheduleResult::Switched
    } else {
        let runtime = runtime();
        if let Some(process) = runtime.processes.current_process_mut() {
            process.state = ProcessState::Running;
        }
        ScheduleResult::Continue
    }
}

pub fn wake_timed_from_interrupt() {
    wake_timed_processes(read_tsc());
}

pub fn fault_current_process(
    reason: &str,
    address: u64,
    error_code: u64,
    frame: &mut SyscallFrame,
) -> ScheduleResult {
    let (name, initial_faulted, faulted_pid) = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return ScheduleResult::Halt { ok: false };
        };
        let initial = process.pid.raw() == 1;
        let name = process.name;
        let pid = process.pid;
        process.state = ProcessState::Exited;
        process.has_saved_frame = false;
        process.exit_status = STATUS_PROCESS_FAULT;
        process.has_exited = true;
        process.clear_file_handles();
        runtime.release_process_file_descriptions(pid);
        (name, initial, pid)
    };
    let _ = cancel_blocked_receivers_for_endpoint_owner(faulted_pid, STATUS_BAD_CAPABILITY);
    release_unreferenced_derived_vfs_roots(runtime());
    if !initial_faulted {
        runtime().record_service_lifecycle(
            name,
            ServiceLifecycleState::Failed,
            Some(STATUS_PROCESS_FAULT),
        );
    }
    if name == VERTEX_STATE_PROCESS_NAME {
        abort_vfs_state_transactions(STATUS_VFS_UNSUPPORTED);
    }
    if name == BLOCK_DRIVER_PROCESS_NAME {
        abort_vertexfs_sync_transactions(STATUS_VFS_UNSUPPORTED);
    }

    serial::write_str("User process fault contained: proc=");
    serial::write_str(name);
    serial::write_str(" reason=");
    serial::write_str(reason);
    serial::write_str(" address=");
    serial::write_u64_hex(address);
    serial::write_str(" error=");
    serial::write_u64_hex(error_code);
    serial::write_str("\n");

    if initial_faulted {
        return ScheduleResult::Halt { ok: false };
    }

    if schedule_next_ready(frame) {
        let _ = reap_process_context(faulted_pid);
        ScheduleResult::Switched
    } else {
        ScheduleResult::Halt {
            ok: runtime().processes.all_exited_successfully(),
        }
    }
}

pub fn send(cap_slot: u64, source: *const u8, len: usize) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }

    let endpoint_id = match endpoint_from_cap(cap_slot, capability::RIGHT_SEND) {
        Ok(endpoint_id) => endpoint_id,
        Err(error) => {
            print_negative("send");
            return Err(error);
        }
    };

    let mut message = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut message, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    let sender = runtime()
        .processes
        .current_process()
        .map(|process| process.pid)
        .unwrap_or_else(ProcessId::empty);

    let endpoint = runtime()
        .objects
        .get_endpoint_mut(endpoint_id)
        .ok_or(IpcError::BadCapability)?;

    endpoint.enqueue(sender, &message, len)?;

    serial::write_str("IPC send accepted: endpoint=");
    serial::write_u64_dec(endpoint.id.raw());
    serial::write_str(" bytes=");
    serial::write_u64_dec(len as u64);
    serial::write_str("\n");

    wake_blocked_receiver(endpoint_id);
    wake_blocked_vfs_state_reply(endpoint_id);
    wake_blocked_vertexfs_sync_reply(endpoint_id);

    Ok(())
}

pub fn receive(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    receive_with_timeout(cap_slot, destination, max_len, None, frame)
}

pub fn receive_timeout(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
    timeout_ms: u64,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    receive_with_timeout(
        cap_slot,
        destination,
        max_len,
        Some(deadline_after_ms(timeout_ms)),
        frame,
    )
}

fn receive_with_timeout(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
    timeout_tsc: Option<u64>,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let endpoint_cap = match endpoint_cap_from_slot(cap_slot, capability::RIGHT_RECEIVE) {
        Ok(endpoint_cap) => endpoint_cap,
        Err(error) => {
            print_negative("receive");
            return Err(error);
        }
    };
    let endpoint_id = endpoint_cap.object;

    usercopy::validate_user_buffer(
        UserPtr::new(destination as u64),
        max_len,
        paging::UserAccess::Write,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;

    let current_pid = runtime()
        .processes
        .current_process()
        .map(|process| process.pid)
        .unwrap_or_else(ProcessId::empty);
    let queued_message = {
        let endpoint = runtime()
            .objects
            .get_endpoint_mut(endpoint_id)
            .ok_or(IpcError::BadCapability)?;
        endpoint.dequeue_for(current_pid)
    };

    let Some(message) = queued_message else {
        if block_current_on_endpoint(
            endpoint_id,
            endpoint_cap.id,
            destination as u64,
            max_len,
            timeout_tsc,
            frame,
        ) {
            return Ok(());
        }

        return Err(IpcError::Empty);
    };

    let copy_len = min(message.len, max_len);
    usercopy::copy_to_user(UserPtr::new(destination as u64), &message.bytes[..copy_len])
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    record_ready_lifecycle(endpoint_id, current_pid, message);

    serial::write_str("IPC receive delivered: endpoint=");
    serial::write_u64_dec(endpoint_id.raw());
    serial::write_str(" bytes=");
    serial::write_u64_dec(copy_len as u64);
    serial::write_str("\n");

    frame.rax = copy_len as u64;
    Ok(())
}

fn record_ready_lifecycle(endpoint: KernelObjectId, receiver: ProcessId, message: IpcMessage) {
    let Some(ready_service_name) = ready_service_name(&message) else {
        return;
    };

    let service = {
        let runtime = runtime();
        let Some(endpoint) = runtime.objects.get_endpoint(endpoint) else {
            return;
        };
        if endpoint.name != "readiness" || receiver.raw() != 1 {
            return;
        }

        let Some(process) = runtime.processes.process(message.sender) else {
            return;
        };
        process.name
    };

    if ready_service_name != service.as_bytes() {
        return;
    }

    runtime().record_service_lifecycle(service, ServiceLifecycleState::Ready, None);
}

fn ready_service_name(message: &IpcMessage) -> Option<&[u8]> {
    if message.len < READY_ENVELOPE_LEN {
        return None;
    }

    let protocol = u16::from_le_bytes([message.bytes[0], message.bytes[1]]);
    let message_type = u16::from_le_bytes([message.bytes[2], message.bytes[3]]);
    if protocol != PROTOCOL_HEALTH_V0 || message_type != MESSAGE_READY {
        return None;
    }

    let payload_len = u32::from_le_bytes([
        message.bytes[4],
        message.bytes[5],
        message.bytes[6],
        message.bytes[7],
    ]) as usize;
    if payload_len > message.len - READY_ENVELOPE_LEN {
        return None;
    }

    Some(&message.bytes[READY_ENVELOPE_LEN..READY_ENVELOPE_LEN + payload_len])
}

pub fn read_boot_module(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
) -> Result<usize, IpcError> {
    if max_len > MAX_BOOT_READ_BYTES {
        return Err(IpcError::MessageTooLarge);
    }

    let module = boot_module_from_cap(cap_slot, capability::RIGHT_READ)?;
    let Ok(module_len) = usize::try_from(module.length) else {
        return Err(IpcError::MessageTooLarge);
    };
    if module_len > max_len {
        return Err(IpcError::MessageTooLarge);
    }
    let copy_len = module_len;

    let bytes = unsafe { core::slice::from_raw_parts(module.base as *const u8, copy_len) };
    usercopy::copy_to_user(UserPtr::new(destination as u64), &bytes[..copy_len])
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Boot module read accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" module=");
    serial::write_str(module.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(copy_len as u64);
    serial::write_str("\n");

    Ok(copy_len)
}

pub fn log_write(cap_slot: u64, source: *const u8, len: usize) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }

    let _endpoint = serial_log_endpoint_from_cap(cap_slot, capability::RIGHT_SEND)?;
    let mut message = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut message, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_ascii_bytes(&message[..len]);
    serial::write_str("\n");
    wake_blocked_vfs_pipe_read(&message[..len]);
    Ok(())
}

pub fn activate_generation(
    cap_slot: u64,
    generation: *const u8,
    len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }

    let _process_control = process_control_from_cap(
        cap_slot,
        capability::RIGHT_CONTROL | capability::RIGHT_REVOKE,
    )?;
    let mut generation_id = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut generation_id, UserPtr::new(generation as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    let requested = &generation_id[..len];
    let target = match generation_runtimes().find(requested) {
        Some(target) => target,
        None => {
            serial::write_str("Krust generation switch rejected: requested=");
            serial::write_ascii_bytes(requested);
            serial::write_str("\n");
            serial::write_str("Native update transaction rejected: missing store object\n");
            serial::write_str("Native update transaction selected_generation unchanged: ");
            serial::write_str(runtime().generation_id);
            serial::write_str("\n");
            serial::write_str(
                "update commit interrupted before final pointer leaves previous generation bootable\n",
            );
            return Err(IpcError::BadCapability);
        }
    };
    if failed_generation_is(target.generation_id) {
        serial::write_str("Krust generation switch rejected: requested=");
        serial::write_ascii_bytes(requested);
        serial::write_str(" failed=yes\n");
        return Err(IpcError::BadCapability);
    }

    let (previous_generation, previous_config, old_cap_count) = {
        let runtime = runtime();
        (
            runtime.generation_id,
            runtime.active_config,
            runtime.generation_cap_count(runtime.generation_id),
        )
    };

    if previous_generation == target.generation_id {
        serial::write_str("Krust generation switch already active: ");
        serial::write_str(target.generation_id);
        serial::write_str("\n");
        return Ok(());
    }

    boot_manager().install_prepare(previous_generation, target.generation_id);
    if verify_generation_transaction(target).is_err() {
        boot_manager().install_abort(target.generation_id, "verification-failed");
        serial::write_str("Native update transaction selected_generation unchanged: ");
        serial::write_str(previous_generation);
        serial::write_str("\n");
        return Err(IpcError::BadCapability);
    }

    if let Some(previous_config) = previous_config {
        set_rollback_runtime(GenerationRuntime {
            generation_id: previous_generation,
            config: previous_config,
        });
    }

    serial::write_str("Krust generation switch accepted: from=");
    serial::write_str(previous_generation);
    serial::write_str(" to=");
    serial::write_str(target.generation_id);
    serial::write_str("\n");
    serial::write_str("Krust generation switch revoked old generation authority: generation=");
    serial::write_str(previous_generation);
    serial::write_str(" caps=");
    serial::write_u64_dec(old_cap_count);
    serial::write_str("\n");
    serial::write_str("old generation service loses old capability\n");

    if init_from_boot_config(target.config).is_err() {
        boot_manager().install_abort(target.generation_id, "runtime-build-failed");
        return Err(IpcError::BadCapability);
    }
    serial::write_str("Native update transaction journal commit\n");
    boot_manager().install_selected(previous_generation, target.generation_id);
    let context = initial_process_context().ok_or(IpcError::BadCapability)?;
    serial::write_str("Krust generation switch entering generation: ");
    serial::write_str(target.generation_id);
    serial::write_str("\n");
    serial::write_str("update commit interrupted after final pointer boots verified generation\n");
    let _ = frame;
    unsafe {
        gdt::enter_user_mode(context.cr3, context.entry, context.stack_top);
    }
}

pub fn verify_generation(cap_slot: u64, generation: *const u8, len: usize) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }

    let _process_control = process_control_from_cap(
        cap_slot,
        capability::RIGHT_CONTROL | capability::RIGHT_REVOKE,
    )?;
    let mut generation_id = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut generation_id, UserPtr::new(generation as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    let requested = &generation_id[..len];
    let target = match generation_runtimes().find(requested) {
        Some(target) => target,
        None => {
            serial::write_str("Native generation verification rejected: requested=");
            serial::write_ascii_bytes(requested);
            serial::write_str(" reason=missing-store-object\n");
            return Err(IpcError::BadCapability);
        }
    };
    if failed_generation_is(target.generation_id) {
        serial::write_str("Native generation verification rejected: requested=");
        serial::write_ascii_bytes(requested);
        serial::write_str(" failed=yes\n");
        return Err(IpcError::BadCapability);
    }

    verify_generation_transaction(target)?;
    serial::write_str("Native generation verification accepted: generation=");
    serial::write_str(target.generation_id);
    serial::write_str("\n");
    Ok(())
}

fn verify_generation_transaction(target: GenerationRuntime) -> Result<(), IpcError> {
    validate_boot_config_installable(target.config).map_err(|_| IpcError::BadCapability)?;
    verify_generation_manifest(target.config)?;
    verify_generation_store_closure(target.config)?;
    Ok(())
}

fn verify_generation_manifest(config: &BootRuntimeConfig) -> Result<(), IpcError> {
    let Some(module) = config.manifest_module else {
        serial::write_str("Native update transaction rejected: missing manifest\n");
        return Err(IpcError::BadCapability);
    };
    let Ok(len) = usize::try_from(module.length) else {
        serial::write_str("Native update transaction rejected: manifest too large\n");
        return Err(IpcError::MessageTooLarge);
    };
    if len == 0 {
        serial::write_str("Native update transaction rejected: empty manifest\n");
        return Err(IpcError::BadCapability);
    }

    let bytes = unsafe { core::slice::from_raw_parts(module.base as *const u8, len) };
    let mut actual = [0u8; 64];
    store_hash_hex(blake3::hash(bytes).as_bytes(), &mut actual);
    if actual != config.manifest_hash {
        serial::write_str("Native update transaction rejected: manifest hash mismatch\n");
        return Err(IpcError::BadCapability);
    }

    serial::write_str("Native update transaction verifies manifest hash: generation=");
    serial::write_str(config.generation_id);
    serial::write_str(" identity=store:blake3:");
    serial::write_ascii_bytes(&config.manifest_hash);
    serial::write_str("\n");
    Ok(())
}

fn verify_generation_store_closure(config: &BootRuntimeConfig) -> Result<(), IpcError> {
    if config.store_object_count == 0 {
        serial::write_str("Native update transaction rejected: missing store closure\n");
        return Err(IpcError::BadCapability);
    }

    let mut index = 0;
    while index < config.store_object_count {
        let Some(object) = config.store_objects[index] else {
            serial::write_str("Native update transaction rejected: store closure gap\n");
            return Err(IpcError::BadCapability);
        };
        let Ok(len) = usize::try_from(object.length) else {
            serial::write_str("Native update transaction rejected: store object too large object=");
            serial::write_str(object.id);
            serial::write_str("\n");
            return Err(IpcError::MessageTooLarge);
        };
        if len == 0 {
            serial::write_str("Native update transaction rejected: missing store object object=");
            serial::write_str(object.id);
            serial::write_str("\n");
            return Err(IpcError::BadCapability);
        }
        let bytes = unsafe { core::slice::from_raw_parts(object.base as *const u8, len) };
        if !store_hash_matches(bytes, object.hash) {
            serial::write_str("Native update transaction rejected: store hash mismatch object=");
            serial::write_str(object.id);
            serial::write_str("\n");
            serial::write_str("vertex-inspect security event: store hash mismatch object=");
            serial::write_str(object.id);
            serial::write_str("\n");
            return Err(IpcError::BadCapability);
        }
        index += 1;
    }

    serial::write_str("Native update transaction verifies store closure: generation=");
    serial::write_str(config.generation_id);
    serial::write_str(" objects=");
    serial::write_u64_dec(config.store_object_count as u64);
    serial::write_str("\n");
    Ok(())
}

pub fn rollback_generation(
    cap_slot: u64,
    generation: *const u8,
    len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }
    let _process_control = process_control_from_cap(
        cap_slot,
        capability::RIGHT_CONTROL | capability::RIGHT_REVOKE,
    )?;

    let mut requested = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut requested, UserPtr::new(generation as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    let rollback = match unsafe { *ROLLBACK_RUNTIME.0.get() } {
        Some(rollback) => rollback,
        None => {
            serial::write_str("Krust rollback rejected: no rollback runtime\n");
            return Err(IpcError::BadCapability);
        }
    };
    if rollback.generation_id.as_bytes() != &requested[..len] {
        serial::write_str("Krust rollback rejected: requested=");
        serial::write_ascii_bytes(&requested[..len]);
        serial::write_str(" available=");
        serial::write_str(rollback.generation_id);
        serial::write_str("\n");
        return Err(IpcError::BadCapability);
    }

    let (previous_generation, previous_config, old_cap_count) = {
        let runtime = runtime();
        (
            runtime.generation_id,
            runtime.active_config,
            runtime.generation_cap_count(runtime.generation_id),
        )
    };
    serial::write_str("Krust rollback generation accepted: target=");
    serial::write_str(rollback.generation_id);
    serial::write_str("\n");
    serial::write_str("Krust rollback revoked failed generation authority: generation=");
    serial::write_str(previous_generation);
    serial::write_str(" caps=");
    serial::write_u64_dec(old_cap_count);
    serial::write_str("\n");

    boot_manager().install_prepare(previous_generation, rollback.generation_id);
    if init_from_boot_config(rollback.config).is_err() {
        boot_manager().install_abort(rollback.generation_id, "rollback-build-failed");
        return Err(IpcError::BadCapability);
    }
    if let Some(previous_config) = previous_config {
        set_rollback_runtime(GenerationRuntime {
            generation_id: previous_generation,
            config: previous_config,
        });
        set_failed_generation(previous_generation);
    }
    boot_manager().mark_failed_and_fallback(previous_generation, rollback.generation_id);
    let context = initial_process_context().ok_or(IpcError::BadCapability)?;
    serial::write_str("Krust rollback entering generation: ");
    serial::write_str(rollback.generation_id);
    serial::write_str("\n");
    let _ = frame;
    unsafe {
        gdt::enter_user_mode(context.cr3, context.entry, context.stack_top);
    }
}

fn recycle_exited_process_template(config_process_index: usize) -> Result<(), IpcError> {
    let existing = {
        let runtime = runtime();
        let config = runtime.active_config.ok_or(IpcError::BadCapability)?;
        if config_process_index >= config.process_count {
            return Err(IpcError::BadCapability);
        }
        runtime.process_template_pids[config_process_index]
    };

    let Some(pid) = existing else {
        return Ok(());
    };

    let state = {
        let runtime = runtime();
        runtime
            .processes
            .process(pid)
            .map(|process| process.state)
            .ok_or(IpcError::BadCapability)?
    };
    if state != ProcessState::Exited {
        return Err(IpcError::BadCapability);
    }

    reap_process_context(pid)?;
    let runtime = runtime();
    runtime
        .processes
        .remove_process(pid)
        .map_err(|_| IpcError::BadCapability)?;
    runtime.process_template_pids[config_process_index] = None;

    serial::write_str("Krust process table slot recycled: pid=");
    serial::write_u64_dec(pid.raw());
    serial::write_str(" template=");
    serial::write_u64_dec(config_process_index as u64);
    serial::write_str("\n");
    Ok(())
}

fn process_config_for_pid(runtime: &RuntimeState, pid: ProcessId) -> Option<BootProcessConfig> {
    let config = runtime.active_config?;
    let mut index = 0;
    while index < config.process_count {
        if runtime.process_template_pids[index] == Some(pid) {
            return config.processes[index];
        }
        index += 1;
    }
    None
}

fn load_process_context(
    name: &'static str,
    image_base: u64,
    image_length: u64,
) -> Result<ProcessContext, IpcError> {
    if image_base == 0 || image_length == 0 {
        return Err(IpcError::BadCapability);
    }
    let len = usize::try_from(image_length).map_err(|_| IpcError::BadCapability)?;
    let bytes = unsafe { core::slice::from_raw_parts(image_base as *const u8, len) };
    let hhdm_offset = limine::hhdm_offset().ok_or(IpcError::BadCapability)?;
    match userspace::load(bytes, hhdm_offset, frame_allocator()?) {
        Ok(image) => {
            serial::write_str("Krust process image loaded from native store: process=");
            serial::write_str(name);
            serial::write_str(" entry=");
            serial::write_u64_hex(image.entry);
            serial::write_str(" stack=");
            serial::write_u64_hex(image.stack_top);
            serial::write_str(" cr3=");
            serial::write_u64_hex(image.cr3);
            serial::write_str("\n");
            Ok(ProcessContext {
                cr3: image.cr3,
                entry: image.entry,
                stack_top: image.stack_top,
            })
        }
        Err(error) => {
            userspace::print_load_error(error);
            Err(IpcError::BadCapability)
        }
    }
}

fn reclaim_detached_address_space(name: &'static str, cr3: u64) {
    if cr3 == 0 {
        return;
    }
    let Some(hhdm_offset) = limine::hhdm_offset() else {
        return;
    };
    let Ok(allocator) = frame_allocator() else {
        return;
    };
    if let Ok(stats) = paging::reclaim_user_address_space(hhdm_offset, cr3, allocator) {
        serial::write_str("Krust detached address space reaped: proc=");
        serial::write_str(name);
        serial::write_str(" user_frames=");
        serial::write_u64_dec(stats.user_leaf_frames);
        serial::write_str(" page_tables=");
        serial::write_u64_dec(stats.page_table_frames);
        serial::write_str(" device_mappings=");
        serial::write_u64_dec(stats.device_mappings);
        serial::write_str("\n");
    }
}

pub fn create_process(cap_slot: u64, config_process_index: u64) -> Result<u64, IpcError> {
    let _process_control = process_control_from_cap(cap_slot, capability::RIGHT_CREATE)?;
    let caller = current_process_name();
    let Ok(config_process_index) = usize::try_from(config_process_index) else {
        return Err(IpcError::BadCapability);
    };

    recycle_exited_process_template(config_process_index)?;

    let process = {
        let runtime = runtime();
        let config = runtime.active_config.ok_or(IpcError::BadCapability)?;
        if config_process_index >= config.process_count {
            return Err(IpcError::BadCapability);
        }
        if runtime.process_template_pids[config_process_index].is_some() {
            return Err(IpcError::BadCapability);
        }
        let process = config.processes[config_process_index].ok_or(IpcError::BadCapability)?;
        if process.initial {
            return Err(IpcError::BadCapability);
        };
        validate_config_caps_for_process(runtime, config, config_process_index)
            .map_err(|_| IpcError::BadCapability)?;
        process
    };
    let context = load_process_context(process.name, process.image_base, process.image_length)?;

    let (pid, name) = {
        let runtime = runtime();
        let config = runtime.active_config.ok_or(IpcError::BadCapability)?;
        if runtime.process_template_pids[config_process_index].is_some() {
            reclaim_detached_address_space(process.name, context.cr3);
            return Err(IpcError::BadCapability);
        }
        let mount_root = VfsPath::from_boot_root_path(process.mount_root)
            .map_err(|_| IpcError::BadCapability)?;
        let pid = runtime
            .processes
            .add_process(
                process.name,
                context,
                process.image_base,
                process.image_length,
                ProcessState::Declared,
                CapabilitySpace::new(),
                mount_root,
            )
            .map_err(|_| {
                reclaim_detached_address_space(process.name, context.cr3);
                IpcError::BadCapability
            })?;
        if install_declared_process_mounts(runtime, process, pid, mount_root).is_err() {
            let _ = runtime.remove_owned_declared_bind_mounts(pid);
            let _ = runtime.processes.remove_last_process(pid);
            reclaim_detached_address_space(process.name, context.cr3);
            return Err(IpcError::BadCapability);
        }
        if grant_config_caps_to_process(runtime, config, config_process_index, pid).is_err() {
            let _ = runtime.remove_owned_declared_bind_mounts(pid);
            let _ = runtime.processes.remove_last_process(pid);
            reclaim_detached_address_space(process.name, context.cr3);
            return Err(IpcError::BadCapability);
        }
        runtime.process_template_pids[config_process_index] = Some(pid);
        runtime.record_service_lifecycle(process.name, ServiceLifecycleState::Declared, None);
        print_process_by_pid(runtime, pid);
        serial::write_str("initial capability grants supplied explicitly: process=");
        serial::write_str(process.name);
        serial::write_str(" pid=");
        serial::write_u64_dec(pid.raw());
        serial::write_str("\n");

        (pid, process.name)
    };

    serial::write_str("Krust process create accepted: proc=");
    serial::write_str(caller);
    serial::write_str(" target=");
    serial::write_str(name);
    serial::write_str(" pid=");
    serial::write_u64_dec(pid.raw());
    serial::write_str(" template=");
    serial::write_u64_dec(config_process_index as u64);
    serial::write_str("\n");
    serial::write_str("immutable launch object accepted: process=");
    serial::write_str(name);
    serial::write_str(" args-env-hash=blake3:metadata-v0\n");
    Ok(pid.raw())
}

pub fn start_process(cap_slot: u64, pid: u64) -> Result<(), IpcError> {
    let _process_control = process_control_from_cap(cap_slot, capability::RIGHT_START)?;
    let caller = current_process_name();
    let pid = ProcessId::new(pid);
    let process_snapshot = {
        let runtime = runtime();
        runtime
            .processes
            .process(pid)
            .copied()
            .ok_or(IpcError::BadCapability)?
    };
    if process_snapshot.state != ProcessState::Declared
        && process_snapshot.state != ProcessState::Exited
    {
        return Err(IpcError::BadCapability);
    }
    let reload_context = if process_snapshot.state == ProcessState::Exited {
        reap_process_context(pid)?;
        Some(load_process_context(
            process_snapshot.name,
            process_snapshot.image_base,
            process_snapshot.image_length,
        )?)
    } else {
        None
    };
    if let Some(context) = reload_context {
        let process_config = {
            let runtime = runtime();
            process_config_for_pid(runtime, pid).ok_or(IpcError::BadCapability)?
        };
        if install_declared_process_mounts(
            runtime(),
            process_config,
            pid,
            process_snapshot.mount_root,
        )
        .is_err()
        {
            reclaim_detached_address_space(process_snapshot.name, context.cr3);
            return Err(IpcError::BadCapability);
        }
    }

    let (target, lifecycle_state, release_files) = {
        let runtime = runtime();
        let Some(process) = runtime.processes.process_mut(pid) else {
            return Err(IpcError::BadCapability);
        };

        let lifecycle_state = if let Some(context) = reload_context {
            process.context = context;
            process.context_reaped = false;
            process.caps = process.initial_caps;
            process.quota = process.initial_quota;
            process.clear_dma_mappings();
            process.clear_file_handles();
            serial::write_str("Krust process restart reload: proc=");
            serial::write_str(process.name);
            serial::write_str("\n");
            serial::write_str("Krust process restart restores quota baseline: proc=");
            serial::write_str(process.name);
            serial::write_str("\n");
            ServiceLifecycleState::Restarting
        } else {
            ServiceLifecycleState::Starting
        };

        process.state = ProcessState::Ready;
        process.has_saved_frame = false;
        process.exit_status = 0;
        process.has_exited = false;
        process.start_count = process.start_count.saturating_add(1);
        (process.name, lifecycle_state, reload_context.is_some())
    };
    if release_files {
        runtime().release_process_file_descriptions(pid);
    }
    release_unreferenced_derived_vfs_roots(runtime());
    runtime().record_service_lifecycle(target, lifecycle_state, None);

    serial::write_str("Krust process start accepted: proc=");
    serial::write_str(caller);
    serial::write_str(" target=");
    serial::write_str(target);
    serial::write_str(" pid=");
    serial::write_u64_dec(pid.raw());
    serial::write_str("\n");
    Ok(())
}

pub fn process_attempt() -> Result<u64, IpcError> {
    let runtime = runtime();
    runtime
        .processes
        .current_process()
        .map(|process| process.start_count)
        .ok_or(IpcError::BadCapability)
}

pub fn process_wait(cap_slot: u64, pid: u64) -> Result<u64, IpcError> {
    wake_timed_processes(read_tsc());
    let _process_control = process_control_from_cap(cap_slot, capability::RIGHT_WAIT)?;
    let pid = ProcessId::new(pid);

    let process = {
        let runtime = runtime();
        let Some(process) = runtime.processes.process(pid).copied() else {
            return Err(IpcError::BadCapability);
        };
        process
    };

    if process.state == ProcessState::Exited {
        serial::write_str("Krust process wait observed exit: proc=");
        serial::write_str(process.name);
        serial::write_str(" pid=");
        serial::write_u64_dec(pid.raw());
        serial::write_str(" status=");
        serial::write_u64_dec(process.exit_status);
        serial::write_str("\n");
        reap_process_context(pid)?;
        Ok(process.exit_status)
    } else {
        Ok(u64::MAX - 8)
    }
}

pub fn kill_process(cap_slot: u64, pid: u64) -> Result<(), IpcError> {
    let _process_control = process_control_from_cap(cap_slot, capability::RIGHT_KILL)?;
    let pid = ProcessId::new(pid);
    let caller = current_process_name();
    if runtime()
        .processes
        .current_process()
        .map(|process| process.pid == pid)
        .unwrap_or(false)
    {
        return Err(IpcError::BadCapability);
    }
    let target = {
        let runtime = runtime();
        let Some(process) = runtime.processes.process_mut(pid) else {
            return Err(IpcError::BadCapability);
        };
        if process.pid.raw() == 1 {
            return Err(IpcError::BadCapability);
        }
        process.state = ProcessState::Exited;
        process.has_saved_frame = false;
        process.exit_status = u64::MAX - 11;
        process.has_exited = true;
        process.clear_file_handles();
        process.name
    };
    runtime().release_process_file_descriptions(pid);
    let _ = cancel_blocked_receivers_for_endpoint_owner(pid, STATUS_BAD_CAPABILITY);
    release_unreferenced_derived_vfs_roots(runtime());
    if target == VERTEX_STATE_PROCESS_NAME {
        abort_vfs_state_transactions(STATUS_VFS_UNSUPPORTED);
    }
    if target == BLOCK_DRIVER_PROCESS_NAME {
        abort_vertexfs_sync_transactions(STATUS_VFS_UNSUPPORTED);
    }

    serial::write_str("Krust process kill accepted: proc=");
    serial::write_str(caller);
    serial::write_str(" target=");
    serial::write_str(target);
    serial::write_str(" pid=");
    serial::write_u64_dec(pid.raw());
    serial::write_str("\n");
    reap_process_context(pid)?;
    Ok(())
}

pub fn cap_derive(parent_slot: u64, new_slot: u64, rights_mask: u64) -> Result<(), IpcError> {
    let parent = lookup_capability(parent_slot, 0)?;
    if rights_mask == 0 || rights_mask & !parent.rights != 0 {
        return Err(IpcError::BadCapability);
    }

    let process_name = current_process_name();
    let runtime = runtime();
    let owner = runtime
        .processes
        .current_process()
        .map(|process| process.pid)
        .ok_or(IpcError::BadCapability)?;
    {
        let Some(process) = runtime.processes.current_process() else {
            return Err(IpcError::BadCapability);
        };
        if !process.caps.can_grant(new_slot) {
            return Err(IpcError::BadCapability);
        }
    }
    let cap = runtime.new_capability(parent.object, rights_mask, owner, parent.id, owner)?;
    let Some(process) = runtime.processes.current_process_mut() else {
        return Err(IpcError::BadCapability);
    };
    process
        .caps
        .grant(new_slot, cap)
        .map_err(|_| IpcError::BadCapability)?;

    serial::write_str("Capability derive accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" parent=");
    serial::write_u64_dec(parent_slot);
    serial::write_str(" new=");
    serial::write_u64_dec(new_slot);
    serial::write_str(" rights=");
    print_rights(rights_mask);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(cap.id);
    serial::write_str(" parent_cap_id=");
    serial::write_u64_dec(parent.id);
    serial::write_str("\n");
    Ok(())
}

pub fn cap_drop(slot: u64) -> Result<(), IpcError> {
    let process_name = current_process_name();
    let runtime = runtime();
    let dropped = {
        let Some(process) = runtime.processes.current_process_mut() else {
            return Err(IpcError::BadCapability);
        };
        process.caps.clear(slot)?
    };
    release_unreferenced_derived_vfs_root(runtime, dropped.object);

    serial::write_str("Capability drop accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" slot=");
    serial::write_u64_dec(slot);
    serial::write_str("\n");
    Ok(())
}

pub fn cap_revoke(slot: u64) -> Result<(), IpcError> {
    let cap = lookup_capability(slot, 0)?;
    let process_name = current_process_name();
    {
        let runtime = runtime();
        runtime.revoke_cap_id(cap.id)?;
        release_unreferenced_derived_vfs_roots(runtime);
    }
    let canceled = cancel_unauthorized_blocked_receivers(STATUS_BAD_CAPABILITY);
    if canceled > 0 {
        serial::write_str("Capability revoke canceled blocked receives: count=");
        serial::write_u64_dec(canceled as u64);
        serial::write_str("\n");
    }

    serial::write_str("Capability revoke accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" slot=");
    serial::write_u64_dec(slot);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(cap.id);
    serial::write_str("\n");
    Ok(())
}

fn release_unreferenced_derived_vfs_root(runtime: &mut RuntimeState, object: KernelObjectId) {
    if object_reachable_by_cap(runtime, object) {
        return;
    }
    if runtime.objects.remove_derived_vfs_root(object) {
        log_derived_vfs_root_released(object);
    }
}

fn release_unreferenced_derived_vfs_roots(runtime: &mut RuntimeState) {
    let mut index = 0;
    while index < runtime.objects.count {
        if let Some(KernelObject::VfsRoot(root)) = runtime.objects.objects[index]
            && root.derived
            && !object_reachable_by_cap(runtime, root.id)
        {
            let object = root.id;
            if runtime.objects.remove_derived_vfs_root(object) {
                log_derived_vfs_root_released(object);
                continue;
            }
        }
        index += 1;
    }
}

fn log_derived_vfs_root_released(object: KernelObjectId) {
    serial::write_str("Derived VFS root released: object=");
    serial::write_u64_dec(object.raw());
    serial::write_str("\n");
}

pub fn cap_inspect(slot: u64) -> Result<u64, IpcError> {
    let cap = lookup_capability(slot, 0)?;
    serial::write_str("Capability inspect: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" slot=");
    serial::write_u64_dec(slot);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(cap.id);
    serial::write_str(" object_id=");
    serial::write_u64_dec(cap.object.raw());
    serial::write_str(" parent_cap_id=");
    serial::write_u64_dec(cap.parent_cap_id);
    serial::write_str(" owner_process=");
    serial::write_u64_dec(cap.owner_process.raw());
    serial::write_str(" delegated_by=");
    serial::write_u64_dec(cap.delegated_by.raw());
    serial::write_str(" generation=");
    serial::write_str(cap.generation_id);
    serial::write_str(" revoked=");
    serial::write_str(if cap.revoked { "yes" } else { "no" });
    serial::write_str("\n");
    Ok(cap.parent_cap_id)
}

pub fn cap_copy(source_slot: u64, target_slot: u64, rights_mask: u64) -> Result<(), IpcError> {
    let source = lookup_capability(source_slot, 0)?;
    if rights_mask == 0 || rights_mask & !source.rights != 0 {
        return Err(IpcError::BadCapability);
    }

    let runtime = runtime();
    let owner = runtime
        .processes
        .current_process()
        .map(|process| process.pid)
        .ok_or(IpcError::BadCapability)?;
    {
        let Some(process) = runtime.processes.current_process() else {
            return Err(IpcError::BadCapability);
        };
        if !process.caps.can_grant(target_slot) {
            return Err(IpcError::BadCapability);
        }
    }
    let copied = runtime.new_capability(source.object, rights_mask, owner, source.id, owner)?;
    let Some(process) = runtime.processes.current_process_mut() else {
        return Err(IpcError::BadCapability);
    };
    process
        .caps
        .grant(target_slot, copied)
        .map_err(|_| IpcError::BadCapability)?;

    serial::write_str("Capability copy accepted: proc=");
    serial::write_str(process.name);
    serial::write_str(" source=");
    serial::write_u64_dec(source_slot);
    serial::write_str(" target=");
    serial::write_u64_dec(target_slot);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(copied.id);
    serial::write_str(" parent_cap_id=");
    serial::write_u64_dec(source.id);
    serial::write_str("\n");
    Ok(())
}

pub fn cap_move(source_slot: u64, target_slot: u64) -> Result<(), IpcError> {
    let process_name = current_process_name();
    let runtime = runtime();
    let Some(process) = runtime.processes.current_process_mut() else {
        return Err(IpcError::BadCapability);
    };
    if !process.caps.can_grant(target_slot) {
        return Err(IpcError::BadCapability);
    }
    let cap = process.caps.clear(source_slot)?;
    process
        .caps
        .grant(target_slot, cap)
        .map_err(|_| IpcError::BadCapability)?;

    serial::write_str("Capability move accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" source=");
    serial::write_u64_dec(source_slot);
    serial::write_str(" target=");
    serial::write_u64_dec(target_slot);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(cap.id);
    serial::write_str("\n");
    Ok(())
}

pub fn cap_transfer(
    control_slot: u64,
    target_pid: u64,
    packed_transfer: u64,
) -> Result<(), IpcError> {
    let _process_control = process_control_from_cap(control_slot, capability::RIGHT_DELEGATE)?;
    let cap_slot = packed_transfer & 0xffff;
    let target_slot = (packed_transfer >> 16) & 0xffff;
    let rights_mask = packed_transfer >> 32;
    let cap = lookup_capability(cap_slot, 0)?;
    if rights_mask == 0 || rights_mask & !cap.rights != 0 {
        return Err(IpcError::BadCapability);
    }
    let target_pid = ProcessId::new(target_pid);

    let (caller, target, transferred_id, parent_cap_id, target_pid_raw) = {
        let runtime = runtime();
        let caller = runtime
            .processes
            .current_process()
            .map(|process| process.name)
            .unwrap_or("<none>");
        let delegated_by = runtime
            .processes
            .current_process()
            .map(|process| process.pid)
            .ok_or(IpcError::BadCapability)?;
        let (target_name, persist_for_restart) = {
            let Some(target_process) = runtime.processes.process(target_pid) else {
                return Err(IpcError::BadCapability);
            };
            if !target_process.caps.can_grant(target_slot) {
                return Err(IpcError::BadCapability);
            }
            if target_process.state == ProcessState::Declared
                && !target_process.initial_caps.can_grant(target_slot)
            {
                return Err(IpcError::BadCapability);
            }
            (
                target_process.name,
                target_process.state == ProcessState::Declared,
            )
        };
        let transferred =
            runtime.new_capability(cap.object, rights_mask, target_pid, cap.id, delegated_by)?;
        let transferred_id = transferred.id;
        let Some(target_process) = runtime.processes.process_mut(target_pid) else {
            return Err(IpcError::BadCapability);
        };
        let mut next_caps = target_process.caps;
        let mut next_initial_caps = target_process.initial_caps;
        next_caps
            .grant(target_slot, transferred)
            .map_err(|_| IpcError::BadCapability)?;
        if persist_for_restart {
            next_initial_caps
                .grant(target_slot, transferred)
                .map_err(|_| IpcError::BadCapability)?;
        }
        target_process.caps = next_caps;
        if persist_for_restart {
            target_process.initial_caps = next_initial_caps;
        }
        (
            caller,
            target_name,
            transferred_id,
            cap.id,
            target_pid.raw(),
        )
    };

    serial::write_str("Capability transfer accepted: proc=");
    serial::write_str(caller);
    serial::write_str(" target=");
    serial::write_str(target);
    serial::write_str(" target_pid=");
    serial::write_u64_dec(target_pid_raw);
    serial::write_str(" slot=");
    serial::write_u64_dec(target_slot);
    serial::write_str(" rights=");
    print_rights(rights_mask);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(transferred_id);
    serial::write_str(" parent_cap_id=");
    serial::write_u64_dec(parent_cap_id);
    serial::write_str("\n");
    Ok(())
}

pub fn endpoint_create(control_slot: u64, cap_slot: u64) -> Result<(), IpcError> {
    let _process_control = process_control_from_cap(control_slot, capability::RIGHT_ALLOCATE)?;
    let process_name = current_process_name();
    let runtime = runtime();
    let (owner, quota, cap_slot_available) = {
        let Some(process) = runtime.processes.current_process() else {
            return Err(IpcError::BadCapability);
        };
        (process.pid, process.quota, process.caps.can_grant(cap_slot))
    };
    if quota.used_endpoints >= quota.max_endpoints {
        serial::write_str("Endpoint create rejected: proc=");
        serial::write_str(process_name);
        serial::write_str(" quota=max_endpoints\n");
        return Err(IpcError::BadCapability);
    }
    if !cap_slot_available {
        serial::write_str("Endpoint create rejected: proc=");
        serial::write_str(process_name);
        serial::write_str(" target cap slot unavailable\n");
        return Err(IpcError::BadCapability);
    }
    if !runtime.can_allocate_capability() {
        serial::write_str("Endpoint create rejected: cap lineage full\n");
        return Err(IpcError::BadCapability);
    }

    let endpoint_id = runtime
        .objects
        .add_endpoint_owned("dynamic-endpoint", owner)
        .map_err(|_| {
            serial::write_str("Endpoint create rejected: object arena full\n");
            IpcError::BadCapability
        })?;
    let cap = match runtime.new_capability(
        endpoint_id,
        capability::RIGHT_SEND | capability::RIGHT_RECEIVE,
        owner,
        0,
        owner,
    ) {
        Ok(cap) => cap,
        Err(error) => {
            let _ = runtime.objects.remove_owned_endpoint(endpoint_id, owner);
            return Err(error);
        }
    };
    let quota_after = {
        let Some(process) = runtime.processes.current_process_mut() else {
            runtime.rollback_last_capability(cap);
            let _ = runtime.objects.remove_owned_endpoint(endpoint_id, owner);
            return Err(IpcError::BadCapability);
        };
        if process.caps.grant(cap_slot, cap).is_err() {
            None
        } else {
            process.quota.used_endpoints = process.quota.used_endpoints.saturating_add(1);
            Some((process.quota.used_endpoints, process.quota.max_endpoints))
        }
    };
    let Some((used_endpoints, max_endpoints)) = quota_after else {
        runtime.rollback_last_capability(cap);
        let _ = runtime.objects.remove_owned_endpoint(endpoint_id, owner);
        return Err(IpcError::BadCapability);
    };

    serial::write_str("Endpoint create accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" slot=");
    serial::write_u64_dec(cap_slot);
    serial::write_str(" endpoint_id=");
    serial::write_u64_dec(endpoint_id.raw());
    serial::write_str(" quota=");
    serial::write_u64_dec(used_endpoints);
    serial::write_str("/");
    serial::write_u64_dec(max_endpoints);
    serial::write_str("\n");
    Ok(())
}

pub fn quota_delegate(
    control_slot: u64,
    target_pid: u64,
    max_endpoints: u64,
) -> Result<(), IpcError> {
    let _process_control = process_control_from_cap(control_slot, capability::RIGHT_DELEGATE)?;
    let target_pid = ProcessId::new(target_pid);
    let runtime = runtime();
    let (caller_name, caller_quota) = runtime
        .processes
        .current_process()
        .map(|process| (process.name, process.quota))
        .ok_or(IpcError::BadCapability)?;
    if max_endpoints > caller_quota.max_endpoints {
        serial::write_str("Quota delegate rejected: requested exceeds parent quota\n");
        return Err(IpcError::BadCapability);
    }
    let Some(target) = runtime.processes.process_mut(target_pid) else {
        return Err(IpcError::BadCapability);
    };
    let persist_for_restart = target.state == ProcessState::Declared;
    target.quota.max_endpoints = max_endpoints;
    if persist_for_restart {
        target.quota.used_endpoints = 0;
        target.initial_quota.max_endpoints = max_endpoints;
        target.initial_quota.used_endpoints = 0;
    }

    serial::write_str("Quota delegate accepted: proc=");
    serial::write_str(caller_name);
    serial::write_str(" target=");
    serial::write_str(target.name);
    serial::write_str(" target_pid=");
    serial::write_u64_dec(target_pid.raw());
    serial::write_str(" max_endpoints=");
    serial::write_u64_dec(max_endpoints);
    serial::write_str("\n");
    Ok(())
}

pub fn legacy_object_read(
    _cap_slot: u64,
    _destination: *mut u8,
    _max_len: usize,
) -> Result<usize, IpcError> {
    serial::write_str("Legacy object-read syscall rejected: use VFS handles\n");
    Err(IpcError::BadCapability)
}

pub fn vfs_open(cap_slot: u64, path: *const u8, packed_len_flags: u64) -> Result<u64, IpcError> {
    let path_len = usize::try_from(packed_len_flags & 0xffff_ffff).unwrap_or(usize::MAX);
    let flags = packed_len_flags >> 32;
    if path_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    if flags & !VFS_OPEN_KNOWN_FLAGS != 0 {
        return Err(IpcError::VfsUnsupported);
    }
    if flags & (VFS_OPEN_CREATE | VFS_OPEN_TRUNC | VFS_OPEN_APPEND) != 0
        && flags & VFS_OPEN_WRITE == 0
    {
        return Err(IpcError::VfsPermission);
    }

    let mut path_bytes = [0u8; MAX_VFS_PATH_BYTES];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    let cap = lookup_capability(cap_slot, 0).map_err(|_| IpcError::VfsPermission)?;
    let path = &path_bytes[..path_len];
    if flags & (VFS_OPEN_WRITE | VFS_OPEN_CREATE | VFS_OPEN_TRUNC | VFS_OPEN_APPEND) != 0
        && runtime().objects.get_vfs_root(cap.object).is_some()
        && vfs_request_path_is_read_only(path)?
    {
        return Err(IpcError::VfsPermission);
    }
    let mut created_node = None;
    let (node, available_rights) = match resolve_vfs_node_from_cap(cap, path) {
        Ok(resolved) => resolved,
        Err(IpcError::VfsNotFound) if flags & VFS_OPEN_CREATE != 0 => {
            let requested_rights = vfs_regular_file_open_rights(flags)?;
            let (node, available) = vfs_create_memory_file_node(cap, path, requested_rights)?;
            created_node = Some(node);
            (node, available)
        }
        Err(error) => return Err(error),
    };
    let requested_rights = match vfs_open_rights(flags, node) {
        Ok(rights) => rights,
        Err(error) => {
            release_created_vfs_memory_node(runtime(), created_node);
            return Err(error);
        }
    };
    if requested_rights & !available_rights != 0 {
        if !matches!(node.backing, VfsBacking::Device(_)) {
            release_created_vfs_memory_node(runtime(), created_node);
            return Err(IpcError::VfsPermission);
        }
    }
    if let VfsBacking::Device(device_object) = node.backing {
        if let Err(error) = validate_vfs_device_open(flags, available_rights, device_object) {
            release_created_vfs_memory_node(runtime(), created_node);
            return Err(error);
        }
    }
    if matches!(
        node.backing,
        VfsBacking::StateVolumeValue(_) | VfsBacking::StateVolumeControl(_)
    ) && flags & (VFS_OPEN_CREATE | VFS_OPEN_TRUNC | VFS_OPEN_APPEND) != 0
    {
        release_created_vfs_memory_node(runtime(), created_node);
        return Err(IpcError::VfsUnsupported);
    }
    if matches!(node.backing, VfsBacking::FsServiceReport)
        && flags & (VFS_OPEN_WRITE | VFS_OPEN_CREATE | VFS_OPEN_TRUNC | VFS_OPEN_APPEND) != 0
    {
        release_created_vfs_memory_node(runtime(), created_node);
        return Err(IpcError::VfsUnsupported);
    }
    if flags & VFS_OPEN_TRUNC != 0
        && !matches!(
            node.backing,
            VfsBacking::MemoryFile(_) | VfsBacking::VertexFsFile(_)
        )
    {
        release_created_vfs_memory_node(runtime(), created_node);
        return Err(IpcError::VfsUnsupported);
    }

    let owner = current_process_id();
    let (raw_handle, description) = {
        let runtime = runtime();
        let description =
            match runtime.open_file_description(node.id, requested_rights, flags, owner, cap.id) {
                Ok(description) => description,
                Err(error) => {
                    release_created_vfs_memory_node(runtime, created_node);
                    return Err(error);
                }
            };
        let Some(process) = runtime.processes.current_process_mut() else {
            let _ = runtime.release_file_description(description);
            release_created_vfs_memory_node(runtime, created_node);
            return Err(IpcError::VfsPermission);
        };
        let handle = FileHandle { description };
        match process.open_file_handle(handle) {
            Ok(raw) => (raw, description),
            Err(error) => {
                let _ = runtime.release_file_description(description);
                release_created_vfs_memory_node(runtime, created_node);
                return Err(error);
            }
        }
    };
    if flags & VFS_OPEN_TRUNC != 0 {
        if let Err(error) = vfs_truncate_node(node, 0) {
            let runtime = runtime();
            if let Some(process) = runtime.processes.current_process_mut() {
                let _ = process.close_file_handle(raw_handle);
            }
            let _ = runtime.release_file_description(description);
            release_created_vfs_memory_node(runtime, created_node);
            return Err(error);
        }
    }

    if created_node.is_some() {
        if let Some(parent) = node.parent {
            runtime().record_vfs_event(parent, VFS_EVENT_CREATE, node.name);
        }
        serial::write_str("VFS open-create accepted: proc=");
        serial::write_str(current_process_name());
        serial::write_str(" path=");
        serial::write_ascii_bytes(path);
        serial::write_str("\n");
    }
    serial::write_str("VFS open accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" handle=");
    serial::write_u64_dec(raw_handle);
    serial::write_str(" description=");
    serial::write_u64_dec(description.raw());
    serial::write_str("\n");
    Ok(raw_handle)
}

pub fn vfs_read(
    handle: u64,
    destination: *mut u8,
    max_len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let (description, node) = current_open_file(handle)?;
    if description.rights & capability::RIGHT_READ == 0 {
        return Err(IpcError::VfsPermission);
    }
    if matches!(node.backing, VfsBacking::Pipe) {
        if max_len == 0 {
            frame.rax = 0;
            return Ok(());
        }
        usercopy::validate_user_buffer(
            UserPtr::new(destination as u64),
            max_len,
            paging::UserAccess::Write,
        )
        .map_err(|_| IpcError::InvalidUserBuffer)?;
        if !runtime().vfs_pipe.is_empty() {
            let copy_len = min(runtime().vfs_pipe.len, max_len);
            usercopy::copy_to_user(
                UserPtr::new(destination as u64),
                &runtime().vfs_pipe.bytes[..copy_len],
            )
            .map_err(|_| IpcError::InvalidUserBuffer)?;
            runtime().vfs_pipe.len = 0;
            serial::write_str("VFS pipe buffered read accepted: proc=");
            serial::write_str(current_process_name());
            serial::write_str(" file=");
            serial_write_vfs_name(node.name);
            serial::write_str(" bytes=");
            serial::write_u64_dec(copy_len as u64);
            serial::write_str("\n");
            frame.rax = copy_len as u64;
            return Ok(());
        }
        if block_current_on_vfs_read(node.id, description.id, destination as u64, max_len, frame) {
            return Ok(());
        }
        return Err(IpcError::Empty);
    }
    if let VfsBacking::StateVolumeValue(state) = node.backing {
        return vfs_state_value_read(
            state,
            node,
            description,
            description.offset,
            destination,
            max_len,
            true,
            frame,
        );
    }
    if matches!(node.backing, VfsBacking::FsServiceReport) {
        return start_vfs_service_read_transaction(
            node,
            description,
            description.offset,
            destination as u64,
            max_len,
            true,
            frame,
        );
    }
    let (copy_len, new_offset) = vfs_read_node(node, description.offset, destination, max_len)?;
    runtime()
        .file_description_mut(description.id)
        .ok_or(IpcError::VfsBadHandle)?
        .offset = new_offset;

    serial::write_str("VFS read accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(copy_len as u64);
    serial::write_str("\n");
    frame.rax = copy_len as u64;
    Ok(())
}

pub fn vfs_pread(
    handle: u64,
    destination: *mut u8,
    packed_len_offset: u64,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let max_len = usize::try_from(packed_len_offset & 0xffff_ffff).unwrap_or(usize::MAX);
    let offset = packed_len_offset >> 32;
    let (description, node) = current_open_file(handle)?;
    if description.rights & capability::RIGHT_READ == 0 {
        return Err(IpcError::VfsPermission);
    }
    if let VfsBacking::StateVolumeValue(state) = node.backing {
        return vfs_state_value_read(
            state,
            node,
            description,
            offset,
            destination,
            max_len,
            false,
            frame,
        );
    }
    if matches!(node.backing, VfsBacking::FsServiceReport) {
        return start_vfs_service_read_transaction(
            node,
            description,
            offset,
            destination as u64,
            max_len,
            false,
            frame,
        );
    }
    let (copy_len, _) = vfs_read_node(node, offset, destination, max_len)?;
    serial::write_str("VFS pread accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(copy_len as u64);
    serial::write_str("\n");
    frame.rax = copy_len as u64;
    Ok(())
}

pub fn vfs_write(
    handle: u64,
    source: *const u8,
    len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let (description, node) = current_open_file(handle)?;
    if let VfsBacking::StateVolumeControl(state) = node.backing {
        if description.rights & capability::RIGHT_CONTROL == 0 {
            return Err(IpcError::VfsPermission);
        }
        if description.flags & VFS_OPEN_APPEND != 0 {
            return Err(IpcError::VfsUnsupported);
        }
        return vfs_state_control_write(
            state,
            node,
            description,
            description.offset,
            source,
            len,
            true,
            frame,
        );
    }
    if description.rights & capability::RIGHT_WRITE == 0 {
        return Err(IpcError::VfsPermission);
    }
    if let VfsBacking::StateVolumeValue(state) = node.backing {
        if description.flags & VFS_OPEN_APPEND != 0 {
            return Err(IpcError::VfsUnsupported);
        }
        return vfs_state_value_write(
            state,
            node,
            description,
            description.offset,
            source,
            len,
            true,
            frame,
        );
    }
    if len > MAX_VFS_MEM_FILE_BYTES {
        return Err(IpcError::VfsNoSpace);
    }
    let mut bytes = [0u8; MAX_VFS_MEM_FILE_BYTES];
    usercopy::copy_from_user(&mut bytes, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let offset = if description.flags & VFS_OPEN_APPEND != 0 {
        vfs_node_len(node)?
    } else {
        description.offset
    };
    let (written, new_offset) = vfs_write_node(node, offset, &bytes[..len])?;
    runtime()
        .file_description_mut(description.id)
        .ok_or(IpcError::VfsBadHandle)?
        .offset = new_offset;
    serial::write_str("VFS write accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(written as u64);
    serial::write_str("\n");
    frame.rax = written as u64;
    Ok(())
}

pub fn vfs_pwrite(
    handle: u64,
    source: *const u8,
    packed_len_offset: u64,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let len = usize::try_from(packed_len_offset & 0xffff_ffff).unwrap_or(usize::MAX);
    let offset = packed_len_offset >> 32;
    let (description, node) = current_open_file(handle)?;
    if let VfsBacking::StateVolumeControl(state) = node.backing {
        if description.rights & capability::RIGHT_CONTROL == 0 {
            return Err(IpcError::VfsPermission);
        }
        return vfs_state_control_write(
            state,
            node,
            description,
            offset,
            source,
            len,
            false,
            frame,
        );
    }
    if description.rights & capability::RIGHT_WRITE == 0 {
        return Err(IpcError::VfsPermission);
    }
    if let VfsBacking::StateVolumeValue(state) = node.backing {
        return vfs_state_value_write(state, node, description, offset, source, len, false, frame);
    }
    if len > MAX_VFS_MEM_FILE_BYTES {
        return Err(IpcError::VfsNoSpace);
    }
    let mut bytes = [0u8; MAX_VFS_MEM_FILE_BYTES];
    usercopy::copy_from_user(&mut bytes, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let (written, _) = vfs_write_node(node, offset, &bytes[..len])?;
    serial::write_str("VFS pwrite accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(written as u64);
    serial::write_str("\n");
    frame.rax = written as u64;
    Ok(())
}

fn vfs_state_value_read(
    state: KernelObjectId,
    node: VfsNode,
    description: OpenFileDescription,
    offset: u64,
    destination: *mut u8,
    max_len: usize,
    update_offset: bool,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    if max_len == 0 {
        frame.rax = 0;
        return Ok(());
    }
    usercopy::validate_user_buffer(
        UserPtr::new(destination as u64),
        max_len,
        paging::UserAccess::Write,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;

    let request = [0u8; MAX_MESSAGE_BYTES];
    start_vfs_state_transaction(
        state,
        node,
        description,
        VfsStateOperation::Read,
        offset,
        destination as u64,
        max_len,
        0,
        update_offset,
        &request,
        0,
        frame,
    )
}

fn vfs_state_value_write(
    state: KernelObjectId,
    node: VfsNode,
    description: OpenFileDescription,
    offset: u64,
    source: *const u8,
    len: usize,
    update_offset: bool,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    if offset != 0 {
        return Err(IpcError::VfsUnsupported);
    }
    if len > MAX_STATE_VOLUME_VALUE_BYTES {
        return Err(IpcError::VfsNoSpace);
    }

    let mut request = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut request, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    start_vfs_state_transaction(
        state,
        node,
        description,
        VfsStateOperation::Write,
        offset,
        0,
        0,
        len,
        update_offset,
        &request,
        len,
        frame,
    )
}

fn vfs_state_control_write(
    state: KernelObjectId,
    node: VfsNode,
    description: OpenFileDescription,
    offset: u64,
    source: *const u8,
    len: usize,
    update_offset: bool,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    if offset != 0 || len != 1 {
        return Err(IpcError::VfsUnsupported);
    }

    let mut request = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut request[..len], UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    if request[0] != b'Q' {
        return Err(IpcError::VfsUnsupported);
    }
    start_vfs_state_transaction(
        state,
        node,
        description,
        VfsStateOperation::Control,
        offset,
        0,
        0,
        len,
        update_offset,
        &request,
        1,
        frame,
    )
}

pub fn vfs_close(handle: u64) -> Result<(), IpcError> {
    let (process_name, file) = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return Err(IpcError::VfsPermission);
        };
        (process.name, process.close_file_handle(handle)?)
    };
    runtime().release_file_description(file.description)?;
    serial::write_str("VFS close accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" handle=");
    serial::write_u64_dec(handle);
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_seek(handle: u64, offset: u64, whence: u64) -> Result<u64, IpcError> {
    let (description, node) = current_open_file(handle)?;
    let size = vfs_node_len(node)?;
    let next = match whence {
        VFS_SEEK_SET => offset,
        VFS_SEEK_CURRENT => description
            .offset
            .checked_add(offset)
            .ok_or(IpcError::VfsUnsupported)?,
        VFS_SEEK_END => size.checked_add(offset).ok_or(IpcError::VfsUnsupported)?,
        _ => return Err(IpcError::VfsUnsupported),
    };
    if next > size {
        return Err(IpcError::VfsUnsupported);
    }
    runtime()
        .file_description_mut(description.id)
        .ok_or(IpcError::VfsBadHandle)?
        .offset = next;
    Ok(next)
}

pub fn vfs_stat(
    handle: u64,
    destination: *mut u8,
    max_len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    if max_len < VFS_STAT_BYTES {
        return Err(IpcError::InvalidUserBuffer);
    }
    let (description, node) = current_open_file(handle)?;
    if let VfsBacking::StateVolumeValue(state) = node.backing {
        return vfs_state_value_stat(state, node, description, destination, max_len, frame);
    }
    let mut stat = [0u8; VFS_STAT_BYTES];
    write_vfs_stat_record(&mut stat, node, vfs_node_len(node)?, description.rights);
    usercopy::copy_to_user(UserPtr::new(destination as u64), &stat)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    serial::write_str("VFS stat accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str("\n");
    frame.rax = VFS_STAT_BYTES as u64;
    Ok(())
}

fn write_vfs_stat_record(stat: &mut [u8; VFS_STAT_BYTES], node: VfsNode, size: u64, rights: u64) {
    write_u64_le(stat, 0, vfs_node_kind_value(node.kind));
    write_u64_le(stat, 8, size);
    write_u64_le(stat, 16, node.id.raw());
    write_u64_le(stat, 24, rights);
    write_u64_le(stat, 32, node.metadata_version);
    write_u64_le(stat, 40, runtime().vfs_node_link_count(node));
    write_u64_le(stat, 48, 0);
    write_u64_le(stat, 56, 0);
}

fn vfs_state_value_stat(
    state: KernelObjectId,
    node: VfsNode,
    description: OpenFileDescription,
    destination: *mut u8,
    max_len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    usercopy::validate_user_buffer(
        UserPtr::new(destination as u64),
        VFS_STAT_BYTES,
        paging::UserAccess::Write,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;

    let request = [0u8; MAX_MESSAGE_BYTES];
    start_vfs_state_transaction(
        state,
        node,
        description,
        VfsStateOperation::Stat,
        0,
        destination as u64,
        max_len,
        0,
        false,
        &request,
        0,
        frame,
    )
}

pub fn vfs_readdir(handle: u64, destination: *mut u8, max_len: usize) -> Result<usize, IpcError> {
    if max_len < VFS_DIRENT_BYTES {
        return Err(IpcError::InvalidUserBuffer);
    }
    let (description, node) = current_open_file(handle)?;
    if !matches!(node.kind, VfsNodeKind::Directory) {
        return Err(IpcError::VfsNotDirectory);
    }
    if description.rights & capability::RIGHT_RESOLVE == 0 {
        return Err(IpcError::VfsPermission);
    }
    let entry_index = usize::try_from(description.offset).map_err(|_| IpcError::VfsUnsupported)?;
    let Some(child) = runtime().vfs_child_by_entry_index(node.id, entry_index) else {
        return Ok(0);
    };

    let mut dirent = [0u8; VFS_DIRENT_BYTES];
    write_u64_le(&mut dirent, 0, vfs_node_kind_value(child.kind));
    write_u64_le(&mut dirent, 8, child.id.raw());
    write_u64_le(&mut dirent, 16, child.name.len as u64);
    let name = child.name.as_bytes();
    let mut index = 0;
    while index < name.len() {
        dirent[24 + index] = name[index];
        index += 1;
    }
    usercopy::copy_to_user(UserPtr::new(destination as u64), &dirent)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    runtime()
        .file_description_mut(description.id)
        .ok_or(IpcError::VfsBadHandle)?
        .offset = description
        .offset
        .checked_add(1)
        .ok_or(IpcError::VfsUnsupported)?;

    serial::write_str("VFS readdir accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" dir=");
    serial_write_vfs_name(node.name);
    serial::write_str(" entry=");
    serial_write_vfs_name(child.name);
    serial::write_str(" vnode=");
    serial::write_u64_dec(child.id.raw());
    serial::write_str("\n");
    Ok(VFS_DIRENT_BYTES)
}

pub fn vfs_mount(cap_slot: u64, path: *const u8, packed_len_flags: u64) -> Result<(), IpcError> {
    let path_len = usize::try_from(packed_len_flags & 0xffff_ffff).unwrap_or(usize::MAX);
    let flags = packed_len_flags >> 32;
    if path_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    if flags & !VFS_MOUNT_KNOWN_FLAGS != 0
        || flags == 0
        || (flags & VFS_MOUNT_VOLATILE != 0 && flags != VFS_MOUNT_VOLATILE)
        || (flags & VFS_MOUNT_BIND != 0 && flags & VFS_MOUNT_VOLATILE != 0)
        || (flags & VFS_MOUNT_READ_ONLY != 0 && flags & VFS_MOUNT_BIND == 0)
    {
        return Err(IpcError::VfsUnsupported);
    }
    let mut path_bytes = [0u8; MAX_VFS_PATH_BYTES];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let requested_path = &path_bytes[..path_len];
    let root_path = resolve_process_vfs_path(requested_path)?;
    let path = root_path.as_bytes();
    let (parent_path, child_name) = split_vfs_parent_child(path)?;
    let child_name = VfsName::from_user_component(child_name).map_err(|_| IpcError::VfsBadPath)?;
    let cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    let available = resolve_vfs_root_authority(cap, parent_path)?;
    if available & capability::RIGHT_MOUNT == 0 {
        return Err(IpcError::VfsPermission);
    }
    let parent = runtime()
        .vfs_node_by_path(parent_path)
        .ok_or(IpcError::VfsNotFound)?;
    if !matches!(parent.kind, VfsNodeKind::Directory) {
        return Err(IpcError::VfsNotDirectory);
    }
    if runtime().vfs_node_by_path(path).is_some() {
        return Err(IpcError::VfsExists);
    }
    if flags & VFS_MOUNT_BIND != 0 {
        let source_root = runtime()
            .objects
            .get_vfs_root(cap.object)
            .ok_or(IpcError::VfsPermission)?;
        let source_node = runtime()
            .vfs_node_by_path(source_root.root_path.as_bytes())
            .ok_or(IpcError::VfsNotFound)?;
        if !matches!(source_node.kind, VfsNodeKind::Directory) {
            return Err(IpcError::VfsNotDirectory);
        }
        let source_mount_flags = runtime()
            .objects
            .get_vfs_mount_by_path(source_root.root_path.as_bytes())
            .map(|mount| mount.flags)
            .unwrap_or(0);
        let bind_flags = flags | (source_mount_flags & VFS_MOUNT_READ_ONLY);
        runtime()
            .add_vfs_mount(
                "mount:bind",
                source_node.id,
                root_path,
                source_node.mount_source,
                bind_flags,
                true,
                current_process_id(),
            )
            .map_err(|_| IpcError::VfsNoSpace)?;

        serial::write_str("VFS bind mount accepted: proc=");
        serial::write_str(current_process_name());
        serial::write_str(" path=");
        serial::write_ascii_bytes(requested_path);
        serial::write_str(" canonical=");
        serial::write_ascii_bytes(path);
        serial::write_str(" source=");
        serial::write_ascii_bytes(source_root.root_path.as_bytes());
        serial::write_str(" flags=");
        serial_write_vfs_mount_flags(bind_flags);
        serial::write_str("\n");
        return Ok(());
    }

    let runtime = runtime();
    let node_id = runtime
        .add_vfs_node_with_name(
            child_name,
            Some(parent.id),
            VfsNodeKind::Directory,
            VfsBacking::None,
            "volatilefs",
        )
        .map_err(|_| IpcError::VfsNoSpace)?;
    if runtime
        .add_vfs_mount(
            "mount:volatile",
            node_id,
            root_path,
            "volatilefs",
            flags,
            true,
            current_process_id(),
        )
        .is_err()
    {
        let _ = runtime.remove_vfs_node(node_id);
        return Err(IpcError::VfsNoSpace);
    }

    serial::write_str("VFS mount accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" path=");
    serial::write_ascii_bytes(requested_path);
    serial::write_str(" canonical=");
    serial::write_ascii_bytes(path);
    serial::write_str(" source=volatilefs\n");
    Ok(())
}

pub fn vfs_unmount(cap_slot: u64, path: *const u8, path_len: usize) -> Result<(), IpcError> {
    if path_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    let mut path_bytes = [0u8; MAX_VFS_PATH_BYTES];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let requested_path = &path_bytes[..path_len];
    let canonical_path = resolve_process_vfs_path(requested_path)?;
    let path = canonical_path.as_bytes();
    let cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    let available = resolve_vfs_root_authority(cap, path)?;
    if available & capability::RIGHT_MOUNT == 0 {
        return Err(IpcError::VfsPermission);
    }
    let exact_dynamic_bind = runtime()
        .objects
        .get_vfs_mount_by_exact_path(path)
        .filter(|mount| mount.dynamic && mount.flags & VFS_MOUNT_BIND != 0);
    let node = runtime().vfs_node_by_path(path);
    let mount = if let Some(mount) = exact_dynamic_bind {
        mount
    } else {
        let node = node.ok_or(IpcError::VfsNotFound)?;
        if !matches!(node.kind, VfsNodeKind::Directory) {
            return Err(IpcError::VfsNotDirectory);
        }
        runtime()
            .objects
            .get_vfs_mount_by_root_node(node.id)
            .ok_or(IpcError::VfsUnsupported)?
    };
    if !mount.dynamic {
        return Err(IpcError::VfsUnsupported);
    }
    if runtime().vfs_subtree_has_open_description(mount.root_node)
        || (mount.flags & VFS_MOUNT_BIND == 0
            && node.is_some_and(|node| runtime().vfs_node_has_children(node.id)))
    {
        return Err(IpcError::VfsBusy);
    }

    let runtime = runtime();
    if mount.flags & VFS_MOUNT_BIND == 0
        && let Some(node) = node
    {
        runtime.remove_vfs_node(node.id)?;
    }
    let removed_mount = if mount.flags & VFS_MOUNT_BIND != 0 {
        runtime.objects.remove_dynamic_vfs_mount_by_path(path)
    } else {
        runtime.objects.remove_dynamic_vfs_mount(mount.root_node)
    };
    if let Some(mount_id) = removed_mount {
        runtime.remove_vfs_mount_id(mount_id);
    }

    serial::write_str("VFS unmount accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" path=");
    serial::write_ascii_bytes(requested_path);
    serial::write_str(" canonical=");
    serial::write_ascii_bytes(path);
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_sync(handle: u64, frame: &mut SyscallFrame) -> Result<(), IpcError> {
    let (_description, node) = current_open_file(handle)?;
    match node.backing {
        VfsBacking::StoreObject(_)
        | VfsBacking::StateVolumeValue(_)
        | VfsBacking::MemoryFile(_)
        | VfsBacking::VertexFsFile(_)
        | VfsBacking::Synthetic(_) => {}
        _ => return Err(IpcError::VfsUnsupported),
    }
    if let VfsBacking::VertexFsFile(backing) = node.backing {
        match runtime().prepare_vertexfs_sync_file(backing)? {
            VertexFsSyncResult::Journaled {
                inode_id,
                checksum,
                write_count,
            } => {
                return start_vertexfs_sync_transaction(
                    backing,
                    inode_id,
                    checksum,
                    write_count,
                    frame,
                );
            }
            VertexFsSyncResult::Cached { checksum } => {
                serial::write_str("VertexFS v1 fsync cached runtime file=");
                serial_write_vfs_name(node.name);
                serial::write_str(" checksum=");
                serial::write_u64_dec(checksum as u64);
                serial::write_str("\n");
            }
        }
    }
    serial::write_str("VFS sync accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str("\n");
    frame.rax = STATUS_OK;
    Ok(())
}

pub fn vfs_dup(handle: u64, flags: u64) -> Result<u64, IpcError> {
    if flags & !VFS_DUP_SHARE_OFFSET != 0 {
        return Err(IpcError::VfsUnsupported);
    }
    let file = current_file_handle(handle)?;
    let description = runtime()
        .file_description(file.description)
        .ok_or(IpcError::VfsBadHandle)?;
    if runtime().cap_id_revoked_or_has_revoked_ancestor(description.authority_cap_id) {
        return Err(IpcError::VfsPermission);
    }
    let new_description = if flags & VFS_DUP_SHARE_OFFSET != 0 {
        runtime().retain_file_description(description.id)?;
        description.id
    } else {
        let runtime = runtime();
        let new_id = runtime.open_file_description(
            description.node,
            description.rights,
            description.flags,
            description.owner,
            description.authority_cap_id,
        )?;
        runtime
            .file_description_mut(new_id)
            .ok_or(IpcError::VfsBadHandle)?
            .offset = description.offset;
        new_id
    };

    let raw = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            runtime.release_file_description(new_description)?;
            return Err(IpcError::VfsPermission);
        };
        match process.open_file_handle(FileHandle {
            description: new_description,
        }) {
            Ok(raw) => raw,
            Err(error) => {
                runtime.release_file_description(new_description)?;
                return Err(error);
            }
        }
    };
    serial::write_str("VFS dup accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" handle=");
    serial::write_u64_dec(handle);
    serial::write_str(" new_handle=");
    serial::write_u64_dec(raw);
    serial::write_str(if flags & VFS_DUP_SHARE_OFFSET != 0 {
        " shared-offset=yes\n"
    } else {
        " shared-offset=no\n"
    });
    Ok(raw)
}

pub fn vfs_poll(handle: u64, events: u64, _timeout_ms: u64) -> Result<u64, IpcError> {
    if events == 0 || events & !VFS_POLL_KNOWN_EVENTS != 0 {
        return Err(IpcError::VfsUnsupported);
    }
    let (description, node) = current_open_file(handle)?;
    if events & VFS_POLL_READABLE != 0 && description.rights & capability::RIGHT_READ == 0 {
        return Err(IpcError::VfsPermission);
    }
    if events & VFS_POLL_WRITABLE != 0 && description.rights & capability::RIGHT_WRITE == 0 {
        return Err(IpcError::VfsPermission);
    }
    let ready = vfs_poll_ready(description, node, events)?;
    serial::write_str("VFS poll accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" events=");
    serial::write_u64_dec(events);
    serial::write_str(" ready=");
    serial::write_u64_dec(ready);
    serial::write_str("\n");
    Ok(ready)
}

pub fn vfs_watch(handle: u64, destination: *mut u8, max_len: usize) -> Result<usize, IpcError> {
    if max_len < VFS_WATCH_EVENT_BYTES {
        return Err(IpcError::InvalidUserBuffer);
    }
    let (description, node) = current_open_file(handle)?;
    if !matches!(node.kind, VfsNodeKind::Directory) {
        return Err(IpcError::VfsNotDirectory);
    }
    if description.rights & capability::RIGHT_RESOLVE == 0 {
        return Err(IpcError::VfsPermission);
    }
    let start = min(description.watch_cursor, runtime().vfs_event_count);
    let mut event_index = start;
    let mut event = None;
    while event_index < runtime().vfs_event_count {
        if let Some(candidate) = runtime().vfs_events[event_index]
            && candidate.parent == node.id
        {
            event = Some(candidate);
            break;
        }
        event_index += 1;
    }
    let Some(event) = event else {
        return Ok(0);
    };
    let mut record = [0u8; VFS_WATCH_EVENT_BYTES];
    write_vfs_watch_event_record(&mut record, event);
    usercopy::copy_to_user(UserPtr::new(destination as u64), &record)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    runtime()
        .file_description_mut(description.id)
        .ok_or(IpcError::VfsBadHandle)?
        .watch_cursor = event_index + 1;

    serial::write_str("VFS watch event delivered: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" dir=");
    serial_write_vfs_name(node.name);
    serial::write_str(" event=");
    serial::write_u64_dec(event.kind);
    serial::write_str(" name=");
    serial_write_vfs_name(event.name);
    serial::write_str("\n");
    Ok(VFS_WATCH_EVENT_BYTES)
}

pub fn vfs_lock(handle: u64, flags: u64, packed_range: u64) -> Result<(), IpcError> {
    let mode = match flags & VFS_LOCK_MODE_MASK {
        VFS_LOCK_SHARED => VfsLockMode::Shared,
        VFS_LOCK_EXCLUSIVE => VfsLockMode::Exclusive,
        _ => return Err(IpcError::VfsUnsupported),
    };
    if flags & !(VFS_LOCK_MODE_MASK | VFS_LOCK_RANGE) != 0 {
        return Err(IpcError::VfsUnsupported);
    }
    let (start, len) = if flags & VFS_LOCK_RANGE != 0 {
        let len = packed_range >> 32;
        let start = packed_range & 0xffff_ffff;
        if len == 0 {
            return Err(IpcError::VfsUnsupported);
        }
        (start, len)
    } else {
        (0, u64::MAX)
    };
    let (description, node) = current_open_file(handle)?;
    if !matches!(node.kind, VfsNodeKind::RegularFile) {
        return Err(IpcError::VfsNotFile);
    }
    match mode {
        VfsLockMode::Shared => {
            if description.rights & capability::RIGHT_READ == 0 {
                return Err(IpcError::VfsPermission);
            }
        }
        VfsLockMode::Exclusive => {
            if description.rights & capability::RIGHT_WRITE == 0 {
                return Err(IpcError::VfsPermission);
            }
        }
    }
    runtime().acquire_vfs_lock(description, mode, start, len)?;
    serial::write_str("VFS lock accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" description=");
    serial::write_u64_dec(description.id.raw());
    if flags & VFS_LOCK_RANGE != 0 {
        serial::write_str(" range=");
        serial::write_u64_dec(start);
        serial::write_str("+");
        serial::write_u64_dec(len);
    }
    serial::write_str(match mode {
        VfsLockMode::Shared => " mode=shared\n",
        VfsLockMode::Exclusive => " mode=exclusive\n",
    });
    Ok(())
}

pub fn vfs_unlock(handle: u64) -> Result<(), IpcError> {
    let (description, node) = current_open_file(handle)?;
    if !runtime().release_vfs_lock(description.id) {
        return Err(IpcError::VfsBadHandle);
    }
    serial::write_str("VFS unlock accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" description=");
    serial::write_u64_dec(description.id.raw());
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_create(cap_slot: u64, path: *const u8, packed_len_flags: u64) -> Result<(), IpcError> {
    let path_len = usize::try_from(packed_len_flags & 0xffff_ffff).unwrap_or(usize::MAX);
    let flags = packed_len_flags >> 32;
    if path_len > MAX_VFS_PATH_BYTES || flags != 0 {
        return Err(IpcError::VfsBadPath);
    }
    let mut path_bytes = [0u8; MAX_VFS_PATH_BYTES];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let path = &path_bytes[..path_len];
    if vfs_request_path_is_read_only(path)? {
        return Err(IpcError::VfsPermission);
    }
    let cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    let (node, _) = vfs_create_memory_file_node(cap, path, 0)?;
    if let Some(parent) = node.parent {
        runtime().record_vfs_event(parent, VFS_EVENT_CREATE, node.name);
    }

    serial::write_str("VFS create accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" path=");
    serial::write_ascii_bytes(path);
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_mkdir(cap_slot: u64, path: *const u8, packed_len_flags: u64) -> Result<(), IpcError> {
    let path_len = usize::try_from(packed_len_flags & 0xffff_ffff).unwrap_or(usize::MAX);
    let flags = packed_len_flags >> 32;
    if path_len > MAX_VFS_PATH_BYTES || flags != 0 {
        return Err(IpcError::VfsBadPath);
    }
    let mut path_bytes = [0u8; MAX_VFS_PATH_BYTES];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let requested_path = &path_bytes[..path_len];
    if vfs_request_path_is_read_only(requested_path)? {
        return Err(IpcError::VfsPermission);
    }
    let cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    let node = vfs_create_directory_node(cap, requested_path)?;
    if let Some(parent) = node.parent {
        runtime().record_vfs_event(parent, VFS_EVENT_CREATE, node.name);
    }

    serial::write_str("VFS mkdir accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" path=");
    serial::write_ascii_bytes(requested_path);
    serial::write_str(" vnode=");
    serial::write_u64_dec(node.id.raw());
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_unlink(cap_slot: u64, path: *const u8, path_len: usize) -> Result<(), IpcError> {
    if path_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    let mut path_bytes = [0u8; MAX_VFS_PATH_BYTES];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let requested_path = &path_bytes[..path_len];
    let canonical_path = resolve_process_vfs_path(requested_path)?;
    let path = canonical_path.as_bytes();
    if vfs_path_is_read_only(path) {
        return Err(IpcError::VfsPermission);
    }
    let cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    let available = resolve_vfs_root_authority(cap, path)?;
    if available & capability::RIGHT_UNLINK == 0 {
        return Err(IpcError::VfsPermission);
    }

    let node = runtime()
        .vfs_node_by_path(path)
        .ok_or(IpcError::VfsNotFound)?;
    let VfsBacking::MemoryFile(backing) = node.backing else {
        return Err(IpcError::VfsUnsupported);
    };
    if runtime().vfs_node_has_children(node.id) {
        return Err(IpcError::VfsBusy);
    }
    {
        let runtime = runtime();
        if runtime.vfs_node_has_open_description(node.id) {
            runtime.detach_vfs_node(node.id)?;
            runtime.touch_vfs_memory_file_nodes(backing)?;
        } else {
            runtime.remove_vfs_node(node.id)?;
            if runtime.vfs_memory_file_in_use(backing) {
                runtime.touch_vfs_memory_file_nodes(backing)?;
            } else {
                let _ = runtime.release_vfs_memory_file(backing);
            }
        }
    }
    if let Some(parent) = node.parent {
        runtime().record_vfs_event(parent, VFS_EVENT_UNLINK, node.name);
    }

    serial::write_str("VFS unlink accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" path=");
    serial::write_ascii_bytes(requested_path);
    serial::write_str(" canonical=");
    serial::write_ascii_bytes(path);
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_rmdir(cap_slot: u64, path: *const u8, path_len: usize) -> Result<(), IpcError> {
    if path_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    let mut path_bytes = [0u8; MAX_VFS_PATH_BYTES];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let requested_path = &path_bytes[..path_len];
    let canonical_path = resolve_process_vfs_path(requested_path)?;
    let path = canonical_path.as_bytes();
    if vfs_path_is_read_only(path) {
        return Err(IpcError::VfsPermission);
    }
    let (parent_path, _) = split_vfs_parent_child(path)?;
    let cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    let available = resolve_vfs_root_authority(cap, parent_path)?;
    if available & capability::RIGHT_UNLINK == 0 {
        return Err(IpcError::VfsPermission);
    }

    let node = runtime()
        .vfs_node_by_path(path)
        .ok_or(IpcError::VfsNotFound)?;
    if !matches!(node.kind, VfsNodeKind::Directory) {
        return Err(IpcError::VfsNotDirectory);
    }
    if node.parent.is_none()
        || runtime()
            .objects
            .get_vfs_mount_by_root_node(node.id)
            .is_some()
    {
        return Err(IpcError::VfsUnsupported);
    }
    if runtime().vfs_node_has_children(node.id) || runtime().vfs_node_has_open_description(node.id)
    {
        return Err(IpcError::VfsBusy);
    }
    runtime().remove_vfs_node(node.id)?;
    if let Some(parent) = node.parent {
        runtime().record_vfs_event(parent, VFS_EVENT_UNLINK, node.name);
    }

    serial::write_str("VFS rmdir accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" path=");
    serial::write_ascii_bytes(requested_path);
    serial::write_str(" canonical=");
    serial::write_ascii_bytes(path);
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_rename(cap_slot: u64, request: *const u8, request_len: usize) -> Result<(), IpcError> {
    if !(VFS_RENAME_REQUEST_HEADER_BYTES..=VFS_RENAME_REQUEST_MAX_BYTES).contains(&request_len) {
        return Err(IpcError::VfsBadPath);
    }
    let mut request_bytes = [0u8; VFS_RENAME_REQUEST_MAX_BYTES];
    usercopy::copy_from_user(
        &mut request_bytes,
        UserPtr::new(request as u64),
        request_len,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;
    let old_len =
        usize::try_from(read_u64_le(&request_bytes, 0)).map_err(|_| IpcError::VfsBadPath)?;
    let new_len =
        usize::try_from(read_u64_le(&request_bytes, 8)).map_err(|_| IpcError::VfsBadPath)?;
    if old_len > MAX_VFS_PATH_BYTES || new_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    let expected_len = VFS_RENAME_REQUEST_HEADER_BYTES
        .checked_add(old_len)
        .and_then(|len| len.checked_add(new_len))
        .ok_or(IpcError::VfsBadPath)?;
    if expected_len != request_len {
        return Err(IpcError::VfsBadPath);
    }

    let old_requested =
        &request_bytes[VFS_RENAME_REQUEST_HEADER_BYTES..VFS_RENAME_REQUEST_HEADER_BYTES + old_len];
    let new_requested = &request_bytes[VFS_RENAME_REQUEST_HEADER_BYTES + old_len..expected_len];
    let old_canonical = resolve_process_vfs_path(old_requested)?;
    let new_canonical = resolve_process_vfs_path(new_requested)?;
    let old_path = old_canonical.as_bytes();
    let new_path = new_canonical.as_bytes();
    if vfs_path_is_read_only(old_path) || vfs_path_is_read_only(new_path) {
        return Err(IpcError::VfsPermission);
    }
    let (old_parent_path, _) = split_vfs_parent_child(old_path)?;
    let (new_parent_path, new_child_name) = split_vfs_parent_child(new_path)?;
    let new_child_name =
        VfsName::from_user_component(new_child_name).map_err(|_| IpcError::VfsBadPath)?;

    let cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    let old_available = resolve_vfs_root_authority(cap, old_parent_path)?;
    let new_available = resolve_vfs_root_authority(cap, new_parent_path)?;
    if old_available & capability::RIGHT_RENAME == 0
        || new_available & capability::RIGHT_RENAME == 0
    {
        return Err(IpcError::VfsPermission);
    }

    let old_parent = runtime()
        .vfs_node_by_path(old_parent_path)
        .ok_or(IpcError::VfsNotFound)?;
    let new_parent = runtime()
        .vfs_node_by_path(new_parent_path)
        .ok_or(IpcError::VfsNotFound)?;
    if !matches!(old_parent.kind, VfsNodeKind::Directory)
        || !matches!(new_parent.kind, VfsNodeKind::Directory)
    {
        return Err(IpcError::VfsNotDirectory);
    }
    let node = runtime()
        .vfs_node_by_path(old_path)
        .ok_or(IpcError::VfsNotFound)?;
    let VfsBacking::MemoryFile(_) = node.backing else {
        return Err(IpcError::VfsUnsupported);
    };
    let old_mount = runtime()
        .objects
        .get_vfs_mount_by_path(old_path)
        .ok_or(IpcError::VfsUnsupported)?;
    let new_mount = runtime()
        .objects
        .get_vfs_mount_by_path(new_parent_path)
        .ok_or(IpcError::VfsUnsupported)?;
    if old_mount.id != new_mount.id {
        return Err(IpcError::VfsUnsupported);
    }
    if runtime().vfs_node_by_path(new_path).is_some() {
        return Err(IpcError::VfsExists);
    }

    runtime().rename_vfs_node(node.id, new_parent.id, new_child_name)?;
    runtime().record_vfs_event(new_parent.id, VFS_EVENT_RENAME, new_child_name);

    serial::write_str("VFS rename accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" old=");
    serial::write_ascii_bytes(old_requested);
    serial::write_str(" new=");
    serial::write_ascii_bytes(new_requested);
    serial::write_str(" canonical_old=");
    serial::write_ascii_bytes(old_path);
    serial::write_str(" canonical_new=");
    serial::write_ascii_bytes(new_path);
    serial::write_str(" vnode=");
    serial::write_u64_dec(node.id.raw());
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_link(cap_slot: u64, request: *const u8, request_len: usize) -> Result<(), IpcError> {
    if !(VFS_RENAME_REQUEST_HEADER_BYTES..=VFS_RENAME_REQUEST_MAX_BYTES).contains(&request_len) {
        return Err(IpcError::VfsBadPath);
    }
    let mut request_bytes = [0u8; VFS_RENAME_REQUEST_MAX_BYTES];
    usercopy::copy_from_user(
        &mut request_bytes,
        UserPtr::new(request as u64),
        request_len,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;
    let old_len =
        usize::try_from(read_u64_le(&request_bytes, 0)).map_err(|_| IpcError::VfsBadPath)?;
    let new_len =
        usize::try_from(read_u64_le(&request_bytes, 8)).map_err(|_| IpcError::VfsBadPath)?;
    if old_len > MAX_VFS_PATH_BYTES || new_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    let expected_len = VFS_RENAME_REQUEST_HEADER_BYTES
        .checked_add(old_len)
        .and_then(|len| len.checked_add(new_len))
        .ok_or(IpcError::VfsBadPath)?;
    if expected_len != request_len {
        return Err(IpcError::VfsBadPath);
    }

    let old_requested =
        &request_bytes[VFS_RENAME_REQUEST_HEADER_BYTES..VFS_RENAME_REQUEST_HEADER_BYTES + old_len];
    let new_requested = &request_bytes[VFS_RENAME_REQUEST_HEADER_BYTES + old_len..expected_len];
    let old_canonical = resolve_process_vfs_path(old_requested)?;
    let new_canonical = resolve_process_vfs_path(new_requested)?;
    let old_path = old_canonical.as_bytes();
    let new_path = new_canonical.as_bytes();
    if vfs_path_is_read_only(new_path) {
        return Err(IpcError::VfsPermission);
    }
    let (old_parent_path, _) = split_vfs_parent_child(old_path)?;
    let (new_parent_path, new_child_name) = split_vfs_parent_child(new_path)?;
    let new_child_name =
        VfsName::from_user_component(new_child_name).map_err(|_| IpcError::VfsBadPath)?;

    let cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    resolve_vfs_root_authority(cap, old_parent_path)?;
    let new_available = resolve_vfs_root_authority(cap, new_parent_path)?;
    if new_available & capability::RIGHT_CREATE == 0 {
        return Err(IpcError::VfsPermission);
    }

    let node = runtime()
        .vfs_node_by_path(old_path)
        .ok_or(IpcError::VfsNotFound)?;
    let VfsBacking::MemoryFile(backing) = node.backing else {
        return Err(IpcError::VfsUnsupported);
    };
    let new_parent = runtime()
        .vfs_node_by_path(new_parent_path)
        .ok_or(IpcError::VfsNotFound)?;
    if !matches!(new_parent.kind, VfsNodeKind::Directory) {
        return Err(IpcError::VfsNotDirectory);
    }
    let old_mount = runtime()
        .objects
        .get_vfs_mount_by_path(old_path)
        .ok_or(IpcError::VfsUnsupported)?;
    let new_mount = runtime()
        .objects
        .get_vfs_mount_by_path(new_parent_path)
        .ok_or(IpcError::VfsUnsupported)?;
    if old_mount.id != new_mount.id {
        return Err(IpcError::VfsUnsupported);
    }
    if runtime().vfs_node_by_path(new_path).is_some() {
        return Err(IpcError::VfsExists);
    }

    let runtime = runtime();
    let new_node = runtime
        .add_vfs_node_with_name(
            new_child_name,
            Some(new_parent.id),
            VfsNodeKind::RegularFile,
            VfsBacking::MemoryFile(backing),
            node.mount_source,
        )
        .map_err(|_| IpcError::VfsNoSpace)?;
    runtime.touch_vfs_memory_file_nodes(backing)?;
    runtime.record_vfs_event(new_parent.id, VFS_EVENT_CREATE, new_child_name);

    serial::write_str("VFS link accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" old=");
    serial::write_ascii_bytes(old_requested);
    serial::write_str(" new=");
    serial::write_ascii_bytes(new_requested);
    serial::write_str(" vnode=");
    serial::write_u64_dec(node.id.raw());
    serial::write_str(" link_vnode=");
    serial::write_u64_dec(new_node.raw());
    serial::write_str("\n");
    Ok(())
}

pub fn vfs_derive_root(
    cap_slot: u64,
    path: *const u8,
    packed_len_target: u64,
) -> Result<(), IpcError> {
    let path_len = usize::try_from(packed_len_target & 0xffff_ffff).unwrap_or(usize::MAX);
    let target_slot = packed_len_target >> 32;
    if path_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    let mut path_bytes = [0u8; MAX_VFS_PATH_BYTES];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let requested_path = &path_bytes[..path_len];
    let root_path = resolve_process_vfs_path(requested_path)?;
    let source = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)
        .map_err(|_| IpcError::VfsPermission)?;
    let source_root = runtime()
        .objects
        .get_vfs_root(source.object)
        .ok_or(IpcError::VfsPermission)?;
    if !vfs_authority_path_covers(source_root.root_path.as_bytes(), root_path.as_bytes()) {
        return Err(IpcError::VfsPermission);
    }
    let node = runtime()
        .vfs_node_by_path(root_path.as_bytes())
        .ok_or(IpcError::VfsNotFound)?;
    if !matches!(node.kind, VfsNodeKind::Directory) {
        return Err(IpcError::VfsNotDirectory);
    }

    let process_name = current_process_name();
    let runtime = runtime();
    let owner = runtime
        .processes
        .current_process()
        .map(|process| process.pid)
        .ok_or(IpcError::VfsPermission)?;
    {
        let Some(process) = runtime.processes.current_process() else {
            return Err(IpcError::VfsPermission);
        };
        if !process.caps.can_grant(target_slot) {
            return Err(IpcError::VfsBadHandle);
        }
    }
    if runtime.next_cap_id == 0
        || runtime.next_cap_id == u64::MAX
        || runtime.cap_lineage_count == runtime.cap_lineage.len()
    {
        return Err(IpcError::VfsNoSpace);
    }
    let object = runtime.objects.add_derived_vfs_root(root_path)?;
    let cap = runtime.new_capability(
        object,
        source.rights & vfs_file_right_mask(),
        owner,
        source.id,
        owner,
    )?;
    let Some(process) = runtime.processes.current_process_mut() else {
        return Err(IpcError::VfsPermission);
    };
    process
        .caps
        .grant(target_slot, cap)
        .map_err(|_| IpcError::VfsBadHandle)?;

    serial::write_str("VFS root derive accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" source=");
    serial::write_u64_dec(cap_slot);
    serial::write_str(" target=");
    serial::write_u64_dec(target_slot);
    serial::write_str(" root=");
    serial::write_ascii_bytes(root_path.as_bytes());
    serial::write_str(" rights=");
    print_rights(cap.rights);
    serial::write_str("\n");
    Ok(())
}

fn current_file_handle(handle: u64) -> Result<FileHandle, IpcError> {
    let runtime = runtime();
    let process = runtime
        .processes
        .current_process()
        .ok_or(IpcError::VfsPermission)?;
    let (_, file) = process.file_handle(handle)?;
    Ok(file)
}

fn current_open_file(handle: u64) -> Result<(OpenFileDescription, VfsNode), IpcError> {
    let file = current_file_handle(handle)?;
    let description = runtime()
        .file_description(file.description)
        .ok_or(IpcError::VfsBadHandle)?;
    let node = runtime()
        .vfs_node(description.node)
        .ok_or(IpcError::VfsBadHandle)?;
    Ok((description, node))
}

fn validate_vfs_absolute_path(path: &[u8]) -> Result<(), IpcError> {
    if path.len() < 2 || path[0] != b'/' || path[path.len() - 1] == b'/' {
        return Err(IpcError::VfsBadPath);
    }
    let mut start = 1;
    while start < path.len() {
        let mut end = start;
        while end < path.len() && path[end] != b'/' {
            if path[end] == 0 {
                return Err(IpcError::VfsBadPath);
            }
            end += 1;
        }
        if end == start
            || end - start > MAX_VFS_NAME_BYTES
            || &path[start..end] == b"."
            || &path[start..end] == b".."
        {
            return Err(IpcError::VfsBadPath);
        }
        start = end + 1;
    }
    Ok(())
}

fn valid_vfs_root_path(path: &[u8]) -> bool {
    if path.is_empty() || path[0] != b'/' || (path.len() > 1 && path[path.len() - 1] == b'/') {
        return false;
    }
    if path == b"/" {
        return true;
    }
    let mut start = 1;
    while start < path.len() {
        let mut end = start;
        while end < path.len() && path[end] != b'/' {
            if path[end] == 0 {
                return false;
            }
            end += 1;
        }
        if end == start
            || end - start > MAX_VFS_NAME_BYTES
            || &path[start..end] == b"."
            || &path[start..end] == b".."
        {
            return false;
        }
        start = end + 1;
    }
    true
}

fn valid_boot_process_mounts(process: BootProcessConfig) -> bool {
    if process.mount_count > MAX_BOOT_PROCESS_MOUNTS {
        return false;
    }
    let mut index = 0;
    while index < process.mount_count {
        let Some(mount) = process.mounts[index] else {
            return false;
        };
        if !valid_vfs_root_path(mount.path.as_bytes())
            || !valid_vfs_root_path(mount.source.as_bytes())
            || mount.path == "/"
            || mount.flags & !known_boot_process_mount_flags() != 0
            || mount.flags & BOOT_PROCESS_MOUNT_BIND == 0
        {
            return false;
        }
        let mut prior = 0;
        while prior < index {
            let Some(existing) = process.mounts[prior] else {
                return false;
            };
            if existing.path == mount.path {
                return false;
            }
            prior += 1;
        }
        index += 1;
    }
    true
}

fn known_boot_process_mount_flags() -> u16 {
    BOOT_PROCESS_MOUNT_BIND | BOOT_PROCESS_MOUNT_READ_ONLY
}

fn split_vfs_parent_child(path: &[u8]) -> Result<(&[u8], &[u8]), IpcError> {
    validate_vfs_absolute_path(path)?;
    let mut slash = path.len();
    while slash > 0 {
        slash -= 1;
        if path[slash] == b'/' {
            break;
        }
    }
    if slash == 0 {
        Ok((&path[..1], &path[1..]))
    } else {
        Ok((&path[..slash], &path[slash + 1..]))
    }
}

fn current_process_mount_root() -> Result<VfsPath, IpcError> {
    runtime()
        .processes
        .current_process()
        .map(|process| process.mount_root)
        .ok_or(IpcError::VfsPermission)
}

fn resolve_process_vfs_path(path: &[u8]) -> Result<VfsPath, IpcError> {
    if path == b"/" {
        let root = current_process_mount_root()?;
        if root.as_bytes() != b"/" {
            serial::write_str("VFS namespace root resolved: proc=");
            serial::write_str(current_process_name());
            serial::write_str(" root=");
            serial::write_ascii_bytes(root.as_bytes());
            serial::write_str("\n");
        }
        return Ok(root);
    }
    validate_vfs_absolute_path(path)?;
    let root = current_process_mount_root()?;
    if root.as_bytes() == b"/" {
        return VfsPath::from_root_path(path);
    }
    let combined_len = root
        .len
        .checked_add(path.len())
        .ok_or(IpcError::VfsBadPath)?;
    if combined_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    let mut resolved = VfsPath::empty();
    let mut index = 0;
    while index < root.len {
        resolved.bytes[index] = root.bytes[index];
        index += 1;
    }
    let mut path_index = 0;
    while path_index < path.len() {
        resolved.bytes[index] = path[path_index];
        index += 1;
        path_index += 1;
    }
    resolved.len = combined_len;
    Ok(resolved)
}

fn resolve_vfs_path_under_root(root: VfsPath, path: &[u8]) -> Result<VfsPath, IpcError> {
    if path == b"/" {
        return Ok(root);
    }
    validate_vfs_absolute_path(path)?;
    if root.as_bytes() == b"/" {
        return VfsPath::from_root_path(path);
    }
    let combined_len = root
        .len
        .checked_add(path.len())
        .ok_or(IpcError::VfsBadPath)?;
    if combined_len > MAX_VFS_PATH_BYTES {
        return Err(IpcError::VfsBadPath);
    }
    let mut resolved = VfsPath::empty();
    let mut index = 0;
    while index < root.len {
        resolved.bytes[index] = root.bytes[index];
        index += 1;
    }
    let mut path_index = 0;
    while path_index < path.len() {
        resolved.bytes[index] = path[path_index];
        index += 1;
        path_index += 1;
    }
    resolved.len = combined_len;
    Ok(resolved)
}

fn vfs_request_path_is_read_only(path: &[u8]) -> Result<bool, IpcError> {
    let canonical_path = if path.is_empty() {
        resolve_process_vfs_path(b"/")?
    } else {
        resolve_process_vfs_path(path)?
    };
    Ok(vfs_path_is_read_only(canonical_path.as_bytes()))
}

fn vfs_path_is_read_only(path: &[u8]) -> bool {
    runtime()
        .objects
        .get_vfs_mount_by_path(path)
        .is_some_and(|mount| mount.flags & VFS_MOUNT_READ_ONLY != 0)
}

fn resolve_vfs_root_authority(cap: Capability, path: &[u8]) -> Result<u64, IpcError> {
    let root = runtime()
        .objects
        .get_vfs_root(cap.object)
        .ok_or(IpcError::VfsPermission)?;
    if !vfs_authority_path_covers(root.root_path.as_bytes(), path) {
        return Err(IpcError::VfsPermission);
    }
    let available = cap.rights & vfs_file_right_mask();
    if available & capability::RIGHT_RESOLVE == 0 {
        return Err(IpcError::VfsPermission);
    }
    Ok(available)
}

fn vfs_create_memory_file_node(
    cap: Capability,
    path: &[u8],
    required_file_rights: u64,
) -> Result<(VfsNode, u64), IpcError> {
    let canonical_path = resolve_process_vfs_path(path)?;
    let path = canonical_path.as_bytes();
    let (parent_path, child_name) = split_vfs_parent_child(path)?;
    let child_name = VfsName::from_user_component(child_name).map_err(|_| IpcError::VfsBadPath)?;
    let available = resolve_vfs_root_authority(cap, parent_path)?;
    if available & capability::RIGHT_CREATE == 0 || required_file_rights & !available != 0 {
        return Err(IpcError::VfsPermission);
    }

    let parent = runtime()
        .vfs_node_by_path(parent_path)
        .ok_or(IpcError::VfsNotFound)?;
    if !matches!(parent.kind, VfsNodeKind::Directory) {
        return Err(IpcError::VfsNotDirectory);
    }
    if runtime().vfs_node_by_path(path).is_some() {
        return Err(IpcError::VfsExists);
    }

    let runtime = runtime();
    let node_id = if parent.mount_source == "vertexfs" {
        let parent_inode = vertexfs_directory_inode_for_node(parent)?;
        let backing = runtime.add_empty_vertexfs_file(child_name, parent_inode)?;
        match runtime.add_vfs_node_with_name(
            child_name,
            Some(parent.id),
            VfsNodeKind::RegularFile,
            VfsBacking::VertexFsFile(backing),
            parent.mount_source,
        ) {
            Ok(node_id) => node_id,
            Err(_) => {
                let _ = runtime.release_vertexfs_file(backing);
                return Err(IpcError::VfsNoSpace);
            }
        }
    } else {
        let backing = runtime
            .add_vfs_empty_memory_file()
            .map_err(|_| IpcError::VfsNoSpace)?;
        match runtime.add_vfs_node_with_name(
            child_name,
            Some(parent.id),
            VfsNodeKind::RegularFile,
            VfsBacking::MemoryFile(backing),
            parent.mount_source,
        ) {
            Ok(node_id) => node_id,
            Err(_) => {
                let _ = runtime.release_vfs_memory_file(backing);
                return Err(IpcError::VfsNoSpace);
            }
        }
    };
    let node = runtime.vfs_node(node_id).ok_or(IpcError::VfsBadHandle)?;
    Ok((node, available))
}

fn vfs_create_directory_node(cap: Capability, path: &[u8]) -> Result<VfsNode, IpcError> {
    let canonical_path = resolve_process_vfs_path(path)?;
    let path = canonical_path.as_bytes();
    let (parent_path, child_name) = split_vfs_parent_child(path)?;
    let child_name = VfsName::from_user_component(child_name).map_err(|_| IpcError::VfsBadPath)?;
    let available = resolve_vfs_root_authority(cap, parent_path)?;
    if available & capability::RIGHT_CREATE == 0 {
        return Err(IpcError::VfsPermission);
    }

    let parent = runtime()
        .vfs_node_by_path(parent_path)
        .ok_or(IpcError::VfsNotFound)?;
    if !matches!(parent.kind, VfsNodeKind::Directory) {
        return Err(IpcError::VfsNotDirectory);
    }
    if runtime().vfs_node_by_path(path).is_some() {
        return Err(IpcError::VfsExists);
    }

    let runtime = runtime();
    let node_id = runtime
        .add_vfs_node_with_name(
            child_name,
            Some(parent.id),
            VfsNodeKind::Directory,
            VfsBacking::None,
            parent.mount_source,
        )
        .map_err(|_| IpcError::VfsNoSpace)?;
    runtime.vfs_node(node_id).ok_or(IpcError::VfsBadHandle)
}

fn vertexfs_directory_inode_for_node(node: VfsNode) -> Result<u32, IpcError> {
    if node.mount_source != "vertexfs" {
        return Err(IpcError::VfsUnsupported);
    }
    if node.name.as_bytes() == b"app" {
        return Ok(VERTEXFS_INODE_APP_DIR);
    }
    Err(IpcError::VfsUnsupported)
}

fn release_created_vfs_memory_node(runtime: &mut RuntimeState, created_node: Option<VfsNode>) {
    let Some(node) = created_node else {
        return;
    };
    if runtime.vfs_node_has_open_description(node.id) {
        return;
    }
    match node.backing {
        VfsBacking::MemoryFile(backing) => {
            let _ = runtime.remove_vfs_node(node.id);
            let _ = runtime.release_vfs_memory_file(backing);
        }
        VfsBacking::VertexFsFile(backing) => {
            let _ = runtime.remove_vfs_node(node.id);
            let _ = runtime.release_vertexfs_file(backing);
        }
        _ => {}
    }
}

fn resolve_vfs_node_from_cap(cap: Capability, path: &[u8]) -> Result<(VfsNode, u64), IpcError> {
    if let Some(store_node) = runtime().vfs_node_for_store_object(cap.object) {
        if !path.is_empty() && path != b"." {
            return Err(IpcError::VfsBadPath);
        }
        return Ok((store_node, cap.rights & vfs_file_right_mask()));
    }
    if let Some(root) = runtime().objects.get_vfs_root(cap.object) {
        let canonical_path = if path.is_empty() {
            resolve_process_vfs_path(b"/")?
        } else {
            resolve_process_vfs_path(path)?
        };
        let path = canonical_path.as_bytes();
        if cap.rights & capability::RIGHT_RESOLVE == 0
            || !vfs_authority_path_covers(root.root_path.as_bytes(), path)
        {
            return Err(IpcError::VfsPermission);
        }
        let node = runtime()
            .vfs_node_by_path(path)
            .ok_or(IpcError::VfsNotFound)?;
        let available = cap.rights & vfs_file_right_mask();
        if available & capability::RIGHT_RESOLVE == 0 {
            return Err(IpcError::VfsPermission);
        }
        return Ok((node, available));
    }
    Err(IpcError::VfsPermission)
}

fn vfs_open_rights(flags: u64, node: VfsNode) -> Result<u64, IpcError> {
    match node.kind {
        VfsNodeKind::Directory => {
            if flags & (VFS_OPEN_WRITE | VFS_OPEN_CREATE | VFS_OPEN_TRUNC | VFS_OPEN_APPEND) != 0 {
                return Err(IpcError::VfsNotFile);
            }
            if flags & VFS_OPEN_READ == 0 {
                return Err(IpcError::VfsUnsupported);
            }
            Ok(capability::RIGHT_RESOLVE)
        }
        VfsNodeKind::DeviceNode => {
            if flags & (VFS_OPEN_CREATE | VFS_OPEN_TRUNC | VFS_OPEN_APPEND) != 0 {
                return Err(IpcError::VfsUnsupported);
            }
            if flags & (VFS_OPEN_READ | VFS_OPEN_WRITE) == 0 {
                return Err(IpcError::VfsUnsupported);
            }
            Ok(capability::RIGHT_CONTROL)
        }
        _ if matches!(node.backing, VfsBacking::StateVolumeControl(_)) => {
            if flags != VFS_OPEN_WRITE {
                return Err(IpcError::VfsUnsupported);
            }
            Ok(capability::RIGHT_CONTROL)
        }
        _ => vfs_regular_file_open_rights(flags),
    }
}

fn validate_vfs_device_open(
    flags: u64,
    available_rights: u64,
    device_object: KernelObjectId,
) -> Result<(), IpcError> {
    let mut path_rights = 0;
    if flags & VFS_OPEN_READ != 0 {
        path_rights |= capability::RIGHT_READ;
    }
    if flags & VFS_OPEN_WRITE != 0 {
        path_rights |= capability::RIGHT_WRITE;
    }
    if path_rights == 0 {
        return Err(IpcError::VfsUnsupported);
    }
    if path_rights & !available_rights != 0 {
        return Err(IpcError::VfsPermission);
    }
    if !current_process_has_live_cap_for_object(device_object, capability::RIGHT_CONTROL) {
        return Err(IpcError::VfsPermission);
    }
    Ok(())
}

fn current_process_has_live_cap_for_object(object: KernelObjectId, required_rights: u64) -> bool {
    let runtime = runtime();
    let Some(process) = runtime.processes.current_process() else {
        return false;
    };
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = process.caps.caps[slot]
            && cap.object == object
            && cap.rights & required_rights == required_rights
            && !cap.revoked
            && !runtime.cap_id_revoked(cap.id)
            && !capability_has_revoked_ancestor(runtime, cap)
            && cap.generation_id == runtime.generation_id
        {
            return true;
        }
        slot += 1;
    }
    false
}

fn vfs_regular_file_open_rights(flags: u64) -> Result<u64, IpcError> {
    let mut rights = 0;
    if flags & VFS_OPEN_READ != 0 {
        rights |= capability::RIGHT_READ;
    }
    if flags & VFS_OPEN_WRITE != 0 {
        rights |= capability::RIGHT_WRITE;
    }
    if rights == 0 {
        return Err(IpcError::VfsUnsupported);
    }
    Ok(rights)
}

fn vfs_file_right_mask() -> u64 {
    capability::RIGHT_READ
        | capability::RIGHT_WRITE
        | capability::RIGHT_CONTROL
        | capability::RIGHT_CREATE
        | capability::RIGHT_UNLINK
        | capability::RIGHT_RENAME
        | capability::RIGHT_MOUNT
        | capability::RIGHT_RESOLVE
        | capability::RIGHT_EXECUTE
        | capability::RIGHT_INSPECT_METADATA
}

fn vfs_poll_ready(
    description: OpenFileDescription,
    node: VfsNode,
    events: u64,
) -> Result<u64, IpcError> {
    let mut ready = 0;
    if events & VFS_POLL_READABLE != 0 {
        let readable = match node.kind {
            VfsNodeKind::Directory => runtime().vfs_node_has_children(node.id),
            _ if matches!(node.backing, VfsBacking::Pipe) => !runtime().vfs_pipe.is_empty(),
            _ => vfs_node_len(node)? > description.offset,
        };
        if readable {
            ready |= VFS_POLL_READABLE;
        }
    }
    if events & VFS_POLL_WRITABLE != 0 {
        let writable = match node.backing {
            VfsBacking::MemoryFile(_)
            | VfsBacking::VertexFsFile(_)
            | VfsBacking::StateVolumeValue(_)
            | VfsBacking::StateVolumeControl(_) => true,
            VfsBacking::Pipe => runtime().vfs_pipe.is_empty(),
            _ => false,
        };
        if writable {
            ready |= VFS_POLL_WRITABLE;
        }
    }
    if events & VFS_POLL_METADATA != 0 {
        let mut index = min(description.watch_cursor, runtime().vfs_event_count);
        while index < runtime().vfs_event_count {
            if let Some(event) = runtime().vfs_events[index]
                && event.parent == node.id
            {
                ready |= VFS_POLL_METADATA;
                break;
            }
            index += 1;
        }
    }
    Ok(ready)
}

fn write_vfs_watch_event_record(record: &mut [u8; VFS_WATCH_EVENT_BYTES], event: VfsEvent) {
    write_u64_le(record, 0, event.kind);
    write_u64_le(record, 8, event.metadata_version);
    write_u64_le(record, 16, event.name.len as u64);
    let name = event.name.as_bytes();
    let mut index = 0;
    while index < name.len() && 24 + index < record.len() {
        record[24 + index] = name[index];
        index += 1;
    }
}

fn vfs_read_node(
    node: VfsNode,
    offset: u64,
    destination: *mut u8,
    max_len: usize,
) -> Result<(usize, u64), IpcError> {
    match node.backing {
        VfsBacking::StoreObject(object_id) => {
            let object = runtime()
                .objects
                .get_store_object(object_id)
                .ok_or(IpcError::VfsBadHandle)?;
            let object_len = store_object_len(object)?;
            let start = min(usize::try_from(offset).unwrap_or(usize::MAX), object_len);
            let remaining = object_len - start;
            let copy_len = min(remaining, max_len);
            let bytes = store_object_bytes(object)?;
            usercopy::copy_to_user(
                UserPtr::new(destination as u64),
                &bytes[start..start + copy_len],
            )
            .map_err(|_| IpcError::InvalidUserBuffer)?;
            Ok((
                copy_len,
                offset
                    .checked_add(copy_len as u64)
                    .ok_or(IpcError::VfsUnsupported)?,
            ))
        }
        VfsBacking::MemoryFile(index) => {
            let runtime = runtime();
            if index >= runtime.vfs_mem_file_count {
                return Err(IpcError::VfsBadHandle);
            }
            let file = runtime.vfs_mem_files[index];
            let start = min(usize::try_from(offset).unwrap_or(usize::MAX), file.len);
            let remaining = file.len - start;
            let copy_len = min(remaining, max_len);
            usercopy::copy_to_user(
                UserPtr::new(destination as u64),
                &file.bytes[start..start + copy_len],
            )
            .map_err(|_| IpcError::InvalidUserBuffer)?;
            Ok((
                copy_len,
                offset
                    .checked_add(copy_len as u64)
                    .ok_or(IpcError::VfsUnsupported)?,
            ))
        }
        VfsBacking::VertexFsFile(index) => {
            let runtime = runtime();
            if index >= runtime.vertexfs_file_count {
                return Err(IpcError::VfsBadHandle);
            }
            let file = runtime.vertexfs_files[index];
            let start = min(usize::try_from(offset).unwrap_or(usize::MAX), file.len);
            let remaining = file.len - start;
            let copy_len = min(remaining, max_len);
            usercopy::copy_to_user(
                UserPtr::new(destination as u64),
                &file.bytes[start..start + copy_len],
            )
            .map_err(|_| IpcError::InvalidUserBuffer)?;
            Ok((
                copy_len,
                offset
                    .checked_add(copy_len as u64)
                    .ok_or(IpcError::VfsUnsupported)?,
            ))
        }
        VfsBacking::Synthetic(bytes) => {
            let start = min(usize::try_from(offset).unwrap_or(usize::MAX), bytes.len());
            let remaining = bytes.len() - start;
            let copy_len = min(remaining, max_len);
            usercopy::copy_to_user(
                UserPtr::new(destination as u64),
                &bytes[start..start + copy_len],
            )
            .map_err(|_| IpcError::InvalidUserBuffer)?;
            Ok((
                copy_len,
                offset
                    .checked_add(copy_len as u64)
                    .ok_or(IpcError::VfsUnsupported)?,
            ))
        }
        VfsBacking::None
        | VfsBacking::StateVolume(_)
        | VfsBacking::StateVolumeValue(_)
        | VfsBacking::StateVolumeControl(_)
        | VfsBacking::Device(_)
        | VfsBacking::FsServiceReport
        | VfsBacking::Pipe => Err(IpcError::VfsNotFile),
    }
}

fn vfs_write_node(node: VfsNode, offset: u64, bytes: &[u8]) -> Result<(usize, u64), IpcError> {
    match node.backing {
        VfsBacking::MemoryFile(index) => {
            if bytes.len() > MAX_VFS_MEM_FILE_BYTES {
                return Err(IpcError::VfsNoSpace);
            }
            let start = usize::try_from(offset).map_err(|_| IpcError::VfsNoSpace)?;
            let end = start.checked_add(bytes.len()).ok_or(IpcError::VfsNoSpace)?;
            if end > MAX_VFS_MEM_FILE_BYTES {
                return Err(IpcError::VfsNoSpace);
            }
            let runtime = runtime();
            if index >= runtime.vfs_mem_file_count {
                return Err(IpcError::VfsBadHandle);
            }
            {
                let file = &mut runtime.vfs_mem_files[index];
                let mut cursor = 0;
                while cursor < bytes.len() {
                    file.bytes[start + cursor] = bytes[cursor];
                    cursor += 1;
                }
                if end > file.len {
                    file.len = end;
                }
            }
            runtime.touch_vfs_memory_file_nodes(index)?;
            Ok((bytes.len(), end as u64))
        }
        VfsBacking::VertexFsFile(index) => {
            if bytes.len() > MAX_VERTEXFS_FILE_BYTES {
                return Err(IpcError::VfsNoSpace);
            }
            let start = usize::try_from(offset).map_err(|_| IpcError::VfsNoSpace)?;
            let end = start.checked_add(bytes.len()).ok_or(IpcError::VfsNoSpace)?;
            if end > MAX_VERTEXFS_FILE_BYTES {
                return Err(IpcError::VfsNoSpace);
            }
            let runtime = runtime();
            if index >= runtime.vertexfs_file_count {
                return Err(IpcError::VfsBadHandle);
            }
            {
                let file = &mut runtime.vertexfs_files[index];
                let mut cursor = 0;
                while cursor < bytes.len() {
                    file.bytes[start + cursor] = bytes[cursor];
                    cursor += 1;
                }
                if end > file.len {
                    file.len = end;
                }
                file.dirty = true;
                file.checksum = vertexfs_checksum32(&file.bytes[..file.len]);
            }
            runtime.touch_vertexfs_file_nodes(index)?;
            Ok((bytes.len(), end as u64))
        }
        VfsBacking::Pipe => {
            if bytes.len() > MAX_VFS_PIPE_BYTES {
                return Err(IpcError::VfsNoSpace);
            }
            if wake_blocked_vfs_pipe_read(bytes) {
                return Ok((bytes.len(), 0));
            }
            runtime().vfs_pipe.enqueue(bytes)?;
            Ok((bytes.len(), 0))
        }
        _ => Err(IpcError::VfsUnsupported),
    }
}

fn vfs_truncate_node(node: VfsNode, len: usize) -> Result<(), IpcError> {
    match node.backing {
        VfsBacking::MemoryFile(index) => {
            if len > MAX_VFS_MEM_FILE_BYTES {
                return Err(IpcError::VfsNoSpace);
            }
            let runtime = runtime();
            if index >= runtime.vfs_mem_file_count {
                return Err(IpcError::VfsBadHandle);
            }
            runtime.vfs_mem_files[index].len = len;
            runtime.touch_vfs_memory_file_nodes(index)?;
            Ok(())
        }
        VfsBacking::VertexFsFile(index) => {
            if len > MAX_VERTEXFS_FILE_BYTES {
                return Err(IpcError::VfsNoSpace);
            }
            let runtime = runtime();
            if index >= runtime.vertexfs_file_count {
                return Err(IpcError::VfsBadHandle);
            }
            runtime.vertexfs_files[index].len = len;
            runtime.vertexfs_files[index].dirty = true;
            runtime.vertexfs_files[index].checksum =
                vertexfs_checksum32(&runtime.vertexfs_files[index].bytes[..len]);
            runtime.touch_vertexfs_file_nodes(index)?;
            Ok(())
        }
        _ => Err(IpcError::VfsUnsupported),
    }
}

fn vfs_node_len(node: VfsNode) -> Result<u64, IpcError> {
    match node.backing {
        VfsBacking::StoreObject(object_id) => runtime()
            .objects
            .get_store_object(object_id)
            .map(|object| object.length)
            .ok_or(IpcError::VfsBadHandle),
        VfsBacking::MemoryFile(index) => {
            let runtime = runtime();
            if index >= runtime.vfs_mem_file_count {
                return Err(IpcError::VfsBadHandle);
            }
            Ok(runtime.vfs_mem_files[index].len as u64)
        }
        VfsBacking::VertexFsFile(index) => {
            let runtime = runtime();
            if index >= runtime.vertexfs_file_count {
                return Err(IpcError::VfsBadHandle);
            }
            Ok(runtime.vertexfs_files[index].len as u64)
        }
        VfsBacking::Synthetic(bytes) => Ok(bytes.len() as u64),
        VfsBacking::FsServiceReport => Ok(VFS_SERVICE_REPORT_BYTES.len() as u64),
        VfsBacking::Pipe => Ok(runtime().vfs_pipe.len as u64),
        VfsBacking::None
        | VfsBacking::StateVolume(_)
        | VfsBacking::StateVolumeValue(_)
        | VfsBacking::StateVolumeControl(_)
        | VfsBacking::Device(_) => Ok(0),
    }
}

fn vfs_node_kind_value(kind: VfsNodeKind) -> u64 {
    match kind {
        VfsNodeKind::RegularFile => VFS_NODE_KIND_REGULAR,
        VfsNodeKind::Directory => VFS_NODE_KIND_DIRECTORY,
        VfsNodeKind::DeviceNode => VFS_NODE_KIND_DEVICE,
        VfsNodeKind::Pipe => VFS_NODE_KIND_PIPE,
        VfsNodeKind::SyntheticNode => VFS_NODE_KIND_SYNTHETIC,
    }
}

fn serial_write_vfs_name(name: VfsName) {
    serial::write_ascii_bytes(name.as_bytes());
}

fn serial_write_vfs_mount_flags(flags: u64) {
    let mut wrote = false;
    if flags & VFS_MOUNT_VOLATILE != 0 {
        serial::write_str("volatile");
        wrote = true;
    }
    if flags & VFS_MOUNT_BIND != 0 {
        if wrote {
            serial::write_str("|");
        }
        serial::write_str("bind");
        wrote = true;
    }
    if flags & VFS_MOUNT_READ_ONLY != 0 {
        if wrote {
            serial::write_str("|");
        }
        serial::write_str("read-only");
        wrote = true;
    }
    if !wrote {
        serial::write_str("none");
    }
}

fn store_object_len(object: StoreObject) -> Result<usize, IpcError> {
    usize::try_from(object.length).map_err(|_| IpcError::MessageTooLarge)
}

fn store_object_bytes(object: StoreObject) -> Result<&'static [u8], IpcError> {
    let object_len = store_object_len(object)?;
    let bytes = unsafe { core::slice::from_raw_parts(object.base as *const u8, object_len) };
    if !store_hash_matches(bytes, object.hash) {
        if object.name.starts_with("config:") {
            serial::write_str("Krust native config hash mismatch: config=");
        } else {
            serial::write_str("Krust native store hash mismatch: object=");
        }
        serial::write_str(object.name);
        serial::write_str("\n");
        serial::write_str("vertex-inspect security event: store hash mismatch object=");
        serial::write_str(object.name);
        serial::write_str("\n");
        return Err(IpcError::BadCapability);
    }
    if object.name.starts_with("config:") {
        serial::write_str("Krust native config hash verified: config=");
        serial::write_str(object.name);
        serial::write_str("\n");
    }
    Ok(bytes)
}

fn write_u64_le(destination: &mut [u8], offset: usize, value: u64) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        destination[offset + index] = bytes[index];
        index += 1;
    }
}

fn write_u16_le(destination: &mut [u8], offset: usize, value: u16) {
    let bytes = value.to_le_bytes();
    destination[offset] = bytes[0];
    destination[offset + 1] = bytes[1];
}

fn write_u32_le(destination: &mut [u8], offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        destination[offset + index] = bytes[index];
        index += 1;
    }
}

fn read_u16_le(source: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([source[offset], source[offset + 1]])
}

fn read_u32_le(source: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        source[offset],
        source[offset + 1],
        source[offset + 2],
        source[offset + 3],
    ])
}

fn read_u64_le(source: &[u8], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    let mut index = 0;
    while index < bytes.len() {
        bytes[index] = source[offset + index];
        index += 1;
    }
    u64::from_le_bytes(bytes)
}

pub fn secret_read(cap_slot: u64, destination: *mut u8, max_len: usize) -> Result<usize, IpcError> {
    let secret = secret_from_cap(cap_slot, capability::RIGHT_READ)?;
    let copy_len = min(secret.value.len(), max_len);
    usercopy::copy_to_user(UserPtr::new(destination as u64), &secret.value[..copy_len])
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Secret read accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" secret=");
    serial::write_str(secret.name);
    serial::write_str(" bytes=<redacted>\n");
    serial::write_str("vertex-inspect security event: secret metadata access secret=");
    serial::write_str(secret.name);
    serial::write_str(" proc=");
    serial::write_str(current_process_name());
    serial::write_str("\n");
    Ok(copy_len)
}

pub fn virtio_device_probe(cap_slot: u64) -> Result<(), IpcError> {
    let device = virtio_device_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    if device.transport != VIRTIO_PCI_IO_TRANSPORT_ID {
        return Err(IpcError::BadCapability);
    }
    record_virtio_device_owner(device.id)?;
    serial::write_str("Virtio device probe accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" virtio-device=");
    serial::write_str(device.name);
    serial::write_str(" transport=");
    serial::write_str(device.transport);
    serial::write_str("\n");
    Ok(())
}

pub fn virtio_device_report(cap_slot: u64, source: *const u8, len: usize) -> Result<(), IpcError> {
    if len != VIRTIO_DRIVER_REPORT_BYTES {
        return Err(IpcError::InvalidUserBuffer);
    }
    let device = virtio_device_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    if device.transport != VIRTIO_PCI_IO_TRANSPORT_ID {
        return Err(IpcError::BadCapability);
    }

    let mut bytes = [0u8; VIRTIO_DRIVER_REPORT_BYTES];
    usercopy::copy_from_user(&mut bytes, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let queue_size = read_report_u64(&bytes, 0);
    if queue_size > VIRTIO_QUEUE_MAX_SIZE as u64 {
        return Err(IpcError::BadCapability);
    }
    let avail_idx = read_report_u64(&bytes, 8);
    let used_idx = read_report_u64(&bytes, 16);
    if avail_idx > u16::MAX as u64 || used_idx > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }
    let last_error = virtio_error_label(read_report_u64(&bytes, 56))?;

    let process = current_process_id();
    let process_name = current_process_name();
    let runtime = runtime();
    let Some(device) = runtime.objects.get_virtio_device_mut(device.id) else {
        return Err(IpcError::BadCapability);
    };
    if device.owner != ProcessId::empty() && device.owner != process {
        return Err(IpcError::BadCapability);
    }
    device.owner = process;
    device.queue_size = queue_size as u16;
    device.avail_idx = avail_idx as u16;
    device.used_idx = used_idx as u16;
    device.submissions = read_report_u64(&bytes, 24);
    device.completions = read_report_u64(&bytes, 32);
    device.timeouts = read_report_u64(&bytes, 40);
    device.reset_count = read_report_u64(&bytes, 48);
    device.last_error = last_error;

    serial::write_str("Virtio driver report accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" virtio-device=");
    serial::write_str(device.name);
    serial::write_str(" submissions=");
    serial::write_u64_dec(device.submissions);
    serial::write_str(" completions=");
    serial::write_u64_dec(device.completions);
    serial::write_str(" timeouts=");
    serial::write_u64_dec(device.timeouts);
    serial::write_str(" resets=");
    serial::write_u64_dec(device.reset_count);
    serial::write_str(" last_error=");
    serial::write_str(device.last_error);
    serial::write_str("\n");
    Ok(())
}

fn record_virtio_device_owner(device_id: KernelObjectId) -> Result<(), IpcError> {
    let process = current_process_id();
    let runtime = runtime();
    let Some(device) = runtime.objects.get_virtio_device_mut(device_id) else {
        return Err(IpcError::BadCapability);
    };
    if device.owner != ProcessId::empty() && device.owner != process {
        return Err(IpcError::BadCapability);
    }
    device.owner = process;
    Ok(())
}

fn virtio_error_label(code: u64) -> Result<&'static str, IpcError> {
    match code {
        VIRTIO_ERROR_NONE => Ok("none"),
        VIRTIO_ERROR_COMPLETION_TIMEOUT => Ok("completion-timeout"),
        VIRTIO_ERROR_RESET_FAILED => Ok("reset-failed"),
        VIRTIO_ERROR_INIT_FAILED => Ok("init-failed"),
        VIRTIO_ERROR_STATUS => Ok("status-error"),
        _ => Err(IpcError::BadCapability),
    }
}

fn read_report_u64(bytes: &[u8; VIRTIO_DRIVER_REPORT_BYTES], offset: usize) -> u64 {
    let mut value = 0u64;
    let mut index = 0;
    while index < 8 {
        value |= (bytes[offset + index] as u64) << (index * 8);
        index += 1;
    }
    value
}

pub fn virtio_rng_read(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
) -> Result<usize, IpcError> {
    let device = virtio_device_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    if !virtio_device_is(device, VIRTIO_RNG_DEVICE_ID) {
        return Err(IpcError::BadCapability);
    }
    let copy_len = min(max_len, 32);
    usercopy::validate_user_buffer(
        UserPtr::new(destination as u64),
        copy_len,
        paging::UserAccess::Write,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;
    if copy_len == 0 {
        return Ok(0);
    }
    let mut bytes = [0u8; 32];
    let actual_len = virtio_rng_fill(&mut bytes[..copy_len])?;
    usercopy::copy_to_user(UserPtr::new(destination as u64), &bytes[..actual_len])
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Virtio RNG read accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" virtio-device=");
    serial::write_str(device.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(actual_len as u64);
    serial::write_str("\n");
    Ok(actual_len)
}

pub fn virtio_net_tx(cap_slot: u64, source: *const u8, len: usize) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }
    let device = virtio_device_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    if !virtio_device_is(device, VIRTIO_NET_DEVICE_ID) {
        return Err(IpcError::BadCapability);
    }
    let mut frame = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut frame, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    virtio_net_send_frame(&frame[..len])?;

    serial::write_str("Virtio net TX completed: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" virtio-device=");
    serial::write_str(device.name);
    serial::write_str(" frame-bytes=");
    serial::write_u64_dec(len as u64);
    serial::write_str("\n");
    Ok(())
}

pub fn virtio_net_rx(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
) -> Result<usize, IpcError> {
    let device = virtio_device_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    if !virtio_device_is(device, VIRTIO_NET_DEVICE_ID) {
        return Err(IpcError::BadCapability);
    }
    if max_len < MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }
    usercopy::validate_user_buffer(
        UserPtr::new(destination as u64),
        MAX_MESSAGE_BYTES,
        paging::UserAccess::Write,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;
    let mut frame = [0u8; MAX_MESSAGE_BYTES];
    let frame_len = virtio_net_receive_frame(&mut frame)?;
    usercopy::copy_to_user(UserPtr::new(destination as u64), &frame[..frame_len])
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Virtio net RX completed: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" virtio-device=");
    serial::write_str(device.name);
    serial::write_str(" frame-bytes=");
    serial::write_u64_dec(frame_len as u64);
    serial::write_str("\n");
    Ok(frame_len)
}

pub fn network_send_udp(cap_slot: u64, source: *const u8, len: usize) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }
    let port = network_port_from_cap(cap_slot, capability::RIGHT_BIND | capability::RIGHT_LISTEN)?;
    let mut payload = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut payload, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let sender = runtime()
        .processes
        .current_process()
        .map(|process| process.pid)
        .ok_or(IpcError::BadCapability)?;
    {
        let runtime = runtime();
        let Some(port) = runtime.objects.get_network_port_mut(port.id) else {
            return Err(IpcError::BadCapability);
        };
        port.enqueue_udp(sender, &payload, len)?;
    }

    serial::write_str("UDP send queued for netstack: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" network-port=");
    serial::write_str(port.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(len as u64);
    serial::write_str("\n");
    serial::write_str("network-port bind/listen rights enforced by netstack boundary\n");
    wake_blocked_network_receiver(port.id);
    Ok(())
}

pub fn network_recv_udp(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let port = network_port_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    usercopy::validate_user_buffer(
        UserPtr::new(destination as u64),
        max_len,
        paging::UserAccess::Write,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;

    let message = {
        let runtime = runtime();
        let Some(port) = runtime.objects.get_network_port_mut(port.id) else {
            return Err(IpcError::BadCapability);
        };
        port.dequeue_udp()
    };

    let Some(message) = message else {
        if block_current_on_network_port(port.id, destination as u64, max_len, frame) {
            return Ok(());
        }
        return Err(IpcError::Empty);
    };

    let copy_len = min(message.len, max_len);
    usercopy::copy_to_user(UserPtr::new(destination as u64), &message.bytes[..copy_len])
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Network-port UDP request delivered to netstack: network-port=");
    serial::write_str(port.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(copy_len as u64);
    serial::write_str("\n");
    frame.rax = copy_len as u64;
    Ok(())
}

fn virtio_rng_fill(destination: &mut [u8]) -> Result<usize, IpcError> {
    if destination.is_empty() {
        return Ok(0);
    }

    let state = virtio_rng_state()?;
    let data = queue_data_virtual(&state.queue);
    zero_dma(data, destination.len());
    let used_len = match virtio_submit_single(
        state.io_base,
        0,
        &mut state.queue,
        destination.len() as u32,
        true,
    ) {
        Ok(used_len) => used_len,
        Err(error) => {
            if error == IpcError::Empty {
                reset_virtio_rng_state(state, "rng-timeout");
            }
            return Err(error);
        }
    };
    let actual_len = min(destination.len(), used_len as usize);
    read_dma_bytes(data, &mut destination[..actual_len]);
    Ok(actual_len)
}

fn virtio_net_send_frame(frame: &[u8]) -> Result<(), IpcError> {
    let state = virtio_net_state()?;
    virtio_net_send_frame_locked(state, frame)
}

fn virtio_net_receive_frame(destination: &mut [u8]) -> Result<usize, IpcError> {
    let state = virtio_net_state()?;
    if !state.rx_posted {
        virtio_net_post_rx_buffer(state)?;
    }

    serial::write_str("virtio-net RX waits for interrupt-backed completion\n");
    let used_len = match virtio_wait_used(state.io_base, &mut state.rx) {
        Ok(used_len) => used_len,
        Err(error) => {
            if error == IpcError::Empty {
                reset_virtio_net_state(state, "net-rx-timeout");
            }
            return Err(error);
        }
    };
    state.rx_posted = false;
    if used_len as usize <= VIRTIO_NET_HDR_LEN {
        virtio_net_post_rx_buffer(state)?;
        return Err(IpcError::BadCapability);
    }

    let frame_len = (used_len as usize) - VIRTIO_NET_HDR_LEN;
    if frame_len > destination.len() {
        virtio_net_post_rx_buffer(state)?;
        return Err(IpcError::MessageTooLarge);
    }

    read_dma_bytes(
        queue_data_virtual(&state.rx) + VIRTIO_NET_HDR_LEN as u64,
        &mut destination[..frame_len],
    );
    virtio_net_post_rx_buffer(state)?;
    Ok(frame_len)
}

fn virtio_device_is(device: VirtioDeviceObject, expected_name: &str) -> bool {
    device.name == expected_name && device.transport == VIRTIO_PCI_IO_TRANSPORT_ID
}

fn virtio_rng_state() -> Result<&'static mut VirtioRngState, IpcError> {
    let state = unsafe { &mut *VIRTIO_RNG_STATE.0.get() };
    if !state.initialized {
        init_virtio_rng(state)?;
    }
    Ok(state)
}

fn virtio_net_state() -> Result<&'static mut VirtioNetState, IpcError> {
    let state = unsafe { &mut *VIRTIO_NET_STATE.0.get() };
    if !state.initialized {
        init_virtio_net(state)?;
    }
    Ok(state)
}

fn init_virtio_rng(state: &mut VirtioRngState) -> Result<(), IpcError> {
    let io_base = discover_virtio_pci_io_device(PCI_DEVICE_VIRTIO_RNG_IO_TRANSPORT)?;
    let (dma_physical, dma_virtual) = if state.queue.dma_physical == 0 {
        allocate_virtio_dma(VIRTIO_RNG_DMA_FRAMES)?
    } else {
        (state.queue.dma_physical, state.queue.dma_virtual)
    };
    let mut queue = VirtioQueueState::new(dma_physical, dma_virtual);
    let reset_count = state.reset_count;
    let last_error = state.last_error;

    virtio_write8(io_base, VIRTIO_PCI_STATUS, 0);
    virtio_write8(io_base, VIRTIO_PCI_STATUS, VIRTIO_STATUS_ACKNOWLEDGE);
    virtio_write8(
        io_base,
        VIRTIO_PCI_STATUS,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER,
    );
    let _features = virtio_read32(io_base, VIRTIO_PCI_HOST_FEATURES);
    virtio_write32(io_base, VIRTIO_PCI_GUEST_FEATURES, 0);
    if virtio_setup_queue(io_base, 0, &mut queue).is_err() {
        virtio_write8(io_base, VIRTIO_PCI_STATUS, VIRTIO_STATUS_FAILED);
        return Err(IpcError::BadCapability);
    }
    virtio_write8(
        io_base,
        VIRTIO_PCI_STATUS,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_DRIVER_OK,
    );

    *state = VirtioRngState {
        initialized: true,
        io_base,
        queue,
        owner: current_process_id(),
        reset_count,
        last_error,
    };
    serial::write_str("Virtio RNG PCI queue initialized\n");
    Ok(())
}

fn init_virtio_net(state: &mut VirtioNetState) -> Result<(), IpcError> {
    let io_base = discover_virtio_pci_io_device(PCI_DEVICE_VIRTIO_NET_IO_TRANSPORT)?;
    let (dma_physical, dma_virtual) = if state.rx.dma_physical == 0 {
        allocate_virtio_dma(VIRTIO_NET_DMA_FRAMES)?
    } else {
        (state.rx.dma_physical, state.rx.dma_virtual)
    };
    let mut rx = VirtioQueueState::new(dma_physical, dma_virtual);
    let mut tx = VirtioQueueState::new(
        dma_physical + VIRTIO_QUEUE_STRIDE as u64,
        dma_virtual + VIRTIO_QUEUE_STRIDE as u64,
    );
    let reset_count = state.reset_count;
    let last_error = state.last_error;

    virtio_write8(io_base, VIRTIO_PCI_STATUS, 0);
    virtio_write8(io_base, VIRTIO_PCI_STATUS, VIRTIO_STATUS_ACKNOWLEDGE);
    virtio_write8(
        io_base,
        VIRTIO_PCI_STATUS,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER,
    );
    let _features = virtio_read32(io_base, VIRTIO_PCI_HOST_FEATURES);
    virtio_write32(io_base, VIRTIO_PCI_GUEST_FEATURES, 0);
    if virtio_setup_queue(io_base, 0, &mut rx).is_err()
        || virtio_setup_queue(io_base, 1, &mut tx).is_err()
    {
        virtio_write8(io_base, VIRTIO_PCI_STATUS, VIRTIO_STATUS_FAILED);
        return Err(IpcError::BadCapability);
    }
    virtio_write8(
        io_base,
        VIRTIO_PCI_STATUS,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_DRIVER_OK,
    );

    *state = VirtioNetState {
        initialized: true,
        io_base,
        rx,
        tx,
        rx_posted: false,
        owner: current_process_id(),
        reset_count,
        last_error,
    };
    virtio_net_post_rx_buffer(state)?;
    serial::write_str("Virtio net PCI queues initialized\n");
    Ok(())
}

fn reset_virtio_rng_state(state: &mut VirtioRngState, reason: &'static str) {
    if state.io_base != 0 {
        virtio_write8(state.io_base, VIRTIO_PCI_STATUS, 0);
    }
    state.initialized = false;
    state.reset_count = state.reset_count.saturating_add(1);
    state.last_error = reason;
    state.queue.last_error = reason;
    serial::write_str("Virtio RNG reset after error: reason=");
    serial::write_str(reason);
    serial::write_str(" resets=");
    serial::write_u64_dec(state.reset_count);
    serial::write_str("\n");
}

fn reset_virtio_net_state(state: &mut VirtioNetState, reason: &'static str) {
    if state.io_base != 0 {
        virtio_write8(state.io_base, VIRTIO_PCI_STATUS, 0);
    }
    state.initialized = false;
    state.rx_posted = false;
    state.reset_count = state.reset_count.saturating_add(1);
    state.last_error = reason;
    state.rx.last_error = reason;
    state.tx.last_error = reason;
    serial::write_str("Virtio net reset after error: reason=");
    serial::write_str(reason);
    serial::write_str(" resets=");
    serial::write_u64_dec(state.reset_count);
    serial::write_str("\n");
}

fn virtio_setup_queue(
    io_base: u16,
    queue_index: u16,
    queue: &mut VirtioQueueState,
) -> Result<(), IpcError> {
    virtio_write16(io_base, VIRTIO_PCI_QUEUE_SEL, queue_index);
    let queue_size = virtio_read16(io_base, VIRTIO_PCI_QUEUE_NUM);
    if !(VIRTIO_QUEUE_MIN_SIZE..=VIRTIO_QUEUE_MAX_SIZE).contains(&queue_size) {
        return Err(IpcError::BadCapability);
    }
    let avail_offset = VIRTIO_QUEUE_DESC_OFFSET + queue_size as usize * 16;
    let used_offset = align_up_usize(
        avail_offset + 6 + queue_size as usize * 2,
        VIRTIO_QUEUE_RING_ALIGN,
    );
    let data_offset = align_up_usize(
        used_offset + 6 + queue_size as usize * 8,
        VIRTIO_QUEUE_RING_ALIGN,
    );
    if data_offset >= VIRTIO_QUEUE_STRIDE {
        return Err(IpcError::BadCapability);
    }
    queue.queue_size = queue_size;
    queue.avail_offset = avail_offset;
    queue.used_offset = used_offset;
    queue.data_offset = data_offset;
    queue.avail_idx = 0;
    queue.used_idx = 0;

    zero_dma(queue.dma_virtual, VIRTIO_QUEUE_STRIDE);
    write_dma_u16(
        queue.dma_virtual + queue.avail_offset as u64,
        VIRTIO_AVAIL_F_NO_INTERRUPT,
    );
    virtio_write32(
        io_base,
        VIRTIO_PCI_QUEUE_PFN,
        (queue.dma_physical >> 12) as u32,
    );
    Ok(())
}

fn virtio_net_post_rx_buffer(state: &mut VirtioNetState) -> Result<(), IpcError> {
    zero_dma(queue_data_virtual(&state.rx), VIRTIO_NET_RX_BUFFER_LEN);
    virtio_post_single(
        state.io_base,
        0,
        &mut state.rx,
        VIRTIO_NET_RX_BUFFER_LEN as u32,
        true,
    )?;
    state.rx_posted = true;
    Ok(())
}

fn virtio_net_send_frame_locked(state: &mut VirtioNetState, frame: &[u8]) -> Result<(), IpcError> {
    if frame.len() > MAX_MESSAGE_BYTES + UDP_IPV4_HEADER_LEN {
        return Err(IpcError::MessageTooLarge);
    }
    let payload_len = if frame.len() < ETHERNET_MIN_FRAME_LEN {
        ETHERNET_MIN_FRAME_LEN
    } else {
        frame.len()
    };
    let total_len = payload_len + VIRTIO_NET_HDR_LEN;
    if total_len > VIRTIO_NET_RX_BUFFER_LEN {
        return Err(IpcError::MessageTooLarge);
    }

    let data = queue_data_virtual(&state.tx);
    let data_physical = queue_data_physical(&state.tx);
    zero_dma(data, total_len);
    write_dma_bytes(data + VIRTIO_NET_HDR_LEN as u64, frame);
    write_virtio_desc(
        &state.tx,
        0,
        data_physical,
        VIRTIO_NET_HDR_LEN as u32,
        VIRTIO_DESC_F_NEXT,
        1,
    );
    write_virtio_desc(
        &state.tx,
        1,
        data_physical + VIRTIO_NET_HDR_LEN as u64,
        payload_len as u32,
        0,
        0,
    );
    virtio_kick_queue_head(state.io_base, 1, &mut state.tx, 0);
    if let Err(error) = virtio_wait_used(state.io_base, &mut state.tx) {
        if error == IpcError::Empty {
            reset_virtio_net_state(state, "net-tx-timeout");
        }
        return Err(error);
    }
    Ok(())
}

fn virtio_submit_single(
    io_base: u16,
    queue_index: u16,
    queue: &mut VirtioQueueState,
    data_len: u32,
    writable: bool,
) -> Result<u32, IpcError> {
    virtio_post_single(io_base, queue_index, queue, data_len, writable)?;
    virtio_wait_used(io_base, queue)
}

fn virtio_post_single(
    io_base: u16,
    queue_index: u16,
    queue: &mut VirtioQueueState,
    data_len: u32,
    writable: bool,
) -> Result<(), IpcError> {
    let flags = if writable { VIRTIO_DESC_F_WRITE } else { 0 };
    write_virtio_desc(queue, 0, queue_data_physical(queue), data_len, flags, 0);
    virtio_kick_queue_head(io_base, queue_index, queue, 0);
    Ok(())
}

fn virtio_kick_queue_head(io_base: u16, queue_index: u16, queue: &mut VirtioQueueState, head: u16) {
    let ring_offset = queue.avail_offset + 4 + ((queue.avail_idx % queue.queue_size) as usize * 2);
    write_dma_u16(queue.dma_virtual + ring_offset as u64, head);
    queue.avail_idx = queue.avail_idx.wrapping_add(1);
    queue.submissions = queue.submissions.saturating_add(1);
    compiler_fence(Ordering::SeqCst);
    write_dma_u16(
        queue.dma_virtual + queue.avail_offset as u64 + 2,
        queue.avail_idx,
    );
    compiler_fence(Ordering::SeqCst);
    virtio_write16(io_base, VIRTIO_PCI_QUEUE_NOTIFY, queue_index);
}

fn virtio_wait_used(io_base: u16, queue: &mut VirtioQueueState) -> Result<u32, IpcError> {
    let target_used = queue.used_idx.wrapping_add(1);
    let mut spins = 0u64;
    while read_dma_u16(queue.dma_virtual + queue.used_offset as u64 + 2) != target_used {
        spins += 1;
        if spins > VIRTIO_POLL_SPINS {
            queue.timeouts = queue.timeouts.saturating_add(1);
            queue.last_error = "completion-timeout";
            return Err(IpcError::Empty);
        }
        if spins & 0xffff == 0 {
            queue.interrupt_waits = queue.interrupt_waits.saturating_add(1);
            timer::wait_for_interrupt();
        } else if spins & 0xfff == 0 {
            pause_cpu();
        }
    }
    compiler_fence(Ordering::SeqCst);
    let used_offset = queue.used_offset + 4 + ((queue.used_idx % queue.queue_size) as usize * 8);
    let used_len = read_dma_u32(queue.dma_virtual + used_offset as u64 + 4);
    queue.used_idx = target_used;
    queue.completions = queue.completions.saturating_add(1);
    let _isr = virtio_read8(io_base, VIRTIO_PCI_ISR);
    Ok(used_len)
}

fn queue_data_virtual(queue: &VirtioQueueState) -> u64 {
    queue.dma_virtual + queue.data_offset as u64
}

fn queue_data_physical(queue: &VirtioQueueState) -> u64 {
    queue.dma_physical + queue.data_offset as u64
}

fn align_up_usize(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

fn write_virtio_desc(
    queue: &VirtioQueueState,
    index: usize,
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
) {
    let offset = VIRTIO_QUEUE_DESC_OFFSET + index * 16;
    write_dma_u64(queue.dma_virtual + offset as u64, addr);
    write_dma_u32(queue.dma_virtual + offset as u64 + 8, len);
    write_dma_u16(queue.dma_virtual + offset as u64 + 12, flags);
    write_dma_u16(queue.dma_virtual + offset as u64 + 14, next);
}

fn allocate_virtio_dma(frame_count: u64) -> Result<(u64, u64), IpcError> {
    let frame = frame_allocator()?
        .allocate_contiguous_owned(frame_count, memory::FrameOwner::dma(frame_count))
        .ok_or(IpcError::BadCapability)?;
    let hhdm_offset = limine::hhdm_offset().ok_or(IpcError::BadCapability)?;
    let bytes = frame_count
        .checked_mul(memory::FRAME_SIZE)
        .ok_or(IpcError::BadCapability)? as usize;
    let virtual_base = hhdm_offset + frame.start();
    zero_dma(virtual_base, bytes);
    Ok((frame.start(), virtual_base))
}

fn discover_virtio_pci_io_device(device_id: u16) -> Result<u16, IpcError> {
    let mut slot = 0u8;
    while slot < 32 {
        let vendor = pci_read_u16(0, slot, 0, 0x00);
        let device = pci_read_u16(0, slot, 0, 0x02);
        if vendor == PCI_VENDOR_VIRTIO && device == device_id {
            let command = pci_read_u16(0, slot, 0, PCI_COMMAND);
            pci_write_u16(
                0,
                slot,
                0,
                PCI_COMMAND,
                (command | PCI_COMMAND_IO | PCI_COMMAND_BUS_MASTER)
                    & !PCI_COMMAND_INTERRUPT_DISABLE,
            );
            let bar0 = pci_read_u32(0, slot, 0, PCI_BAR0);
            if bar0 & 1 == 0 {
                return Err(IpcError::BadCapability);
            }
            return Ok((bar0 & !0x3) as u16);
        }
        slot += 1;
    }
    Err(IpcError::BadCapability)
}

fn pci_address(bus: u8, slot: u8, function: u8, offset: u8) -> u32 {
    0x8000_0000
        | ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xfc)
}

fn pci_select(bus: u8, slot: u8, function: u8, offset: u8) -> u16 {
    unsafe {
        serial::outl_raw(PCI_CONFIG_ADDRESS, pci_address(bus, slot, function, offset));
    }
    PCI_CONFIG_DATA + ((offset as u16) & 0x3)
}

fn pci_read_u16(bus: u8, slot: u8, function: u8, offset: u8) -> u16 {
    let port = pci_select(bus, slot, function, offset);
    unsafe { serial::inw_raw(port) }
}

fn pci_read_u32(bus: u8, slot: u8, function: u8, offset: u8) -> u32 {
    let port = pci_select(bus, slot, function, offset);
    unsafe { serial::inl_raw(port) }
}

fn pci_write_u16(bus: u8, slot: u8, function: u8, offset: u8, value: u16) {
    let port = pci_select(bus, slot, function, offset);
    unsafe {
        serial::outw_raw(port, value);
    }
}

fn virtio_read8(io_base: u16, offset: u16) -> u8 {
    unsafe { serial::inb_raw(io_base + offset) }
}

fn virtio_read16(io_base: u16, offset: u16) -> u16 {
    unsafe { serial::inw_raw(io_base + offset) }
}

fn virtio_read32(io_base: u16, offset: u16) -> u32 {
    unsafe { serial::inl_raw(io_base + offset) }
}

fn virtio_write8(io_base: u16, offset: u16, value: u8) {
    unsafe {
        serial::outb_raw(io_base + offset, value);
    }
}

fn virtio_write16(io_base: u16, offset: u16, value: u16) {
    unsafe {
        serial::outw_raw(io_base + offset, value);
    }
}

fn virtio_write32(io_base: u16, offset: u16, value: u32) {
    unsafe {
        serial::outl_raw(io_base + offset, value);
    }
}

fn zero_dma(base: u64, len: usize) {
    let mut index = 0;
    while index < len {
        write_dma_u8(base + index as u64, 0);
        index += 1;
    }
}

fn write_dma_bytes(base: u64, value: &[u8]) {
    let mut index = 0;
    while index < value.len() {
        write_dma_u8(base + index as u64, value[index]);
        index += 1;
    }
}

fn read_dma_bytes(base: u64, out: &mut [u8]) {
    let mut index = 0;
    while index < out.len() {
        out[index] = read_dma_u8(base + index as u64);
        index += 1;
    }
}

fn write_dma_u8(address: u64, value: u8) {
    unsafe {
        (address as *mut u8).write_volatile(value);
    }
}

fn read_dma_u8(address: u64) -> u8 {
    unsafe { (address as *const u8).read_volatile() }
}

fn write_dma_u16(address: u64, value: u16) {
    write_dma_bytes(address, &value.to_le_bytes());
}

fn read_dma_u16(address: u64) -> u16 {
    u16::from_le_bytes([read_dma_u8(address), read_dma_u8(address + 1)])
}

fn write_dma_u32(address: u64, value: u32) {
    write_dma_bytes(address, &value.to_le_bytes());
}

fn read_dma_u32(address: u64) -> u32 {
    u32::from_le_bytes([
        read_dma_u8(address),
        read_dma_u8(address + 1),
        read_dma_u8(address + 2),
        read_dma_u8(address + 3),
    ])
}

fn write_dma_u64(address: u64, value: u64) {
    write_dma_bytes(address, &value.to_le_bytes());
}

fn pause_cpu() {
    unsafe {
        asm!("pause", options(nomem, nostack, preserves_flags));
    }
}

pub fn namespace_resolve(
    cap_slot: u64,
    path: *const u8,
    path_len: usize,
    target_slot: u64,
) -> Result<(), IpcError> {
    if path_len > 128 {
        return Err(IpcError::MessageTooLarge);
    }
    let namespace = namespace_from_cap(cap_slot, capability::RIGHT_RESOLVE)?;
    let mut path_bytes = [0u8; 128];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let Some(entry) = namespace.resolve(&path_bytes[..path_len]) else {
        serial::write_str("Namespace resolve rejected: proc=");
        serial::write_str(current_process_name());
        serial::write_str(" namespace=");
        serial::write_str(namespace.name);
        serial::write_str(" path=");
        serial::write_ascii_bytes(&path_bytes[..path_len]);
        serial::write_str("\n");
        return Err(IpcError::BadCapability);
    };

    let namespace_cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)?;
    let runtime = runtime();
    let owner = runtime
        .processes
        .current_process()
        .map(|process| process.pid)
        .ok_or(IpcError::BadCapability)?;
    {
        let Some(process) = runtime.processes.current_process() else {
            return Err(IpcError::BadCapability);
        };
        if !process.caps.can_grant(target_slot) {
            return Err(IpcError::BadCapability);
        }
    }
    let cap = runtime.new_capability(entry.object, entry.rights, owner, namespace_cap.id, owner)?;
    let Some(process) = runtime.processes.current_process_mut() else {
        return Err(IpcError::BadCapability);
    };
    process
        .caps
        .grant(target_slot, cap)
        .map_err(|_| IpcError::BadCapability)?;

    serial::write_str("Namespace resolve accepted: proc=");
    serial::write_str(process.name);
    serial::write_str(" namespace=");
    serial::write_str(namespace.name);
    serial::write_str(" path=");
    serial::write_ascii_bytes(&path_bytes[..path_len]);
    serial::write_str(" target_cap[");
    serial::write_u64_dec(target_slot);
    serial::write_str("] rights=");
    print_rights(entry.rights);
    serial::write_str("\n");
    Ok(())
}

pub fn io_read(cap_slot: u64, port: u64) -> Result<u64, IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_READ)?;
    if !port_span_in_range(range, port, 1) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    let value = unsafe { serial::inb_raw(port as u16) };
    Ok(value as u64)
}

pub fn io_write(cap_slot: u64, port: u64, value: u64) -> Result<(), IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_WRITE)?;
    if !port_span_in_range(range, port, 1) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    unsafe {
        serial::outb_raw(port as u16, value as u8);
    }
    Ok(())
}

pub fn io_read16(cap_slot: u64, port: u64) -> Result<u64, IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_READ)?;
    if !port_span_in_range(range, port, 2) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    let value = unsafe { serial::inw_raw(port as u16) };
    Ok(value as u64)
}

pub fn io_write16(cap_slot: u64, port: u64, value: u64) -> Result<(), IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_WRITE)?;
    if !port_span_in_range(range, port, 2) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    unsafe {
        serial::outw_raw(port as u16, value as u16);
    }
    Ok(())
}

pub fn io_read32(cap_slot: u64, port: u64) -> Result<u64, IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_READ)?;
    if !port_span_in_range(range, port, 4) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    let value = unsafe { serial::inl_raw(port as u16) };
    Ok(value as u64)
}

pub fn io_write32(cap_slot: u64, port: u64, value: u64) -> Result<(), IpcError> {
    let range = io_port_from_cap(cap_slot, capability::RIGHT_WRITE)?;
    if !port_span_in_range(range, port, 4) || port > u16::MAX as u64 {
        return Err(IpcError::BadCapability);
    }

    unsafe {
        serial::outl_raw(port as u16, value as u32);
    }
    Ok(())
}

pub fn irq_wait(cap_slot: u64, timeout_ms: u64, frame: &mut SyscallFrame) -> Result<(), IpcError> {
    let line = interrupt_line_from_cap(cap_slot, capability::RIGHT_LISTEN)?;
    if consume_pending_interrupt(line.id) {
        serial::write_str("IRQ wait delivered pending: proc=");
        serial::write_str(current_process_name());
        serial::write_str(" interrupt-line=");
        serial::write_str(line.name);
        serial::write_str(" line=");
        serial::write_u64_dec(line.line);
        serial::write_str("\n");
        frame.rax = STATUS_OK;
        return Ok(());
    }

    serial::write_str("IRQ wait accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" interrupt-line=");
    serial::write_str(line.name);
    serial::write_str(" line=");
    serial::write_u64_dec(line.line);
    serial::write_str("\n");

    if timeout_ms == 0 {
        frame.rax = STATUS_OK;
        return Ok(());
    }

    let timeout_tsc = Some(deadline_after_ms(timeout_ms));
    if block_current_on_interrupt(line.id, timeout_tsc, frame) {
        return Ok(());
    }

    Err(IpcError::Empty)
}

fn consume_pending_interrupt(interrupt: KernelObjectId) -> bool {
    let runtime = runtime();
    let Some(line) = runtime.objects.get_interrupt_line_mut(interrupt) else {
        return false;
    };
    if line.pending_count == 0 {
        return false;
    }
    line.pending_count -= 1;
    line.delivered_count = line.delivered_count.saturating_add(1);
    true
}

fn block_current_on_interrupt(
    interrupt: KernelObjectId,
    timeout_tsc: Option<u64>,
    frame: &mut SyscallFrame,
) -> bool {
    let (name, line_name, line_number) = {
        let runtime = runtime();
        let Some(line) = runtime.objects.get_interrupt_line(interrupt) else {
            return false;
        };
        let Some(process) = runtime.processes.current_process_mut() else {
            return false;
        };

        process.saved_frame = *frame;
        process.saved_frame.rax = STATUS_OK;
        process.has_saved_frame = true;
        process.state = ProcessState::BlockedOnInterrupt {
            interrupt,
            timeout_tsc,
        };
        (process.name, line.name, line.line)
    };

    serial::write_str("IRQ wait blocked: proc=");
    serial::write_str(name);
    serial::write_str(" interrupt-line=");
    serial::write_str(line_name);
    serial::write_str(" line=");
    serial::write_u64_dec(line_number);
    if timeout_tsc.is_some() {
        serial::write_str(" timeout=yes");
    }
    serial::write_str("\n");

    if schedule_next_ready(frame) {
        return true;
    }

    let runtime = runtime();
    if let Some(process) = runtime.processes.current_process_mut() {
        process.state = ProcessState::Running;
    }

    serial::write_str("Scheduler blocked: proc=");
    serial::write_str(name);
    serial::write_str(" no ready process\n");
    false
}

pub fn record_hardware_irq(irq_line: u64) {
    let Some(line) = runtime().objects.get_interrupt_line_by_number(irq_line) else {
        serial::write_str("Spurious legacy IRQ: line=");
        serial::write_u64_dec(irq_line);
        serial::write_str("\n");
        return;
    };

    if let Some(waiter_index) = blocked_interrupt_waiter_index(line.id) {
        let waiter_name = {
            let runtime = runtime();
            let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() else {
                return;
            };
            waiter.saved_frame.rax = STATUS_OK;
            waiter.state = ProcessState::Ready;
            waiter.name
        };
        if let Some(line) = runtime().objects.get_interrupt_line_mut(line.id) {
            line.delivered_count = line.delivered_count.saturating_add(1);
        }
        serial::write_str("IRQ delivered: line=");
        serial::write_u64_dec(irq_line);
        serial::write_str(" interrupt-line=");
        serial::write_str(line.name);
        serial::write_str("\n");
        serial::write_str("IRQ wake waiter: proc=");
        serial::write_str(waiter_name);
        serial::write_str(" interrupt-line=");
        serial::write_str(line.name);
        serial::write_str("\n");
        return;
    }

    if let Some(line) = runtime().objects.get_interrupt_line_mut(line.id) {
        line.pending_count = line.pending_count.saturating_add(1);
        serial::write_str("IRQ pending recorded: line=");
        serial::write_u64_dec(irq_line);
        serial::write_str(" interrupt-line=");
        serial::write_str(line.name);
        serial::write_str(" pending=");
        serial::write_u64_dec(line.pending_count);
        serial::write_str("\n");
    }
}

fn blocked_interrupt_waiter_index(interrupt: KernelObjectId) -> Option<usize> {
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && let ProcessState::BlockedOnInterrupt {
                interrupt: waiting_interrupt,
                ..
            } = process.state
            && waiting_interrupt == interrupt
        {
            return Some(index);
        }
        index += 1;
    }

    None
}

fn interrupt_waiter_count(runtime: &RuntimeState, interrupt: KernelObjectId) -> u64 {
    let mut count = 0;
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && let ProcessState::BlockedOnInterrupt {
                interrupt: waiting_interrupt,
                ..
            } = process.state
            && waiting_interrupt == interrupt
        {
            count += 1;
        }
        index += 1;
    }
    count
}

fn interrupt_owner_name(runtime: &RuntimeState, interrupt: KernelObjectId) -> &'static str {
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index] {
            if capability_space_has_live_right(
                runtime,
                process.caps,
                interrupt,
                capability::RIGHT_LISTEN,
            ) || capability_space_has_live_right(
                runtime,
                process.initial_caps,
                interrupt,
                capability::RIGHT_LISTEN,
            ) {
                return process.name;
            }
        }
        index += 1;
    }

    "<none>"
}

fn capability_space_has_live_right(
    runtime: &RuntimeState,
    space: CapabilitySpace,
    object: KernelObjectId,
    right: u64,
) -> bool {
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = space.caps[slot]
            && cap.object == object
            && cap.rights & right != 0
            && !cap.revoked
            && !runtime.cap_id_revoked(cap.id)
        {
            return true;
        }
        slot += 1;
    }

    false
}

pub fn mmio_map(cap_slot: u64) -> Result<u64, IpcError> {
    let region = mmio_region_from_cap(cap_slot, capability::RIGHT_MAP)?;
    let physical_base = align_down(region.base, memory::FRAME_SIZE);
    let page_offset = region
        .base
        .checked_sub(physical_base)
        .ok_or(IpcError::BadCapability)?;
    let map_len = align_up(
        region
            .length
            .checked_add(page_offset)
            .ok_or(IpcError::BadCapability)?,
        memory::FRAME_SIZE,
    )
    .ok_or(IpcError::BadCapability)?;
    let virtual_base = device_user_mapping_base(USER_MMIO_MAPPING_BASE, region.id, map_len)?;
    let user_base = virtual_base
        .checked_add(page_offset)
        .ok_or(IpcError::BadCapability)?;
    map_current_process_physical_range(
        virtual_base,
        physical_base,
        map_len,
        paging::PageFlags::user_device(),
    )?;
    serial::write_str("MMIO map accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" mmio-region=");
    serial::write_str(region.name);
    serial::write_str(" base=");
    serial::write_u64_hex(region.base);
    serial::write_str(" length=");
    serial::write_u64_hex(region.length);
    serial::write_str(" virt=");
    serial::write_u64_hex(user_base);
    serial::write_str("\n");
    Ok(user_base)
}

pub fn dma_map(cap_slot: u64, destination: *mut u8, max_len: usize) -> Result<(), IpcError> {
    if max_len < DMA_MAPPING_INFO_BYTES {
        return Err(IpcError::InvalidUserBuffer);
    }

    let region = dma_region_from_cap(
        cap_slot,
        capability::RIGHT_READ | capability::RIGHT_WRITE | capability::RIGHT_MAP,
    )?;
    let owner = runtime()
        .processes
        .current_process()
        .map(|process| process.pid)
        .ok_or(IpcError::BadCapability)?;
    if (destination as u64) & 7 != 0 {
        return Err(IpcError::InvalidUserBuffer);
    }
    usercopy::validate_user_buffer(
        UserPtr::new(destination as u64),
        DMA_MAPPING_INFO_BYTES,
        paging::UserAccess::Write,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;
    let map_len = align_up(region.length, memory::FRAME_SIZE).ok_or(IpcError::BadCapability)?;
    let virtual_base = device_user_mapping_base(USER_DMA_MAPPING_BASE, region.id, map_len)?;
    if let Some(mapping) = runtime()
        .processes
        .current_process()
        .and_then(|process| process.dma_mapping(region.id))
    {
        let mut info = [0u8; DMA_MAPPING_INFO_BYTES];
        write_dma_mapping_info(&mut info, mapping);
        usercopy::copy_to_user(UserPtr::new(destination as u64), &info)
            .map_err(|_| IpcError::InvalidUserBuffer)?;
        serial::write_str("DMA map reused: proc=");
        serial::write_str(current_process_name());
        serial::write_str(" dma-region=");
        serial::write_str(region.name);
        serial::write_str(" virt=");
        serial::write_u64_hex(mapping.virtual_base);
        serial::write_str(" length=");
        serial::write_u64_hex(mapping.length);
        serial::write_str("\n");
        return Ok(());
    }

    claim_dma_region(region.id, owner)?;
    if !zero_dma_physical_range(region.base, region.length) {
        release_dma_region_claim(region.id, owner);
        return Err(IpcError::BadCapability);
    }

    map_current_process_physical_range(
        virtual_base,
        region.base,
        map_len,
        paging::PageFlags::user(true, false),
    )
    .map_err(|error| {
        release_dma_region_claim(region.id, owner);
        error
    })?;

    let mut info = [0u8; DMA_MAPPING_INFO_BYTES];
    let mapping = DmaUserMapping {
        region: region.id,
        virtual_base,
        physical_base: region.base,
        length: region.length,
    };
    write_dma_mapping_info(&mut info, mapping);
    if usercopy::copy_to_user(UserPtr::new(destination as u64), &info).is_err() {
        let _ = unmap_current_process_physical_range(virtual_base, map_len);
        release_dma_region_claim(region.id, owner);
        return Err(IpcError::InvalidUserBuffer);
    }
    let Some(process) = runtime().processes.current_process_mut() else {
        let _ = unmap_current_process_physical_range(virtual_base, map_len);
        release_dma_region_claim(region.id, owner);
        return Err(IpcError::BadCapability);
    };
    if process.add_dma_mapping(mapping).is_err() {
        let _ = unmap_current_process_physical_range(virtual_base, map_len);
        release_dma_region_claim(region.id, owner);
        return Err(IpcError::BadCapability);
    }

    serial::write_str("DMA map accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" dma-region=");
    serial::write_str(region.name);
    serial::write_str(" phys=");
    serial::write_u64_hex(region.base);
    serial::write_str(" virt=");
    serial::write_u64_hex(virtual_base);
    serial::write_str(" length=");
    serial::write_u64_hex(region.length);
    serial::write_str("\n");
    Ok(())
}

fn claim_dma_region(region_id: KernelObjectId, owner: ProcessId) -> Result<(), IpcError> {
    let (name, mapped_by) = {
        let runtime = runtime();
        let Some(region) = runtime.objects.get_dma_region(region_id) else {
            return Err(IpcError::BadCapability);
        };
        (region.name, region.mapped_by)
    };
    if mapped_by != ProcessId::empty() && mapped_by != owner {
        serial::write_str("DMA map rejected: dma-region=");
        serial::write_str(name);
        serial::write_str(" already-owned-by=");
        serial::write_str(process_name_by_pid(runtime(), mapped_by));
        serial::write_str("\n");
        return Err(IpcError::BadCapability);
    }

    if mapped_by == ProcessId::empty() {
        let runtime = runtime();
        let Some(region) = runtime.objects.get_dma_region_mut(region_id) else {
            return Err(IpcError::BadCapability);
        };
        region.mapped_by = owner;
        region.map_count = region.map_count.saturating_add(1);
    }
    Ok(())
}

fn release_dma_region_claim(region_id: KernelObjectId, owner: ProcessId) {
    let runtime = runtime();
    if let Some(region) = runtime.objects.get_dma_region_mut(region_id)
        && region.mapped_by == owner
    {
        region.mapped_by = ProcessId::empty();
        region.release_count = region.release_count.saturating_add(1);
    }
}

fn release_process_dma_mappings(pid: ProcessId) {
    let mut slot = 0;
    while slot < MAX_OBJECTS {
        let release = {
            let runtime = runtime();
            let Some(process) = runtime.processes.process_mut(pid) else {
                return;
            };
            let name = process.name;
            process
                .take_dma_mapping(slot)
                .map(|mapping| (name, mapping))
        };
        if let Some((name, mapping)) = release {
            release_dma_mapping(pid, name, mapping);
        }
        slot += 1;
    }
}

fn release_all_runtime_dma_mappings() {
    let mut process_index = 0;
    loop {
        let pid = {
            let runtime = runtime();
            if process_index >= runtime.processes.count {
                break;
            }
            let Some(process) = runtime.processes.processes[process_index] else {
                process_index += 1;
                continue;
            };
            process.pid
        };
        release_process_dma_mappings(pid);
        process_index += 1;
    }
}

fn release_process_virtio_ownership(pid: ProcessId) {
    let owner_name = process_name_by_pid(runtime(), pid);
    release_process_kernel_virtio_ownership(pid, owner_name);
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.objects.count {
        if let Some(KernelObject::VirtioDevice(device)) = runtime.objects.objects[index].as_mut()
            && device.owner == pid
        {
            device.owner = ProcessId::empty();
            serial::write_str("Virtio device ownership released: proc=");
            serial::write_str(owner_name);
            serial::write_str(" virtio-device=");
            serial::write_str(device.name);
            serial::write_str("\n");
        }
        index += 1;
    }
}

fn release_process_kernel_virtio_ownership(pid: ProcessId, owner_name: &'static str) {
    let rng = unsafe { &mut *VIRTIO_RNG_STATE.0.get() };
    if rng.owner == pid {
        if rng.io_base != 0 {
            virtio_write8(rng.io_base, VIRTIO_PCI_STATUS, 0);
        }
        rng.initialized = false;
        rng.owner = ProcessId::empty();
        rng.reset_count = rng.reset_count.saturating_add(1);
        rng.last_error = "owner-release";
        rng.queue.last_error = "owner-release";
        serial::write_str("Virtio kernel device ownership released: proc=");
        serial::write_str(owner_name);
        serial::write_str(" virtio-device=");
        serial::write_str(VIRTIO_RNG_DEVICE_ID);
        serial::write_str("\n");
    }

    let net = unsafe { &mut *VIRTIO_NET_STATE.0.get() };
    if net.owner == pid {
        if net.io_base != 0 {
            virtio_write8(net.io_base, VIRTIO_PCI_STATUS, 0);
        }
        net.initialized = false;
        net.rx_posted = false;
        net.owner = ProcessId::empty();
        net.reset_count = net.reset_count.saturating_add(1);
        net.last_error = "owner-release";
        net.rx.last_error = "owner-release";
        net.tx.last_error = "owner-release";
        serial::write_str("Virtio kernel device ownership released: proc=");
        serial::write_str(owner_name);
        serial::write_str(" virtio-device=");
        serial::write_str(VIRTIO_NET_DEVICE_ID);
        serial::write_str("\n");
    }
}

fn release_dma_mapping(owner: ProcessId, owner_name: &'static str, mapping: DmaUserMapping) {
    let _ = zero_dma_physical_range(mapping.physical_base, mapping.length);
    release_dma_region_claim(mapping.region, owner);
    serial::write_str("DMA mapping released: proc=");
    serial::write_str(owner_name);
    serial::write_str(" phys=");
    serial::write_u64_hex(mapping.physical_base);
    serial::write_str(" length=");
    serial::write_u64_hex(mapping.length);
    serial::write_str("\n");
}

fn zero_dma_physical_range(physical_base: u64, length: u64) -> bool {
    if length == 0 {
        return true;
    }
    let Some(hhdm_offset) = limine::hhdm_offset() else {
        return false;
    };
    let Ok(length) = usize::try_from(length) else {
        return false;
    };
    let Some(virtual_base) = hhdm_offset.checked_add(physical_base) else {
        return false;
    };
    unsafe {
        core::ptr::write_bytes(virtual_base as *mut u8, 0, length);
    }
    true
}

pub fn runtime_inspect(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
) -> Result<usize, IpcError> {
    let _process_control = process_control_from_cap(cap_slot, capability::RIGHT_INSPECT)?;
    let caller = current_process_name();
    let report = inspect_report();
    report.clear();

    {
        let runtime = runtime();
        build_inspect_report(runtime, report);
    }

    if report.truncated || report.len > max_len {
        return Err(IpcError::MessageTooLarge);
    }

    usercopy::copy_to_user(UserPtr::new(destination as u64), report.as_slice())
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Runtime inspect accepted: proc=");
    serial::write_str(caller);
    serial::write_str(" bytes=");
    serial::write_u64_dec(report.len as u64);
    serial::write_str("\n");
    Ok(report.len)
}

pub fn sleep_ms(
    cap_slot: u64,
    milliseconds: u64,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let timer = timer_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    serial::write_str("Timer sleep accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" timer=");
    serial::write_str(timer.name);
    serial::write_str(" ms=");
    serial::write_u64_dec(milliseconds);
    serial::write_str("\n");

    if milliseconds == 0 {
        frame.rax = STATUS_OK;
        return Ok(());
    }

    let wake_tsc = deadline_after_ms(milliseconds);
    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return Err(IpcError::BadCapability);
        };

        process.saved_frame = *frame;
        process.saved_frame.rax = STATUS_OK;
        process.has_saved_frame = true;
        process.state = ProcessState::Sleeping { wake_tsc };
        process.name
    };

    serial::write_str("Timer sleep blocked: proc=");
    serial::write_str(current);
    serial::write_str("\n");

    if schedule_next_ready(frame) {
        return Ok(());
    }

    let runtime = runtime();
    if let Some(process) = runtime.processes.current_process_mut() {
        process.state = ProcessState::Running;
    }

    Err(IpcError::Empty)
}

fn read_tsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!(
            "rdtsc",
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags)
        );
    }
    ((high as u64) << 32) | low as u64
}

fn deadline_after_ms(milliseconds: u64) -> u64 {
    read_tsc().saturating_add(milliseconds.saturating_mul(tsc_ticks_per_ms()))
}

fn tsc_ticks_per_ms() -> u64 {
    let leaf15 = __cpuid_count(0x15, 0);
    if leaf15.eax != 0 && leaf15.ebx != 0 && leaf15.ecx != 0 {
        let hz = (leaf15.ecx as u64)
            .saturating_mul(leaf15.ebx as u64)
            .saturating_div(leaf15.eax as u64);
        if hz != 0 {
            return hz / 1_000;
        }
    }

    let leaf16 = __cpuid_count(0x16, 0);
    if leaf16.eax != 0 {
        return (leaf16.eax as u64).saturating_mul(1_000);
    }

    FALLBACK_TSC_TICKS_PER_MS
}

fn block_current_on_endpoint(
    endpoint: KernelObjectId,
    cap_id: u64,
    destination: u64,
    max_len: usize,
    timeout_tsc: Option<u64>,
    frame: &mut SyscallFrame,
) -> bool {
    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return false;
        };

        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::BlockedOnEndpoint {
            endpoint,
            cap_id,
            destination,
            max_len,
            timeout_tsc,
        };
        process.name
    };

    serial::write_str("IPC receive blocked: proc=");
    serial::write_str(current);
    serial::write_str(" endpoint=");
    serial::write_u64_dec(endpoint.raw());
    if timeout_tsc.is_some() {
        serial::write_str(" timeout=yes");
    }
    serial::write_str("\n");

    if schedule_next_ready(frame) {
        true
    } else {
        let runtime = runtime();
        if let Some(process) = runtime.processes.current_process_mut() {
            process.state = ProcessState::Running;
        }

        serial::write_str("Scheduler blocked: proc=");
        serial::write_str(current);
        serial::write_str(" no ready process\n");
        false
    }
}

fn wake_blocked_receiver(endpoint: KernelObjectId) {
    wake_timed_processes(read_tsc());

    loop {
        let Some(waiter_index) = blocked_receiver_index(endpoint) else {
            return;
        };

        let should_cancel = {
            let runtime = runtime();
            let Some(waiter) = runtime.processes.processes[waiter_index] else {
                return;
            };
            let ProcessState::BlockedOnEndpoint {
                endpoint, cap_id, ..
            } = waiter.state
            else {
                return;
            };
            !process_has_live_endpoint_cap(
                runtime,
                waiter,
                endpoint,
                cap_id,
                capability::RIGHT_RECEIVE,
            )
        };
        if should_cancel {
            let runtime = runtime();
            let _ = cancel_blocked_endpoint_waiter_at(
                runtime,
                waiter_index,
                STATUS_BAD_CAPABILITY,
                "authority-revoked",
            );
            continue;
        }

        let (name, receiver_pid, receiver_cr3, destination, max_len, current_cr3) = {
            let runtime = runtime();
            let Some(waiter) = runtime.processes.processes[waiter_index] else {
                return;
            };
            let ProcessState::BlockedOnEndpoint {
                destination,
                max_len,
                ..
            } = waiter.state
            else {
                return;
            };

            let current_cr3 = runtime
                .processes
                .current_process()
                .map(|process| process.context.cr3)
                .unwrap_or_else(paging::active_root_table_physical);

            (
                waiter.name,
                waiter.pid,
                waiter.context.cr3,
                destination,
                max_len,
                current_cr3,
            )
        };

        let Some(message) = ({
            let runtime = runtime();
            let Some(endpoint_object) = runtime.objects.get_endpoint_mut(endpoint) else {
                return;
            };
            endpoint_object.dequeue_for(receiver_pid)
        }) else {
            return;
        };

        let copy_len = min(message.len, max_len);
        let copy_result = unsafe {
            gdt::switch_address_space(receiver_cr3);
            let result =
                usercopy::copy_to_user(UserPtr::new(destination), &message.bytes[..copy_len]);
            gdt::switch_address_space(current_cr3);
            result
        };

        match copy_result {
            Ok(()) => {
                {
                    let runtime = runtime();
                    let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() else {
                        return;
                    };
                    waiter.saved_frame.rax = copy_len as u64;
                    waiter.state = ProcessState::Ready;
                }
                record_ready_lifecycle(endpoint, receiver_pid, message);

                serial::write_str("IPC receive delivered: endpoint=");
                serial::write_u64_dec(endpoint.raw());
                serial::write_str(" bytes=");
                serial::write_u64_dec(copy_len as u64);
                serial::write_str("\n");

                serial::write_str("IPC wake receiver: proc=");
                serial::write_str(name);
                serial::write_str(" endpoint=");
                serial::write_u64_dec(endpoint.raw());
                serial::write_str("\n");
            }
            Err(_) => {
                {
                    let runtime = runtime();
                    let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() else {
                        return;
                    };
                    waiter.saved_frame.rax = STATUS_BAD_BUFFER;
                    waiter.state = ProcessState::Ready;
                }
                serial::write_str("IPC wake receiver failed: bad user buffer proc=");
                serial::write_str(name);
                serial::write_str("\n");
            }
        }
        return;
    }
}

fn blocked_receiver_index(endpoint: KernelObjectId) -> Option<usize> {
    let runtime = runtime();
    let mut index = 0;

    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && let ProcessState::BlockedOnEndpoint {
                endpoint: waiting_endpoint,
                ..
            } = process.state
            && waiting_endpoint == endpoint
            && runtime
                .objects
                .get_endpoint(endpoint)
                .map(|endpoint_object| endpoint_object.has_message_for(process.pid))
                .unwrap_or(false)
        {
            return Some(index);
        }
        index += 1;
    }

    None
}

fn process_has_live_endpoint_cap(
    runtime: &RuntimeState,
    process: Process,
    endpoint: KernelObjectId,
    cap_id: u64,
    required_right: u64,
) -> bool {
    if runtime.objects.get_endpoint(endpoint).is_none() {
        return false;
    }

    let mut slot = 0;
    while slot < process.caps.caps.len() {
        if let Some(cap) = process.caps.caps[slot]
            && cap.id == cap_id
            && cap.object == endpoint
            && !cap.revoked
            && !runtime.cap_id_revoked(cap.id)
            && !capability_has_revoked_ancestor(runtime, cap)
            && cap.generation_id == runtime.generation_id
            && cap.rights & required_right == required_right
        {
            return true;
        }
        slot += 1;
    }
    false
}

fn cancel_blocked_endpoint_waiter_at(
    runtime: &mut RuntimeState,
    index: usize,
    status: u64,
    reason: &'static str,
) -> bool {
    let Some(process) = runtime.processes.processes[index].as_mut() else {
        return false;
    };
    let ProcessState::BlockedOnEndpoint {
        endpoint, cap_id, ..
    } = process.state
    else {
        return false;
    };

    process.saved_frame.rax = status;
    process.state = ProcessState::Ready;

    serial::write_str("IPC receive canceled: proc=");
    serial::write_str(process.name);
    serial::write_str(" endpoint=");
    serial::write_u64_dec(endpoint.raw());
    serial::write_str(" cap_id=");
    serial::write_u64_dec(cap_id);
    serial::write_str(" reason=");
    serial::write_str(reason);
    serial::write_str(" status=");
    serial::write_u64_dec(status);
    serial::write_str("\n");
    true
}

fn cancel_unauthorized_blocked_receivers(status: u64) -> usize {
    let runtime = runtime();
    let mut canceled = 0;
    let mut index = 0;
    while index < runtime.processes.count {
        let should_cancel = runtime.processes.processes[index]
            .map(|process| {
                if let ProcessState::BlockedOnEndpoint {
                    endpoint, cap_id, ..
                } = process.state
                {
                    !process_has_live_endpoint_cap(
                        runtime,
                        process,
                        endpoint,
                        cap_id,
                        capability::RIGHT_RECEIVE,
                    )
                } else {
                    false
                }
            })
            .unwrap_or(false);
        if should_cancel
            && cancel_blocked_endpoint_waiter_at(runtime, index, status, "authority-revoked")
        {
            canceled += 1;
        }
        index += 1;
    }
    canceled
}

fn cancel_blocked_receivers_for_endpoint_owner(owner: ProcessId, status: u64) -> usize {
    let runtime = runtime();
    let mut canceled = 0;
    let mut index = 0;
    while index < runtime.processes.count {
        let should_cancel = runtime.processes.processes[index]
            .map(|process| {
                if let ProcessState::BlockedOnEndpoint { endpoint, .. } = process.state {
                    runtime
                        .objects
                        .get_endpoint(endpoint)
                        .map(|endpoint_object| endpoint_object.owner == owner)
                        .unwrap_or(true)
                } else {
                    false
                }
            })
            .unwrap_or(false);
        if should_cancel
            && cancel_blocked_endpoint_waiter_at(runtime, index, status, "endpoint-destroyed")
        {
            canceled += 1;
        }
        index += 1;
    }
    canceled
}

fn wake_timed_processes(now: u64) -> usize {
    let runtime = runtime();
    let mut woke = 0;
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index].as_mut() {
            match process.state {
                ProcessState::Sleeping { wake_tsc } if deadline_reached(now, wake_tsc) => {
                    process.saved_frame.rax = STATUS_OK;
                    process.state = ProcessState::Ready;
                    woke += 1;
                    serial::write_str("Timer wake: proc=");
                    serial::write_str(process.name);
                    serial::write_str("\n");
                }
                ProcessState::BlockedOnEndpoint {
                    timeout_tsc: Some(timeout_tsc),
                    ..
                } if deadline_reached(now, timeout_tsc) => {
                    process.saved_frame.rax = STATUS_TIMEOUT;
                    process.state = ProcessState::Ready;
                    woke += 1;
                    serial::write_str("IPC receive timeout: proc=");
                    serial::write_str(process.name);
                    serial::write_str("\n");
                }
                ProcessState::BlockedOnInterrupt {
                    timeout_tsc: Some(timeout_tsc),
                    ..
                } if deadline_reached(now, timeout_tsc) => {
                    process.saved_frame.rax = STATUS_TIMEOUT;
                    process.state = ProcessState::Ready;
                    woke += 1;
                    serial::write_str("IRQ wait timeout: proc=");
                    serial::write_str(process.name);
                    serial::write_str("\n");
                }
                _ => {}
            }
        }
        index += 1;
    }
    woke
}

fn next_deadline_tsc() -> Option<u64> {
    let runtime = runtime();
    let mut next = None;
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index] {
            let deadline = match process.state {
                ProcessState::Sleeping { wake_tsc } => Some(wake_tsc),
                ProcessState::BlockedOnEndpoint {
                    timeout_tsc: Some(timeout_tsc),
                    ..
                } => Some(timeout_tsc),
                ProcessState::BlockedOnInterrupt {
                    timeout_tsc: Some(timeout_tsc),
                    ..
                } => Some(timeout_tsc),
                _ => None,
            };
            if let Some(deadline) = deadline
                && next
                    .map(|current| deadline_before(deadline, current))
                    .unwrap_or(true)
            {
                next = Some(deadline);
            }
        }
        index += 1;
    }
    next
}

fn wait_until_deadline(deadline: u64, include_current: bool) {
    while !deadline_reached(read_tsc(), deadline)
        && runtime()
            .processes
            .next_ready_index_round_robin(include_current)
            .is_none()
        && wake_timed_processes(read_tsc()) == 0
    {
        crate::timer::wait_for_interrupt();
    }
}

fn deadline_reached(now: u64, deadline: u64) -> bool {
    (now as i64).wrapping_sub(deadline as i64) >= 0
}

fn deadline_before(left: u64, right: u64) -> bool {
    (left as i64).wrapping_sub(right as i64) < 0
}

fn start_vfs_state_transaction(
    state: KernelObjectId,
    node: VfsNode,
    description: OpenFileDescription,
    operation: VfsStateOperation,
    offset: u64,
    destination: u64,
    max_len: usize,
    write_len: usize,
    update_offset: bool,
    payload: &[u8; MAX_MESSAGE_BYTES],
    payload_len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let (state_name, request_endpoint, reply_endpoint) = {
        let runtime = runtime();
        let state_object = runtime
            .objects
            .get_state_volume(state)
            .ok_or(IpcError::VfsBadHandle)?;
        let request_endpoint = runtime
            .state_vfs_request_endpoint
            .ok_or(IpcError::VfsUnsupported)?;
        let reply_endpoint = runtime
            .state_vfs_reply_endpoint
            .ok_or(IpcError::VfsUnsupported)?;
        (state_object.name, request_endpoint, reply_endpoint)
    };
    let state_name_len = state_name.len();
    let request_len = VFS_STATE_TRANSACTION_ID_BYTES
        .checked_add(VFS_STATE_REQUEST_HEADER_BYTES)
        .and_then(|len| len.checked_add(state_name_len))
        .and_then(|len| len.checked_add(payload_len))
        .ok_or(IpcError::VfsNoSpace)?;
    if state_name_len > u16::MAX as usize
        || payload_len > u16::MAX as usize
        || request_len > MAX_MESSAGE_BYTES
    {
        return Err(IpcError::VfsNoSpace);
    }
    let transaction_id = {
        let runtime = runtime();
        let id = runtime.next_vfs_state_transaction_id;
        if id == 0 || id == u64::MAX {
            return Err(IpcError::VfsNoSpace);
        }
        runtime.next_vfs_state_transaction_id = id + 1;
        id
    };

    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return Err(IpcError::VfsPermission);
        };
        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::BlockedOnVfsState {
            reply_endpoint,
            node: node.id,
            description: description.id,
            operation,
            transaction_id,
            offset,
            destination,
            max_len,
            write_len,
            update_offset,
        };
        process.name
    };

    let mut queued_request = [0u8; MAX_MESSAGE_BYTES];
    write_u64_le(&mut queued_request, 0, transaction_id);
    let request_offset = VFS_STATE_TRANSACTION_ID_BYTES;
    queued_request[request_offset] = VFS_STATE_REQUEST_MAGIC[0];
    queued_request[request_offset + 1] = VFS_STATE_REQUEST_MAGIC[1];
    queued_request[request_offset + 2] = VFS_STATE_REQUEST_VERSION;
    queued_request[request_offset + 3] = vfs_state_operation_code(operation);
    write_u16_le(
        &mut queued_request,
        request_offset + 4,
        state_name_len as u16,
    );
    write_u16_le(&mut queued_request, request_offset + 6, payload_len as u16);
    let state_offset = request_offset + VFS_STATE_REQUEST_HEADER_BYTES;
    queued_request[state_offset..state_offset + state_name_len]
        .copy_from_slice(state_name.as_bytes());
    let payload_offset = state_offset + state_name_len;
    queued_request[payload_offset..payload_offset + payload_len]
        .copy_from_slice(&payload[..payload_len]);
    let queued_request_len = request_len;
    let enqueue_result = {
        let runtime = runtime();
        runtime
            .objects
            .get_endpoint_mut(request_endpoint)
            .ok_or(IpcError::BadCapability)?
            .enqueue(ProcessId::empty(), &queued_request, queued_request_len)
    };
    if let Err(error) = enqueue_result {
        restore_current_vfs_state_waiter(reply_endpoint);
        return Err(error);
    }

    serial::write_str("VFS state transaction request: proc=");
    serial::write_str(current);
    serial::write_str(" state=");
    serial::write_str(state_name);
    serial::write_str(" op=");
    serial::write_str(vfs_state_operation_label(operation));
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" description=");
    serial::write_u64_dec(description.id.raw());
    serial::write_str(" tx=");
    serial::write_u64_dec(transaction_id);
    serial::write_str("\n");

    wake_blocked_receiver(request_endpoint);

    if schedule_next_ready(frame) {
        return Ok(());
    }

    restore_current_vfs_state_waiter(reply_endpoint);
    if let Some(endpoint) = runtime().objects.get_endpoint_mut(request_endpoint) {
        let _ = endpoint.remove_vfs_state_request(ProcessId::empty(), transaction_id);
    }

    serial::write_str("Scheduler blocked: proc=");
    serial::write_str(current);
    serial::write_str(" no ready process for VFS state transaction\n");
    Err(IpcError::Empty)
}

fn start_vfs_service_read_transaction(
    node: VfsNode,
    description: OpenFileDescription,
    offset: u64,
    destination: u64,
    max_len: usize,
    update_offset: bool,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let (request_endpoint, reply_endpoint) = {
        let runtime = runtime();
        (
            runtime
                .state_vfs_request_endpoint
                .ok_or(IpcError::VfsUnsupported)?,
            runtime
                .state_vfs_reply_endpoint
                .ok_or(IpcError::VfsUnsupported)?,
        )
    };
    let transaction_id = {
        let runtime = runtime();
        let id = runtime.next_vfs_state_transaction_id;
        if id == 0 || id == u64::MAX {
            return Err(IpcError::VfsNoSpace);
        }
        runtime.next_vfs_state_transaction_id = id + 1;
        id
    };

    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return Err(IpcError::VfsPermission);
        };
        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::BlockedOnVfsState {
            reply_endpoint,
            node: node.id,
            description: description.id,
            operation: VfsStateOperation::ServiceRead,
            transaction_id,
            offset,
            destination,
            max_len,
            write_len: 0,
            update_offset,
        };
        process.name
    };

    let mut queued_request = [0u8; MAX_MESSAGE_BYTES];
    write_u64_le(&mut queued_request, 0, transaction_id);
    queued_request[8] = VFS_SERVICE_REQUEST_MAGIC[0];
    queued_request[9] = VFS_SERVICE_REQUEST_MAGIC[1];
    queued_request[10] = VFS_SERVICE_REQUEST_VERSION;
    queued_request[11] = VFS_SERVICE_OP_READ_REPORT;
    let enqueue_result = {
        let runtime = runtime();
        runtime
            .objects
            .get_endpoint_mut(request_endpoint)
            .ok_or(IpcError::BadCapability)?
            .enqueue(
                ProcessId::empty(),
                &queued_request,
                VFS_SERVICE_REQUEST_BYTES,
            )
    };
    if let Err(error) = enqueue_result {
        restore_current_vfs_state_waiter(reply_endpoint);
        return Err(error);
    }

    serial::write_str("VFS filesystem service request: proc=");
    serial::write_str(current);
    serial::write_str(" file=");
    serial_write_vfs_name(node.name);
    serial::write_str(" tx=");
    serial::write_u64_dec(transaction_id);
    serial::write_str("\n");

    wake_blocked_receiver(request_endpoint);

    if schedule_next_ready(frame) {
        return Ok(());
    }

    restore_current_vfs_state_waiter(reply_endpoint);
    if let Some(endpoint) = runtime().objects.get_endpoint_mut(request_endpoint) {
        let _ = endpoint.remove_vfs_state_request(ProcessId::empty(), transaction_id);
    }

    serial::write_str("Scheduler blocked: proc=");
    serial::write_str(current);
    serial::write_str(" no ready process for VFS filesystem service transaction\n");
    Err(IpcError::Empty)
}

fn restore_current_vfs_state_waiter(reply_endpoint: KernelObjectId) {
    let runtime = runtime();
    if let Some(process) = runtime.processes.current_process_mut()
        && let ProcessState::BlockedOnVfsState {
            reply_endpoint: waiting_endpoint,
            ..
        } = process.state
        && waiting_endpoint == reply_endpoint
    {
        process.state = ProcessState::Running;
    }
}

fn start_vertexfs_sync_transaction(
    backing: usize,
    inode_id: u32,
    checksum: u32,
    write_count: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    if write_count == 0 || write_count > VERTEXFS_SYNC_MAX_DEVICE_WRITES {
        return Err(IpcError::VfsUnsupported);
    }
    let (request_endpoint, reply_endpoint, first_sector) = {
        let runtime = runtime();
        let request_endpoint = runtime
            .vertexfs_device_request_endpoint
            .ok_or(IpcError::VfsUnsupported)?;
        let reply_endpoint = runtime
            .vertexfs_device_reply_endpoint
            .ok_or(IpcError::VfsUnsupported)?;
        if blocked_vertexfs_sync_waiter_index(reply_endpoint).is_some() {
            return Err(IpcError::VfsBusy);
        }
        let first_sector = vertexfs_device_absolute_sector(runtime.vertexfs_sync_writes[0].sector)?;
        (request_endpoint, reply_endpoint, first_sector)
    };

    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return Err(IpcError::VfsPermission);
        };
        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::BlockedOnVertexFsSync {
            request_endpoint,
            reply_endpoint,
            backing,
            inode_id,
            checksum,
            write_count,
            next_write: 1,
            expected_sector: first_sector,
        };
        process.name
    };

    if let Err(error) = queue_vertexfs_device_write(request_endpoint, 0) {
        restore_current_vertexfs_sync_waiter(reply_endpoint);
        return Err(error);
    }
    wake_blocked_receiver(request_endpoint);

    serial::write_str("VertexFS v1 fsync device transaction started: proc=");
    serial::write_str(current);
    serial::write_str(" inode=");
    serial::write_u64_dec(inode_id as u64);
    serial::write_str(" sectors=");
    serial::write_u64_dec(write_count as u64);
    serial::write_str("\n");

    if schedule_next_ready(frame) {
        return Ok(());
    }

    restore_current_vertexfs_sync_waiter(reply_endpoint);
    if let Some(endpoint) = runtime().objects.get_endpoint_mut(request_endpoint) {
        let _ = endpoint.remove_all_from_sender(ProcessId::empty());
    }
    serial::write_str("Scheduler blocked: proc=");
    serial::write_str(current);
    serial::write_str(" no ready process for VertexFS device sync\n");
    Err(IpcError::Empty)
}

fn restore_current_vertexfs_sync_waiter(reply_endpoint: KernelObjectId) {
    let runtime = runtime();
    if let Some(process) = runtime.processes.current_process_mut()
        && let ProcessState::BlockedOnVertexFsSync {
            reply_endpoint: waiting_endpoint,
            ..
        } = process.state
        && waiting_endpoint == reply_endpoint
    {
        process.state = ProcessState::Running;
    }
}

fn queue_vertexfs_device_write(
    request_endpoint: KernelObjectId,
    write_index: usize,
) -> Result<u64, IpcError> {
    let write = {
        let runtime = runtime();
        if write_index >= runtime.vertexfs_sync_write_count {
            return Err(IpcError::VfsBadHandle);
        }
        runtime.vertexfs_sync_writes[write_index]
    };
    let absolute_sector = vertexfs_device_absolute_sector(write.sector)?;
    let mut request = [0u8; MAX_MESSAGE_BYTES];
    write_u16_le(&mut request, 0, BLOCK_PROTOCOL_V1);
    write_u16_le(&mut request, 2, BLOCK_OP_WRITE_SECTOR);
    write_u16_le(&mut request, 4, 0);
    write_u64_le(&mut request, 8, absolute_sector);

    let enqueue_result = {
        let runtime = runtime();
        let endpoint = runtime
            .objects
            .get_endpoint_mut(request_endpoint)
            .ok_or(IpcError::BadCapability)?;
        endpoint.enqueue(ProcessId::empty(), &request, BLOCK_REQUEST_LEN)?;
        let mut payload = [0u8; MAX_MESSAGE_BYTES];
        payload.copy_from_slice(&write.bytes);
        if let Err(error) = endpoint.enqueue(ProcessId::empty(), &payload, VERTEXFS_SECTOR_SIZE) {
            let _ = endpoint.remove_all_from_sender(ProcessId::empty());
            return Err(error);
        }
        Ok(())
    };
    enqueue_result?;
    Ok(absolute_sector)
}

fn vertexfs_device_absolute_sector(vertexfs_sector: u64) -> Result<u64, IpcError> {
    if vertexfs_sector >= VERTEXFS_SECTORS as u64 {
        return Err(IpcError::VfsUnsupported);
    }
    VERTEXDISK_VERTEXFS_IMAGE_SECTOR
        .checked_add(vertexfs_sector)
        .ok_or(IpcError::VfsUnsupported)
}

fn vertexfs_device_ack_ok(message: IpcMessage, expected_sector: u64) -> bool {
    message.len == BLOCK_WRITE_ACK_LEN
        && read_u16_le(&message.bytes, 0) == BLOCK_PROTOCOL_V1
        && read_u16_le(&message.bytes, 2) == BLOCK_OP_WRITE_SECTOR
        && read_u16_le(&message.bytes, 4) == 0
        && read_u64_le(&message.bytes, 8) == expected_sector
}

fn abort_vfs_state_transactions(status: u64) {
    let (request_endpoint, reply_endpoint) = {
        let runtime = runtime();
        (
            runtime.state_vfs_request_endpoint,
            runtime.state_vfs_reply_endpoint,
        )
    };
    if let Some(request_endpoint) = request_endpoint
        && let Some(endpoint) = runtime().objects.get_endpoint_mut(request_endpoint)
    {
        let removed = endpoint.remove_all_from_sender(ProcessId::empty());
        if removed > 0 {
            serial::write_str("VFS state transaction requests dropped: count=");
            serial::write_u64_dec(removed as u64);
            serial::write_str("\n");
        }
    }
    let Some(reply_endpoint) = reply_endpoint else {
        return;
    };

    let mut aborted = 0;
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index].as_mut()
            && let ProcessState::BlockedOnVfsState {
                reply_endpoint: waiting_endpoint,
                operation,
                ..
            } = process.state
            && waiting_endpoint == reply_endpoint
        {
            process.saved_frame.rax = status;
            process.state = ProcessState::Ready;
            aborted += 1;
            serial::write_str("VFS state transaction aborted: proc=");
            serial::write_str(process.name);
            serial::write_str(" op=");
            serial::write_str(vfs_state_operation_label(operation));
            serial::write_str(" status=");
            serial::write_u64_dec(status);
            serial::write_str("\n");
        }
        index += 1;
    }
    if aborted > 0 {
        serial::write_str("VFS state transaction abort wake count=");
        serial::write_u64_dec(aborted);
        serial::write_str("\n");
    }
}

fn vfs_state_operation_label(operation: VfsStateOperation) -> &'static str {
    match operation {
        VfsStateOperation::Read => "read",
        VfsStateOperation::Stat => "stat",
        VfsStateOperation::Write => "write",
        VfsStateOperation::Control => "control",
        VfsStateOperation::ServiceRead => "service-read",
    }
}

fn abort_vertexfs_sync_transactions(status: u64) {
    let (request_endpoint, reply_endpoint) = {
        let runtime = runtime();
        (
            runtime.vertexfs_device_request_endpoint,
            runtime.vertexfs_device_reply_endpoint,
        )
    };
    if let Some(endpoint_id) = request_endpoint
        && let Some(endpoint) = runtime().objects.get_endpoint_mut(endpoint_id)
    {
        let _ = endpoint.remove_all_from_sender(ProcessId::empty());
    }
    let Some(reply_endpoint) = reply_endpoint else {
        return;
    };
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index].as_mut()
            && let ProcessState::BlockedOnVertexFsSync {
                reply_endpoint: waiting_endpoint,
                ..
            } = process.state
            && waiting_endpoint == reply_endpoint
        {
            process.saved_frame.rax = status;
            process.state = ProcessState::Ready;
            serial::write_str("VertexFS v1 fsync device transaction aborted: proc=");
            serial::write_str(process.name);
            serial::write_str("\n");
        }
        index += 1;
    }
}

fn vfs_state_operation_code(operation: VfsStateOperation) -> u8 {
    match operation {
        VfsStateOperation::Read => VFS_STATE_OP_READ_VALUE,
        VfsStateOperation::Stat => VFS_STATE_OP_STAT_VALUE,
        VfsStateOperation::Write => VFS_STATE_OP_WRITE_VALUE,
        VfsStateOperation::Control => VFS_STATE_OP_CONTROL,
        VfsStateOperation::ServiceRead => VFS_SERVICE_OP_READ_REPORT,
    }
}

fn block_current_on_vfs_read(
    node: VfsNodeId,
    description: FileDescriptionId,
    destination: u64,
    max_len: usize,
    frame: &mut SyscallFrame,
) -> bool {
    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return false;
        };

        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::BlockedOnVfsRead {
            node,
            description,
            destination,
            max_len,
        };
        process.name
    };

    serial::write_str("VFS read blocked: proc=");
    serial::write_str(current);
    serial::write_str(" vnode=");
    serial::write_u64_dec(node.raw());
    serial::write_str(" description=");
    serial::write_u64_dec(description.raw());
    serial::write_str("\n");

    if schedule_next_ready(frame) {
        true
    } else {
        let runtime = runtime();
        if let Some(process) = runtime.processes.current_process_mut() {
            process.state = ProcessState::Running;
        }

        serial::write_str("Scheduler blocked: proc=");
        serial::write_str(current);
        serial::write_str(" no ready process\n");
        false
    }
}

fn block_current_on_network_port(
    port: KernelObjectId,
    destination: u64,
    max_len: usize,
    frame: &mut SyscallFrame,
) -> bool {
    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return false;
        };

        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::BlockedOnNetworkPort {
            port,
            destination,
            max_len,
        };
        process.name
    };

    serial::write_str("Network-port receive blocked: proc=");
    serial::write_str(current);
    serial::write_str(" port=");
    serial::write_u64_dec(port.raw());
    serial::write_str("\n");

    if schedule_next_ready(frame) {
        true
    } else {
        let runtime = runtime();
        if let Some(process) = runtime.processes.current_process_mut() {
            process.state = ProcessState::Running;
        }

        serial::write_str("Scheduler blocked: proc=");
        serial::write_str(current);
        serial::write_str(" no ready process for network-port receive\n");
        false
    }
}

fn wake_blocked_network_receiver(port: KernelObjectId) {
    let Some(waiter_index) = blocked_network_receiver_index(port) else {
        return;
    };

    let (name, receiver_cr3, destination, max_len, current_cr3) = {
        let runtime = runtime();
        let Some(waiter) = runtime.processes.processes[waiter_index] else {
            return;
        };
        let ProcessState::BlockedOnNetworkPort {
            destination,
            max_len,
            ..
        } = waiter.state
        else {
            return;
        };

        let current_cr3 = runtime
            .processes
            .current_process()
            .map(|process| process.context.cr3)
            .unwrap_or_else(paging::active_root_table_physical);

        (
            waiter.name,
            waiter.context.cr3,
            destination,
            max_len,
            current_cr3,
        )
    };

    let (port_name, message) = {
        let runtime = runtime();
        let Some(port_object) = runtime.objects.get_network_port_mut(port) else {
            return;
        };
        let Some(message) = port_object.dequeue_udp() else {
            return;
        };
        (port_object.name, message)
    };

    let copy_len = min(message.len, max_len);
    let copy_result = unsafe {
        gdt::switch_address_space(receiver_cr3);
        let result = usercopy::copy_to_user(UserPtr::new(destination), &message.bytes[..copy_len]);
        gdt::switch_address_space(current_cr3);
        result
    };

    match copy_result {
        Ok(()) => {
            let runtime = runtime();
            if let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() {
                waiter.saved_frame.rax = copy_len as u64;
                waiter.state = ProcessState::Ready;
            }
            serial::write_str("Network-port UDP request delivered to netstack: network-port=");
            serial::write_str(port_name);
            serial::write_str(" bytes=");
            serial::write_u64_dec(copy_len as u64);
            serial::write_str("\n");
            serial::write_str("Network-port receive wake: proc=");
            serial::write_str(name);
            serial::write_str("\n");
        }
        Err(_) => {
            let runtime = runtime();
            if let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() {
                waiter.saved_frame.rax = STATUS_BAD_BUFFER;
                waiter.state = ProcessState::Ready;
            }
            serial::write_str("Network-port receive wake failed: bad user buffer proc=");
            serial::write_str(name);
            serial::write_str("\n");
        }
    }
}

fn wake_blocked_vfs_pipe_read(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let Some(waiter_index) = blocked_vfs_pipe_reader_index() else {
        return false;
    };

    let (name, reader_cr3, destination, max_len, description, node, current_cr3) = {
        let runtime = runtime();
        let Some(waiter) = runtime.processes.processes[waiter_index] else {
            return false;
        };
        let ProcessState::BlockedOnVfsRead {
            node,
            description,
            destination,
            max_len,
        } = waiter.state
        else {
            return false;
        };

        let current_cr3 = runtime
            .processes
            .current_process()
            .map(|process| process.context.cr3)
            .unwrap_or_else(paging::active_root_table_physical);

        (
            waiter.name,
            waiter.context.cr3,
            destination,
            max_len,
            description,
            node,
            current_cr3,
        )
    };

    let copy_len = min(bytes.len(), max_len);
    let copy_result = unsafe {
        gdt::switch_address_space(reader_cr3);
        let result = usercopy::copy_to_user(UserPtr::new(destination), &bytes[..copy_len]);
        gdt::switch_address_space(current_cr3);
        result
    };

    match copy_result {
        Ok(()) => {
            let file_name = {
                let runtime = runtime();
                runtime
                    .vfs_node(node)
                    .map(|node| node.name)
                    .unwrap_or_else(VfsName::empty)
            };
            {
                let runtime = runtime();
                if let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() {
                    waiter.saved_frame.rax = copy_len as u64;
                    waiter.state = ProcessState::Ready;
                }
            }

            serial::write_str("VFS pipe wake reader: proc=");
            serial::write_str(name);
            serial::write_str(" file=");
            serial_write_vfs_name(file_name);
            serial::write_str(" description=");
            serial::write_u64_dec(description.raw());
            serial::write_str(" bytes=");
            serial::write_u64_dec(copy_len as u64);
            serial::write_str("\n");
            true
        }
        Err(_) => {
            {
                let runtime = runtime();
                if let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() {
                    waiter.saved_frame.rax = STATUS_BAD_BUFFER;
                    waiter.state = ProcessState::Ready;
                }
            }
            serial::write_str("VFS pipe wake reader failed: bad user buffer proc=");
            serial::write_str(name);
            serial::write_str("\n");
            true
        }
    }
}

fn wake_blocked_vfs_state_reply(endpoint: KernelObjectId) {
    let Some(waiter_index) = blocked_vfs_state_waiter_index(endpoint) else {
        return;
    };

    let (
        name,
        receiver_pid,
        receiver_cr3,
        destination,
        max_len,
        operation,
        transaction_id,
        offset,
        update_offset,
        write_len,
        description,
        node,
        current_cr3,
    ) = {
        let runtime = runtime();
        let Some(waiter) = runtime.processes.processes[waiter_index] else {
            return;
        };
        let ProcessState::BlockedOnVfsState {
            destination,
            max_len,
            operation,
            transaction_id,
            offset,
            update_offset,
            write_len,
            description,
            node,
            ..
        } = waiter.state
        else {
            return;
        };

        let current_cr3 = runtime
            .processes
            .current_process()
            .map(|process| process.context.cr3)
            .unwrap_or_else(paging::active_root_table_physical);

        (
            waiter.name,
            waiter.pid,
            waiter.context.cr3,
            destination,
            max_len,
            operation,
            transaction_id,
            offset,
            update_offset,
            write_len,
            description,
            node,
            current_cr3,
        )
    };

    let Some(message) = ({
        let runtime = runtime();
        let Some(endpoint_object) = runtime.objects.get_endpoint_mut(endpoint) else {
            return;
        };
        endpoint_object.dequeue_vfs_state_reply_for(receiver_pid, transaction_id)
    }) else {
        return;
    };

    let result = match operation {
        VfsStateOperation::Read | VfsStateOperation::ServiceRead => wake_blocked_vfs_state_read(
            receiver_cr3,
            current_cr3,
            destination,
            max_len,
            offset,
            description,
            update_offset,
            message,
        ),
        VfsStateOperation::Stat => wake_blocked_vfs_state_stat(
            receiver_cr3,
            current_cr3,
            destination,
            max_len,
            description,
            node,
            message,
        ),
        VfsStateOperation::Write => {
            wake_blocked_vfs_state_write(offset, description, update_offset, write_len, message)
        }
        VfsStateOperation::Control => {
            wake_blocked_vfs_state_write(offset, description, update_offset, write_len, message)
        }
    };

    {
        let runtime = runtime();
        if let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() {
            waiter.saved_frame.rax = result;
            waiter.state = ProcessState::Ready;
        }
    }

    let file_name = {
        let runtime = runtime();
        runtime
            .vfs_node(node)
            .map(|node| node.name)
            .unwrap_or_else(VfsName::empty)
    };
    if operation == VfsStateOperation::ServiceRead {
        serial::write_str("VFS filesystem service transaction wake: proc=");
    } else {
        serial::write_str("VFS state transaction wake: proc=");
    }
    serial::write_str(name);
    serial::write_str(" file=");
    serial_write_vfs_name(file_name);
    serial::write_str(" op=");
    serial::write_str(vfs_state_operation_label(operation));
    serial::write_str(" result=");
    serial::write_u64_dec(result);
    serial::write_str("\n");
}

fn wake_blocked_vertexfs_sync_reply(endpoint: KernelObjectId) {
    let Some(waiter_index) = blocked_vertexfs_sync_waiter_index(endpoint) else {
        return;
    };

    let (
        name,
        receiver_pid,
        request_endpoint,
        backing,
        inode_id,
        checksum,
        write_count,
        next_write,
        expected_sector,
    ) = {
        let runtime = runtime();
        let Some(waiter) = runtime.processes.processes[waiter_index] else {
            return;
        };
        let ProcessState::BlockedOnVertexFsSync {
            request_endpoint,
            backing,
            inode_id,
            checksum,
            write_count,
            next_write,
            expected_sector,
            ..
        } = waiter.state
        else {
            return;
        };
        (
            waiter.name,
            waiter.pid,
            request_endpoint,
            backing,
            inode_id,
            checksum,
            write_count,
            next_write,
            expected_sector,
        )
    };

    let Some(message) = ({
        let runtime = runtime();
        let Some(endpoint_object) = runtime.objects.get_endpoint_mut(endpoint) else {
            return;
        };
        endpoint_object.dequeue_for(receiver_pid)
    }) else {
        return;
    };

    if !vertexfs_device_ack_ok(message, expected_sector) {
        if let Some(waiter) = runtime().processes.processes[waiter_index].as_mut() {
            waiter.saved_frame.rax = STATUS_VFS_UNSUPPORTED;
            waiter.state = ProcessState::Ready;
        }
        serial::write_str("VertexFS v1 fsync device write rejected: proc=");
        serial::write_str(name);
        serial::write_str(" sector=");
        serial::write_u64_dec(expected_sector);
        serial::write_str("\n");
        return;
    }

    if next_write < write_count {
        let Ok(next_sector) = queue_vertexfs_device_write(request_endpoint, next_write) else {
            if let Some(waiter) = runtime().processes.processes[waiter_index].as_mut() {
                waiter.saved_frame.rax = STATUS_VFS_UNSUPPORTED;
                waiter.state = ProcessState::Ready;
            }
            serial::write_str("VertexFS v1 fsync device queue failed: proc=");
            serial::write_str(name);
            serial::write_str("\n");
            return;
        };
        if let Some(waiter) = runtime().processes.processes[waiter_index].as_mut()
            && let ProcessState::BlockedOnVertexFsSync {
                next_write: waiting_next_write,
                expected_sector: waiting_expected_sector,
                ..
            } = &mut waiter.state
        {
            *waiting_next_write = next_write + 1;
            *waiting_expected_sector = next_sector;
        }
        wake_blocked_receiver(request_endpoint);
        return;
    }

    let result = runtime().finish_vertexfs_sync_file(backing, checksum);
    if let Some(waiter) = runtime().processes.processes[waiter_index].as_mut() {
        waiter.saved_frame.rax = if result.is_ok() {
            STATUS_OK
        } else {
            STATUS_VFS_BAD_HANDLE
        };
        waiter.state = ProcessState::Ready;
    }
    serial::write_str("VertexFS v1 fsync device transaction committed: proc=");
    serial::write_str(name);
    serial::write_str(" inode=");
    serial::write_u64_dec(inode_id as u64);
    serial::write_str(" sectors=");
    serial::write_u64_dec(write_count as u64);
    serial::write_str(" checksum=");
    serial::write_u64_dec(checksum as u64);
    serial::write_str("\n");
}

fn wake_blocked_vfs_state_read(
    receiver_cr3: u64,
    current_cr3: u64,
    destination: u64,
    max_len: usize,
    offset: u64,
    description: FileDescriptionId,
    update_offset: bool,
    message: IpcMessage,
) -> u64 {
    if message.len < VFS_STATE_TRANSACTION_ID_BYTES {
        return STATUS_VFS_UNSUPPORTED;
    }
    let payload_len = message.len - VFS_STATE_TRANSACTION_ID_BYTES;
    let start = min(usize::try_from(offset).unwrap_or(usize::MAX), payload_len);
    let copy_len = min(payload_len - start, max_len);
    let payload_start = VFS_STATE_TRANSACTION_ID_BYTES + start;
    let copy_result = unsafe {
        gdt::switch_address_space(receiver_cr3);
        let result = usercopy::copy_to_user(
            UserPtr::new(destination),
            &message.bytes[payload_start..payload_start + copy_len],
        );
        gdt::switch_address_space(current_cr3);
        result
    };
    if copy_result.is_err() {
        return STATUS_BAD_BUFFER;
    }
    if update_offset {
        let Some(new_offset) = offset.checked_add(copy_len as u64) else {
            return STATUS_VFS_UNSUPPORTED;
        };
        let Some(file) = runtime().file_description_mut(description) else {
            return STATUS_VFS_BAD_HANDLE;
        };
        file.offset = new_offset;
    }
    copy_len as u64
}

fn wake_blocked_vfs_state_stat(
    receiver_cr3: u64,
    current_cr3: u64,
    destination: u64,
    max_len: usize,
    description: FileDescriptionId,
    node_id: VfsNodeId,
    message: IpcMessage,
) -> u64 {
    if max_len < VFS_STAT_BYTES || message.len != VFS_STATE_TRANSACTION_ID_BYTES + 8 {
        return STATUS_VFS_UNSUPPORTED;
    }
    let node = {
        let runtime = runtime();
        let Some(node) = runtime.vfs_node(node_id) else {
            return STATUS_VFS_BAD_HANDLE;
        };
        node
    };
    let rights = {
        let runtime = runtime();
        let Some(file) = runtime.file_description(description) else {
            return STATUS_VFS_BAD_HANDLE;
        };
        file.rights
    };
    let mut stat = [0u8; VFS_STAT_BYTES];
    write_vfs_stat_record(
        &mut stat,
        node,
        read_u64_le(&message.bytes, VFS_STATE_TRANSACTION_ID_BYTES),
        rights,
    );
    let copy_result = unsafe {
        gdt::switch_address_space(receiver_cr3);
        let result = usercopy::copy_to_user(UserPtr::new(destination), &stat);
        gdt::switch_address_space(current_cr3);
        result
    };
    if copy_result.is_err() {
        return STATUS_BAD_BUFFER;
    }
    VFS_STAT_BYTES as u64
}

fn wake_blocked_vfs_state_write(
    offset: u64,
    description: FileDescriptionId,
    update_offset: bool,
    write_len: usize,
    message: IpcMessage,
) -> u64 {
    if message.len != VFS_STATE_TRANSACTION_ID_BYTES + 2
        || message.bytes[VFS_STATE_TRANSACTION_ID_BYTES] != b'O'
        || message.bytes[VFS_STATE_TRANSACTION_ID_BYTES + 1] != b'K'
    {
        return STATUS_VFS_UNSUPPORTED;
    }
    if update_offset {
        let Some(new_offset) = offset.checked_add(write_len as u64) else {
            return STATUS_VFS_UNSUPPORTED;
        };
        let Some(file) = runtime().file_description_mut(description) else {
            return STATUS_VFS_BAD_HANDLE;
        };
        file.offset = new_offset;
    }
    write_len as u64
}

fn blocked_vfs_state_waiter_index(endpoint: KernelObjectId) -> Option<usize> {
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && let ProcessState::BlockedOnVfsState {
                reply_endpoint,
                transaction_id,
                ..
            } = process.state
            && reply_endpoint == endpoint
            && runtime
                .objects
                .get_endpoint(endpoint)
                .map(|endpoint_object| {
                    endpoint_object.has_vfs_state_reply_for(process.pid, transaction_id)
                })
                .unwrap_or(false)
        {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn blocked_vertexfs_sync_waiter_index(endpoint: KernelObjectId) -> Option<usize> {
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && let ProcessState::BlockedOnVertexFsSync { reply_endpoint, .. } = process.state
            && reply_endpoint == endpoint
            && runtime
                .objects
                .get_endpoint(endpoint)
                .map(|endpoint_object| endpoint_object.has_message_for(process.pid))
                .unwrap_or(false)
        {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn blocked_vfs_pipe_reader_index() -> Option<usize> {
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && let ProcessState::BlockedOnVfsRead { node, .. } = process.state
            && runtime
                .vfs_node(node)
                .map(|node| matches!(node.backing, VfsBacking::Pipe))
                .unwrap_or(false)
        {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn blocked_network_receiver_index(port: KernelObjectId) -> Option<usize> {
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && let ProcessState::BlockedOnNetworkPort {
                port: waiting_port, ..
            } = process.state
            && waiting_port == port
            && runtime
                .objects
                .get_network_port(port)
                .map(|port_object| port_object.queue_len > 0)
                .unwrap_or(false)
        {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn schedule_next_ready(frame: &mut SyscallFrame) -> bool {
    schedule_next_ready_inner(frame, true, true)
}

fn schedule_next_ready_excluding_current(frame: &mut SyscallFrame) -> bool {
    schedule_next_ready_inner(frame, false, true)
}

fn schedule_next_ready_no_wait_excluding_current(frame: &mut SyscallFrame) -> bool {
    schedule_next_ready_inner(frame, false, false)
}

fn schedule_next_ready_inner(
    frame: &mut SyscallFrame,
    include_current: bool,
    wait_for_deadline: bool,
) -> bool {
    wake_timed_processes(read_tsc());
    if wait_for_deadline
        && runtime()
            .processes
            .next_ready_index_round_robin(include_current)
            .is_none()
        && let Some(deadline) = next_deadline_tsc()
    {
        wait_until_deadline(deadline, include_current);
        wake_timed_processes(read_tsc());
    }

    if !wait_for_deadline
        && runtime()
            .processes
            .next_ready_index_round_robin(include_current)
            .is_none()
    {
        return false;
    }

    let (from, to, next_frame, next_cr3) = {
        let runtime = runtime();
        let from = runtime
            .processes
            .current_process()
            .map(|process| process.name)
            .unwrap_or("<none>");
        let Some(next_index) = runtime
            .processes
            .next_ready_index_round_robin(include_current)
        else {
            return false;
        };
        let (next_pid, to, next_frame, next_cr3) = {
            let Some(next) = runtime.processes.processes[next_index].as_mut() else {
                return false;
            };

            next.state = ProcessState::Running;

            let next_frame = if next.has_saved_frame {
                next.saved_frame
            } else {
                SyscallFrame::from_context(next.context)
            };

            (next.pid, next.name, next_frame, next.context.cr3)
        };

        runtime.processes.current = Some(next_pid);

        (from, to, next_frame, next_cr3)
    };

    *frame = next_frame;

    serial::write_str("Scheduler switch: from=");
    serial::write_str(from);
    serial::write_str(" to=");
    serial::write_str(to);
    serial::write_str("\n");

    unsafe {
        gdt::switch_address_space(next_cr3);
    }

    true
}

fn endpoint_from_cap(cap_slot: u64, required_right: u64) -> Result<KernelObjectId, IpcError> {
    endpoint_cap_from_slot(cap_slot, required_right).map(|cap| cap.object)
}

fn endpoint_cap_from_slot(cap_slot: u64, required_right: u64) -> Result<Capability, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;

    match runtime().objects.get_endpoint(cap.object) {
        Some(_) => Ok(cap),
        None => Err(IpcError::BadCapability),
    }
}

fn serial_log_endpoint_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<IpcEndpoint, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    let runtime = runtime();
    let log_endpoint_id = runtime.endpoint_ids[0].ok_or(IpcError::BadCapability)?;
    if cap.object != log_endpoint_id {
        return Err(IpcError::BadCapability);
    }
    let endpoint = runtime
        .objects
        .get_endpoint(cap.object)
        .ok_or(IpcError::BadCapability)?;
    if endpoint.name != LOG_ENDPOINT_NAME {
        return Err(IpcError::BadCapability);
    }
    Ok(endpoint)
}

fn boot_module_from_cap(cap_slot: u64, required_right: u64) -> Result<BootModuleObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_boot_module(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn timer_from_cap(cap_slot: u64, required_right: u64) -> Result<TimerObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_timer(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn network_port_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<NetworkPortObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_network_port(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn io_port_from_cap(cap_slot: u64, required_right: u64) -> Result<IoPortRangeObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_io_port(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn mmio_region_from_cap(cap_slot: u64, required_right: u64) -> Result<MmioRegionObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_mmio_region(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn interrupt_line_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<InterruptLineObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_interrupt_line(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn dma_region_from_cap(cap_slot: u64, required_right: u64) -> Result<DmaRegionObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_dma_region(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn virtio_device_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<VirtioDeviceObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_virtio_device(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn namespace_from_cap(cap_slot: u64, required_right: u64) -> Result<NamespaceObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_namespace(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn process_control_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<ProcessControlObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_process_control(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn secret_from_cap(cap_slot: u64, required_right: u64) -> Result<SecretObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_secret(cap.object)
        .ok_or(IpcError::BadCapability)
}

fn lookup_capability(cap_slot: u64, required_right: u64) -> Result<Capability, IpcError> {
    let runtime = runtime();
    let process = runtime
        .processes
        .current_process()
        .ok_or(IpcError::BadCapability)?;
    let cap = process
        .caps
        .lookup(cap_slot)
        .ok_or(IpcError::BadCapability)?;

    if cap.revoked
        || runtime.cap_id_revoked(cap.id)
        || capability_has_revoked_ancestor(runtime, cap)
        || cap.generation_id != runtime.generation_id
    {
        return Err(IpcError::BadCapability);
    }

    if required_right != 0 && cap.rights & required_right != required_right {
        return Err(IpcError::BadCapability);
    }

    Ok(cap)
}

fn port_in_range(range: IoPortRangeObject, port: u64) -> bool {
    port >= range.base
        && port
            .checked_sub(range.base)
            .map(|offset| offset < range.length)
            .unwrap_or(false)
}

fn port_span_in_range(range: IoPortRangeObject, port: u64, width: u64) -> bool {
    if width == 0 {
        return false;
    }
    let Some(last_port) = port.checked_add(width - 1) else {
        return false;
    };
    if last_port > u16::MAX as u64 {
        return false;
    }
    port_in_range(range, port) && port_in_range(range, last_port)
}

fn capability_has_revoked_ancestor(runtime: &RuntimeState, cap: Capability) -> bool {
    let mut parent = cap.parent_cap_id;
    while parent != 0 {
        if runtime.cap_id_revoked(parent) {
            return true;
        }
        parent = find_cap_parent(runtime, parent).unwrap_or(0);
    }
    false
}

fn find_cap_parent(runtime: &RuntimeState, cap_id: u64) -> Option<u64> {
    if let Some(parent) = runtime.cap_parent_from_lineage(cap_id) {
        return Some(parent);
    }

    let mut process_index = 0;
    while process_index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[process_index] {
            if let Some(parent) = find_cap_parent_in_space(process.caps, cap_id) {
                return Some(parent);
            }
            if let Some(parent) = find_cap_parent_in_space(process.initial_caps, cap_id) {
                return Some(parent);
            }
        }
        process_index += 1;
    }
    None
}

fn find_cap_parent_in_space(space: CapabilitySpace, cap_id: u64) -> Option<u64> {
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = space.caps[slot]
            && cap.id == cap_id
        {
            return Some(cap.parent_cap_id);
        }
        slot += 1;
    }
    None
}

fn build_inspect_report(runtime: &RuntimeState, report: &mut InspectReport) {
    report.push_str("native-runtime-report v=1\n");
    report.push_str("generation=");
    report.push_str(runtime.generation_id);
    report.push_byte(b'\n');
    write_generation_manager_report(report);
    write_graph_store_report(runtime, report);
    report.push_str("processes=");
    report.push_u64_dec(runtime.processes.count as u64);
    report.push_byte(b'\n');
    report.push_str("objects=");
    report.push_u64_dec(runtime.objects.live_count() as u64);
    report.push_byte(b'\n');
    report.push_str("vfs_nodes=");
    report.push_u64_dec(runtime.vfs_node_count as u64);
    report.push_str(" file_handles=");
    report.push_u64_dec(runtime_file_handle_count(runtime));
    report.push_byte(b'\n');
    report.push_str("caps=");
    report.push_u64_dec(runtime_cap_count(runtime));
    report.push_byte(b'\n');
    report.push_str("objects_unreachable=");
    report.push_u64_dec(unreachable_object_count(runtime));
    report.push_byte(b'\n');
    write_unreachable_object_report(runtime, report);
    write_vfs_report(runtime, report);
    if let Some(stats) = frame_allocator_stats() {
        report.push_str("frames total=");
        report.push_u64_dec(stats.total_frames);
        report.push_str(" allocated=");
        report.push_u64_dec(stats.allocated_frames);
        report.push_str(" free=");
        report.push_u64_dec(stats.free_frames);
        report.push_str(" reserved=0 reclaimed=");
        report.push_u64_dec(stats.reclaimed_frames);
        report.push_str(" high_water=");
        report.push_u64_dec(stats.high_water_frames);
        report.push_str(" failed_allocations=");
        report.push_u64_dec(stats.failed_allocations);
        report.push_str(" recycled=");
        report.push_u64_dec(stats.recycled_frames as u64);
        report.push_str(" ledger_entries=");
        report.push_u64_dec(stats.ledger_entries as u64);
        report.push_str(" owner_kernel=");
        report.push_u64_dec(stats.kernel_frames);
        report.push_str(" owner_page_table=");
        report.push_u64_dec(stats.page_table_frames);
        report.push_str(" owner_process=");
        report.push_u64_dec(stats.process_memory_frames);
        report.push_str(" owner_dma=");
        report.push_u64_dec(stats.dma_frames);
        report.push_str(" owner_scratch=");
        report.push_u64_dec(stats.scratch_frames);
        report.push_byte(b'\n');
    }

    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index] {
            report.push_str("process[");
            report.push_u64_dec(index as u64);
            report.push_str("] name=");
            report.push_str(process.name);
            report.push_str(" pid=");
            report.push_u64_dec(process.pid.raw());
            report.push_str(" state=");
            report.push_str(process.state.label());
            report.push_str(" mount_root=");
            report.push_bytes(process.mount_root.as_bytes());
            report.push_str(" context_reaped=");
            if process.context_reaped {
                report.push_str("yes");
            } else {
                report.push_str("no");
            }
            report.push_str(" cr3=");
            report.push_u64_dec(process.context.cr3);
            report.push_str(" generation=");
            report.push_str(runtime.generation_id);
            report.push_str(" graph_node=");
            report.push_str(process_graph_node(runtime, process.name));
            report.push_byte(b'\n');

            write_capability_space_report(runtime, report, process, "current", process.caps);
            write_capability_space_report(
                runtime,
                report,
                process,
                "initial",
                process.initial_caps,
            );
        }
        index += 1;
    }

    let mut interrupt_report_index = 0;
    let mut object_index = 0;
    while object_index < runtime.objects.count {
        if let Some(KernelObject::InterruptLine(_)) = runtime.objects.objects[object_index] {
            interrupt_report_index += 1;
        }
        object_index += 1;
    }

    report.push_str("interrupt_lines=");
    report.push_u64_dec(interrupt_report_index);
    report.push_byte(b'\n');
    interrupt_report_index = 0;
    object_index = 0;
    while object_index < runtime.objects.count {
        if let Some(KernelObject::InterruptLine(line)) = runtime.objects.objects[object_index] {
            report.push_str("interrupt-line[");
            report.push_u64_dec(interrupt_report_index);
            report.push_str("] name=");
            report.push_str(line.name);
            report.push_str(" line=");
            report.push_u64_dec(line.line);
            report.push_str(" owner=");
            report.push_str(interrupt_owner_name(runtime, line.id));
            report.push_str(" pending=");
            report.push_u64_dec(line.pending_count);
            report.push_str(" delivered=");
            report.push_u64_dec(line.delivered_count);
            report.push_str(" waiters=");
            report.push_u64_dec(interrupt_waiter_count(runtime, line.id));
            report.push_str(" spurious=");
            report.push_u64_dec(line.spurious_count);
            report.push_byte(b'\n');
            interrupt_report_index += 1;
        }
        object_index += 1;
    }

    let mut dma_report_index = 0;
    object_index = 0;
    while object_index < runtime.objects.count {
        if let Some(KernelObject::DmaRegion(_)) = runtime.objects.objects[object_index] {
            dma_report_index += 1;
        }
        object_index += 1;
    }

    report.push_str("dma_regions=");
    report.push_u64_dec(dma_report_index);
    report.push_byte(b'\n');
    dma_report_index = 0;
    object_index = 0;
    while object_index < runtime.objects.count {
        if let Some(KernelObject::DmaRegion(region)) = runtime.objects.objects[object_index] {
            report.push_str("dma-region[");
            report.push_u64_dec(dma_report_index);
            report.push_str("] name=");
            report.push_str(region.name);
            report.push_str(" base=");
            report.push_u64_dec(region.base);
            report.push_str(" length=");
            report.push_u64_dec(region.length);
            report.push_str(" owner=");
            report.push_str(process_name_by_pid(runtime, region.mapped_by));
            report.push_str(" mapped=");
            report.push_str(if region.mapped_by == ProcessId::empty() {
                "no"
            } else {
                "yes"
            });
            report.push_str(" map_count=");
            report.push_u64_dec(region.map_count);
            report.push_str(" release_count=");
            report.push_u64_dec(region.release_count);
            report.push_byte(b'\n');
            dma_report_index += 1;
        }
        object_index += 1;
    }

    write_virtio_runtime_report(runtime, report);

    report.push_str("service_lifecycle_events=");
    report.push_u64_dec(runtime.service_lifecycle_event_count as u64);
    report.push_byte(b'\n');
    let mut event_index = 0;
    while event_index < runtime.service_lifecycle_event_count {
        if let Some(event) = runtime.service_lifecycle_events[event_index] {
            report.push_str("service-lifecycle[");
            report.push_u64_dec(event_index as u64);
            report.push_str("] generation=");
            report.push_str(runtime.generation_id);
            report.push_str(" service=");
            report.push_str(event.service);
            report.push_str(" state=");
            report.push_str(event.state.label());
            if event.has_status {
                report.push_str(" status=");
                report.push_u64_dec(event.status);
            }
            report.push_byte(b'\n');
        }
        event_index += 1;
    }
}

fn write_generation_manager_report(report: &mut InspectReport) {
    let manager = boot_manager_state();
    report.push_str("generation-manager v=1 selected=");
    push_generation_field(report, manager.selected_generation);
    report.push_str(" previous=");
    push_generation_field(report, manager.previous_generation);
    report.push_str(" known_good=");
    push_generation_field(report, manager.known_good_generation);
    report.push_str(" last_failed=");
    push_generation_field(report, manager.last_failed_generation);
    report.push_str(" transaction=");
    report.push_str(manager.last_transaction_state);
    report.push_str(" target=");
    push_generation_field(report, manager.last_transaction_target);
    report.push_str(" tx_counter=");
    report.push_u64_dec(manager.transaction_counter);
    report.push_str(" failure_reason=");
    push_generation_field(report, manager.last_failure_reason);
    report.push_str(" failure_service=");
    push_generation_field(report, manager.last_failure_service);
    report.push_str(" failure_dependency=");
    push_generation_field(report, manager.last_failure_dependency);
    report.push_str(" failure_policy=");
    push_generation_field(report, manager.last_failure_policy);
    report.push_byte(b'\n');
}

fn push_generation_field(report: &mut InspectReport, value: &str) {
    if value.is_empty() {
        report.push_str("<none>");
    } else {
        report.push_str(value);
    }
}

fn write_graph_store_report(runtime: &RuntimeState, report: &mut InspectReport) {
    let Some(config) = runtime.active_config else {
        report.push_str("graph-store v=1 status=unavailable\n");
        return;
    };
    report.push_str("graph-store v=1 generation=");
    report.push_str(config.generation_id);
    report.push_str(" hash=");
    report.push_bytes(&config.graph_store_hash);
    report.push_str(" checksum=");
    report.push_u64_dec(config.graph_store_checksum as u64);
    report.push_str(" nodes=");
    report.push_u64_dec(config.graph_node_count as u64);
    report.push_str(" edges=");
    report.push_u64_dec(config.graph_edge_count as u64);
    report.push_str(" source=");
    report.push_str(config.graph_store_source);
    report.push_byte(b'\n');

    report.push_str("graph-store-object-counts generation=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_GENERATION));
    report.push_str(" services=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_SERVICE));
    report.push_str(" endpoints=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_ENDPOINT));
    report.push_str(" store_objects=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_STORE_OBJECT));
    report.push_str(" configs=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_CONFIG));
    report.push_str(" state=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_STATE_VOLUME));
    report.push_str(" devices=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_DEVICE));
    report.push_str(" namespaces=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_NAMESPACE));
    report.push_str(" vfs_roots=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_VFS_ROOT));
    report.push_str(" secrets=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_SECRET));
    report.push_byte(b'\n');

    let mut index = 0;
    while index < config.graph_node_count {
        if let Some(node) = config.graph_nodes[index] {
            report.push_str("graph-node[");
            report.push_u64_dec(index as u64);
            report.push_str("] kind=");
            report.push_str(graph_node_kind_label(node.kind));
            report.push_str(" id=");
            report.push_str(node.id);
            report.push_str(" object_kind=");
            report.push_str(boot_object_kind_label(node.object_kind));
            report.push_str(" label=");
            report.push_str(node.label);
            report.push_byte(b'\n');
        }
        index += 1;
    }

    index = 0;
    while index < config.graph_edge_count {
        if let Some(edge) = config.graph_edges[index] {
            report.push_str("graph-edge[");
            report.push_u64_dec(index as u64);
            report.push_str("] kind=");
            report.push_str(graph_edge_kind_label(edge.kind));
            report.push_str(" id=");
            report.push_str(edge.id);
            report.push_str(" from=");
            report.push_str(graph_node_id(config, edge.from_index));
            report.push_str(" to=");
            report.push_str(graph_node_id(config, edge.to_index));
            report.push_str(" rights=");
            write_rights_report(report, edge.rights);
            report.push_byte(b'\n');
        }
        index += 1;
    }
}

fn graph_node_kind_count(config: &BootRuntimeConfig, kind: u16) -> u64 {
    let mut count = 0;
    let mut index = 0;
    while index < config.graph_node_count {
        if let Some(node) = config.graph_nodes[index]
            && node.kind == kind
        {
            count += 1;
        }
        index += 1;
    }
    count
}

fn graph_node_id(config: &BootRuntimeConfig, index: usize) -> &'static str {
    if index < config.graph_node_count
        && let Some(node) = config.graph_nodes[index]
    {
        return node.id;
    }
    "<invalid>"
}

fn graph_node_kind_label(kind: u16) -> &'static str {
    match kind {
        GRAPH_NODE_GENERATION => "generation",
        GRAPH_NODE_SERVICE => "service",
        GRAPH_NODE_ENDPOINT => "endpoint",
        GRAPH_NODE_STORE_OBJECT => "store-object",
        GRAPH_NODE_CONFIG => "config",
        GRAPH_NODE_STATE_VOLUME => "state-volume",
        GRAPH_NODE_DEVICE => "device",
        GRAPH_NODE_NAMESPACE => "namespace",
        GRAPH_NODE_VFS_ROOT => "vfs-root",
        GRAPH_NODE_TIMER => "timer",
        GRAPH_NODE_SECRET => "secret",
        _ => "unknown",
    }
}

fn graph_edge_kind_label(kind: u16) -> &'static str {
    match kind {
        GRAPH_EDGE_ACTIVATION => "activation",
        GRAPH_EDGE_CAPABILITY => "capability",
        GRAPH_EDGE_MOUNT => "mount",
        _ => "unknown",
    }
}

fn boot_object_kind_label(kind: u16) -> &'static str {
    match kind {
        0 => "none",
        BOOT_OBJECT_ENDPOINT => "endpoint",
        BOOT_OBJECT_STORE => "store",
        BOOT_OBJECT_STATE => "state",
        BOOT_OBJECT_TIMER => "timer",
        BOOT_OBJECT_NETWORK_PORT => "network-port",
        BOOT_OBJECT_IO_PORT_RANGE => "io-port",
        BOOT_OBJECT_MMIO_REGION => "mmio-region",
        BOOT_OBJECT_INTERRUPT_LINE => "interrupt-line",
        BOOT_OBJECT_DMA_REGION => "dma-region",
        BOOT_OBJECT_PCI_DEVICE => "pci-device",
        BOOT_OBJECT_VIRTIO_DEVICE => "virtio-device",
        BOOT_OBJECT_NAMESPACE => "namespace",
        BOOT_OBJECT_VFS_ROOT => "vfs-root",
        _ => "unknown",
    }
}

fn write_virtio_runtime_report(runtime: &RuntimeState, report: &mut InspectReport) {
    let rng = unsafe { *VIRTIO_RNG_STATE.0.get() };
    let net = unsafe { *VIRTIO_NET_STATE.0.get() };

    report.push_str("virtio_runtime_devices=2\n");
    report.push_str("virtio-runtime[0] device=");
    report.push_str(VIRTIO_RNG_DEVICE_ID);
    report.push_str(" initialized=");
    write_yes_no(report, rng.initialized);
    report.push_str(" owner=");
    report.push_str(process_name_by_pid(runtime, rng.owner));
    report.push_str(" resets=");
    report.push_u64_dec(rng.reset_count);
    report.push_str(" last_error=");
    report.push_str(rng.last_error);
    report.push_str(" io_base=");
    report.push_u64_dec(rng.io_base as u64);
    report.push_byte(b'\n');
    write_virtio_queue_report(report, "virtio-queue[0]", "rng", &rng.queue);

    report.push_str("virtio-runtime[1] device=");
    report.push_str(VIRTIO_NET_DEVICE_ID);
    report.push_str(" initialized=");
    write_yes_no(report, net.initialized);
    report.push_str(" owner=");
    report.push_str(process_name_by_pid(runtime, net.owner));
    report.push_str(" resets=");
    report.push_u64_dec(net.reset_count);
    report.push_str(" last_error=");
    report.push_str(net.last_error);
    report.push_str(" io_base=");
    report.push_u64_dec(net.io_base as u64);
    report.push_str(" rx_posted=");
    write_yes_no(report, net.rx_posted);
    report.push_byte(b'\n');
    write_virtio_queue_report(report, "virtio-queue[1]", "net-rx", &net.rx);
    write_virtio_queue_report(report, "virtio-queue[2]", "net-tx", &net.tx);

    let mut device_count = 0;
    let mut object_index = 0;
    while object_index < runtime.objects.count {
        if let Some(KernelObject::VirtioDevice(_)) = runtime.objects.objects[object_index] {
            device_count += 1;
        }
        object_index += 1;
    }
    report.push_str("virtio_driver_devices=");
    report.push_u64_dec(device_count);
    report.push_byte(b'\n');

    let mut device_index = 0;
    object_index = 0;
    while object_index < runtime.objects.count {
        if let Some(KernelObject::VirtioDevice(device)) = runtime.objects.objects[object_index] {
            report.push_str("virtio-device-runtime[");
            report.push_u64_dec(device_index);
            report.push_str("] device=");
            report.push_str(device.name);
            report.push_str(" transport=");
            report.push_str(device.transport);
            report.push_str(" owner=");
            report.push_str(process_name_by_pid(runtime, device.owner));
            report.push_str(" queue_size=");
            report.push_u64_dec(device.queue_size as u64);
            report.push_str(" avail_idx=");
            report.push_u64_dec(device.avail_idx as u64);
            report.push_str(" used_idx=");
            report.push_u64_dec(device.used_idx as u64);
            report.push_str(" submissions=");
            report.push_u64_dec(device.submissions);
            report.push_str(" completions=");
            report.push_u64_dec(device.completions);
            report.push_str(" timeouts=");
            report.push_u64_dec(device.timeouts);
            report.push_str(" resets=");
            report.push_u64_dec(device.reset_count);
            report.push_str(" last_error=");
            report.push_str(device.last_error);
            report.push_byte(b'\n');
            device_index += 1;
        }
        object_index += 1;
    }
}

fn write_virtio_queue_report(
    report: &mut InspectReport,
    slot: &'static str,
    name: &'static str,
    queue: &VirtioQueueState,
) {
    report.push_str(slot);
    report.push_str(" name=");
    report.push_str(name);
    report.push_str(" queue_size=");
    report.push_u64_dec(queue.queue_size as u64);
    report.push_str(" avail_idx=");
    report.push_u64_dec(queue.avail_idx as u64);
    report.push_str(" used_idx=");
    report.push_u64_dec(queue.used_idx as u64);
    report.push_str(" submissions=");
    report.push_u64_dec(queue.submissions);
    report.push_str(" completions=");
    report.push_u64_dec(queue.completions);
    report.push_str(" interrupt_waits=");
    report.push_u64_dec(queue.interrupt_waits);
    report.push_str(" timeouts=");
    report.push_u64_dec(queue.timeouts);
    report.push_str(" last_error=");
    report.push_str(queue.last_error);
    report.push_str(" dma_physical=");
    report.push_u64_dec(queue.dma_physical);
    report.push_str(" dma_virtual=");
    report.push_u64_dec(queue.dma_virtual);
    report.push_byte(b'\n');
}

fn write_yes_no(report: &mut InspectReport, value: bool) {
    if value {
        report.push_str("yes");
    } else {
        report.push_str("no");
    }
}

fn write_unreachable_object_report(runtime: &RuntimeState, report: &mut InspectReport) {
    let mut leak_index = 0;
    let mut index = 0;
    while index < runtime.objects.count {
        if let Some(object) = runtime.objects.objects[index]
            && !object_reachable_by_cap(runtime, object.id())
            && !object_reachable_by_config(runtime, object.id())
            && !object_reachable_by_owner(runtime, object)
        {
            report.push_str("object-unreachable[");
            report.push_u64_dec(leak_index);
            report.push_str("] ");
            write_capability_object_report(runtime, report, object.id());
            report.push_byte(b'\n');
            leak_index += 1;
        }
        index += 1;
    }
}

fn frame_allocator_stats() -> Option<memory::AllocatorStats> {
    let allocator = unsafe { *FRAME_ALLOCATOR.0.get() }?;
    unsafe { allocator.as_ref().map(|allocator| allocator.stats()) }
}

fn runtime_cap_count(runtime: &RuntimeState) -> u64 {
    let mut count = 0;
    let mut process_index = 0;
    while process_index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[process_index] {
            count += cap_count_in_space(process.caps);
            count += cap_count_in_space(process.initial_caps);
        }
        process_index += 1;
    }
    count
}

fn runtime_file_handle_count(runtime: &RuntimeState) -> u64 {
    let mut count = 0;
    let mut process_index = 0;
    while process_index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[process_index] {
            let mut handle_index = 0;
            while handle_index < process.file_handles.len() {
                if process.file_handles[handle_index].handle.is_some() {
                    count += 1;
                }
                handle_index += 1;
            }
        }
        process_index += 1;
    }
    count
}

fn write_vfs_report(runtime: &RuntimeState, report: &mut InspectReport) {
    let mut mount_index = 0;
    while mount_index < runtime.objects.count {
        if let Some(KernelObject::VfsMount(mount)) = runtime.objects.objects[mount_index] {
            report.push_str("vfs-mount[");
            report.push_u64_dec(mount_index as u64);
            report.push_str("] id=");
            report.push_u64_dec(mount.id.raw());
            report.push_str(" name=");
            report.push_str(mount.name);
            report.push_str(" root=");
            report.push_bytes(mount.root_path.as_bytes());
            report.push_str(" root_vnode=");
            report.push_u64_dec(mount.root_node.raw());
            report.push_str(" source=");
            report.push_str(mount.source);
            report.push_str(" flags=");
            write_vfs_mount_flags(report, mount.flags);
            report.push_str(" dynamic=");
            report.push_str(if mount.dynamic { "yes" } else { "no" });
            report.push_str(" owner=");
            if mount.owner == ProcessId::empty() {
                report.push_str("system");
            } else {
                report.push_str(process_name_by_pid(runtime, mount.owner));
                report.push_str(":");
                report.push_u64_dec(mount.owner.raw());
            }
            report.push_byte(b'\n');
        }
        mount_index += 1;
    }

    let mut index = 0;
    while index < runtime.vfs_node_count {
        if let Some(node) = runtime.vfs_nodes[index] {
            report.push_str("vfs-node[");
            report.push_u64_dec(index as u64);
            report.push_str("] id=");
            report.push_u64_dec(node.id.raw());
            report.push_str(" kind=");
            match node.kind {
                VfsNodeKind::RegularFile => report.push_str("regular"),
                VfsNodeKind::Directory => report.push_str("directory"),
                VfsNodeKind::DeviceNode => report.push_str("device"),
                VfsNodeKind::Pipe => report.push_str("pipe"),
                VfsNodeKind::SyntheticNode => report.push_str("synthetic"),
            }
            report.push_str(" name=");
            report.push_bytes(node.name.as_bytes());
            report.push_str(" mount=");
            report.push_str(node.mount_source);
            if let Some(parent) = node.parent {
                report.push_str(" parent=");
                report.push_u64_dec(parent.raw());
            }
            match node.backing {
                VfsBacking::None => report.push_str(" backing=none"),
                VfsBacking::StoreObject(object) => {
                    report.push_str(" backing=store-object object_id=");
                    report.push_u64_dec(object.raw());
                }
                VfsBacking::StateVolume(object) => {
                    report.push_str(" backing=state-volume object_id=");
                    report.push_u64_dec(object.raw());
                }
                VfsBacking::StateVolumeValue(object) => {
                    report.push_str(" backing=state-volume-value object_id=");
                    report.push_u64_dec(object.raw());
                }
                VfsBacking::StateVolumeControl(object) => {
                    report.push_str(" backing=state-volume-control object_id=");
                    report.push_u64_dec(object.raw());
                }
                VfsBacking::MemoryFile(file) => {
                    report.push_str(" backing=memory-file index=");
                    report.push_u64_dec(file as u64);
                }
                VfsBacking::VertexFsFile(file) => {
                    report.push_str(" backing=vertexfs-file index=");
                    report.push_u64_dec(file as u64);
                    if file < runtime.vertexfs_file_count {
                        report.push_str(" dirty=");
                        write_yes_no(report, runtime.vertexfs_files[file].dirty);
                        report.push_str(" checksum=");
                        report.push_u64_dec(runtime.vertexfs_files[file].checksum as u64);
                    }
                }
                VfsBacking::Device(object) => {
                    report.push_str(" backing=device object_id=");
                    report.push_u64_dec(object.raw());
                }
                VfsBacking::Synthetic(_) => report.push_str(" backing=synthetic"),
                VfsBacking::FsServiceReport => report.push_str(" backing=fs-service"),
                VfsBacking::Pipe => report.push_str(" backing=pipe"),
            }
            report.push_byte(b'\n');
        }
        index += 1;
    }

    let mut process_index = 0;
    let mut handle_row = 0;
    while process_index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[process_index] {
            let mut handle_index = 0;
            while handle_index < process.file_handles.len() {
                if let Some(handle) = process.file_handles[handle_index].handle
                    && let Some(description) = runtime.file_description(handle.description)
                {
                    report.push_str("vfs-handle[");
                    report.push_u64_dec(handle_row);
                    report.push_str("] owner=");
                    report.push_str(process.name);
                    report.push_str(" slot=");
                    report.push_u64_dec((handle_index + 1) as u64);
                    report.push_str(" description=");
                    report.push_u64_dec(description.id.raw());
                    report.push_str(" vnode=");
                    report.push_u64_dec(description.node.raw());
                    report.push_str(" rights=");
                    write_rights_report(report, description.rights);
                    report.push_str(" flags=");
                    report.push_u64_dec(description.flags);
                    report.push_str(" offset=");
                    report.push_u64_dec(description.offset);
                    report.push_str(" refs=");
                    report.push_u64_dec(description.ref_count);
                    report.push_byte(b'\n');
                    handle_row += 1;
                }
                handle_index += 1;
            }
        }
        process_index += 1;
    }

    let mut lock_index = 0;
    while lock_index < runtime.vfs_locks.len() {
        if let Some(lock) = runtime.vfs_locks[lock_index] {
            report.push_str("vfs-lock[");
            report.push_u64_dec(lock_index as u64);
            report.push_str("] owner=");
            if let Some(process) = runtime.processes.process(lock.owner) {
                report.push_str(process.name);
            } else {
                report.push_str("<dead>");
            }
            report.push_str(" vnode=");
            report.push_u64_dec(lock.node.raw());
            report.push_str(" description=");
            report.push_u64_dec(lock.description.raw());
            report.push_str(" mode=");
            match lock.mode {
                VfsLockMode::Shared => report.push_str("shared"),
                VfsLockMode::Exclusive => report.push_str("exclusive"),
            }
            report.push_str(" range=");
            report.push_u64_dec(lock.start);
            report.push_str("+");
            report.push_u64_dec(lock.len);
            report.push_byte(b'\n');
        }
        lock_index += 1;
    }

    report.push_str("vfs-pipe buffered=");
    report.push_u64_dec(runtime.vfs_pipe.len as u64);
    report.push_byte(b'\n');

    let mut event_index = 0;
    while event_index < runtime.vfs_event_count {
        if let Some(event) = runtime.vfs_events[event_index] {
            report.push_str("vfs-event[");
            report.push_u64_dec(event_index as u64);
            report.push_str("] parent=");
            report.push_u64_dec(event.parent.raw());
            report.push_str(" kind=");
            report.push_u64_dec(event.kind);
            report.push_str(" version=");
            report.push_u64_dec(event.metadata_version);
            report.push_str(" name=");
            report.push_bytes(event.name.as_bytes());
            report.push_byte(b'\n');
        }
        event_index += 1;
    }
}

fn write_vfs_mount_flags(report: &mut InspectReport, flags: u64) {
    let mut wrote = false;
    if flags & VFS_MOUNT_VOLATILE != 0 {
        report.push_str("volatile");
        wrote = true;
    }
    if flags & VFS_MOUNT_BIND != 0 {
        if wrote {
            report.push_str("|");
        }
        report.push_str("bind");
        wrote = true;
    }
    if flags & VFS_MOUNT_READ_ONLY != 0 {
        if wrote {
            report.push_str("|");
        }
        report.push_str("read-only");
        wrote = true;
    }
    if !wrote {
        report.push_str("none");
    }
}

fn cap_count_in_space(space: CapabilitySpace) -> u64 {
    let mut count = 0;
    let mut slot = 0;
    while slot < MAX_CAPS {
        if space.caps[slot].is_some() {
            count += 1;
        }
        slot += 1;
    }
    count
}

fn unreachable_object_count(runtime: &RuntimeState) -> u64 {
    let mut count = 0;
    let mut index = 0;
    while index < runtime.objects.count {
        if let Some(object) = runtime.objects.objects[index]
            && !object_reachable_by_cap(runtime, object.id())
            && !object_reachable_by_config(runtime, object.id())
            && !object_reachable_by_owner(runtime, object)
        {
            count += 1;
        }
        index += 1;
    }
    count
}

fn object_reachable_by_cap(runtime: &RuntimeState, object_id: KernelObjectId) -> bool {
    let mut process_index = 0;
    while process_index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[process_index]
            && process.state != ProcessState::Empty
            && ((process.state != ProcessState::Exited
                && cap_space_reaches_live_object(runtime, process.caps, object_id))
                || cap_space_reaches_live_object(runtime, process.initial_caps, object_id))
        {
            return true;
        }
        process_index += 1;
    }
    false
}

fn object_reachable_by_config(runtime: &RuntimeState, object_id: KernelObjectId) -> bool {
    id_list_contains(&runtime.endpoint_ids, object_id)
        || id_list_contains(&runtime.store_object_ids, object_id)
        || id_list_contains(&runtime.state_volume_ids, object_id)
        || id_list_contains(&runtime.network_port_ids, object_id)
        || id_list_contains(&runtime.io_port_ids, object_id)
        || id_list_contains(&runtime.mmio_region_ids, object_id)
        || id_list_contains(&runtime.interrupt_line_ids, object_id)
        || id_list_contains(&runtime.dma_region_ids, object_id)
        || id_list_contains(&runtime.pci_device_ids, object_id)
        || id_list_contains(&runtime.virtio_device_ids, object_id)
        || id_list_contains(&runtime.namespace_ids, object_id)
        || id_list_contains(&runtime.vfs_root_ids, object_id)
        || id_list_contains(&runtime.vfs_mount_ids, object_id)
        || runtime.timer_id == Some(object_id)
        || runtime.process_control_id == Some(object_id)
        || runtime.secret_id == Some(object_id)
        || runtime.state_vfs_request_endpoint == Some(object_id)
        || runtime.state_vfs_reply_endpoint == Some(object_id)
        || runtime.vertexfs_device_request_endpoint == Some(object_id)
        || runtime.vertexfs_device_reply_endpoint == Some(object_id)
}

fn object_reachable_by_owner(runtime: &RuntimeState, object: KernelObject) -> bool {
    match object {
        KernelObject::IpcEndpoint(endpoint) if endpoint.owner != ProcessId::empty() => runtime
            .processes
            .process(endpoint.owner)
            .map(|process| {
                process.state != ProcessState::Empty && process.state != ProcessState::Exited
            })
            .unwrap_or(false),
        _ => false,
    }
}

fn id_list_contains(ids: &[Option<KernelObjectId>], object_id: KernelObjectId) -> bool {
    let mut index = 0;
    while index < ids.len() {
        if ids[index] == Some(object_id) {
            return true;
        }
        index += 1;
    }
    false
}

fn cap_space_reaches_live_object(
    runtime: &RuntimeState,
    space: CapabilitySpace,
    object_id: KernelObjectId,
) -> bool {
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = space.caps[slot]
            && cap.object == object_id
            && !cap.revoked
            && !runtime.cap_id_revoked(cap.id)
            && !capability_has_revoked_ancestor(runtime, cap)
            && cap.generation_id == runtime.generation_id
        {
            return true;
        }
        slot += 1;
    }
    false
}

fn write_capability_space_report(
    runtime: &RuntimeState,
    report: &mut InspectReport,
    process: Process,
    space_name: &str,
    space: CapabilitySpace,
) {
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = space.caps[slot] {
            report.push_str("space=");
            report.push_str(space_name);
            report.push_str(" proc=");
            report.push_str(process.name);
            report.push_str(" cap[");
            report.push_u64_dec(slot as u64);
            report.push_str("] ");
            write_capability_object_report(runtime, report, cap.object);
            report.push_str(" rights=");
            write_rights_report(report, cap.rights);
            report.push_str(" cap_id=");
            report.push_u64_dec(cap.id);
            report.push_str(" parent_cap_id=");
            report.push_u64_dec(cap.parent_cap_id);
            report.push_str(" generation=");
            report.push_str(cap.generation_id);
            report.push_str(" graph_from=");
            report.push_str(process_graph_node(runtime, process.name));
            report.push_str(" graph_target=");
            write_capability_graph_target(runtime, report, cap.object);
            report.push_str(" graph_edge=");
            write_capability_graph_edge(runtime, report, process.name, slot, cap);
            report.push_str(" owner_pid=");
            report.push_u64_dec(cap.owner_process.raw());
            report.push_str(" owner=");
            report.push_str(process_name_by_pid(runtime, cap.owner_process));
            report.push_str(" delegated_by_pid=");
            report.push_u64_dec(cap.delegated_by.raw());
            report.push_str(" delegated_by=");
            report.push_str(process_name_by_pid(runtime, cap.delegated_by));
            report.push_str(" revoked=");
            report.push_str(if cap.revoked || runtime.cap_id_revoked(cap.id) {
                "yes"
            } else {
                "no"
            });
            report.push_byte(b'\n');
        }
        slot += 1;
    }
}

fn process_graph_node(runtime: &RuntimeState, process_name: &str) -> &'static str {
    let Some(config) = runtime.active_config else {
        return "<unknown>";
    };
    let mut index = 0;
    while index < config.process_count {
        if let Some(process) = config.processes[index]
            && process.name == process_name
        {
            return process.graph_node;
        }
        index += 1;
    }
    "<unknown>"
}

fn write_capability_graph_target(
    runtime: &RuntimeState,
    report: &mut InspectReport,
    object: KernelObjectId,
) {
    if let Some(target) = graph_node_for_object(runtime, object) {
        report.push_str(target);
    } else {
        report.push_str("<unknown>");
    }
}

fn write_capability_graph_edge(
    runtime: &RuntimeState,
    report: &mut InspectReport,
    process_name: &str,
    slot: usize,
    cap: Capability,
) {
    if let Some(index) = boot_grant_index_for_cap(runtime, process_name, slot, cap.object) {
        report.push_str("grant:");
        report.push_u64_dec(index as u64);
        return;
    }
    if let Some(target) = graph_node_for_object(runtime, cap.object)
        && target == "secret:logd-token"
    {
        report.push_str("grant:secret-logd-token");
        return;
    }
    report.push_str("runtime-derived");
}

fn boot_grant_index_for_cap(
    runtime: &RuntimeState,
    process_name: &str,
    slot: usize,
    object: KernelObjectId,
) -> Option<usize> {
    let config = runtime.active_config?;
    let mut process_index = 0;
    while process_index < config.process_count {
        let process = config.processes[process_index]?;
        if process.name == process_name {
            let mut grant_index = 0;
            while grant_index < config.grant_count {
                let grant = config.grants[grant_index]?;
                if grant.process_index == process_index
                    && grant.cap_slot == slot as u64
                    && grant_object_id(runtime, grant).ok() == Some(object)
                {
                    return Some(grant_index);
                }
                grant_index += 1;
            }
        }
        process_index += 1;
    }
    None
}

fn graph_node_for_object(runtime: &RuntimeState, object: KernelObjectId) -> Option<&'static str> {
    let mut index = 0;
    while index < runtime.objects.count {
        if let Some(entry) = runtime.objects.objects[index] {
            match entry {
                KernelObject::IpcEndpoint(endpoint) if endpoint.id == object => {
                    return Some(endpoint.name);
                }
                KernelObject::StoreObject(store) if store.id == object => {
                    return Some(store.name);
                }
                KernelObject::StateVolume(state) if state.id == object => {
                    return Some(state.name);
                }
                KernelObject::Timer(timer) if timer.id == object => {
                    return Some(timer.name);
                }
                KernelObject::NetworkPort(port) if port.id == object => {
                    return Some(port.name);
                }
                KernelObject::IoPortRange(port) if port.id == object => {
                    return Some(port.name);
                }
                KernelObject::MmioRegion(region) if region.id == object => {
                    return Some(region.name);
                }
                KernelObject::InterruptLine(line) if line.id == object => {
                    return Some(line.name);
                }
                KernelObject::DmaRegion(region) if region.id == object => {
                    return Some(region.name);
                }
                KernelObject::PciDevice(device) if device.id == object => {
                    return Some(device.name);
                }
                KernelObject::VirtioDevice(device) if device.id == object => {
                    return Some(device.name);
                }
                KernelObject::Namespace(namespace) if namespace.id == object => {
                    return Some(namespace.name);
                }
                KernelObject::VfsRoot(root) if root.id == object => {
                    return Some(root.name);
                }
                KernelObject::Secret(secret) if secret.id == object => {
                    return Some(secret.name);
                }
                _ => {}
            }
        }
        index += 1;
    }
    None
}

fn process_name_by_pid(runtime: &RuntimeState, pid: ProcessId) -> &'static str {
    if pid == ProcessId::empty() {
        return "kernel";
    }

    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && process.pid == pid
        {
            return process.name;
        }
        index += 1;
    }

    "<unknown>"
}

fn write_capability_object_report(
    runtime: &RuntimeState,
    report: &mut InspectReport,
    object: KernelObjectId,
) {
    let mut index = 0;
    while index < runtime.objects.count {
        if let Some(entry) = runtime.objects.objects[index] {
            match entry {
                KernelObject::IpcEndpoint(endpoint) if endpoint.id == object => {
                    report.push_str("endpoint=");
                    report.push_str(endpoint.name);
                    return;
                }
                KernelObject::BootModule(module) if module.id == object => {
                    report.push_str("boot-module=");
                    report.push_str(module.name);
                    return;
                }
                KernelObject::StoreObject(store) if store.id == object => {
                    if store.name.starts_with("config:") {
                        report.push_str("config=");
                    } else {
                        report.push_str("store-object=");
                    }
                    report.push_str(store.name);
                    return;
                }
                KernelObject::StateVolume(state) if state.id == object => {
                    report.push_str("state-volume=");
                    report.push_str(state.name);
                    return;
                }
                KernelObject::Timer(timer) if timer.id == object => {
                    report.push_str("timer=");
                    report.push_str(timer.name);
                    return;
                }
                KernelObject::NetworkPort(port) if port.id == object => {
                    report.push_str("network-port=");
                    report.push_str(port.name);
                    return;
                }
                KernelObject::IoPortRange(port) if port.id == object => {
                    report.push_str("io-port=");
                    report.push_str(port.name);
                    return;
                }
                KernelObject::MmioRegion(region) if region.id == object => {
                    report.push_str("mmio-region=");
                    report.push_str(region.name);
                    return;
                }
                KernelObject::InterruptLine(line) if line.id == object => {
                    report.push_str("interrupt-line=");
                    report.push_str(line.name);
                    return;
                }
                KernelObject::DmaRegion(region) if region.id == object => {
                    report.push_str("dma-region=");
                    report.push_str(region.name);
                    return;
                }
                KernelObject::PciDevice(device) if device.id == object => {
                    report.push_str("pci-device=");
                    report.push_str(device.name);
                    report.push_str(" kind=");
                    report.push_str(device.kind);
                    return;
                }
                KernelObject::VirtioDevice(device) if device.id == object => {
                    report.push_str("virtio-device=");
                    report.push_str(device.name);
                    report.push_str(" transport=");
                    report.push_str(device.transport);
                    return;
                }
                KernelObject::Namespace(namespace) if namespace.id == object => {
                    report.push_str("namespace=");
                    report.push_str(namespace.name);
                    return;
                }
                KernelObject::VfsRoot(root) if root.id == object => {
                    report.push_str("vfs-root=");
                    report.push_str(root.name);
                    report.push_str(" root=");
                    report.push_bytes(root.root_path.as_bytes());
                    return;
                }
                KernelObject::VfsMount(mount) if mount.id == object => {
                    report.push_str("vfs-mount=");
                    report.push_str(mount.name);
                    report.push_str(" root=");
                    report.push_bytes(mount.root_path.as_bytes());
                    return;
                }
                KernelObject::ProcessControl(process_control) if process_control.id == object => {
                    report.push_str("process-control=");
                    report.push_str(process_control.name);
                    return;
                }
                KernelObject::Secret(secret) if secret.id == object => {
                    report.push_str("secret=");
                    report.push_str(secret.name);
                    report.push_str(" value=<redacted>");
                    return;
                }
                _ => {}
            }
        }
        index += 1;
    }

    report.push_str("object=");
    report.push_u64_dec(object.raw());
}

fn write_rights_report(report: &mut InspectReport, rights: u64) {
    let mut wrote = false;
    wrote = write_right_report(report, rights, capability::RIGHT_READ, "read", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_WRITE, "write", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_SEND, "send", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_RECEIVE, "receive", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_CONTROL, "control", wrote);
    wrote = write_right_report(
        report,
        rights,
        capability::RIGHT_SNAPSHOT,
        "snapshot",
        wrote,
    );
    wrote = write_right_report(report, rights, capability::RIGHT_RESTORE, "restore", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_MAP, "map", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_BIND, "bind", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_LISTEN, "listen", wrote);
    wrote = write_right_report(
        report,
        rights,
        capability::RIGHT_ALLOCATE,
        "allocate",
        wrote,
    );
    wrote = write_right_report(
        report,
        rights,
        capability::RIGHT_DELEGATE,
        "delegate",
        wrote,
    );
    wrote = write_right_report(report, rights, capability::RIGHT_REVOKE, "revoke", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_INSPECT, "inspect", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_CREATE, "create", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_UNLINK, "unlink", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_RENAME, "rename", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_MOUNT, "mount", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_START, "start", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_KILL, "kill", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_WAIT, "wait", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_DERIVE, "derive", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_SEAL, "seal", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_UNSEAL, "unseal", wrote);
    wrote = write_right_report(
        report,
        rights,
        capability::RIGHT_INSPECT_METADATA,
        "inspect-metadata",
        wrote,
    );
    wrote = write_right_report(report, rights, capability::RIGHT_RESOLVE, "resolve", wrote);

    if !wrote {
        report.push_str("none");
    }
}

fn write_right_report(
    report: &mut InspectReport,
    rights: u64,
    right: u64,
    label: &str,
    wrote: bool,
) -> bool {
    if rights & right == 0 {
        return wrote;
    }

    if wrote {
        report.push_byte(b'|');
    }
    report.push_str(label);
    true
}

fn print_boot_tables(runtime: &RuntimeState) {
    serial::write_str("Process table entries: ");
    serial::write_u64_dec(runtime.processes.count as u64);
    serial::write_str("\n");

    serial::write_str("Endpoint table entries: ");
    serial::write_u64_dec(runtime.objects.endpoint_count() as u64);
    serial::write_str("\n");

    print_endpoint_labels(runtime);

    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index] {
            print_process_state(index, &process);
            print_process_caps(&process);
        }
        index += 1;
    }
}

fn print_endpoint_labels(runtime: &RuntimeState) {
    let mut printed = 0;
    let mut index = 0;
    while index < runtime.objects.count {
        if let Some(KernelObject::IpcEndpoint(endpoint)) = runtime.objects.objects[index] {
            serial::write_str("endpoint[");
            serial::write_u64_dec(printed as u64);
            serial::write_str("] id=");
            serial::write_u64_dec(endpoint.id.raw());
            serial::write_str(" name=");
            serial::write_str(endpoint.name);
            serial::write_str("\n");
            printed += 1;
        }
        index += 1;
    }
}

fn print_process_by_pid(runtime: &RuntimeState, pid: ProcessId) {
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && process.pid == pid
        {
            print_process_state(index, &process);
            print_process_caps(&process);
            return;
        }
        index += 1;
    }
}

fn print_process_caps(process: &Process) {
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = process.caps.caps[slot] {
            serial::write_str("proc=");
            serial::write_str(process.name);
            serial::write_str(" cap[");
            serial::write_u64_dec(slot as u64);
            serial::write_str("] ");
            print_capability_object(cap.object);
            serial::write_str(" rights=");
            print_rights(cap.rights);
            serial::write_str("\n");
        }
        slot += 1;
    }
}

fn print_capability_object(object: KernelObjectId) {
    let runtime = runtime();
    let mut index = 0;

    while index < runtime.objects.count {
        if let Some(entry) = runtime.objects.objects[index] {
            match entry {
                KernelObject::IpcEndpoint(endpoint) if endpoint.id == object => {
                    serial::write_str("endpoint=");
                    serial::write_str(endpoint.name);
                    return;
                }
                KernelObject::BootModule(module) if module.id == object => {
                    serial::write_str("boot-module=");
                    serial::write_str(module.name);
                    return;
                }
                KernelObject::StoreObject(store) if store.id == object => {
                    if store.name.starts_with("config:") {
                        serial::write_str("config=");
                    } else {
                        serial::write_str("store-object=");
                    }
                    serial::write_str(store.name);
                    return;
                }
                KernelObject::Timer(timer) if timer.id == object => {
                    serial::write_str("timer=");
                    serial::write_str(timer.name);
                    return;
                }
                KernelObject::NetworkPort(port) if port.id == object => {
                    serial::write_str("network-port=");
                    serial::write_str(port.name);
                    return;
                }
                KernelObject::IoPortRange(port) if port.id == object => {
                    serial::write_str("io-port=");
                    serial::write_str(port.name);
                    return;
                }
                KernelObject::MmioRegion(region) if region.id == object => {
                    serial::write_str("mmio-region=");
                    serial::write_str(region.name);
                    return;
                }
                KernelObject::InterruptLine(line) if line.id == object => {
                    serial::write_str("interrupt-line=");
                    serial::write_str(line.name);
                    return;
                }
                KernelObject::DmaRegion(region) if region.id == object => {
                    serial::write_str("dma-region=");
                    serial::write_str(region.name);
                    serial::write_str(" base=");
                    serial::write_u64_hex(region.base);
                    serial::write_str(" length=");
                    serial::write_u64_hex(region.length);
                    return;
                }
                KernelObject::PciDevice(device) if device.id == object => {
                    serial::write_str("pci-device=");
                    serial::write_str(device.name);
                    serial::write_str(" kind=");
                    serial::write_str(device.kind);
                    return;
                }
                KernelObject::VirtioDevice(device) if device.id == object => {
                    serial::write_str("virtio-device=");
                    serial::write_str(device.name);
                    serial::write_str(" transport=");
                    serial::write_str(device.transport);
                    return;
                }
                KernelObject::Namespace(namespace) if namespace.id == object => {
                    serial::write_str("namespace=");
                    serial::write_str(namespace.name);
                    return;
                }
                KernelObject::VfsRoot(root) if root.id == object => {
                    serial::write_str("vfs-root=");
                    serial::write_str(root.name);
                    serial::write_str(" root=");
                    serial::write_ascii_bytes(root.root_path.as_bytes());
                    return;
                }
                KernelObject::VfsMount(mount) if mount.id == object => {
                    serial::write_str("vfs-mount=");
                    serial::write_str(mount.name);
                    serial::write_str(" root=");
                    serial::write_ascii_bytes(mount.root_path.as_bytes());
                    return;
                }
                KernelObject::ProcessControl(process_control) if process_control.id == object => {
                    serial::write_str("process-control=");
                    serial::write_str(process_control.name);
                    return;
                }
                KernelObject::Secret(secret) if secret.id == object => {
                    serial::write_str("secret=");
                    serial::write_str(secret.name);
                    serial::write_str(" value=<redacted>");
                    return;
                }
                _ => {}
            }
        }
        index += 1;
    }

    serial::write_str("object=");
    serial::write_u64_dec(object.raw());
}

fn print_process_state(index: usize, process: &Process) {
    serial::write_str("process[");
    serial::write_u64_dec(index as u64);
    serial::write_str("] id=");
    serial::write_u64_dec(process.pid.raw());
    serial::write_str(" name=");
    serial::write_str(process.name);
    serial::write_str(" state=");
    serial::write_str(process.state.label());
    serial::write_str(" quota_caps=");
    serial::write_u64_dec(process.quota.max_caps);
    serial::write_str(" quota_endpoints=");
    serial::write_u64_dec(process.quota.max_endpoints);
    serial::write_str(" quota_memory_pages=");
    serial::write_u64_dec(process.quota.max_memory_pages);
    serial::write_str(" quota_child_processes=");
    serial::write_u64_dec(process.quota.max_child_processes);
    serial::write_str(" quota_ipc_bytes=");
    serial::write_u64_dec(process.quota.max_ipc_bytes);
    serial::write_str(" mount_root=");
    serial::write_ascii_bytes(process.mount_root.as_bytes());
    serial::write_str("\n");
}

fn print_rights(rights: u64) {
    let mut wrote = false;
    wrote = print_right(rights, capability::RIGHT_READ, "read", wrote);
    wrote = print_right(rights, capability::RIGHT_WRITE, "write", wrote);
    wrote = print_right(rights, capability::RIGHT_SEND, "send", wrote);
    wrote = print_right(rights, capability::RIGHT_RECEIVE, "receive", wrote);
    wrote = print_right(rights, capability::RIGHT_CONTROL, "control", wrote);
    wrote = print_right(rights, capability::RIGHT_SNAPSHOT, "snapshot", wrote);
    wrote = print_right(rights, capability::RIGHT_RESTORE, "restore", wrote);
    wrote = print_right(rights, capability::RIGHT_MAP, "map", wrote);
    wrote = print_right(rights, capability::RIGHT_BIND, "bind", wrote);
    wrote = print_right(rights, capability::RIGHT_LISTEN, "listen", wrote);
    wrote = print_right(rights, capability::RIGHT_ALLOCATE, "allocate", wrote);
    wrote = print_right(rights, capability::RIGHT_DELEGATE, "delegate", wrote);
    wrote = print_right(rights, capability::RIGHT_REVOKE, "revoke", wrote);
    wrote = print_right(rights, capability::RIGHT_INSPECT, "inspect", wrote);
    wrote = print_right(rights, capability::RIGHT_CREATE, "create", wrote);
    wrote = print_right(rights, capability::RIGHT_UNLINK, "unlink", wrote);
    wrote = print_right(rights, capability::RIGHT_RENAME, "rename", wrote);
    wrote = print_right(rights, capability::RIGHT_MOUNT, "mount", wrote);
    wrote = print_right(rights, capability::RIGHT_START, "start", wrote);
    wrote = print_right(rights, capability::RIGHT_KILL, "kill", wrote);
    wrote = print_right(rights, capability::RIGHT_WAIT, "wait", wrote);
    wrote = print_right(rights, capability::RIGHT_DERIVE, "derive", wrote);
    wrote = print_right(rights, capability::RIGHT_SEAL, "seal", wrote);
    wrote = print_right(rights, capability::RIGHT_UNSEAL, "unseal", wrote);
    wrote = print_right(
        rights,
        capability::RIGHT_INSPECT_METADATA,
        "inspect-metadata",
        wrote,
    );
    wrote = print_right(rights, capability::RIGHT_RESOLVE, "resolve", wrote);

    if !wrote {
        serial::write_str("none");
    }
}

fn print_right(rights: u64, right: u64, label: &str, wrote: bool) -> bool {
    if rights & right == 0 {
        return wrote;
    }

    if wrote {
        serial::write_str("|");
    }
    serial::write_str(label);
    true
}

fn print_negative(operation: &str) {
    serial::write_str("IPC negative test: ");
    serial::write_str(current_process_label());
    serial::write_str(" ");
    serial::write_str(operation);
    serial::write_str(" rejected: bad capability\n");
}

fn current_process_label() -> &'static str {
    runtime()
        .processes
        .current_process()
        .map(|process| process.name)
        .unwrap_or("<none>")
}

fn generation_runtimes() -> &'static mut GenerationRuntimeTable {
    unsafe { &mut *GENERATION_RUNTIMES.0.get() }
}

fn set_rollback_runtime(runtime: GenerationRuntime) {
    unsafe {
        *ROLLBACK_RUNTIME.0.get() = Some(runtime);
    }
}

fn set_failed_generation(generation_id: &'static str) {
    unsafe {
        *FAILED_GENERATION.0.get() = Some(generation_id);
    }
}

fn failed_generation_is(generation_id: &'static str) -> bool {
    unsafe { *FAILED_GENERATION.0.get() == Some(generation_id) }
}

fn boot_manager() -> &'static mut BootManagerState {
    unsafe { &mut *BOOT_MANAGER.0.get() }
}

fn boot_manager_state() -> &'static BootManagerState {
    unsafe { &*BOOT_MANAGER.0.get() }
}

fn store_hash_matches(bytes: &[u8], expected: &str) -> bool {
    if expected.len() != 64 {
        return false;
    }
    let mut actual = [0u8; 64];
    store_hash_hex(blake3::hash(bytes).as_bytes(), &mut actual);
    actual == expected.as_bytes()
}

fn store_hash_hex(bytes: &[u8; 32], out: &mut [u8; 64]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut index = 0;
    while index < bytes.len() {
        out[index * 2] = HEX[(bytes[index] >> 4) as usize];
        out[index * 2 + 1] = HEX[(bytes[index] & 0xf) as usize];
        index += 1;
    }
}

fn runtime() -> &'static mut RuntimeState {
    unsafe { &mut *RUNTIME.0.get() }
}

fn inspect_report() -> &'static mut InspectReport {
    unsafe { &mut *INSPECT_REPORT.0.get() }
}

fn staging_runtime() -> &'static mut RuntimeState {
    unsafe { &mut *INSTALL_STAGING_RUNTIME.0.get() }
}

fn frame_allocator() -> Result<&'static mut memory::FrameAllocator, IpcError> {
    let allocator = unsafe { *FRAME_ALLOCATOR.0.get() }.ok_or(IpcError::BadCapability)?;
    unsafe { allocator.as_mut().ok_or(IpcError::BadCapability) }
}

fn reap_process_context(pid: ProcessId) -> Result<(), IpcError> {
    let _ = cancel_blocked_receivers_for_endpoint_owner(pid, STATUS_BAD_CAPABILITY);
    let removed_endpoints = runtime().objects.remove_owned_endpoints(pid);
    if removed_endpoints > 0 {
        serial::write_str("Krust process owned endpoints reaped: pid=");
        serial::write_u64_dec(pid.raw());
        serial::write_str(" endpoints=");
        serial::write_u64_dec(removed_endpoints);
        serial::write_str("\n");
    }

    let (name, cr3, already_reaped) = {
        let runtime = runtime();
        let Some(process) = runtime.processes.process(pid) else {
            return Err(IpcError::BadCapability);
        };
        (process.name, process.context.cr3, process.context_reaped)
    };
    release_process_virtio_ownership(pid);
    release_process_dma_mappings(pid);
    let runtime = runtime();
    if let Some(process) = runtime.processes.process_mut(pid) {
        process.clear_file_handles();
    }
    runtime.release_process_file_descriptions(pid);
    let removed_dynamic_bind_mounts = runtime.remove_owned_dynamic_bind_mounts(pid);
    let removed_declared_bind_mounts = runtime.remove_owned_declared_bind_mounts(pid);
    if removed_dynamic_bind_mounts > 0 {
        serial::write_str("Krust process dynamic bind mounts reaped: proc=");
        serial::write_str(name);
        serial::write_str(" mounts=");
        serial::write_u64_dec(removed_dynamic_bind_mounts);
        serial::write_str("\n");
    }
    if removed_declared_bind_mounts > 0 {
        serial::write_str("Krust process declared mount snapshot reaped: proc=");
        serial::write_str(name);
        serial::write_str(" mounts=");
        serial::write_u64_dec(removed_declared_bind_mounts);
        serial::write_str("\n");
    }
    release_unreferenced_derived_vfs_roots(runtime);
    if already_reaped || cr3 == 0 {
        return Ok(());
    }

    let hhdm_offset = limine::hhdm_offset().ok_or(IpcError::BadCapability)?;
    let stats = paging::reclaim_user_address_space(hhdm_offset, cr3, frame_allocator()?)
        .map_err(|_| IpcError::BadCapability)?;
    if let Some(process) = runtime.processes.process_mut(pid) {
        process.context = ProcessContext {
            cr3: 0,
            entry: 0,
            stack_top: 0,
        };
        process.context_reaped = true;
    }

    serial::write_str("Krust process address space reaped: proc=");
    serial::write_str(name);
    serial::write_str(" pid=");
    serial::write_u64_dec(pid.raw());
    serial::write_str(" user_frames=");
    serial::write_u64_dec(stats.user_leaf_frames);
    serial::write_str(" page_tables=");
    serial::write_u64_dec(stats.page_table_frames);
    serial::write_str(" device_mappings=");
    serial::write_u64_dec(stats.device_mappings);
    serial::write_str("\n");
    Ok(())
}

fn map_current_process_physical_range(
    virtual_base: u64,
    physical_base: u64,
    length: u64,
    flags: paging::PageFlags,
) -> Result<(), IpcError> {
    if length == 0
        || length % memory::FRAME_SIZE != 0
        || virtual_base % memory::FRAME_SIZE != 0
        || physical_base % memory::FRAME_SIZE != 0
    {
        return Err(IpcError::BadCapability);
    }
    let virtual_end = virtual_base
        .checked_add(length)
        .ok_or(IpcError::BadCapability)?;
    if virtual_base >= paging::USER_CANONICAL_LIMIT || virtual_end > paging::USER_CANONICAL_LIMIT {
        return Err(IpcError::BadCapability);
    }
    physical_base
        .checked_add(length)
        .ok_or(IpcError::BadCapability)?;

    let hhdm_offset = limine::hhdm_offset().ok_or(IpcError::BadCapability)?;
    let root_table_physical = runtime()
        .processes
        .current_process()
        .map(|process| process.context.cr3)
        .ok_or(IpcError::BadCapability)?;
    if !paging::user_range_is_unmapped(hhdm_offset, root_table_physical, virtual_base, length)
        .map_err(|_| IpcError::BadCapability)?
    {
        return Err(IpcError::BadCapability);
    }
    let allocator = frame_allocator()?;

    let mut offset = 0;
    let mut mapped_length = 0;
    while offset < length {
        let frame = memory::PhysicalFrame::from_start(
            physical_base
                .checked_add(offset)
                .ok_or(IpcError::BadCapability)?,
        )
        .ok_or(IpcError::BadCapability)?;
        let virtual_address = virtual_base
            .checked_add(offset)
            .ok_or(IpcError::BadCapability)?;
        let next_offset = offset
            .checked_add(memory::FRAME_SIZE)
            .ok_or(IpcError::BadCapability)?;
        match paging::map_page_in_root(
            hhdm_offset,
            root_table_physical,
            virtual_address,
            frame,
            flags,
            allocator,
        ) {
            Ok(()) => {}
            Err(_) => {
                rollback_current_process_physical_range(
                    hhdm_offset,
                    root_table_physical,
                    virtual_base,
                    mapped_length,
                    allocator,
                );
                return Err(IpcError::BadCapability);
            }
        }
        mapped_length = next_offset;
        offset = next_offset;
    }

    Ok(())
}

fn rollback_current_process_physical_range(
    hhdm_offset: u64,
    root_table_physical: u64,
    virtual_base: u64,
    length: u64,
    allocator: &mut memory::FrameAllocator,
) {
    let mut offset = 0;
    while offset < length {
        if let Some(virtual_address) = virtual_base.checked_add(offset) {
            let _ = paging::unmap_page_in_root(hhdm_offset, root_table_physical, virtual_address);
        }
        let Some(next_offset) = offset.checked_add(memory::FRAME_SIZE) else {
            return;
        };
        offset = next_offset;
    }

    offset = 0;
    while offset < length {
        if let Some(virtual_address) = virtual_base.checked_add(offset) {
            let _ = paging::prune_empty_user_page_tables(
                hhdm_offset,
                root_table_physical,
                virtual_address,
                allocator,
            );
        }
        let Some(next_offset) = offset.checked_add(memory::FRAME_SIZE) else {
            return;
        };
        offset = next_offset;
    }
}

fn unmap_current_process_physical_range(virtual_base: u64, length: u64) -> Result<(), IpcError> {
    let hhdm_offset = limine::hhdm_offset().ok_or(IpcError::BadCapability)?;
    let root_table_physical = runtime()
        .processes
        .current_process()
        .map(|process| process.context.cr3)
        .ok_or(IpcError::BadCapability)?;
    let allocator = frame_allocator()?;
    rollback_current_process_physical_range(
        hhdm_offset,
        root_table_physical,
        virtual_base,
        length,
        allocator,
    );
    Ok(())
}

fn device_user_mapping_base(
    window_base: u64,
    object: KernelObjectId,
    length: u64,
) -> Result<u64, IpcError> {
    if length == 0 || length > USER_DEVICE_MAPPING_STRIDE {
        return Err(IpcError::BadCapability);
    }

    let offset = object
        .raw()
        .checked_mul(USER_DEVICE_MAPPING_STRIDE)
        .ok_or(IpcError::BadCapability)?;
    let base = window_base
        .checked_add(offset)
        .ok_or(IpcError::BadCapability)?;
    let end = base.checked_add(length).ok_or(IpcError::BadCapability)?;
    if base >= paging::USER_CANONICAL_LIMIT || end > paging::USER_CANONICAL_LIMIT {
        return Err(IpcError::BadCapability);
    }
    Ok(base)
}

fn align_up(value: u64, align: u64) -> Option<u64> {
    Some(value.checked_add(align - 1)? & !(align - 1))
}

fn align_down(value: u64, align: u64) -> u64 {
    value & !(align - 1)
}

fn write_u64(buffer: &mut [u8], offset: usize, value: u64) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        buffer[offset + index] = bytes[index];
        index += 1;
    }
}

fn write_dma_mapping_info(buffer: &mut [u8; DMA_MAPPING_INFO_BYTES], mapping: DmaUserMapping) {
    write_u64(buffer, 0, mapping.virtual_base);
    write_u64(buffer, 8, mapping.physical_base);
    write_u64(buffer, 16, mapping.length);
}

fn min(left: usize, right: usize) -> usize {
    if left < right { left } else { right }
}

fn ranges_overlap(left_start: u64, left_len: u64, right_start: u64, right_len: u64) -> bool {
    if left_len == 0 || right_len == 0 {
        return false;
    }
    let left_end = left_start.saturating_add(left_len);
    let right_end = right_start.saturating_add(right_len);
    left_start < right_end && right_start < left_end
}
