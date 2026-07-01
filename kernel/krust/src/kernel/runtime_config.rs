use crate::vfs::{MAX_NAMESPACE_ENTRIES, state_volume_mount_component, valid_vfs_root_path};
use vertex_abi::graph as graph_abi;

use super::{InitError, MAX_OBJECTS, MAX_PROCESSES};

pub(crate) const MAX_BOOT_GRANTS: usize = 128;
pub(crate) const MAX_BOOT_NAMESPACES: usize = 4;
pub(crate) const MAX_BOOT_VFS_ROOTS: usize = 8;
pub(crate) const MAX_BOOT_STATE_VOLUMES: usize = 4;
pub(crate) const MAX_BOOT_PROCESS_MOUNTS: usize = 4;
pub(crate) const MAX_BOOT_GRAPH_NODES: usize = 128;
pub(crate) const MAX_BOOT_GRAPH_EDGES: usize = 224;
pub(crate) const MAX_BOOT_POLICY_CAPABILITIES: usize = 128;
pub(crate) const MAX_BOOT_POLICY_REQUIREMENTS: usize = 160;
pub(crate) const MAX_BOOT_POLICY_PROVIDES: usize = 64;
pub(crate) const MAX_BOOT_POLICY_MOUNTS: usize = 96;
pub(crate) const MAX_BOOT_POLICY_STATE_PATHS: usize = 96;
pub(crate) const MAX_BOOT_POLICY_BOOTSTRAPS: usize = 96;
pub const BOOT_POLICY_VERSION: u16 = vertex_abi::krustboot::POLICY_VERSION;

pub(crate) const BOOT_PROCESS_MOUNT_BIND: u16 = 1;
pub(crate) const BOOT_PROCESS_MOUNT_READ_ONLY: u16 = 1 << 1;

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
pub const BOOT_OBJECT_FRAMEBUFFER: u16 = 14;
pub const BOOT_OBJECT_SECRET: u16 = 15;

