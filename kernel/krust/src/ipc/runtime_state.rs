use super::*;

#[derive(Clone, Copy)]
pub(super) struct CapabilityLineage {
    pub(super) cap_id: u64,
    pub(super) parent_cap_id: u64,
}

pub(super) const MAX_POLICY_DENIAL_RECORDS: usize = 8;
pub(super) const POLICY_DENIAL_FIELD_BYTES: usize = 64;

#[derive(Clone, Copy)]
pub(super) struct PolicyDenialRecord {
    pub(super) sequence: u64,
    pub(super) generation: [u8; POLICY_DENIAL_FIELD_BYTES],
    pub(super) generation_len: usize,
    pub(super) policy_hash: [u8; POLICY_DENIAL_FIELD_BYTES],
    pub(super) source: [u8; POLICY_DENIAL_FIELD_BYTES],
    pub(super) source_len: usize,
    pub(super) target: [u8; POLICY_DENIAL_FIELD_BYTES],
    pub(super) target_len: usize,
    pub(super) rule: [u8; POLICY_DENIAL_FIELD_BYTES],
    pub(super) rule_len: usize,
    pub(super) reason: [u8; POLICY_DENIAL_FIELD_BYTES],
    pub(super) reason_len: usize,
}

impl PolicyDenialRecord {
    const fn empty() -> Self {
        Self {
            sequence: 0,
            generation: [0; POLICY_DENIAL_FIELD_BYTES],
            generation_len: 0,
            policy_hash: [0; POLICY_DENIAL_FIELD_BYTES],
            source: [0; POLICY_DENIAL_FIELD_BYTES],
            source_len: 0,
            target: [0; POLICY_DENIAL_FIELD_BYTES],
            target_len: 0,
            rule: [0; POLICY_DENIAL_FIELD_BYTES],
            rule_len: 0,
            reason: [0; POLICY_DENIAL_FIELD_BYTES],
            reason_len: 0,
        }
    }

    fn new(
        sequence: u64,
        generation: &str,
        policy_hash: &[u8],
        source: &str,
        target: &str,
        rule: &str,
        reason: &str,
    ) -> Self {
        let mut record = Self::empty();
        record.sequence = sequence;
        record.generation_len =
            copy_policy_denial_field(generation.as_bytes(), &mut record.generation);
        copy_policy_hash(policy_hash, &mut record.policy_hash);
        record.source_len = copy_policy_denial_field(source.as_bytes(), &mut record.source);
        record.target_len = copy_policy_denial_field(target.as_bytes(), &mut record.target);
        record.rule_len = copy_policy_denial_field(rule.as_bytes(), &mut record.rule);
        record.reason_len = copy_policy_denial_field(reason.as_bytes(), &mut record.reason);
        record
    }
}

pub(super) struct PolicyDenialLog {
    records: [PolicyDenialRecord; MAX_POLICY_DENIAL_RECORDS],
    pub(super) count: usize,
    next: usize,
    sequence: u64,
}

impl PolicyDenialLog {
    const fn new() -> Self {
        Self {
            records: [PolicyDenialRecord::empty(); MAX_POLICY_DENIAL_RECORDS],
            count: 0,
            next: 0,
            sequence: 0,
        }
    }

    pub(super) fn record(
        &mut self,
        generation: &str,
        policy_hash: &[u8],
        source: &str,
        target: &str,
        rule: &str,
        reason: &str,
    ) {
        self.sequence = self.sequence.saturating_add(1);
        self.records[self.next] = PolicyDenialRecord::new(
            self.sequence,
            generation,
            policy_hash,
            source,
            target,
            rule,
            reason,
        );
        self.next = (self.next + 1) % MAX_POLICY_DENIAL_RECORDS;
        if self.count < MAX_POLICY_DENIAL_RECORDS {
            self.count += 1;
        }
    }

    pub(super) fn record_at(&self, offset: usize) -> Option<PolicyDenialRecord> {
        if offset >= self.count {
            return None;
        }
        let start = if self.count == MAX_POLICY_DENIAL_RECORDS {
            self.next
        } else {
            0
        };
        Some(self.records[(start + offset) % MAX_POLICY_DENIAL_RECORDS])
    }
}

