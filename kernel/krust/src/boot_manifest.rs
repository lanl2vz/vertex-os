use core::{cell::UnsafeCell, str};
use vertex_abi::{graph as graph_abi, krustboot as krustboot_abi};

use crate::serial;

pub const MODULE_STRING: &[u8] = b"krustboot-manifest";
pub const FALLBACK_MODULE_STRING: &[u8] = b"krustboot-fallback-manifest";
pub const BAD_GENERATION_MODULE_STRING: &[u8] = b"krustboot-bad-generation-manifest";

const COMPACT_MAGIC: &[u8; 16] = krustboot_abi::COMPACT_MAGIC;
const COMPACT_VERSION: u16 = krustboot_abi::COMPACT_VERSION;
const V1_MAGIC: &[u8; 16] = krustboot_abi::V1_MAGIC;
const V1_VERSION: u16 = krustboot_abi::V1_VERSION;
const V1_HEADER_SIZE: usize = krustboot_abi::V1_HEADER_SIZE;
const V1_CHECKSUM_OFFSET: usize = krustboot_abi::V1_CHECKSUM_OFFSET;
const V1_RECORD_SIZE: usize = krustboot_abi::V1_RECORD_SIZE;
const V1_RECORD_COUNT: usize = krustboot_abi::V1_RECORD_COUNT;
const V1_PAYLOAD_OFFSET: usize = krustboot_abi::V1_PAYLOAD_OFFSET;
const STRING_LEN: usize = graph_abi::STRING_LEN;
const MAX_BOOT_MODULES: usize = 16;
const MAX_PROCESSES: usize = 16;
const MAX_ENDPOINTS: usize = 16;
const MAX_GRANTS: usize = 96;
const MAX_STORE_OBJECTS: usize = 32;
const MAX_STATE_VOLUMES: usize = 4;
const MAX_NETWORK_PORTS: usize = 4;
const MAX_IO_PORT_RANGES: usize = 4;
const MAX_MMIO_REGIONS: usize = 4;
const MAX_FRAMEBUFFERS: usize = 1;
const MAX_INTERRUPT_LINES: usize = 4;
const MAX_DMA_REGIONS: usize = 4;
const MAX_PCI_DEVICES: usize = 4;
const MAX_VIRTIO_DEVICES: usize = 4;
const MAX_NAMESPACES: usize = 4;
const MAX_VFS_ROOTS: usize = 8;
const MAX_GRAPH_NODES: usize = 128;
const MAX_GRAPH_EDGES: usize = 224;
const MAX_POLICY_CAPABILITIES: usize = 128;
const MAX_POLICY_REQUIREMENTS: usize = 160;
const MAX_POLICY_PROVIDES: usize = 64;
const POLICY_VERSION: u16 = krustboot_abi::POLICY_VERSION;
const MAX_RUNTIME_OBJECTS: usize = 64;
const FIXED_RUNTIME_OBJECTS: usize = 4;
const SERIAL_LOG_ENDPOINT_NAME: &str = "serial-log";
const PAGE_SIZE: u64 = 4096;
const DMA_KERNEL_ALLOCATED_BASE: u64 = u64::MAX;
const MAX_DEVICE_MAPPING_LENGTH: u64 = 1 << 30;
const MAX_LEGACY_IRQ_LINE: u64 = 15;
pub const MAX_NAMESPACE_ENTRIES: usize = 4;
pub const MAX_PROCESS_REFS: usize = 5;
pub const MAX_PROCESS_MOUNTS: usize = 4;

pub const GRAPH_NODE_GENERATION: u16 = graph_abi::NODE_GENERATION;
pub const GRAPH_NODE_SERVICE: u16 = graph_abi::NODE_SERVICE;
pub const GRAPH_NODE_ENDPOINT: u16 = graph_abi::NODE_ENDPOINT;
pub const GRAPH_NODE_STORE_OBJECT: u16 = graph_abi::NODE_STORE_OBJECT;
pub const GRAPH_NODE_CONFIG: u16 = graph_abi::NODE_CONFIG;
pub const GRAPH_NODE_STATE_VOLUME: u16 = graph_abi::NODE_STATE_VOLUME;
pub const GRAPH_NODE_DEVICE: u16 = graph_abi::NODE_DEVICE;
pub const GRAPH_EDGE_CAPABILITY: u16 = graph_abi::EDGE_CAPABILITY;

pub const RIGHT_SEND: u16 = 1 << 0;
pub const RIGHT_RECEIVE: u16 = 1 << 1;
pub const RIGHT_READ: u16 = 1 << 2;
pub const RIGHT_WRITE: u16 = 1 << 3;
pub const RIGHT_SNAPSHOT: u16 = 1 << 4;
pub const RIGHT_RESTORE: u16 = 1 << 5;
pub const RIGHT_CONTROL: u16 = 1 << 6;
pub const RIGHT_BIND: u16 = 1 << 7;
pub const RIGHT_LISTEN: u16 = 1 << 8;
pub const RIGHT_MAP: u16 = 1 << 9;
pub const RIGHT_RESOLVE: u16 = 1 << 10;
pub const RIGHT_CREATE: u16 = 1 << 11;
pub const RIGHT_UNLINK: u16 = 1 << 12;
pub const RIGHT_RENAME: u16 = 1 << 13;
pub const RIGHT_MOUNT: u16 = 1 << 14;

pub const OBJECT_ENDPOINT: u16 = 1;
pub const OBJECT_STORE: u16 = 2;
pub const OBJECT_STATE: u16 = 3;
pub const OBJECT_TIMER: u16 = 4;
pub const OBJECT_NETWORK_PORT: u16 = 5;
pub const OBJECT_IO_PORT_RANGE: u16 = 6;
pub const OBJECT_MMIO_REGION: u16 = 7;
pub const OBJECT_INTERRUPT_LINE: u16 = 8;
pub const OBJECT_DMA_REGION: u16 = 9;
pub const OBJECT_PCI_DEVICE: u16 = 10;
pub const OBJECT_VIRTIO_DEVICE: u16 = 11;
pub const OBJECT_NAMESPACE: u16 = 12;
pub const OBJECT_VFS_ROOT: u16 = 13;
pub const OBJECT_FRAMEBUFFER: u16 = 14;
pub const PROCESS_MOUNT_FLAG_BIND: u16 = 1;
pub const PROCESS_MOUNT_FLAG_READ_ONLY: u16 = 1 << 1;

#[derive(Clone, Copy)]
pub struct BootModule<'a> {
    pub name: &'a str,
    pub module_string: &'a str,
}

#[derive(Clone, Copy)]
pub struct Process<'a> {
    pub name: &'a str,
    pub module_string: &'a str,
    pub initial: bool,
    pub restart_policy: u16,
    pub service_id: &'a str,
    pub health_kind: &'a str,
    pub mount_root: &'a str,
    pub mounts: [Option<ProcessMount<'a>>; MAX_PROCESS_MOUNTS],
    pub mount_count: usize,
    pub start_after: [u16; MAX_PROCESS_REFS],
    pub start_after_count: usize,
    pub requires_endpoint: [u16; MAX_PROCESS_REFS],
    pub requires_endpoint_rights: [u16; MAX_PROCESS_REFS],
    pub requires_endpoint_count: usize,
    pub provides_endpoint: [u16; MAX_PROCESS_REFS],
    pub provides_endpoint_count: usize,
}

#[derive(Clone, Copy)]
pub struct ProcessMount<'a> {
    pub path: &'a str,
    pub source: &'a str,
    pub flags: u16,
}

#[derive(Clone, Copy)]
pub struct Endpoint<'a> {
    pub name: &'a str,
}

#[derive(Clone, Copy)]
pub struct Grant {
    pub process_index: usize,
    pub object_kind: u16,
    pub object_index: usize,
    pub cap_slot: u64,
    pub rights: u16,
}

#[derive(Clone, Copy)]
pub struct StoreObject<'a> {
    pub id: &'a str,
    pub module_string: &'a str,
    pub hash: &'a str,
    pub size: u64,
}

#[derive(Clone, Copy)]
pub struct StateVolume<'a> {
    pub id: &'a str,
    pub owner: &'a str,
    pub schema_version: &'a str,
    pub storage_class: &'a str,
    pub migration_policy: &'a str,
    pub retention_policy: &'a str,
    pub sharing_policy: &'a str,
}

#[derive(Clone, Copy)]
pub struct NetworkPort<'a> {
    pub id: &'a str,
}

#[derive(Clone, Copy)]
pub struct IoPortRange<'a> {
    pub id: &'a str,
    pub base: u64,
    pub length: u64,
}

#[derive(Clone, Copy)]
pub struct MmioRegion<'a> {
    pub id: &'a str,
    pub base: u64,
    pub length: u64,
}

#[derive(Clone, Copy)]
pub struct Framebuffer<'a> {
    pub id: &'a str,
}

#[derive(Clone, Copy)]
pub struct InterruptLine<'a> {
    pub id: &'a str,
    pub line: u64,
}

#[derive(Clone, Copy)]
pub struct DmaRegion<'a> {
    pub id: &'a str,
    pub base: u64,
    pub length: u64,
}

#[derive(Clone, Copy)]
pub struct PciDevice<'a> {
    pub id: &'a str,
    pub kind: &'a str,
}

#[derive(Clone, Copy)]
pub struct VirtioDevice<'a> {
    pub id: &'a str,
    pub transport: &'a str,
}

#[derive(Clone, Copy)]
pub struct NamespaceEntry<'a> {
    pub path: &'a str,
    pub object_kind: u16,
    pub object_index: usize,
    pub rights: u16,
}

#[derive(Clone, Copy)]
pub struct Namespace<'a> {
    pub id: &'a str,
    pub entries: [Option<NamespaceEntry<'a>>; MAX_NAMESPACE_ENTRIES],
    pub entry_count: usize,
}

#[derive(Clone, Copy)]
pub struct VfsRoot<'a> {
    pub id: &'a str,
    pub root_path: &'a str,
}

#[derive(Clone, Copy)]
pub struct GraphNode<'a> {
    pub kind: u16,
    pub object_kind: u16,
    pub id: &'a str,
    pub label: &'a str,
}

#[derive(Clone, Copy)]
pub struct GraphEdge<'a> {
    pub kind: u16,
    pub from_index: usize,
    pub to_index: usize,
    pub rights: u16,
    pub id: &'a str,
}

#[derive(Clone, Copy)]
pub struct PolicyCapability<'a> {
    pub id: &'a str,
    pub provider: &'a str,
    pub object_kind: u16,
    pub object_index: usize,
    pub rights: u16,
}

#[derive(Clone, Copy)]
pub struct PolicyRequirement<'a> {
    pub service: &'a str,
    pub capability: &'a str,
    pub rights: u16,
}

#[derive(Clone, Copy)]
pub struct PolicyProvide<'a> {
    pub service: &'a str,
    pub capability: &'a str,
}