#[derive(Clone, Copy)]
pub struct BootProcessConfig {
    pub name: &'static str,
    pub graph_node: &'static str,
    pub image_base: u64,
    pub image_length: u64,
    pub initial: bool,
    pub restart_policy: u16,
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
    pub owner: &'static str,
    pub schema_version: &'static str,
    pub storage_class: &'static str,
    pub migration_policy: &'static str,
    pub retention_policy: &'static str,
    pub sharing_policy: &'static str,
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
pub struct BootFramebufferConfig {
    pub id: &'static str,
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
pub struct BootPolicyCapabilityConfig {
    pub id: &'static str,
    pub provider: &'static str,
    pub object_kind: u16,
    pub object_index: usize,
    pub rights: u64,
}

#[derive(Clone, Copy)]
pub struct BootPolicyRequirementConfig {
    pub service: &'static str,
    pub capability: &'static str,
    pub rights: u64,
}

#[derive(Clone, Copy)]
pub struct BootPolicyProvideConfig {
    pub service: &'static str,
    pub capability: &'static str,
}

#[derive(Clone, Copy)]
pub struct BootPolicyMountConfig {
    pub service: &'static str,
    pub mount_root: &'static str,
    pub path: &'static str,
    pub source: &'static str,
    pub flags: u16,
}

#[derive(Clone, Copy)]
pub struct BootPolicyStatePathConfig {
    pub service: &'static str,
    pub state: &'static str,
    pub root: &'static str,
    pub rights: u64,
}

#[derive(Clone, Copy)]
pub struct BootPolicyBootstrapConfig {
    pub service: &'static str,
    pub authority: &'static str,
    pub rule: &'static str,
    pub rights: u64,
}

#[derive(Clone, Copy)]
pub struct BootRuntimeConfig {
    pub(crate) generation_id: &'static str,
    pub(crate) manifest_hash: [u8; 64],
    pub(crate) graph_store_hash: [u8; 64],
    pub(crate) graph_store_checksum: u32,
    pub(crate) graph_store_source: &'static str,
    pub(crate) policy_version: u16,
    pub(crate) policy_hash: [u8; 64],
    pub(crate) processes: [Option<BootProcessConfig>; MAX_PROCESSES],
    pub(crate) process_count: usize,
    pub(crate) endpoints: [Option<BootEndpointConfig>; MAX_OBJECTS],
    pub(crate) endpoint_count: usize,
    pub(crate) manifest_module: Option<BootModuleConfig>,
    pub(crate) store_objects: [Option<BootStoreObjectConfig>; MAX_OBJECTS],
    pub(crate) store_object_count: usize,
    pub(crate) state_volumes: [Option<BootStateVolumeConfig>; MAX_BOOT_STATE_VOLUMES],
    pub(crate) state_volume_count: usize,
    pub(crate) network_ports: [Option<BootNetworkPortConfig>; MAX_OBJECTS],
    pub(crate) network_port_count: usize,
    pub(crate) io_ports: [Option<BootIoPortRangeConfig>; MAX_OBJECTS],
    pub(crate) io_port_count: usize,
    pub(crate) mmio_regions: [Option<BootMmioRegionConfig>; MAX_OBJECTS],
    pub(crate) mmio_region_count: usize,
    pub(crate) framebuffers: [Option<BootFramebufferConfig>; MAX_OBJECTS],
    pub(crate) framebuffer_count: usize,
    pub(crate) interrupt_lines: [Option<BootInterruptLineConfig>; MAX_OBJECTS],
    pub(crate) interrupt_line_count: usize,
    pub(crate) dma_regions: [Option<BootDmaRegionConfig>; MAX_OBJECTS],
    pub(crate) dma_region_count: usize,
    pub(crate) pci_devices: [Option<BootPciDeviceConfig>; MAX_OBJECTS],
    pub(crate) pci_device_count: usize,
    pub(crate) virtio_devices: [Option<BootVirtioDeviceConfig>; MAX_OBJECTS],
    pub(crate) virtio_device_count: usize,
    pub(crate) namespaces: [Option<BootNamespaceConfig>; MAX_BOOT_NAMESPACES],
    pub(crate) namespace_count: usize,
    pub(crate) vfs_roots: [Option<BootVfsRootConfig>; MAX_BOOT_VFS_ROOTS],
    pub(crate) vfs_root_count: usize,
    pub(crate) graph_nodes: [Option<BootGraphNodeConfig>; MAX_BOOT_GRAPH_NODES],
    pub(crate) graph_node_count: usize,
    pub(crate) graph_edges: [Option<BootGraphEdgeConfig>; MAX_BOOT_GRAPH_EDGES],
    pub(crate) graph_edge_count: usize,
    pub(crate) grants: [Option<BootGrantConfig>; MAX_BOOT_GRANTS],
    pub(crate) grant_count: usize,
    pub(crate) policy_capabilities:
        [Option<BootPolicyCapabilityConfig>; MAX_BOOT_POLICY_CAPABILITIES],
    pub(crate) policy_capability_count: usize,
    pub(crate) policy_requirements:
        [Option<BootPolicyRequirementConfig>; MAX_BOOT_POLICY_REQUIREMENTS],
    pub(crate) policy_requirement_count: usize,
    pub(crate) policy_provides: [Option<BootPolicyProvideConfig>; MAX_BOOT_POLICY_PROVIDES],
    pub(crate) policy_provide_count: usize,
    pub(crate) policy_mounts: [Option<BootPolicyMountConfig>; MAX_BOOT_POLICY_MOUNTS],
    pub(crate) policy_mount_count: usize,
    pub(crate) policy_state_paths: [Option<BootPolicyStatePathConfig>; MAX_BOOT_POLICY_STATE_PATHS],
    pub(crate) policy_state_path_count: usize,
    pub(crate) policy_bootstraps: [Option<BootPolicyBootstrapConfig>; MAX_BOOT_POLICY_BOOTSTRAPS],
    pub(crate) policy_bootstrap_count: usize,
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

pub(crate) fn known_boot_process_mount_flags() -> u16 {
    BOOT_PROCESS_MOUNT_BIND | BOOT_PROCESS_MOUNT_READ_ONLY
}

impl BootRuntimeConfig {
    pub const fn new() -> Self {
        Self {
            generation_id: "",
            manifest_hash: [0; 64],
            graph_store_hash: [0; 64],
            graph_store_checksum: 0,
            graph_store_source: "",
            policy_version: 0,
            policy_hash: [0; 64],
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
            framebuffers: [None; MAX_OBJECTS],
            framebuffer_count: 0,
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
            policy_capabilities: [None; MAX_BOOT_POLICY_CAPABILITIES],
            policy_capability_count: 0,
            policy_requirements: [None; MAX_BOOT_POLICY_REQUIREMENTS],
            policy_requirement_count: 0,
            policy_provides: [None; MAX_BOOT_POLICY_PROVIDES],
            policy_provide_count: 0,
            policy_mounts: [None; MAX_BOOT_POLICY_MOUNTS],
            policy_mount_count: 0,
            policy_state_paths: [None; MAX_BOOT_POLICY_STATE_PATHS],
            policy_state_path_count: 0,
            policy_bootstraps: [None; MAX_BOOT_POLICY_BOOTSTRAPS],
            policy_bootstrap_count: 0,
        }
    }