fn copy_policy_denial_field(source: &[u8], target: &mut [u8; POLICY_DENIAL_FIELD_BYTES]) -> usize {
    let mut index = 0;
    while index < source.len() && index < target.len() {
        let byte = source[index];
        target[index] = if byte == b'\n' || byte == b'\r' || byte == b'\t' {
            b'?'
        } else {
            byte
        };
        index += 1;
    }
    index
}

fn copy_policy_hash(source: &[u8], target: &mut [u8; POLICY_DENIAL_FIELD_BYTES]) {
    let mut index = 0;
    while index < target.len() {
        target[index] = if source.len() == target.len() {
            source[index]
        } else {
            b'0'
        };
        index += 1;
    }
}

pub(super) struct RuntimeState {
    pub(super) objects: ObjectTable,
    pub(super) processes: ProcessTable,
    pub(super) generation_id: &'static str,
    pub(super) active_config: Option<&'static BootRuntimeConfig>,
    pub(super) next_cap_id: u64,
    pub(super) revoked_caps: [u64; MAX_REVOKED_CAPS],
    pub(super) revoked_cap_count: usize,
    pub(super) cap_lineage: [Option<CapabilityLineage>; MAX_CAP_LINEAGE],
    pub(super) cap_lineage_count: usize,
    pub(super) vfs_nodes: [Option<VfsNode>; MAX_VFS_NODES],
    pub(super) vfs_node_count: usize,
    pub(super) next_vfs_node_id: u64,
    pub(super) next_vfs_metadata_version: u64,
    pub(super) vfs_mem_files: [VfsMemoryFile; MAX_VFS_MEM_FILES],
    pub(super) vfs_mem_file_count: usize,
    pub(super) vertexfs_image: [u8; VERTEXFS_IMAGE_BYTES],
    pub(super) vertexfs_image_loaded: bool,
    pub(super) vertexfs_files: [VfsVertexFsFile; MAX_VERTEXFS_FILES],
    pub(super) vertexfs_file_count: usize,
    pub(super) open_file_descriptions: [Option<OpenFileDescription>; MAX_OPEN_FILE_DESCRIPTIONS],
    pub(super) next_file_description_id: u64,
    pub(super) vfs_locks: [Option<VfsLock>; MAX_VFS_LOCKS],
    pub(super) vfs_events: [Option<VfsEvent>; MAX_VFS_EVENTS],
    pub(super) vfs_event_count: usize,
    pub(super) vfs_pipe: VfsPipeBuffer,
    pub(super) endpoint_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    pub(super) store_object_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    pub(super) state_volume_ids: [Option<KernelObjectId>; MAX_BOOT_STATE_VOLUMES],
    pub(super) network_port_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    pub(super) io_port_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    pub(super) mmio_region_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    pub(super) framebuffer_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    pub(super) interrupt_line_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    pub(super) dma_region_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    pub(super) pci_device_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    pub(super) virtio_device_ids: [Option<KernelObjectId>; MAX_OBJECTS],
    pub(super) namespace_ids: [Option<KernelObjectId>; MAX_BOOT_NAMESPACES],
    pub(super) vfs_root_ids: [Option<KernelObjectId>; MAX_BOOT_VFS_ROOTS],
    pub(super) vfs_mount_ids: [Option<KernelObjectId>; MAX_VFS_MOUNTS],
    pub(super) vfs_mount_count: usize,
    pub(super) timer_id: Option<KernelObjectId>,
    pub(super) process_control_id: Option<KernelObjectId>,
    pub(super) secret_id: Option<KernelObjectId>,
    pub(super) state_vfs_request_endpoint: Option<KernelObjectId>,
    pub(super) state_vfs_reply_endpoint: Option<KernelObjectId>,
    pub(super) vertexfs_device_request_endpoint: Option<KernelObjectId>,
    pub(super) vertexfs_device_reply_endpoint: Option<KernelObjectId>,
    pub(super) generation_metadata_block_request_endpoint: Option<KernelObjectId>,
    pub(super) generation_metadata_block_reply_endpoint: Option<KernelObjectId>,
    pub(super) next_vfs_state_transaction_id: u64,
    pub(super) vertexfs_sync_writes: [VertexFsDeviceWrite; VERTEXFS_SYNC_MAX_DEVICE_WRITES],
    pub(super) vertexfs_sync_write_count: usize,
    pub(super) process_template_pids: [Option<ProcessId>; MAX_PROCESSES],
    pub(super) service_lifecycle_events:
        [Option<ServiceLifecycleEvent>; MAX_SERVICE_LIFECYCLE_EVENTS],
    pub(super) service_lifecycle_event_count: usize,
}