pub struct Manifest<'a> {
    generation_id: &'a str,
    parent_generation_id: &'a str,
    source_base: u64,
    source_len: u64,
    graph_store_base: u64,
    graph_store_len: u64,
    graph_store_checksum: u32,
    layout_version: u16,
    record_count: usize,
    boot_modules: [Option<BootModule<'a>>; MAX_BOOT_MODULES],
    boot_module_count: usize,
    processes: [Option<Process<'a>>; MAX_PROCESSES],
    process_count: usize,
    endpoints: [Option<Endpoint<'a>>; MAX_ENDPOINTS],
    endpoint_count: usize,
    grants: [Option<Grant>; MAX_GRANTS],
    grant_count: usize,
    store_objects: [Option<StoreObject<'a>>; MAX_STORE_OBJECTS],
    store_object_count: usize,
    state_volumes: [Option<StateVolume<'a>>; MAX_STATE_VOLUMES],
    state_volume_count: usize,
    network_ports: [Option<NetworkPort<'a>>; MAX_NETWORK_PORTS],
    network_port_count: usize,
    io_ports: [Option<IoPortRange<'a>>; MAX_IO_PORT_RANGES],
    io_port_count: usize,
    mmio_regions: [Option<MmioRegion<'a>>; MAX_MMIO_REGIONS],
    mmio_region_count: usize,
    framebuffers: [Option<Framebuffer<'a>>; MAX_FRAMEBUFFERS],
    framebuffer_count: usize,
    interrupt_lines: [Option<InterruptLine<'a>>; MAX_INTERRUPT_LINES],
    interrupt_line_count: usize,
    dma_regions: [Option<DmaRegion<'a>>; MAX_DMA_REGIONS],
    dma_region_count: usize,
    pci_devices: [Option<PciDevice<'a>>; MAX_PCI_DEVICES],
    pci_device_count: usize,
    virtio_devices: [Option<VirtioDevice<'a>>; MAX_VIRTIO_DEVICES],
    virtio_device_count: usize,
    namespaces: [Option<Namespace<'a>>; MAX_NAMESPACES],
    namespace_count: usize,
    vfs_roots: [Option<VfsRoot<'a>>; MAX_VFS_ROOTS],
    vfs_root_count: usize,
    graph_nodes: [Option<GraphNode<'a>>; MAX_GRAPH_NODES],
    graph_node_count: usize,
    graph_edges: [Option<GraphEdge<'a>>; MAX_GRAPH_EDGES],
    graph_edge_count: usize,
    policy_version: u16,
    policy_hash: &'a str,
    policy_capabilities: [Option<PolicyCapability<'a>>; MAX_POLICY_CAPABILITIES],
    policy_capability_count: usize,
    policy_requirements: [Option<PolicyRequirement<'a>>; MAX_POLICY_REQUIREMENTS],
    policy_requirement_count: usize,
    policy_provides: [Option<PolicyProvide<'a>>; MAX_POLICY_PROVIDES],
    policy_provide_count: usize,
}

struct Global<T>(UnsafeCell<T>);

unsafe impl<T> Sync for Global<T> {}

static SELECTED_MANIFEST: Global<Manifest<'static>> = Global(UnsafeCell::new(Manifest::empty()));
static FALLBACK_MANIFEST: Global<Manifest<'static>> = Global(UnsafeCell::new(Manifest::empty()));
static BAD_GENERATION_MANIFEST: Global<Manifest<'static>> =
    Global(UnsafeCell::new(Manifest::empty()));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    Truncated,
    BadMagic,
    UnsupportedVersion,
    TooManyBootModules,
    TooManyProcesses,
    TooManyEndpoints,
    TooManyGrants,
    TooManyStoreObjects,
    TooManyStateVolumes,
    TooManyNetworkPorts,
    TooManyIoPortRanges,
    TooManyMmioRegions,
    TooManyFramebuffers,
    TooManyInterruptLines,
    TooManyDmaRegions,
    TooManyPciDevices,
    TooManyVirtioDevices,
    TooManyNamespaces,
    TooManyNamespaceEntries,
    TooManyVfsRoots,
    TooManyGraphNodes,
    TooManyGraphEdges,
    TooManyPolicyCapabilities,
    TooManyPolicyRequirements,
    TooManyPolicyProvides,
    TooManyRuntimeObjects,
    InvalidString,
    InvalidReference,
    InvalidRights,
    InvalidObjectKind,
    InvalidGraphRecord,
    UnsupportedStateVolumes,
    TrailingBytes,
    BadChecksum,
    BadGraphStoreChecksum,
    BadPolicyHash,
    InvalidPolicy,
    BadRecordTable,
    OutOfBoundsRecord,
}

impl<'a> Manifest<'a> {
    const fn empty() -> Self {
        Self {
            generation_id: "",
            parent_generation_id: "",
            source_base: 0,
            source_len: 0,
            graph_store_base: 0,
            graph_store_len: 0,
            graph_store_checksum: 0,
            layout_version: 0,
            record_count: 0,
            boot_modules: [None; MAX_BOOT_MODULES],
            boot_module_count: 0,
            processes: [None; MAX_PROCESSES],
            process_count: 0,
            endpoints: [None; MAX_ENDPOINTS],
            endpoint_count: 0,
            grants: [None; MAX_GRANTS],
            grant_count: 0,
            store_objects: [None; MAX_STORE_OBJECTS],
            store_object_count: 0,
            state_volumes: [None; MAX_STATE_VOLUMES],
            state_volume_count: 0,
            network_ports: [None; MAX_NETWORK_PORTS],
            network_port_count: 0,
            io_ports: [None; MAX_IO_PORT_RANGES],
            io_port_count: 0,
            mmio_regions: [None; MAX_MMIO_REGIONS],
            mmio_region_count: 0,
            framebuffers: [None; MAX_FRAMEBUFFERS],
            framebuffer_count: 0,
            interrupt_lines: [None; MAX_INTERRUPT_LINES],
            interrupt_line_count: 0,
            dma_regions: [None; MAX_DMA_REGIONS],
            dma_region_count: 0,
            pci_devices: [None; MAX_PCI_DEVICES],
            pci_device_count: 0,
            virtio_devices: [None; MAX_VIRTIO_DEVICES],
            virtio_device_count: 0,
            namespaces: [None; MAX_NAMESPACES],
            namespace_count: 0,
            vfs_roots: [None; MAX_VFS_ROOTS],
            vfs_root_count: 0,
            graph_nodes: [None; MAX_GRAPH_NODES],
            graph_node_count: 0,
            graph_edges: [None; MAX_GRAPH_EDGES],
            graph_edge_count: 0,
            policy_version: 0,
            policy_hash: "",
            policy_capabilities: [None; MAX_POLICY_CAPABILITIES],
            policy_capability_count: 0,
            policy_requirements: [None; MAX_POLICY_REQUIREMENTS],
            policy_requirement_count: 0,
            policy_provides: [None; MAX_POLICY_PROVIDES],
            policy_provide_count: 0,
        }
    }

    fn reset(&mut self) {
        self.generation_id = "";
        self.parent_generation_id = "";
        self.source_base = 0;
        self.source_len = 0;
        self.graph_store_base = 0;
        self.graph_store_len = 0;
        self.graph_store_checksum = 0;
        self.layout_version = 0;
        self.record_count = 0;
        self.boot_modules.fill(None);
        self.boot_module_count = 0;
        self.processes.fill(None);
        self.process_count = 0;
        self.endpoints.fill(None);
        self.endpoint_count = 0;
        self.grants.fill(None);
        self.grant_count = 0;
        self.store_objects.fill(None);
        self.store_object_count = 0;
        self.state_volumes.fill(None);
        self.state_volume_count = 0;
        self.network_ports.fill(None);
        self.network_port_count = 0;
        self.io_ports.fill(None);
        self.io_port_count = 0;
        self.mmio_regions.fill(None);
        self.mmio_region_count = 0;
        self.framebuffers.fill(None);
        self.framebuffer_count = 0;
        self.interrupt_lines.fill(None);
        self.interrupt_line_count = 0;
        self.dma_regions.fill(None);
        self.dma_region_count = 0;
        self.pci_devices.fill(None);
        self.pci_device_count = 0;
        self.virtio_devices.fill(None);
        self.virtio_device_count = 0;
        self.namespaces.fill(None);
        self.namespace_count = 0;
        self.vfs_roots.fill(None);
        self.vfs_root_count = 0;
        self.graph_nodes.fill(None);
        self.graph_node_count = 0;
        self.graph_edges.fill(None);
        self.graph_edge_count = 0;
        self.policy_version = 0;
        self.policy_hash = "";
        self.policy_capabilities.fill(None);
        self.policy_capability_count = 0;
        self.policy_requirements.fill(None);
        self.policy_requirement_count = 0;
        self.policy_provides.fill(None);
        self.policy_provide_count = 0;
    }

    pub fn generation_id(&self) -> &'a str {
        self.generation_id
    }

    pub fn parent_generation_id(&self) -> &'a str {
        self.parent_generation_id
    }

    pub fn source_base(&self) -> u64 {
        self.source_base
    }

    pub fn source_len(&self) -> u64 {
        self.source_len
    }

    pub fn graph_store_base(&self) -> u64 {
        self.graph_store_base
    }

    pub fn graph_store_len(&self) -> u64 {
        self.graph_store_len
    }

    pub fn graph_store_checksum(&self) -> u32 {
        self.graph_store_checksum
    }

    pub fn layout_version(&self) -> u16 {
        self.layout_version
    }

    pub fn record_count(&self) -> usize {
        self.record_count
    }

    pub fn boot_module_count(&self) -> usize {
        self.boot_module_count
    }

    pub fn process_count(&self) -> usize {
        self.process_count
    }

    pub fn endpoint_count(&self) -> usize {
        self.endpoint_count
    }

    pub fn grant_count(&self) -> usize {
        self.grant_count
    }

    pub fn store_object_count(&self) -> usize {
        self.store_object_count
    }

    pub fn state_volume_count(&self) -> usize {
        self.state_volume_count
    }

    pub fn network_port_count(&self) -> usize {
        self.network_port_count
    }

    pub fn io_port_count(&self) -> usize {
        self.io_port_count
    }

    pub fn mmio_region_count(&self) -> usize {
        self.mmio_region_count
    }

    pub fn interrupt_line_count(&self) -> usize {
        self.interrupt_line_count
    }

    pub fn framebuffer_count(&self) -> usize {
        self.framebuffer_count
    }

    pub fn dma_region_count(&self) -> usize {
        self.dma_region_count
    }

    pub fn pci_device_count(&self) -> usize {
        self.pci_device_count
    }

    pub fn virtio_device_count(&self) -> usize {
        self.virtio_device_count
    }

    pub fn namespace_count(&self) -> usize {
        self.namespace_count
    }

    pub fn vfs_root_count(&self) -> usize {
        self.vfs_root_count
    }

    pub fn graph_node_count(&self) -> usize {
        self.graph_node_count
    }

    pub fn graph_edge_count(&self) -> usize {
        self.graph_edge_count
    }

    pub fn policy_version(&self) -> u16 {
        self.policy_version
    }

    pub fn policy_hash(&self) -> &'a str {
        self.policy_hash
    }

    pub fn policy_capability_count(&self) -> usize {
        self.policy_capability_count
    }

    pub fn policy_requirement_count(&self) -> usize {
        self.policy_requirement_count
    }

    pub fn policy_provide_count(&self) -> usize {
        self.policy_provide_count
    }

    pub fn boot_module(&self, index: usize) -> Option<BootModule<'a>> {
        if index < self.boot_module_count {
            self.boot_modules[index]
        } else {
            None
        }
    }

    pub fn process(&self, index: usize) -> Option<Process<'a>> {
        if index < self.process_count {
            self.processes[index]
        } else {
            None
        }
    }

    pub fn endpoint(&self, index: usize) -> Option<Endpoint<'a>> {
        if index < self.endpoint_count {
            self.endpoints[index]
        } else {
            None
        }
    }

    pub fn grant(&self, index: usize) -> Option<Grant> {
        if index < self.grant_count {
            self.grants[index]
        } else {
            None
        }
    }

    pub fn store_object(&self, index: usize) -> Option<StoreObject<'a>> {
        if index < self.store_object_count {
            self.store_objects[index]
        } else {
            None
        }
    }

    pub fn state_volume(&self, index: usize) -> Option<StateVolume<'a>> {
        if index < self.state_volume_count {
            self.state_volumes[index]
        } else {
            None
        }
    }

    pub fn network_port(&self, index: usize) -> Option<NetworkPort<'a>> {
        if index < self.network_port_count {
            self.network_ports[index]
        } else {
            None
        }
    }

    pub fn io_port(&self, index: usize) -> Option<IoPortRange<'a>> {
        if index < self.io_port_count {
            self.io_ports[index]
        } else {
            None
        }
    }

    pub fn mmio_region(&self, index: usize) -> Option<MmioRegion<'a>> {
        if index < self.mmio_region_count {
            self.mmio_regions[index]
        } else {
            None
        }
    }

    pub fn framebuffer(&self, index: usize) -> Option<Framebuffer<'a>> {
        if index < self.framebuffer_count {
            self.framebuffers[index]
        } else {
            None
        }
    }

    pub fn interrupt_line(&self, index: usize) -> Option<InterruptLine<'a>> {
        if index < self.interrupt_line_count {
            self.interrupt_lines[index]
        } else {
            None
        }
    }

    pub fn dma_region(&self, index: usize) -> Option<DmaRegion<'a>> {
        if index < self.dma_region_count {
            self.dma_regions[index]
        } else {
            None
        }
    }

    pub fn pci_device(&self, index: usize) -> Option<PciDevice<'a>> {
        if index < self.pci_device_count {
            self.pci_devices[index]
        } else {
            None
        }
    }

    pub fn virtio_device(&self, index: usize) -> Option<VirtioDevice<'a>> {
        if index < self.virtio_device_count {
            self.virtio_devices[index]
        } else {
            None
        }
    }

    pub fn namespace(&self, index: usize) -> Option<Namespace<'a>> {
        if index < self.namespace_count {
            self.namespaces[index]
        } else {
            None
        }
    }

    pub fn vfs_root(&self, index: usize) -> Option<VfsRoot<'a>> {
        if index < self.vfs_root_count {
            self.vfs_roots[index]
        } else {
            None
        }
    }

    pub fn graph_node(&self, index: usize) -> Option<GraphNode<'a>> {
        if index < self.graph_node_count {
            self.graph_nodes[index]
        } else {
            None
        }
    }

    pub fn graph_edge(&self, index: usize) -> Option<GraphEdge<'a>> {
        if index < self.graph_edge_count {
            self.graph_edges[index]
        } else {
            None
        }
    }

    pub fn policy_capability(&self, index: usize) -> Option<PolicyCapability<'a>> {
        if index < self.policy_capability_count {
            self.policy_capabilities[index]
        } else {
            None
        }
    }

    pub fn policy_requirement(&self, index: usize) -> Option<PolicyRequirement<'a>> {
        if index < self.policy_requirement_count {
            self.policy_requirements[index]
        } else {
            None
        }
    }

    pub fn policy_provide(&self, index: usize) -> Option<PolicyProvide<'a>> {
        if index < self.policy_provide_count {
            self.policy_provides[index]
        } else {
            None
        }
    }
}