    pub fn reset(&mut self) {
        self.generation_id = "";
        self.manifest_hash = [0; 64];
        self.graph_store_hash = [0; 64];
        self.graph_store_checksum = 0;
        self.graph_store_source = "";
        self.policy_version = 0;
        self.policy_hash = [0; 64];
        self.process_count = 0;
        self.endpoint_count = 0;
        self.manifest_module = None;
        self.store_object_count = 0;
        self.state_volume_count = 0;
        self.network_port_count = 0;
        self.io_port_count = 0;
        self.mmio_region_count = 0;
        self.framebuffer_count = 0;
        self.interrupt_line_count = 0;
        self.dma_region_count = 0;
        self.pci_device_count = 0;
        self.virtio_device_count = 0;
        self.namespace_count = 0;
        self.vfs_root_count = 0;
        self.graph_node_count = 0;
        self.graph_edge_count = 0;
        self.grant_count = 0;
        self.policy_capability_count = 0;
        self.policy_requirement_count = 0;
        self.policy_provide_count = 0;
        self.policy_mount_count = 0;
        self.policy_state_path_count = 0;
        self.policy_bootstrap_count = 0;
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

    pub fn set_policy_version(&mut self, version: u16) {
        self.policy_version = version;
    }

    pub fn set_policy_hash(&mut self, hash: [u8; 64]) {
        self.policy_hash = hash;
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

    pub fn add_framebuffer(&mut self, framebuffer: BootFramebufferConfig) -> Result<(), InitError> {
        if self.framebuffer_count == self.framebuffers.len() {
            return Err(InitError::ObjectTableFull);
        }
        self.framebuffers[self.framebuffer_count] = Some(framebuffer);
        self.framebuffer_count += 1;
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
            || (edge.kind == graph_abi::EDGE_CAPABILITY && edge.rights == 0)
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
            BOOT_OBJECT_FRAMEBUFFER if grant.object_index < self.framebuffer_count => {}
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
            | BOOT_OBJECT_FRAMEBUFFER
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

    pub fn add_policy_capability(
        &mut self,
        capability: BootPolicyCapabilityConfig,
    ) -> Result<(), InitError> {
        if self.policy_capability_count == self.policy_capabilities.len()
            || capability.id.is_empty()
            || capability.provider.is_empty()
            || capability.rights == 0
            || !self.policy_object_ref_valid(capability.object_kind, capability.object_index)
        {
            return Err(InitError::InvalidBootManifest);
        }
        let mut index = 0;
        while index < self.policy_capability_count {
            if let Some(existing) = self.policy_capabilities[index]
                && existing.id == capability.id
            {
                return Err(InitError::InvalidBootManifest);
            }
            index += 1;
        }
        self.policy_capabilities[self.policy_capability_count] = Some(capability);
        self.policy_capability_count += 1;
        Ok(())
    }

    pub fn add_policy_requirement(
        &mut self,
        requirement: BootPolicyRequirementConfig,
    ) -> Result<(), InitError> {
        if self.policy_requirement_count == self.policy_requirements.len()
            || requirement.service.is_empty()
            || requirement.capability.is_empty()
            || requirement.rights == 0
        {
            return Err(InitError::InvalidBootManifest);
        }
        self.policy_requirements[self.policy_requirement_count] = Some(requirement);
        self.policy_requirement_count += 1;
        Ok(())
    }

    pub fn add_policy_provide(
        &mut self,
        provide: BootPolicyProvideConfig,
    ) -> Result<(), InitError> {
        if self.policy_provide_count == self.policy_provides.len()
            || provide.service.is_empty()
            || provide.capability.is_empty()
        {
            return Err(InitError::InvalidBootManifest);
        }
        self.policy_provides[self.policy_provide_count] = Some(provide);
        self.policy_provide_count += 1;
        Ok(())
    }

    pub fn add_policy_mount(&mut self, mount: BootPolicyMountConfig) -> Result<(), InitError> {
        if self.policy_mount_count == self.policy_mounts.len()
            || mount.service.is_empty()
            || mount.mount_root.is_empty()
            || !valid_vfs_root_path(mount.mount_root.as_bytes())
        {
            return Err(InitError::InvalidBootManifest);
        }
        if mount.path.is_empty() || mount.source.is_empty() {
            if !mount.path.is_empty() || !mount.source.is_empty() || mount.flags != 0 {
                return Err(InitError::InvalidBootManifest);
            }
        } else if !valid_vfs_root_path(mount.path.as_bytes())
            || !valid_vfs_root_path(mount.source.as_bytes())
            || mount.path == "/"
            || mount.flags & !known_boot_process_mount_flags() != 0
            || mount.flags & BOOT_PROCESS_MOUNT_BIND == 0
        {
            return Err(InitError::InvalidBootManifest);
        }
        self.policy_mounts[self.policy_mount_count] = Some(mount);
        self.policy_mount_count += 1;
        Ok(())
    }

    pub fn add_policy_state_path(
        &mut self,
        state_path: BootPolicyStatePathConfig,
    ) -> Result<(), InitError> {
        if self.policy_state_path_count == self.policy_state_paths.len()
            || state_path.service.is_empty()
            || state_path.state.is_empty()
            || state_path.root.is_empty()
            || state_path.rights == 0
            || !valid_vfs_root_path(state_path.root.as_bytes())
            || !self.state_volume_ref_valid(state_path.state)
        {
            return Err(InitError::InvalidBootManifest);
        }
        self.policy_state_paths[self.policy_state_path_count] = Some(state_path);
        self.policy_state_path_count += 1;
        Ok(())
    }

    pub fn add_policy_bootstrap(
        &mut self,
        bootstrap: BootPolicyBootstrapConfig,
    ) -> Result<(), InitError> {
        if self.policy_bootstrap_count == self.policy_bootstraps.len()
            || bootstrap.service.is_empty()
            || bootstrap.authority.is_empty()
            || bootstrap.rule.is_empty()
            || bootstrap.rights == 0
        {
            return Err(InitError::InvalidBootManifest);
        }
        self.policy_bootstraps[self.policy_bootstrap_count] = Some(bootstrap);
        self.policy_bootstrap_count += 1;
        Ok(())
    }

    fn object_ref_valid(&self, object_kind: u16, object_index: usize) -> bool {
        match object_kind {
            BOOT_OBJECT_ENDPOINT => object_index < self.endpoint_count,
            BOOT_OBJECT_STORE => object_index < self.store_object_count,
            BOOT_OBJECT_TIMER => object_index == 0,
            BOOT_OBJECT_NETWORK_PORT => object_index < self.network_port_count,
            BOOT_OBJECT_IO_PORT_RANGE => object_index < self.io_port_count,
            BOOT_OBJECT_MMIO_REGION => object_index < self.mmio_region_count,
            BOOT_OBJECT_FRAMEBUFFER => object_index < self.framebuffer_count,
            BOOT_OBJECT_INTERRUPT_LINE => object_index < self.interrupt_line_count,
            BOOT_OBJECT_DMA_REGION => object_index < self.dma_region_count,
            BOOT_OBJECT_PCI_DEVICE => object_index < self.pci_device_count,
            BOOT_OBJECT_VIRTIO_DEVICE => object_index < self.virtio_device_count,
            BOOT_OBJECT_NAMESPACE => object_index < self.namespace_count,
            BOOT_OBJECT_VFS_ROOT => object_index < self.vfs_root_count,
            _ => false,
        }
    }

    fn policy_object_ref_valid(&self, object_kind: u16, object_index: usize) -> bool {
        match object_kind {
            BOOT_OBJECT_SECRET => object_index == 0,
            _ => self.object_ref_valid(object_kind, object_index),
        }
    }

    fn state_volume_ref_valid(&self, state_id: &str) -> bool {
        let mut index = 0;
        while index < self.state_volume_count {
            if let Some(state) = self.state_volumes[index]
                && state.id == state_id
            {
                return true;
            }
            index += 1;
        }
        false
    }
}