pub(super) struct Global<T>(pub(super) UnsafeCell<T>);

unsafe impl<T> Sync for Global<T> {}

pub(super) static RUNTIME: Global<RuntimeState> = Global(UnsafeCell::new(RuntimeState::new()));
pub(super) static INSTALL_STAGING_RUNTIME: Global<RuntimeState> =
    Global(UnsafeCell::new(RuntimeState::new()));
pub(super) static POLICY_DENIAL_LOG: Global<PolicyDenialLog> =
    Global(UnsafeCell::new(PolicyDenialLog::new()));
pub(super) static FRAME_ALLOCATOR: Global<Option<*mut memory::FrameAllocator>> =
    Global(UnsafeCell::new(None));
pub(super) static VIRTIO_RNG_STATE: Global<VirtioRngState> =
    Global(UnsafeCell::new(VirtioRngState::new()));
pub(super) static VIRTIO_NET_STATE: Global<VirtioNetState> =
    Global(UnsafeCell::new(VirtioNetState::new()));
pub(super) static INSPECT_REPORT: Global<InspectReport> =
    Global(UnsafeCell::new(InspectReport::new()));

impl RuntimeState {
    pub(super) const fn new() -> Self {
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
            framebuffer_ids: [None; MAX_OBJECTS],
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

    pub(super) fn reset_capability_lifecycle(&mut self, config: &'static BootRuntimeConfig) {
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
            self.framebuffer_ids[index] = None;
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

    pub(super) fn record_service_lifecycle(
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

    pub(super) fn add_vfs_node(
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

    pub(super) fn add_vfs_node_with_name(
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
        let id = VfsNodeId::new(self.next_vfs_node_id);
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

    pub(super) fn allocate_vfs_metadata_version(&mut self) -> u64 {
        let version = self.next_vfs_metadata_version;
        self.next_vfs_metadata_version = self.next_vfs_metadata_version.saturating_add(1);
        version
    }

    pub(super) fn touch_vfs_memory_file_nodes(&mut self, backing: usize) -> Result<u64, IpcError> {
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

    pub(super) fn add_vfs_memory_file(
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

    pub(super) fn add_vfs_empty_memory_file(&mut self) -> Result<usize, InitError> {
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

    pub(super) fn vfs_memory_file_in_use(&self, file_index: usize) -> bool {
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

    pub(super) fn release_vfs_memory_file(&mut self, file_index: usize) -> Result<(), IpcError> {
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

    pub(super) fn vfs_memory_file_link_count(&self, backing: usize) -> u64 {
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

    pub(super) fn touch_vertexfs_file_nodes(&mut self, backing: usize) -> Result<u64, IpcError> {
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

    pub(super) fn add_vertexfs_file(
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

    pub(super) fn add_empty_vertexfs_file(
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
                && !vertexfs_image_has_inode(&self.vertexfs_image, candidate_inode)?
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

    pub(super) fn vertexfs_dynamic_inode_in_use(&self, inode_id: u32) -> bool {
        let mut index = 0;
        while index < self.vertexfs_file_count {
            if self.vertexfs_files[index].inode_id == inode_id {
                return true;
            }
            index += 1;
        }
        false
    }

    pub(super) fn vertexfs_file_in_use(&self, file_index: usize) -> bool {
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

    pub(super) fn release_vertexfs_file(&mut self, file_index: usize) -> Result<(), IpcError> {
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

    pub(super) fn load_vertexfs_image(&mut self, image: &[u8]) -> Result<(), InitError> {
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

    pub(super) fn prepare_vertexfs_sync_file(
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

    pub(super) fn finish_vertexfs_sync_file(
        &mut self,
        backing: usize,
        checksum: u32,
    ) -> Result<(), IpcError> {
        if backing >= self.vertexfs_file_count {
            return Err(IpcError::VfsBadHandle);
        }
        let file = &mut self.vertexfs_files[backing];
        file.checksum = checksum;
        file.dirty = false;
        Ok(())
    }

    pub(super) fn commit_vertexfs_file_to_image(
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
        write_vertexfs_journal_pending(
            &mut self.vertexfs_image,
            file.inode_id,
            &file.bytes[..file.len],
        )?;
        self.record_vertexfs_sync_sector(VERTEXFS_JOURNAL_SECTOR)?;
        write_vertexfs_file_extent(&mut self.vertexfs_image, file)?;
        let mut sector = 0;
        while sector < file.sector_count {
            self.record_vertexfs_sync_sector(file.first_sector + sector as u64)?;
            sector += 1;
        }
        if vertexfs_image_has_inode(&self.vertexfs_image, file.inode_id)? {
            write_vertexfs_inode_record(&mut self.vertexfs_image, file, checksum)?;
            self.record_vertexfs_sync_section(
                VERTEXFS_INODE_TABLE_SECTOR,
                VERTEXFS_INODE_TABLE_SECTORS,
            )?;
        } else {
            write_vertexfs_dynamic_metadata(&mut self.vertexfs_image, file, checksum)?;
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
        write_vertexfs_journal_clean(&mut self.vertexfs_image)?;
        self.record_vertexfs_sync_sector(VERTEXFS_JOURNAL_SECTOR)?;
        parse_vertexfs_image(&self.vertexfs_image).map_err(|_| IpcError::VfsUnsupported)?;
        Ok(self.vertexfs_sync_write_count)
    }

    pub(super) fn record_vertexfs_sync_section(
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

    pub(super) fn record_vertexfs_sync_sector(&mut self, sector: u64) -> Result<(), IpcError> {
        if self.vertexfs_sync_write_count == self.vertexfs_sync_writes.len() {
            return Err(IpcError::VfsNoSpace);
        }
        let mut bytes = [0u8; VERTEXFS_SECTOR_SIZE];
        bytes.copy_from_slice(vertexfs_image_sector(&self.vertexfs_image, sector)?);
        let index = self.vertexfs_sync_write_count;
        self.vertexfs_sync_writes[index].sector = sector;
        self.vertexfs_sync_writes[index].bytes = bytes;
        self.vertexfs_sync_write_count += 1;
        Ok(())
    }

    pub(super) fn vfs_node_link_count(&self, node: VfsNode) -> u64 {
        match node.backing {
            VfsBacking::MemoryFile(backing) => self.vfs_memory_file_link_count(backing),
            _ => 1,
        }
    }

    pub(super) fn add_vfs_mount(
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

    pub(super) fn remove_vfs_mount_id(&mut self, id: KernelObjectId) {
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

    pub(super) fn remove_owned_dynamic_bind_mounts(&mut self, owner: ProcessId) -> u64 {
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

    pub(super) fn remove_owned_declared_bind_mounts(&mut self, owner: ProcessId) -> u64 {
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

    pub(super) fn vfs_node(&self, id: VfsNodeId) -> Option<VfsNode> {
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

    pub(super) fn vfs_node_by_parent_name(
        &self,
        parent: VfsNodeId,
        name: &[u8],
    ) -> Option<VfsNode> {
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

    pub(super) fn vfs_node_by_path_from(&self, mut node: VfsNode, path: &[u8]) -> Option<VfsNode> {
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

    pub(super) fn vfs_node_by_bind_mount_path(&self, path: &[u8]) -> Option<VfsNode> {
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

    pub(super) fn vfs_node_by_path(&self, path: &[u8]) -> Option<VfsNode> {
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

    pub(super) fn vfs_node_index(&self, id: VfsNodeId) -> Option<usize> {
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

    pub(super) fn vfs_node_has_children(&self, id: VfsNodeId) -> bool {
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

    pub(super) fn vfs_child_by_entry_index(
        &self,
        parent: VfsNodeId,
        entry_index: usize,
    ) -> Option<VfsNode> {
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

    pub(super) fn vfs_node_has_open_description(&self, id: VfsNodeId) -> bool {
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

    pub(super) fn vfs_subtree_has_open_description(&self, root: VfsNodeId) -> bool {
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

    pub(super) fn vfs_node_is_descendant_or_self(
        &self,
        mut node: VfsNodeId,
        root: VfsNodeId,
    ) -> bool {
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

    pub(super) fn remove_vfs_node(&mut self, id: VfsNodeId) -> Result<(), IpcError> {
        let index = self.vfs_node_index(id).ok_or(IpcError::BadCapability)?;
        self.vfs_nodes[index] = None;
        while self.vfs_node_count > 0 && self.vfs_nodes[self.vfs_node_count - 1].is_none() {
            self.vfs_node_count -= 1;
        }
        Ok(())
    }

    pub(super) fn detach_vfs_node(&mut self, id: VfsNodeId) -> Result<VfsNode, IpcError> {
        let index = self.vfs_node_index(id).ok_or(IpcError::VfsBadHandle)?;
        let version = self.allocate_vfs_metadata_version();
        let Some(node) = self.vfs_nodes[index].as_mut() else {
            return Err(IpcError::VfsBadHandle);
        };
        node.parent = None;
        node.metadata_version = version;
        Ok(*node)
    }

    pub(super) fn rename_vfs_node(
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

    pub(super) fn vfs_node_for_store_object(&self, object: KernelObjectId) -> Option<VfsNode> {
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

    pub(super) fn open_file_description(
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
                let id = FileDescriptionId::new(self.next_file_description_id);
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

    pub(super) fn file_description(&self, id: FileDescriptionId) -> Option<OpenFileDescription> {
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

    pub(super) fn file_description_mut(
        &mut self,
        id: FileDescriptionId,
    ) -> Option<&mut OpenFileDescription> {
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

    pub(super) fn retain_file_description(
        &mut self,
        id: FileDescriptionId,
    ) -> Result<(), IpcError> {
        let description = self
            .file_description_mut(id)
            .ok_or(IpcError::VfsBadHandle)?;
        description.ref_count = description
            .ref_count
            .checked_add(1)
            .ok_or(IpcError::VfsNoSpace)?;
        Ok(())
    }

    pub(super) fn release_file_description(
        &mut self,
        id: FileDescriptionId,
    ) -> Result<(), IpcError> {
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

    pub(super) fn reap_unlinked_vfs_node_if_idle(&mut self, id: VfsNodeId) {
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

    pub(super) fn release_process_file_descriptions(&mut self, pid: ProcessId) {
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

    pub(super) fn acquire_vfs_lock(
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

    pub(super) fn record_vfs_event(&mut self, parent: VfsNodeId, kind: u64, name: VfsName) {
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

    pub(super) fn cap_id_revoked_or_has_revoked_ancestor(&self, cap_id: u64) -> bool {
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

    pub(super) fn cap_parent_id(&self, cap_id: u64) -> Option<u64> {
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

    pub(super) fn release_vfs_lock(&mut self, description: FileDescriptionId) -> bool {
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

    pub(super) fn release_vfs_locks_for_description(&mut self, description: FileDescriptionId) {
        let _ = self.release_vfs_lock(description);
    }

    pub(super) fn release_vfs_locks_for_process(&mut self, pid: ProcessId) {
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

    pub(super) fn generation_cap_count(&self, generation_id: &'static str) -> u64 {
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

    pub(super) fn can_allocate_capability(&self) -> bool {
        self.next_cap_id != 0
            && self.next_cap_id != u64::MAX
            && self.cap_lineage_count < self.cap_lineage.len()
    }

    pub(super) fn new_capability(
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

    pub(super) fn rollback_last_capability(&mut self, cap: Capability) {
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

    pub(super) fn record_cap_lineage(
        &mut self,
        cap_id: u64,
        parent_cap_id: u64,
    ) -> Result<(), IpcError> {
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

    pub(super) fn cap_parent_from_lineage(&self, cap_id: u64) -> Option<u64> {
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

    pub(super) fn cap_id_revoked(&self, cap_id: u64) -> bool {
        let mut index = 0;
        while index < self.revoked_cap_count {
            if self.revoked_caps[index] == cap_id {
                return true;
            }
            index += 1;
        }
        false
    }

    pub(super) fn revoke_cap_id(&mut self, cap_id: u64) -> Result<(), IpcError> {
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

    pub(super) fn add_revoked_cap(&mut self, cap_id: u64) -> Result<bool, IpcError> {
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

    pub(super) fn mark_all_revoked_caps(&mut self) {
        let mut index = 0;
        while index < self.revoked_cap_count {
            self.mark_cap_revoked(self.revoked_caps[index]);
            index += 1;
        }
    }

    pub(super) fn mark_cap_revoked(&mut self, cap_id: u64) {
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