pub fn parse_selected(bytes: &'static [u8]) -> Result<&'static Manifest<'static>, ParseError> {
    parse_static(bytes, &SELECTED_MANIFEST)
}

pub fn parse_fallback(bytes: &'static [u8]) -> Result<&'static Manifest<'static>, ParseError> {
    parse_static(bytes, &FALLBACK_MANIFEST)
}

pub fn parse_bad_generation(
    bytes: &'static [u8],
) -> Result<&'static Manifest<'static>, ParseError> {
    parse_static(bytes, &BAD_GENERATION_MANIFEST)
}

fn parse_static(
    bytes: &'static [u8],
    slot: &'static Global<Manifest<'static>>,
) -> Result<&'static Manifest<'static>, ParseError> {
    let manifest = unsafe { &mut *slot.0.get() };
    parse_into(bytes, manifest)?;
    Ok(unsafe { &*slot.0.get() })
}

fn parse_into(bytes: &'static [u8], manifest: &mut Manifest<'static>) -> Result<(), ParseError> {
    let payload = parse_v1_payload(bytes)?;
    parse_compact_into(payload, manifest)?;
    manifest.layout_version = V1_VERSION;
    manifest.record_count = V1_RECORD_COUNT;
    Ok(())
}

fn parse_v1_payload(bytes: &'static [u8]) -> Result<&'static [u8], ParseError> {
    if bytes.len() < V1_MAGIC.len() {
        return Err(ParseError::Truncated);
    }

    let mut reader = Reader::new(bytes);
    if reader.read_exact(V1_MAGIC.len())? != V1_MAGIC {
        return Err(ParseError::BadMagic);
    }

    if reader.read_u16()? != V1_VERSION {
        return Err(ParseError::UnsupportedVersion);
    }

    let header_size = reader.read_u16()? as usize;
    let total_size = reader.read_u32()? as usize;
    let record_table_offset = reader.read_u32()? as usize;
    let record_count = reader.read_u16()? as usize;
    let _reserved = reader.read_u16()?;
    let checksum = reader.read_u32()?;
    let _generation_id = reader.read_fixed_str()?;
    let _parent_generation_id = reader.read_fixed_str_allow_empty()?;

    if header_size != V1_HEADER_SIZE
        || record_table_offset != V1_HEADER_SIZE
        || record_count != V1_RECORD_COUNT
        || total_size != bytes.len()
        || total_size < V1_PAYLOAD_OFFSET
    {
        return Err(ParseError::BadRecordTable);
    }

    if checksum != v1_checksum(bytes) {
        return Err(ParseError::BadChecksum);
    }

    let mut seen = [false; V1_RECORD_COUNT + 1];
    let mut record_index = 0;
    while record_index < record_count {
        let offset = record_table_offset + record_index * V1_RECORD_SIZE;
        let record = Record::read(bytes, offset)?;
        if record.kind == 0 || record.kind as usize >= seen.len() {
            return Err(ParseError::BadRecordTable);
        }
        if seen[record.kind as usize] {
            return Err(ParseError::BadRecordTable);
        }
        seen[record.kind as usize] = true;
        let end = record
            .offset
            .checked_add(record.length)
            .ok_or(ParseError::OutOfBoundsRecord)?;
        if record.offset > bytes.len() || end > bytes.len() {
            return Err(ParseError::OutOfBoundsRecord);
        }
        record_index += 1;
    }

    let mut kind = 1;
    while kind <= V1_RECORD_COUNT {
        if !seen[kind] {
            return Err(ParseError::BadRecordTable);
        }
        kind += 1;
    }

    Ok(&bytes[V1_PAYLOAD_OFFSET..])
}

fn parse_compact_into(
    bytes: &'static [u8],
    manifest: &mut Manifest<'static>,
) -> Result<(), ParseError> {
    let mut reader = Reader::new(bytes);
    if reader.read_exact(COMPACT_MAGIC.len())? != COMPACT_MAGIC {
        return Err(ParseError::BadMagic);
    }

    if reader.read_u16()? != COMPACT_VERSION {
        return Err(ParseError::UnsupportedVersion);
    }

    let boot_module_count = reader.read_count(MAX_BOOT_MODULES, ParseError::TooManyBootModules)?;
    let process_count = reader.read_count(MAX_PROCESSES, ParseError::TooManyProcesses)?;
    let endpoint_count = reader.read_count(MAX_ENDPOINTS, ParseError::TooManyEndpoints)?;
    let grant_count = reader.read_count(MAX_GRANTS, ParseError::TooManyGrants)?;
    let store_object_count =
        reader.read_count(MAX_STORE_OBJECTS, ParseError::TooManyStoreObjects)?;
    let state_volume_count =
        reader.read_count(MAX_STATE_VOLUMES, ParseError::TooManyStateVolumes)?;
    let network_port_count =
        reader.read_count(MAX_NETWORK_PORTS, ParseError::TooManyNetworkPorts)?;
    let io_port_count = reader.read_count(MAX_IO_PORT_RANGES, ParseError::TooManyIoPortRanges)?;
    let mmio_region_count = reader.read_count(MAX_MMIO_REGIONS, ParseError::TooManyMmioRegions)?;
    let framebuffer_count = reader.read_count(MAX_FRAMEBUFFERS, ParseError::TooManyFramebuffers)?;
    let interrupt_line_count =
        reader.read_count(MAX_INTERRUPT_LINES, ParseError::TooManyInterruptLines)?;
    let dma_region_count = reader.read_count(MAX_DMA_REGIONS, ParseError::TooManyDmaRegions)?;
    let pci_device_count = reader.read_count(MAX_PCI_DEVICES, ParseError::TooManyPciDevices)?;
    let virtio_device_count =
        reader.read_count(MAX_VIRTIO_DEVICES, ParseError::TooManyVirtioDevices)?;
    let namespace_count = reader.read_count(MAX_NAMESPACES, ParseError::TooManyNamespaces)?;
    let vfs_root_count = reader.read_count(MAX_VFS_ROOTS, ParseError::TooManyVfsRoots)?;
    validate_runtime_object_budget(
        endpoint_count,
        store_object_count,
        state_volume_count,
        network_port_count,
        io_port_count,
        mmio_region_count,
        framebuffer_count,
        interrupt_line_count,
        dma_region_count,
        pci_device_count,
        virtio_device_count,
        namespace_count,
        vfs_root_count,
    )?;
    let generation_id = reader.read_fixed_str()?;
    let parent_generation_id = reader.read_fixed_str_allow_empty()?;
    let graph_node_count = reader.read_count(MAX_GRAPH_NODES, ParseError::TooManyGraphNodes)?;
    let graph_edge_count = reader.read_count(MAX_GRAPH_EDGES, ParseError::TooManyGraphEdges)?;
    let graph_store_checksum = reader.read_u32()?;

    manifest.reset();
    manifest.generation_id = generation_id;
    manifest.parent_generation_id = parent_generation_id;
    manifest.source_base = bytes.as_ptr() as u64;
    manifest.source_len = bytes.len() as u64;
    manifest.graph_store_checksum = graph_store_checksum;
    manifest.boot_module_count = boot_module_count;
    manifest.process_count = process_count;
    manifest.endpoint_count = endpoint_count;
    manifest.grant_count = grant_count;
    manifest.store_object_count = store_object_count;
    manifest.state_volume_count = state_volume_count;
    manifest.network_port_count = network_port_count;
    manifest.io_port_count = io_port_count;
    manifest.mmio_region_count = mmio_region_count;
    manifest.framebuffer_count = framebuffer_count;
    manifest.interrupt_line_count = interrupt_line_count;
    manifest.dma_region_count = dma_region_count;
    manifest.pci_device_count = pci_device_count;
    manifest.virtio_device_count = virtio_device_count;
    manifest.namespace_count = namespace_count;
    manifest.vfs_root_count = vfs_root_count;
    manifest.graph_node_count = graph_node_count;
    manifest.graph_edge_count = graph_edge_count;

    let mut index = 0;
    while index < boot_module_count {
        manifest.boot_modules[index] = Some(BootModule {
            name: reader.read_fixed_str()?,
            module_string: reader.read_fixed_str()?,
        });
        index += 1;
    }

    index = 0;
    while index < process_count {
        let name = reader.read_fixed_str()?;
        let module_string = reader.read_fixed_str()?;
        let flags = reader.read_u16()?;
        let restart_policy = reader.read_u16()?;
        let service_id = reader.read_fixed_str()?;
        let health_kind = reader.read_fixed_str_allow_empty()?;
        let mount_root = reader.read_fixed_str()?;
        let (mounts, mount_count) = reader.read_process_mount_list()?;
        let (start_after, start_after_count) = reader.read_ref_list()?;
        let (requires_endpoint, requires_endpoint_rights, requires_endpoint_count) =
            reader.read_endpoint_requirement_list()?;
        let (provides_endpoint, provides_endpoint_count) = reader.read_ref_list()?;
        manifest.processes[index] = Some(Process {
            name,
            module_string,
            initial: flags & 1 != 0,
            restart_policy,
            service_id,
            health_kind,
            mount_root,
            mounts,
            mount_count,
            start_after,
            start_after_count,
            requires_endpoint,
            requires_endpoint_rights,
            requires_endpoint_count,
            provides_endpoint,
            provides_endpoint_count,
        });
        index += 1;
    }

    index = 0;
    while index < endpoint_count {
        manifest.endpoints[index] = Some(Endpoint {
            name: reader.read_fixed_str()?,
        });
        index += 1;
    }

    index = 0;
    while index < grant_count {
        let process_index = reader.read_u16()? as usize;
        let object_kind = reader.read_u16()?;
        if object_kind == OBJECT_STATE {
            return Err(ParseError::UnsupportedStateVolumes);
        }
        let object_index = reader.read_u16()? as usize;
        let cap_slot = reader.read_u16()? as u64;
        let rights = reader.read_u16()?;
        let _reserved = reader.read_u16()?;
        manifest.grants[index] = Some(Grant {
            process_index,
            object_kind,
            object_index,
            cap_slot,
            rights,
        });
        index += 1;
    }

    index = 0;
    while index < store_object_count {
        manifest.store_objects[index] = Some(StoreObject {
            id: reader.read_fixed_str()?,
            module_string: reader.read_fixed_str()?,
            hash: reader.read_fixed_str()?,
            size: reader.read_u64()?,
        });
        index += 1;
    }

    index = 0;
    while index < state_volume_count {
        manifest.state_volumes[index] = Some(StateVolume {
            id: reader.read_fixed_str()?,
            owner: reader.read_fixed_str()?,
            schema_version: reader.read_fixed_str()?,
            storage_class: reader.read_fixed_str()?,
            migration_policy: reader.read_fixed_str()?,
            retention_policy: reader.read_fixed_str()?,
            sharing_policy: reader.read_fixed_str()?,
        });
        index += 1;
    }

    index = 0;
    while index < network_port_count {
        manifest.network_ports[index] = Some(NetworkPort {
            id: reader.read_fixed_str()?,
        });
        index += 1;
    }

    index = 0;
    while index < io_port_count {
        manifest.io_ports[index] = Some(IoPortRange {
            id: reader.read_fixed_str()?,
            base: reader.read_u64()?,
            length: reader.read_u64()?,
        });
        index += 1;
    }

    index = 0;
    while index < mmio_region_count {
        manifest.mmio_regions[index] = Some(MmioRegion {
            id: reader.read_fixed_str()?,
            base: reader.read_u64()?,
            length: reader.read_u64()?,
        });
        index += 1;
    }

    index = 0;
    while index < framebuffer_count {
        manifest.framebuffers[index] = Some(Framebuffer {
            id: reader.read_fixed_str()?,
        });
        index += 1;
    }

    index = 0;
    while index < interrupt_line_count {
        manifest.interrupt_lines[index] = Some(InterruptLine {
            id: reader.read_fixed_str()?,
            line: reader.read_u64()?,
        });
        index += 1;
    }

    index = 0;
    while index < dma_region_count {
        manifest.dma_regions[index] = Some(DmaRegion {
            id: reader.read_fixed_str()?,
            base: reader.read_u64()?,
            length: reader.read_u64()?,
        });
        index += 1;
    }

    index = 0;
    while index < pci_device_count {
        manifest.pci_devices[index] = Some(PciDevice {
            id: reader.read_fixed_str()?,
            kind: reader.read_fixed_str()?,
        });
        index += 1;
    }

    index = 0;
    while index < virtio_device_count {
        manifest.virtio_devices[index] = Some(VirtioDevice {
            id: reader.read_fixed_str()?,
            transport: reader.read_fixed_str()?,
        });
        index += 1;
    }

    index = 0;
    while index < namespace_count {
        let id = reader.read_fixed_str()?;
        let entry_count =
            reader.read_count(MAX_NAMESPACE_ENTRIES, ParseError::TooManyNamespaceEntries)?;
        let mut entries = [None; MAX_NAMESPACE_ENTRIES];
        let mut entry_index = 0;
        while entry_index < entry_count {
            entries[entry_index] = Some(NamespaceEntry {
                path: reader.read_fixed_str()?,
                object_kind: reader.read_u16()?,
                object_index: reader.read_u16()? as usize,
                rights: reader.read_u16()?,
            });
            let _reserved = reader.read_u16()?;
            entry_index += 1;
        }
        manifest.namespaces[index] = Some(Namespace {
            id,
            entries,
            entry_count,
        });
        index += 1;
    }

    index = 0;
    while index < vfs_root_count {
        manifest.vfs_roots[index] = Some(VfsRoot {
            id: reader.read_fixed_str()?,
            root_path: reader.read_fixed_str()?,
        });
        index += 1;
    }

    let graph_store_start = reader.offset;

    index = 0;
    while index < graph_node_count {
        manifest.graph_nodes[index] = Some(GraphNode {
            kind: reader.read_u16()?,
            object_kind: reader.read_u16()?,
            id: reader.read_fixed_str()?,
            label: reader.read_fixed_str_allow_empty()?,
        });
        index += 1;
    }

    index = 0;
    while index < graph_edge_count {
        manifest.graph_edges[index] = Some(GraphEdge {
            kind: reader.read_u16()?,
            from_index: reader.read_u16()? as usize,
            to_index: reader.read_u16()? as usize,
            rights: reader.read_u16()?,
            id: reader.read_fixed_str()?,
        });
        index += 1;
    }

    let graph_store_end = reader.offset;
    manifest.graph_store_base = bytes[graph_store_start..].as_ptr() as u64;
    manifest.graph_store_len = (graph_store_end - graph_store_start) as u64;
    if checksum32(&bytes[graph_store_start..graph_store_end]) != graph_store_checksum {
        return Err(ParseError::BadGraphStoreChecksum);
    }

    let policy_version = reader.read_u16()?;
    if policy_version != POLICY_VERSION {
        return Err(ParseError::InvalidPolicy);
    }
    let policy_capability_count = reader.read_count(
        MAX_POLICY_CAPABILITIES,
        ParseError::TooManyPolicyCapabilities,
    )?;
    let policy_requirement_count = reader.read_count(
        MAX_POLICY_REQUIREMENTS,
        ParseError::TooManyPolicyRequirements,
    )?;
    let policy_provide_count =
        reader.read_count(MAX_POLICY_PROVIDES, ParseError::TooManyPolicyProvides)?;
    let policy_hash = reader.read_fixed_str()?;
    manifest.policy_version = policy_version;
    manifest.policy_hash = policy_hash;
    manifest.policy_capability_count = policy_capability_count;
    manifest.policy_requirement_count = policy_requirement_count;
    manifest.policy_provide_count = policy_provide_count;

    let policy_records_start = reader.offset;
    index = 0;
    while index < policy_capability_count {
        let id = reader.read_fixed_str()?;
        let provider = reader.read_fixed_str()?;
        let object_kind = reader.read_u16()?;
        let object_index = reader.read_u16()? as usize;
        let rights = reader.read_u16()?;
        let reserved = reader.read_u16()?;
        if reserved != 0 {
            return Err(ParseError::InvalidPolicy);
        }
        manifest.policy_capabilities[index] = Some(PolicyCapability {
            id,
            provider,
            object_kind,
            object_index,
            rights,
        });
        index += 1;
    }

    index = 0;
    while index < policy_requirement_count {
        let service = reader.read_fixed_str()?;
        let capability = reader.read_fixed_str()?;
        let rights = reader.read_u16()?;
        let reserved = reader.read_u16()?;
        if reserved != 0 {
            return Err(ParseError::InvalidPolicy);
        }
        manifest.policy_requirements[index] = Some(PolicyRequirement {
            service,
            capability,
            rights,
        });
        index += 1;
    }

    index = 0;
    while index < policy_provide_count {
        manifest.policy_provides[index] = Some(PolicyProvide {
            service: reader.read_fixed_str()?,
            capability: reader.read_fixed_str()?,
        });
        index += 1;
    }
    let policy_records_end = reader.offset;
    if !policy_hash_matches(
        &bytes[policy_records_start..policy_records_end],
        policy_hash,
    ) {
        return Err(ParseError::BadPolicyHash);
    }

    validate_manifest(manifest)?;

    if !reader.finished() {
        return Err(ParseError::TrailingBytes);
    }

    Ok(())
}

fn validate_runtime_object_budget(
    endpoint_count: usize,
    store_object_count: usize,
    state_volume_count: usize,
    network_port_count: usize,
    io_port_count: usize,
    mmio_region_count: usize,
    framebuffer_count: usize,
    interrupt_line_count: usize,
    dma_region_count: usize,
    pci_device_count: usize,
    virtio_device_count: usize,
    namespace_count: usize,
    vfs_root_count: usize,
) -> Result<(), ParseError> {
    let mut count = FIXED_RUNTIME_OBJECTS;
    count = count
        .checked_add(endpoint_count)
        .and_then(|count| count.checked_add(store_object_count))
        .and_then(|count| count.checked_add(state_volume_count))
        .and_then(|count| count.checked_add(network_port_count))
        .and_then(|count| count.checked_add(io_port_count))
        .and_then(|count| count.checked_add(mmio_region_count))
        .and_then(|count| count.checked_add(framebuffer_count))
        .and_then(|count| count.checked_add(interrupt_line_count))
        .and_then(|count| count.checked_add(dma_region_count))
        .and_then(|count| count.checked_add(pci_device_count))
        .and_then(|count| count.checked_add(virtio_device_count))
        .and_then(|count| count.checked_add(namespace_count))
        .and_then(|count| count.checked_add(vfs_root_count))
        .ok_or(ParseError::TooManyRuntimeObjects)?;
    if count > MAX_RUNTIME_OBJECTS {
        return Err(ParseError::TooManyRuntimeObjects);
    }
    Ok(())
}

fn validate_hardware_authority(manifest: &Manifest<'_>) -> Result<(), ParseError> {
    let mut index = 0;
    while index < manifest.io_port_count {
        let range = manifest
            .io_port(index)
            .ok_or(ParseError::InvalidReference)?;
        validate_io_range(range.base, range.length)?;
        let mut previous = 0;
        while previous < index {
            let prior = manifest
                .io_port(previous)
                .ok_or(ParseError::InvalidReference)?;
            if ranges_overlap(range.base, range.length, prior.base, prior.length)? {
                return Err(ParseError::InvalidReference);
            }
            previous += 1;
        }
        index += 1;
    }

    index = 0;
    while index < manifest.mmio_region_count {
        let region = manifest
            .mmio_region(index)
            .ok_or(ParseError::InvalidReference)?;
        validate_device_range(region.base, region.length, false)?;
        let mut previous = 0;
        while previous < index {
            let prior = manifest
                .mmio_region(previous)
                .ok_or(ParseError::InvalidReference)?;
            if ranges_overlap(region.base, region.length, prior.base, prior.length)? {
                return Err(ParseError::InvalidReference);
            }
            previous += 1;
        }
        index += 1;
    }

    index = 0;
    while index < manifest.framebuffer_count {
        let framebuffer = manifest
            .framebuffer(index)
            .ok_or(ParseError::InvalidReference)?;
        let mut previous = 0;
        while previous < index {
            let prior = manifest
                .framebuffer(previous)
                .ok_or(ParseError::InvalidReference)?;
            if prior.id == framebuffer.id {
                return Err(ParseError::InvalidReference);
            }
            previous += 1;
        }
        index += 1;
    }

    index = 0;
    while index < manifest.interrupt_line_count {
        let line = manifest
            .interrupt_line(index)
            .ok_or(ParseError::InvalidReference)?;
        if line.line > MAX_LEGACY_IRQ_LINE {
            return Err(ParseError::InvalidReference);
        }
        let mut previous = 0;
        while previous < index {
            let prior = manifest
                .interrupt_line(previous)
                .ok_or(ParseError::InvalidReference)?;
            if prior.line == line.line {
                return Err(ParseError::InvalidReference);
            }
            previous += 1;
        }
        index += 1;
    }

    index = 0;
    while index < manifest.dma_region_count {
        let region = manifest
            .dma_region(index)
            .ok_or(ParseError::InvalidReference)?;
        if region.length == 0
            || region.length > MAX_DEVICE_MAPPING_LENGTH
            || region.length % PAGE_SIZE != 0
        {
            return Err(ParseError::InvalidReference);
        }
        if region.base != DMA_KERNEL_ALLOCATED_BASE {
            validate_device_range(region.base, region.length, true)?;
            let mut previous = 0;
            while previous < index {
                let prior = manifest
                    .dma_region(previous)
                    .ok_or(ParseError::InvalidReference)?;
                if prior.base != DMA_KERNEL_ALLOCATED_BASE
                    && ranges_overlap(region.base, region.length, prior.base, prior.length)?
                {
                    return Err(ParseError::InvalidReference);
                }
                previous += 1;
            }
        }
        index += 1;
    }

    Ok(())
}

fn validate_io_range(base: u64, length: u64) -> Result<(), ParseError> {
    if length == 0 {
        return Err(ParseError::InvalidReference);
    }
    let Some(last) = base.checked_add(length - 1) else {
        return Err(ParseError::InvalidReference);
    };
    if last > u16::MAX as u64 {
        return Err(ParseError::InvalidReference);
    }
    Ok(())
}

fn validate_device_range(
    base: u64,
    length: u64,
    page_aligned_base: bool,
) -> Result<(), ParseError> {
    if length == 0 || length > MAX_DEVICE_MAPPING_LENGTH {
        return Err(ParseError::InvalidReference);
    }
    base.checked_add(length - 1)
        .ok_or(ParseError::InvalidReference)?;
    if page_aligned_base && base % PAGE_SIZE != 0 {
        return Err(ParseError::InvalidReference);
    }
    Ok(())
}

fn ranges_overlap(
    base: u64,
    length: u64,
    other_base: u64,
    other_length: u64,
) -> Result<bool, ParseError> {
    if length == 0 || other_length == 0 {
        return Ok(false);
    }
    let end = base
        .checked_add(length)
        .ok_or(ParseError::InvalidReference)?;
    let other_end = other_base
        .checked_add(other_length)
        .ok_or(ParseError::InvalidReference)?;
    Ok(base < other_end && other_base < end)
}

fn validate_manifest(manifest: &Manifest<'_>) -> Result<(), ParseError> {
    validate_graph_store(manifest)?;
    if manifest.endpoint_count == 0 {
        return Err(ParseError::InvalidReference);
    }
    let serial_log = manifest.endpoint(0).ok_or(ParseError::InvalidReference)?;
    if serial_log.name != SERIAL_LOG_ENDPOINT_NAME {
        return Err(ParseError::InvalidReference);
    }
    let mut endpoint_index = 1;
    while endpoint_index < manifest.endpoint_count {
        let endpoint = manifest
            .endpoint(endpoint_index)
            .ok_or(ParseError::InvalidReference)?;
        if endpoint.name == SERIAL_LOG_ENDPOINT_NAME {
            return Err(ParseError::InvalidReference);
        }
        endpoint_index += 1;
    }
    validate_hardware_authority(manifest)?;

    let mut initial_count = 0;
    let mut index = 0;
    while index < manifest.process_count {
        let process = manifest
            .process(index)
            .ok_or(ParseError::InvalidReference)?;
        if process.initial {
            initial_count += 1;
        }
        if !has_store_object_module(manifest, process.module_string) {
            return Err(ParseError::InvalidReference);
        }
        validate_vfs_root_path(process.mount_root)?;
        validate_process_mounts(process)?;
        validate_process_refs(
            process.start_after,
            process.start_after_count,
            manifest.process_count,
        )?;
        validate_process_refs(
            process.requires_endpoint,
            process.requires_endpoint_count,
            manifest.endpoint_count,
        )?;
        validate_endpoint_rights(
            process.requires_endpoint_rights,
            process.requires_endpoint_count,
        )?;
        validate_process_refs(
            process.provides_endpoint,
            process.provides_endpoint_count,
            manifest.endpoint_count,
        )?;
        index += 1;
    }

    if initial_count != 1 {
        return Err(ParseError::InvalidReference);
    }

    index = 0;
    while index < manifest.state_volume_count {
        let state = manifest
            .state_volume(index)
            .ok_or(ParseError::InvalidReference)?;
        validate_state_volume_id(state.id)?;
        let mount_name = state_volume_mount_component(state.id)?;
        let mut previous = 0;
        while previous < index {
            let prior = manifest
                .state_volume(previous)
                .ok_or(ParseError::InvalidReference)?;
            if prior.id == state.id || state_volume_mount_component(prior.id)? == mount_name {
                return Err(ParseError::InvalidReference);
            }
            previous += 1;
        }
        index += 1;
    }

    index = 0;
    while index < manifest.grant_count {
        let grant = manifest.grant(index).ok_or(ParseError::InvalidReference)?;
        if grant.process_index >= manifest.process_count {
            return Err(ParseError::InvalidReference);
        }
        match grant.object_kind {
            OBJECT_ENDPOINT if grant.object_index < manifest.endpoint_count => {}
            OBJECT_STORE if grant.object_index < manifest.store_object_count => {}
            OBJECT_STATE => return Err(ParseError::UnsupportedStateVolumes),
            OBJECT_TIMER if grant.object_index == 0 => {}
            OBJECT_NETWORK_PORT if grant.object_index < manifest.network_port_count => {}
            OBJECT_IO_PORT_RANGE if grant.object_index < manifest.io_port_count => {}
            OBJECT_MMIO_REGION if grant.object_index < manifest.mmio_region_count => {}
            OBJECT_FRAMEBUFFER if grant.object_index < manifest.framebuffer_count => {}
            OBJECT_INTERRUPT_LINE if grant.object_index < manifest.interrupt_line_count => {}
            OBJECT_DMA_REGION if grant.object_index < manifest.dma_region_count => {}
            OBJECT_PCI_DEVICE if grant.object_index < manifest.pci_device_count => {}
            OBJECT_VIRTIO_DEVICE if grant.object_index < manifest.virtio_device_count => {}
            OBJECT_NAMESPACE if grant.object_index < manifest.namespace_count => {}
            OBJECT_VFS_ROOT if grant.object_index < manifest.vfs_root_count => {}
            OBJECT_ENDPOINT | OBJECT_STORE | OBJECT_TIMER | OBJECT_NETWORK_PORT => {
                return Err(ParseError::InvalidReference);
            }
            OBJECT_IO_PORT_RANGE
            | OBJECT_MMIO_REGION
            | OBJECT_FRAMEBUFFER
            | OBJECT_INTERRUPT_LINE
            | OBJECT_DMA_REGION
            | OBJECT_PCI_DEVICE
            | OBJECT_VIRTIO_DEVICE
            | OBJECT_NAMESPACE
            | OBJECT_VFS_ROOT => {
                return Err(ParseError::InvalidReference);
            }
            _ => return Err(ParseError::InvalidObjectKind),
        }
        if grant.object_kind == OBJECT_ENDPOINT {
            if grant.rights != RIGHT_SEND && grant.rights != RIGHT_RECEIVE {
                return Err(ParseError::InvalidRights);
            }
        } else if grant.rights == 0
            || grant.rights
                & !(RIGHT_READ
                    | RIGHT_WRITE
                    | RIGHT_SNAPSHOT
                    | RIGHT_RESTORE
                    | RIGHT_CONTROL
                    | RIGHT_BIND
                    | RIGHT_LISTEN
                    | RIGHT_MAP
                    | RIGHT_RESOLVE
                    | RIGHT_CREATE
                    | RIGHT_UNLINK
                    | RIGHT_RENAME
                    | RIGHT_MOUNT)
                != 0
        {
            return Err(ParseError::InvalidRights);
        }
        index += 1;
    }

    index = 0;
    while index < manifest.namespace_count {
        let namespace = manifest
            .namespace(index)
            .ok_or(ParseError::InvalidReference)?;
        let mut entry_index = 0;
        while entry_index < namespace.entry_count {
            let entry = namespace.entries[entry_index].ok_or(ParseError::InvalidReference)?;
            validate_object_ref(manifest, entry.object_kind, entry.object_index)?;
            if !namespace_entry_object_kind_allowed(entry.object_kind) || entry.rights == 0 {
                return Err(ParseError::InvalidReference);
            }
            if entry.rights
                & !(RIGHT_SEND
                    | RIGHT_RECEIVE
                    | RIGHT_READ
                    | RIGHT_WRITE
                    | RIGHT_SNAPSHOT
                    | RIGHT_RESTORE
                    | RIGHT_CONTROL
                    | RIGHT_BIND
                    | RIGHT_LISTEN
                    | RIGHT_MAP
                    | RIGHT_RESOLVE
                    | RIGHT_CREATE
                    | RIGHT_UNLINK
                    | RIGHT_RENAME
                    | RIGHT_MOUNT)
                != 0
            {
                return Err(ParseError::InvalidRights);
            }
            entry_index += 1;
        }
        index += 1;
    }

    index = 0;
    while index < manifest.vfs_root_count {
        let root = manifest
            .vfs_root(index)
            .ok_or(ParseError::InvalidReference)?;
        validate_vfs_root_path(root.root_path)?;
        index += 1;
    }

    validate_policy_facts(manifest)?;

    Ok(())
}

fn validate_graph_store(manifest: &Manifest<'_>) -> Result<(), ParseError> {
    if manifest.graph_node_count == 0 {
        return Err(ParseError::InvalidGraphRecord);
    }
    let mut generation_nodes = 0;
    let mut index = 0;
    while index < manifest.graph_node_count {
        let node = manifest
            .graph_node(index)
            .ok_or(ParseError::InvalidGraphRecord)?;
        if node.kind == 0 || node.id.is_empty() || node.label.len() > STRING_LEN {
            return Err(ParseError::InvalidGraphRecord);
        }
        if node.kind == GRAPH_NODE_GENERATION {
            generation_nodes += 1;
            if node.id != manifest.generation_id {
                return Err(ParseError::InvalidGraphRecord);
            }
        }
        let mut previous = 0;
        while previous < index {
            let prior = manifest
                .graph_node(previous)
                .ok_or(ParseError::InvalidGraphRecord)?;
            if prior.id == node.id {
                return Err(ParseError::InvalidGraphRecord);
            }
            previous += 1;
        }
        index += 1;
    }
    if generation_nodes != 1 {
        return Err(ParseError::InvalidGraphRecord);
    }

    index = 0;
    while index < manifest.process_count {
        let process = manifest
            .process(index)
            .ok_or(ParseError::InvalidReference)?;
        if !graph_has_node(manifest, GRAPH_NODE_SERVICE, graph_process_node_id(process)) {
            return Err(ParseError::InvalidGraphRecord);
        }
        index += 1;
    }

    index = 0;
    while index < manifest.endpoint_count {
        let endpoint = manifest
            .endpoint(index)
            .ok_or(ParseError::InvalidReference)?;
        if !graph_has_node(manifest, GRAPH_NODE_ENDPOINT, endpoint.name) {
            return Err(ParseError::InvalidGraphRecord);
        }
        index += 1;
    }

    index = 0;
    while index < manifest.store_object_count {
        let object = manifest
            .store_object(index)
            .ok_or(ParseError::InvalidReference)?;
        let kind = if object.id.starts_with("config:") {
            GRAPH_NODE_CONFIG
        } else {
            GRAPH_NODE_STORE_OBJECT
        };
        if !graph_has_node(manifest, kind, object.id) {
            return Err(ParseError::InvalidGraphRecord);
        }
        index += 1;
    }

    index = 0;
    while index < manifest.state_volume_count {
        let state = manifest
            .state_volume(index)
            .ok_or(ParseError::InvalidReference)?;
        if !valid_state_volume_policy(state) {
            return Err(ParseError::InvalidReference);
        }
        if !graph_has_node(manifest, GRAPH_NODE_STATE_VOLUME, state.id) {
            return Err(ParseError::InvalidGraphRecord);
        }
        index += 1;
    }

    validate_graph_device_nodes(manifest)?;

    index = 0;
    while index < manifest.graph_edge_count {
        let edge = manifest
            .graph_edge(index)
            .ok_or(ParseError::InvalidGraphRecord)?;
        if edge.kind == 0
            || edge.id.is_empty()
            || edge.from_index >= manifest.graph_node_count
            || edge.to_index >= manifest.graph_node_count
        {
            return Err(ParseError::InvalidGraphRecord);
        }
        if edge.kind == GRAPH_EDGE_CAPABILITY && edge.rights == 0 {
            return Err(ParseError::InvalidGraphRecord);
        }
        let mut previous = 0;
        while previous < index {
            let prior = manifest
                .graph_edge(previous)
                .ok_or(ParseError::InvalidGraphRecord)?;
            if prior.id == edge.id {
                return Err(ParseError::InvalidGraphRecord);
            }
            previous += 1;
        }
        index += 1;
    }

    index = 0;
    while index < manifest.grant_count {
        let grant = manifest.grant(index).ok_or(ParseError::InvalidReference)?;
        let process = manifest
            .process(grant.process_index)
            .ok_or(ParseError::InvalidReference)?;
        let Some(from_index) = graph_node_index(manifest, graph_process_node_id(process)) else {
            return Err(ParseError::InvalidGraphRecord);
        };
        let Some(to_index) = graph_node_index(manifest, graph_object_node_id(manifest, grant)?)
        else {
            return Err(ParseError::InvalidGraphRecord);
        };
        if !graph_has_capability_edge(manifest, from_index, to_index, grant.rights) {
            return Err(ParseError::InvalidGraphRecord);
        }
        index += 1;
    }

    Ok(())
}

fn valid_state_volume_policy(state: StateVolume<'_>) -> bool {
    !state.owner.is_empty()
        && !state.schema_version.is_empty()
        && !state.storage_class.is_empty()
        && !state.migration_policy.is_empty()
        && !state.retention_policy.is_empty()
        && !state.sharing_policy.is_empty()
        && matches!(
            state.storage_class,
            "vertexdisk-v1" | "hosted-local-directory"
        )
        && matches!(
            state.migration_policy,
            "preserve" | "migrate" | "fork" | "discard"
        )
        && matches!(
            state.retention_policy,
            "retain-while-referenced" | "retain-forever"
        )
        && matches!(state.sharing_policy, "owner-only" | "explicit")
}

fn validate_policy_facts(manifest: &Manifest<'_>) -> Result<(), ParseError> {
    if manifest.policy_version != POLICY_VERSION || manifest.policy_hash.is_empty() {
        return Err(ParseError::InvalidPolicy);
    }

    let mut index = 0;
    while index < manifest.policy_capability_count {
        let capability = manifest
            .policy_capability(index)
            .ok_or(ParseError::InvalidPolicy)?;
        if capability.id.is_empty()
            || capability.provider.is_empty()
            || capability.rights == 0
            || !known_policy_rights(capability.rights)
        {
            return Err(ParseError::InvalidPolicy);
        }
        validate_object_ref(manifest, capability.object_kind, capability.object_index)?;
        let mut prior = 0;
        while prior < index {
            let existing = manifest
                .policy_capability(prior)
                .ok_or(ParseError::InvalidPolicy)?;
            if existing.id == capability.id {
                return Err(ParseError::InvalidPolicy);
            }
            prior += 1;
        }
        index += 1;
    }

    index = 0;
    while index < manifest.policy_requirement_count {
        let requirement = manifest
            .policy_requirement(index)
            .ok_or(ParseError::InvalidPolicy)?;
        let capability = policy_capability_by_id(manifest, requirement.capability)
            .ok_or(ParseError::InvalidPolicy)?;
        if !manifest_has_service(manifest, requirement.service)
            || requirement.rights == 0
            || !known_policy_rights(requirement.rights)
            || requirement.rights & !capability.rights != 0
        {
            return Err(ParseError::InvalidPolicy);
        }
        index += 1;
    }

    index = 0;
    while index < manifest.policy_provide_count {
        let provide = manifest
            .policy_provide(index)
            .ok_or(ParseError::InvalidPolicy)?;
        let capability = policy_capability_by_id(manifest, provide.capability)
            .ok_or(ParseError::InvalidPolicy)?;
        if !manifest_has_service(manifest, provide.service)
            || capability.provider != provide.service
        {
            return Err(ParseError::InvalidPolicy);
        }
        index += 1;
    }

    index = 0;
    while index < manifest.grant_count {
        let grant = manifest.grant(index).ok_or(ParseError::InvalidReference)?;
        if !grant_authorized_by_policy(manifest, grant)? {
            let process = manifest
                .process(grant.process_index)
                .ok_or(ParseError::InvalidReference)?;
            let target = graph_object_node_id(manifest, grant).unwrap_or("<invalid>");
            log_policy_denial(
                graph_process_node_id(process),
                target,
                "grant-authorized",
                "no-policy-edge",
            );
            return Err(ParseError::InvalidPolicy);
        }
        index += 1;
    }

    Ok(())
}

fn grant_authorized_by_policy(manifest: &Manifest<'_>, grant: Grant) -> Result<bool, ParseError> {
    let process = manifest
        .process(grant.process_index)
        .ok_or(ParseError::InvalidReference)?;
    if builtin_grant_authorized(manifest, process, grant)? {
        return Ok(true);
    }

    let service = graph_process_node_id(process);
    let mut index = 0;
    while index < manifest.policy_capability_count {
        let capability = manifest
            .policy_capability(index)
            .ok_or(ParseError::InvalidPolicy)?;
        if capability.object_kind == grant.object_kind
            && capability.object_index == grant.object_index
            && grant.rights & !capability.rights == 0
        {
            if grant.object_kind == OBJECT_ENDPOINT && grant.rights == RIGHT_RECEIVE {
                if policy_provides(manifest, service, capability.id)
                    && capability.provider == service
                {
                    return Ok(true);
                }
            } else if policy_requirement_allows(manifest, service, capability.id, grant.rights) {
                return Ok(true);
            }
        }
        index += 1;
    }
    Ok(false)
}

fn builtin_grant_authorized(
    manifest: &Manifest<'_>,
    process: Process<'_>,
    grant: Grant,
) -> Result<bool, ParseError> {
    if grant.object_kind != OBJECT_ENDPOINT {
        return Ok(false);
    }
    let endpoint = manifest
        .endpoint(grant.object_index)
        .ok_or(ParseError::InvalidReference)?;
    if endpoint.name == SERIAL_LOG_ENDPOINT_NAME && grant.rights == RIGHT_SEND {
        return Ok(true);
    }
    if endpoint.name == "readiness" {
        if process.initial && grant.rights == RIGHT_RECEIVE {
            return Ok(true);
        }
        if !process.initial && !process.health_kind.is_empty() && grant.rights == RIGHT_SEND {
            return Ok(true);
        }
    }
    if process.initial && grant.rights == RIGHT_SEND {
        return Ok(true);
    }
    Ok(false)
}

fn policy_requirement_allows(
    manifest: &Manifest<'_>,
    service: &str,
    capability: &str,
    rights: u16,
) -> bool {
    let mut index = 0;
    while index < manifest.policy_requirement_count {
        if let Some(requirement) = manifest.policy_requirement(index)
            && requirement.service == service
            && requirement.capability == capability
            && rights & !requirement.rights == 0
        {
            return true;
        }
        index += 1;
    }
    false
}

fn policy_provides(manifest: &Manifest<'_>, service: &str, capability: &str) -> bool {
    let mut index = 0;
    while index < manifest.policy_provide_count {
        if let Some(provide) = manifest.policy_provide(index)
            && provide.service == service
            && provide.capability == capability
        {
            return true;
        }
        index += 1;
    }
    false
}

fn policy_capability_by_id<'a>(
    manifest: &'a Manifest<'a>,
    capability: &str,
) -> Option<PolicyCapability<'a>> {
    let mut index = 0;
    while index < manifest.policy_capability_count {
        if let Some(candidate) = manifest.policy_capability(index)
            && candidate.id == capability
        {
            return Some(candidate);
        }
        index += 1;
    }
    None
}

