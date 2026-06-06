mod error;
mod object;
mod object_table;
mod process;
mod process_runtime;
mod runtime;
mod runtime_config;
mod transport;

pub use error::{InitError, IpcError};
pub use object::{BOOT_ENDPOINT_ID, KernelObjectId};
pub(crate) use object::{
    BootModuleObject, KernelObject, MAX_OBJECTS, ProcessControlObject, SecretObject,
    StateVolumeObject, StoreObject,
};
pub(crate) use object_table::ObjectTable;
pub use process::{
    FRAME_R8, FRAME_R9, FRAME_R10, FRAME_R11, FRAME_R12, FRAME_R13, FRAME_R14, FRAME_R15,
    FRAME_RAX, FRAME_RBP, FRAME_RBX, FRAME_RCX, FRAME_RDI, FRAME_RDX, FRAME_RSI, FRAME_SIZE,
    FRAME_USER_CS, FRAME_USER_RFLAGS, FRAME_USER_RIP, FRAME_USER_RSP, FRAME_USER_SS,
    ProcessContext, ProcessId, SyscallFrame,
};
pub(crate) use process_runtime::{
    Capability, CapabilitySpace, DmaUserMapping, MAX_CAPS, MAX_OPEN_FILE_DESCRIPTIONS,
    MAX_PROCESSES, Process, ProcessState, ProcessTable,
};
pub use runtime::ScheduleResult;
pub use runtime_config::{
    BOOT_OBJECT_DMA_REGION, BOOT_OBJECT_ENDPOINT, BOOT_OBJECT_INTERRUPT_LINE,
    BOOT_OBJECT_IO_PORT_RANGE, BOOT_OBJECT_MMIO_REGION, BOOT_OBJECT_NAMESPACE,
    BOOT_OBJECT_NETWORK_PORT, BOOT_OBJECT_PCI_DEVICE, BOOT_OBJECT_STATE, BOOT_OBJECT_STORE,
    BOOT_OBJECT_TIMER, BOOT_OBJECT_VFS_ROOT, BOOT_OBJECT_VIRTIO_DEVICE, BootDmaRegionConfig,
    BootEndpointConfig, BootGrantConfig, BootGraphEdgeConfig, BootGraphNodeConfig,
    BootInterruptLineConfig, BootIoPortRangeConfig, BootMmioRegionConfig, BootModuleConfig,
    BootNamespaceConfig, BootNamespaceEntryConfig, BootNetworkPortConfig, BootPciDeviceConfig,
    BootProcessConfig, BootProcessMountConfig, BootRuntimeConfig, BootStateVolumeConfig,
    BootStoreObjectConfig, BootVfsRootConfig, BootVirtioDeviceConfig,
};
pub(crate) use runtime_config::{
    BOOT_PROCESS_MOUNT_BIND, BOOT_PROCESS_MOUNT_READ_ONLY, MAX_BOOT_NAMESPACES,
    MAX_BOOT_STATE_VOLUMES, MAX_BOOT_VFS_ROOTS, known_boot_process_mount_flags,
};
pub(crate) use transport::{
    IpcEndpoint, IpcMessage, MAX_MESSAGE_BYTES, MESSAGE_QUEUE_CAPACITY, MessageQueue,
};