fn manifest_has_service(manifest: &Manifest<'_>, service: &str) -> bool {
    let mut index = 0;
    while index < manifest.process_count {
        if let Some(process) = manifest.process(index)
            && graph_process_node_id(process) == service
        {
            return true;
        }
        index += 1;
    }
    false
}

fn known_policy_rights(rights: u16) -> bool {
    rights
        & !(RIGHT_SEND
            | RIGHT_RECEIVE
            | RIGHT_READ
            | RIGHT_WRITE
            | RIGHT_SNAPSHOT
            | RIGHT_RESTORE
            | RIGHT_CONTROL
            | RIGHT_BIND
            | RIGHT_LISTEN
            | RIGHT_MAP
            | RIGHT_RESOLVE
            | RIGHT_CREATE
            | RIGHT_UNLINK
            | RIGHT_RENAME
            | RIGHT_MOUNT)
        == 0
}

fn log_policy_denial(source: &str, target: &str, rule: &str, reason: &str) {
    serial::write_str("native policy validation rejected: source=");
    serial::write_str(source);
    serial::write_str(" target=");
    serial::write_str(target);
    serial::write_str(" rule=");
    serial::write_str(rule);
    serial::write_str(" reason=");
    serial::write_str(reason);
    serial::write_str("\n");
}

fn validate_graph_device_nodes(manifest: &Manifest<'_>) -> Result<(), ParseError> {
    let mut index = 0;
    while index < manifest.network_port_count {
        let object = manifest
            .network_port(index)
            .ok_or(ParseError::InvalidReference)?;
        if !graph_has_device_node(manifest, OBJECT_NETWORK_PORT, object.id) {
            return Err(ParseError::InvalidGraphRecord);
        }
        index += 1;
    }
    index = 0;
    while index < manifest.io_port_count {
        let object = manifest
            .io_port(index)
            .ok_or(ParseError::InvalidReference)?;
        if !graph_has_device_node(manifest, OBJECT_IO_PORT_RANGE, object.id) {
            return Err(ParseError::InvalidGraphRecord);
        }
        index += 1;
    }
    index = 0;
    while index < manifest.mmio_region_count {
        let object = manifest
            .mmio_region(index)
            .ok_or(ParseError::InvalidReference)?;
        if !graph_has_device_node(manifest, OBJECT_MMIO_REGION, object.id) {
            return Err(ParseError::InvalidGraphRecord);
        }
        index += 1;
    }
    index = 0;
    while index < manifest.framebuffer_count {
        let object = manifest
            .framebuffer(index)
            .ok_or(ParseError::InvalidReference)?;
        if !graph_has_device_node(manifest, OBJECT_FRAMEBUFFER, object.id) {
            return Err(ParseError::InvalidGraphRecord);
        }
        index += 1;
    }
    index = 0;
    while index < manifest.interrupt_line_count {
        let object = manifest
            .interrupt_line(index)
            .ok_or(ParseError::InvalidReference)?;
        if !graph_has_device_node(manifest, OBJECT_INTERRUPT_LINE, object.id) {
            return Err(ParseError::InvalidGraphRecord);
        }
        index += 1;
    }
    index = 0;
    while index < manifest.dma_region_count {
        let object = manifest
            .dma_region(index)
            .ok_or(ParseError::InvalidReference)?;
        if !graph_has_device_node(manifest, OBJECT_DMA_REGION, object.id) {
            return Err(ParseError::InvalidGraphRecord);
        }
        index += 1;
    }
    index = 0;
    while index < manifest.pci_device_count {
        let object = manifest
            .pci_device(index)
            .ok_or(ParseError::InvalidReference)?;
        if !graph_has_any_device_node(manifest, object.id) {
            return Err(ParseError::InvalidGraphRecord);
        }
        index += 1;
    }
    index = 0;
    while index < manifest.virtio_device_count {
        let object = manifest
            .virtio_device(index)
            .ok_or(ParseError::InvalidReference)?;
        if !graph_has_any_device_node(manifest, object.id) {
            return Err(ParseError::InvalidGraphRecord);
        }
        index += 1;
    }
    Ok(())
}

fn graph_process_node_id(process: Process<'_>) -> &str {
    if process.service_id.is_empty() {
        process.name
    } else {
        process.service_id
    }
}

fn graph_object_node_id<'a>(
    manifest: &'a Manifest<'a>,
    grant: Grant,
) -> Result<&'a str, ParseError> {
    match grant.object_kind {
        OBJECT_ENDPOINT => manifest
            .endpoint(grant.object_index)
            .map(|object| object.name)
            .ok_or(ParseError::InvalidReference),
        OBJECT_STORE => manifest
            .store_object(grant.object_index)
            .map(|object| object.id)
            .ok_or(ParseError::InvalidReference),
        OBJECT_TIMER => Ok("monotonic-timer"),
        OBJECT_NETWORK_PORT => manifest
            .network_port(grant.object_index)
            .map(|object| object.id)
            .ok_or(ParseError::InvalidReference),
        OBJECT_IO_PORT_RANGE => manifest
            .io_port(grant.object_index)
            .map(|object| object.id)
            .ok_or(ParseError::InvalidReference),
        OBJECT_MMIO_REGION => manifest
            .mmio_region(grant.object_index)
            .map(|object| object.id)
            .ok_or(ParseError::InvalidReference),
        OBJECT_FRAMEBUFFER => manifest
            .framebuffer(grant.object_index)
            .map(|object| object.id)
            .ok_or(ParseError::InvalidReference),
        OBJECT_INTERRUPT_LINE => manifest
            .interrupt_line(grant.object_index)
            .map(|object| object.id)
            .ok_or(ParseError::InvalidReference),
        OBJECT_DMA_REGION => manifest
            .dma_region(grant.object_index)
            .map(|object| object.id)
            .ok_or(ParseError::InvalidReference),
        OBJECT_PCI_DEVICE => manifest
            .pci_device(grant.object_index)
            .map(|object| object.id)
            .ok_or(ParseError::InvalidReference),
        OBJECT_VIRTIO_DEVICE => manifest
            .virtio_device(grant.object_index)
            .map(|object| object.id)
            .ok_or(ParseError::InvalidReference),
        OBJECT_NAMESPACE => manifest
            .namespace(grant.object_index)
            .map(|object| object.id)
            .ok_or(ParseError::InvalidReference),
        OBJECT_VFS_ROOT => manifest
            .vfs_root(grant.object_index)
            .map(|object| object.id)
            .ok_or(ParseError::InvalidReference),
        _ => Err(ParseError::InvalidObjectKind),
    }
}

fn graph_has_node(manifest: &Manifest<'_>, kind: u16, id: &str) -> bool {
    let mut index = 0;
    while index < manifest.graph_node_count {
        if let Some(node) = manifest.graph_node(index)
            && node.kind == kind
            && node.id == id
        {
            return true;
        }
        index += 1;
    }
    false
}

fn graph_has_device_node(manifest: &Manifest<'_>, object_kind: u16, id: &str) -> bool {
    let mut index = 0;
    while index < manifest.graph_node_count {
        if let Some(node) = manifest.graph_node(index)
            && node.kind == GRAPH_NODE_DEVICE
            && node.object_kind == object_kind
            && node.id == id
        {
            return true;
        }
        index += 1;
    }
    false
}

fn graph_has_any_device_node(manifest: &Manifest<'_>, id: &str) -> bool {
    let mut index = 0;
    while index < manifest.graph_node_count {
        if let Some(node) = manifest.graph_node(index)
            && node.kind == GRAPH_NODE_DEVICE
            && node.id == id
        {
            return true;
        }
        index += 1;
    }
    false
}

fn graph_node_index(manifest: &Manifest<'_>, id: &str) -> Option<usize> {
    let mut index = 0;
    while index < manifest.graph_node_count {
        if let Some(node) = manifest.graph_node(index)
            && node.id == id
        {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn graph_has_capability_edge(
    manifest: &Manifest<'_>,
    from_index: usize,
    to_index: usize,
    rights: u16,
) -> bool {
    let mut index = 0;
    while index < manifest.graph_edge_count {
        if let Some(edge) = manifest.graph_edge(index)
            && edge.kind == GRAPH_EDGE_CAPABILITY
            && edge.from_index == from_index
            && edge.to_index == to_index
            && edge.rights == rights
        {
            return true;
        }
        index += 1;
    }
    false
}

fn validate_process_mounts(process: Process<'_>) -> Result<(), ParseError> {
    if process.mount_count > MAX_PROCESS_MOUNTS {
        return Err(ParseError::InvalidReference);
    }
    let mut index = 0;
    while index < process.mount_count {
        let mount = process.mounts[index].ok_or(ParseError::InvalidReference)?;
        validate_vfs_root_path(mount.path)?;
        validate_vfs_root_path(mount.source)?;
        if mount.path == "/" {
            return Err(ParseError::InvalidReference);
        }
        if mount.flags & !known_process_mount_flags() != 0
            || mount.flags & PROCESS_MOUNT_FLAG_BIND == 0
        {
            return Err(ParseError::InvalidReference);
        }
        let mut prior = 0;
        while prior < index {
            let existing = process.mounts[prior].ok_or(ParseError::InvalidReference)?;
            if existing.path == mount.path {
                return Err(ParseError::InvalidReference);
            }
            prior += 1;
        }
        index += 1;
    }
    Ok(())
}

fn validate_vfs_root_path(path: &str) -> Result<(), ParseError> {
    let bytes = path.as_bytes();
    if bytes.is_empty() || bytes[0] != b'/' || (bytes.len() > 1 && bytes[bytes.len() - 1] == b'/') {
        return Err(ParseError::InvalidReference);
    }
    let mut index = 1;
    while index < bytes.len() {
        if bytes[index] == 0 || (bytes[index] == b'/' && bytes[index - 1] == b'/') {
            return Err(ParseError::InvalidReference);
        }
        index += 1;
    }
    Ok(())
}

fn validate_state_volume_id(id: &str) -> Result<(), ParseError> {
    state_volume_mount_component(id).map(|_| ())
}

fn state_volume_mount_component(id: &str) -> Result<&str, ParseError> {
    let Some(component) = id.strip_prefix("state:") else {
        return Err(ParseError::InvalidReference);
    };
    let bytes = component.as_bytes();
    if bytes.is_empty() || bytes.len() > STRING_LEN {
        return Err(ParseError::InvalidReference);
    }
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'/' || bytes[index] == 0 {
            return Err(ParseError::InvalidReference);
        }
        index += 1;
    }
    Ok(component)
}

fn namespace_entry_object_kind_allowed(object_kind: u16) -> bool {
    matches!(
        object_kind,
        OBJECT_ENDPOINT | OBJECT_STORE | OBJECT_TIMER | OBJECT_NETWORK_PORT
    )
}

fn validate_object_ref(
    manifest: &Manifest<'_>,
    object_kind: u16,
    object_index: usize,
) -> Result<(), ParseError> {
    match object_kind {
        OBJECT_ENDPOINT if object_index < manifest.endpoint_count => Ok(()),
        OBJECT_STORE if object_index < manifest.store_object_count => Ok(()),
        OBJECT_STATE => Err(ParseError::UnsupportedStateVolumes),
        OBJECT_TIMER if object_index == 0 => Ok(()),
        OBJECT_NETWORK_PORT if object_index < manifest.network_port_count => Ok(()),
        OBJECT_IO_PORT_RANGE if object_index < manifest.io_port_count => Ok(()),
        OBJECT_MMIO_REGION if object_index < manifest.mmio_region_count => Ok(()),
        OBJECT_FRAMEBUFFER if object_index < manifest.framebuffer_count => Ok(()),
        OBJECT_INTERRUPT_LINE if object_index < manifest.interrupt_line_count => Ok(()),
        OBJECT_DMA_REGION if object_index < manifest.dma_region_count => Ok(()),
        OBJECT_PCI_DEVICE if object_index < manifest.pci_device_count => Ok(()),
        OBJECT_VIRTIO_DEVICE if object_index < manifest.virtio_device_count => Ok(()),
        OBJECT_NAMESPACE if object_index < manifest.namespace_count => Ok(()),
        OBJECT_VFS_ROOT if object_index < manifest.vfs_root_count => Ok(()),
        OBJECT_ENDPOINT | OBJECT_STORE | OBJECT_TIMER | OBJECT_NETWORK_PORT => {
            Err(ParseError::InvalidReference)
        }
        OBJECT_IO_PORT_RANGE
        | OBJECT_MMIO_REGION
        | OBJECT_FRAMEBUFFER
        | OBJECT_INTERRUPT_LINE
        | OBJECT_DMA_REGION
        | OBJECT_PCI_DEVICE
        | OBJECT_VIRTIO_DEVICE
        | OBJECT_NAMESPACE
        | OBJECT_VFS_ROOT => Err(ParseError::InvalidReference),
        _ => Err(ParseError::InvalidObjectKind),
    }
}

fn validate_endpoint_rights(
    rights: [u16; MAX_PROCESS_REFS],
    count: usize,
) -> Result<(), ParseError> {
    let mut index = 0;
    while index < count {
        if rights[index] != RIGHT_SEND {
            return Err(ParseError::InvalidRights);
        }
        index += 1;
    }
    Ok(())
}

fn validate_process_refs(
    refs: [u16; MAX_PROCESS_REFS],
    count: usize,
    limit: usize,
) -> Result<(), ParseError> {
    if count > MAX_PROCESS_REFS {
        return Err(ParseError::InvalidReference);
    }

    let mut index = 0;
    while index < count {
        if refs[index] as usize >= limit {
            return Err(ParseError::InvalidReference);
        }
        index += 1;
    }

    Ok(())
}

fn has_store_object_module(manifest: &Manifest<'_>, module_string: &str) -> bool {
    let mut index = 0;
    while index < manifest.store_object_count {
        if let Some(object) = manifest.store_object(index)
            && object.module_string == module_string
        {
            return true;
        }
        index += 1;
    }
    false
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_count(&mut self, max: usize, error: ParseError) -> Result<usize, ParseError> {
        let count = self.read_u16()? as usize;
        if count > max {
            return Err(error);
        }
        Ok(count)
    }

    fn read_u16(&mut self) -> Result<u16, ParseError> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, ParseError> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, ParseError> {
        let bytes = self.read_exact(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_fixed_str(&mut self) -> Result<&'a str, ParseError> {
        let value = self.read_fixed_str_allow_empty()?;
        if value.is_empty() {
            return Err(ParseError::InvalidString);
        }
        Ok(value)
    }

    fn read_fixed_str_allow_empty(&mut self) -> Result<&'a str, ParseError> {
        let bytes = self.read_exact(STRING_LEN)?;
        let mut end = 0;
        while end < bytes.len() && bytes[end] != 0 {
            end += 1;
        }
        str::from_utf8(&bytes[..end]).map_err(|_| ParseError::InvalidString)
    }

    fn read_ref_list(&mut self) -> Result<([u16; MAX_PROCESS_REFS], usize), ParseError> {
        let count = self.read_u16()? as usize;
        if count > MAX_PROCESS_REFS {
            return Err(ParseError::InvalidReference);
        }

        let mut refs = [u16::MAX; MAX_PROCESS_REFS];
        let mut index = 0;
        while index < MAX_PROCESS_REFS {
            refs[index] = self.read_u16()?;
            index += 1;
        }
        Ok((refs, count))
    }

    fn read_endpoint_requirement_list(
        &mut self,
    ) -> Result<([u16; MAX_PROCESS_REFS], [u16; MAX_PROCESS_REFS], usize), ParseError> {
        let count = self.read_u16()? as usize;
        if count > MAX_PROCESS_REFS {
            return Err(ParseError::InvalidReference);
        }

        let mut refs = [u16::MAX; MAX_PROCESS_REFS];
        let mut rights = [0; MAX_PROCESS_REFS];
        let mut index = 0;
        while index < MAX_PROCESS_REFS {
            refs[index] = self.read_u16()?;
            rights[index] = self.read_u16()?;
            index += 1;
        }
        Ok((refs, rights, count))
    }

    fn read_process_mount_list(
        &mut self,
    ) -> Result<([Option<ProcessMount<'a>>; MAX_PROCESS_MOUNTS], usize), ParseError> {
        let count = self.read_u16()? as usize;
        if count > MAX_PROCESS_MOUNTS {
            return Err(ParseError::InvalidReference);
        }

        let mut mounts = [None; MAX_PROCESS_MOUNTS];
        let mut index = 0;
        while index < MAX_PROCESS_MOUNTS {
            let path = self.read_fixed_str_allow_empty()?;
            let source = self.read_fixed_str_allow_empty()?;
            let flags = self.read_u16()?;
            let reserved = self.read_u16()?;
            if reserved != 0 {
                return Err(ParseError::InvalidReference);
            }
            if index < count {
                if path.is_empty()
                    || source.is_empty()
                    || flags & !known_process_mount_flags() != 0
                    || flags & PROCESS_MOUNT_FLAG_BIND == 0
                {
                    return Err(ParseError::InvalidReference);
                }
                mounts[index] = Some(ProcessMount {
                    path,
                    source,
                    flags,
                });
            } else if !path.is_empty() || !source.is_empty() || flags != 0 {
                return Err(ParseError::InvalidReference);
            }
            index += 1;
        }
        Ok((mounts, count))
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], ParseError> {
        let end = self.offset.checked_add(len).ok_or(ParseError::Truncated)?;
        if end > self.bytes.len() {
            return Err(ParseError::Truncated);
        }
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn known_process_mount_flags() -> u16 {
    PROCESS_MOUNT_FLAG_BIND | PROCESS_MOUNT_FLAG_READ_ONLY
}

struct Record {
    kind: u16,
    offset: usize,
    length: usize,
}

impl Record {
    fn read(bytes: &[u8], offset: usize) -> Result<Self, ParseError> {
        let end = offset
            .checked_add(V1_RECORD_SIZE)
            .ok_or(ParseError::BadRecordTable)?;
        if end > bytes.len() {
            return Err(ParseError::BadRecordTable);
        }

        let kind = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        let _id = u16::from_le_bytes([bytes[offset + 2], bytes[offset + 3]]);
        let raw_offset = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);
        let raw_length = u32::from_le_bytes([
            bytes[offset + 8],
            bytes[offset + 9],
            bytes[offset + 10],
            bytes[offset + 11],
        ]);

        Ok(Self {
            kind,
            offset: raw_offset as usize,
            length: raw_length as usize,
        })
    }
}

fn v1_checksum(bytes: &[u8]) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    let mut index = 0;
    while index < bytes.len() {
        let value = if index >= V1_CHECKSUM_OFFSET && index < V1_CHECKSUM_OFFSET + 4 {
            0
        } else {
            bytes[index]
        };
        hash ^= value as u32;
        hash = hash.wrapping_mul(16_777_619);
        index += 1;
    }
    hash
}

fn checksum32(bytes: &[u8]) -> u32 {
    let mut checksum = 0u32;
    let mut index = 0;
    while index < bytes.len() {
        checksum = checksum.wrapping_add((bytes[index] as u32).wrapping_mul(index as u32 + 1));
        index += 1;
    }
    checksum
}

fn policy_hash_matches(records: &[u8], expected: &str) -> bool {
    if expected.len() != 64
        || !expected
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return false;
    }
    let digest = blake3::hash(records);
    let mut actual = [0u8; 64];
    store_hash_hex(digest.as_bytes(), &mut actual);
    actual == expected.as_bytes()
}

fn store_hash_hex(bytes: &[u8; 32], out: &mut [u8; 64]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut index = 0;
    while index < bytes.len() {
        out[index * 2] = HEX[(bytes[index] >> 4) as usize];
        out[index * 2 + 1] = HEX[(bytes[index] & 0x0f) as usize];
        index += 1;
    }
}
