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
    pub(super) vfs_page_cache: [VfsPageCachePage; MAX_VERTEXFS_PAGE_CACHE_PAGES],
    pub(super) vfs_page_cache_stats: VfsPageCacheStats,
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
            vfs_page_cache: [VfsPageCachePage::empty(); MAX_VERTEXFS_PAGE_CACHE_PAGES],
            vfs_page_cache_stats: VfsPageCacheStats::empty(),
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
        while index < self.vfs_page_cache.len() {
            self.vfs_page_cache[index] = VfsPageCachePage::empty();
            index += 1;
        }
        self.vfs_page_cache_stats = VfsPageCacheStats::empty();
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

    pub(super) fn vfs_memory_file_identity(&self, backing: usize, fallback: VfsNodeId) -> u64 {
        let mut identity = u64::MAX;
        let mut index = 0;
        while index < self.vfs_node_count {
            if let Some(node) = self.vfs_nodes[index]
                && let VfsBacking::MemoryFile(node_backing) = node.backing
                && node_backing == backing
                && node.id.raw() < identity
            {
                identity = node.id.raw();
            }
            index += 1;
        }
        if identity == u64::MAX {
            fallback.raw()
        } else {
            identity
        }
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
        while dynamic_index < MAX_VERTEXFS_FILES {
            let Ok(candidate_inode) = vertexfs_dynamic_inode_at(&self.vertexfs_image, dynamic_index)
            else {
                break;
            };
            if !self.vertexfs_dynamic_inode_in_use(candidate_inode)
                && !vertexfs_image_has_inode(&self.vertexfs_image, candidate_inode)?
            {
                inode_id = candidate_inode;
                first_sector = vertexfs_dynamic_data_sector_at(&self.vertexfs_image, dynamic_index)?;
                break;
            }
            dynamic_index += 1;
        }
        if inode_id == 0 {
            return Err(IpcError::VfsNoSpace);
        }

        let mut index = 0;
        while index < self.vertexfs_files.len() {
            if !self.vertexfs_file_in_use(index) {
                self.invalidate_vertexfs_page_cache(index);
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

    pub(super) fn vertexfs_file_link_count(&self, backing: usize) -> u64 {
        let mut count = 0;
        let mut index = 0;
        while index < self.vfs_node_count {
            if let Some(node) = self.vfs_nodes[index]
                && let VfsBacking::VertexFsFile(node_backing) = node.backing
                && node_backing == backing
                && node.parent.is_some()
            {
                count += 1;
            }
            index += 1;
        }
        count
    }

    pub(super) fn vertexfs_file_identity(&self, backing: usize, fallback: VfsNodeId) -> u64 {
        if backing < self.vertexfs_files.len() {
            let inode_id = self.vertexfs_files[backing].inode_id;
            if inode_id != 0 {
                return inode_id as u64;
            }
        }

        let mut identity = u64::MAX;
        let mut index = 0;
        while index < self.vfs_node_count {
            if let Some(node) = self.vfs_nodes[index]
                && let VfsBacking::VertexFsFile(node_backing) = node.backing
                && node_backing == backing
                && node.id.raw() < identity
            {
                identity = node.id.raw();
            }
            index += 1;
        }
        if identity == u64::MAX {
            fallback.raw()
        } else {
            identity
        }
    }

    fn vertexfs_page_cache_mount_id(&self) -> u64 {
        let mut index = 0;
        while index < self.objects.count {
            if let Some(KernelObject::VfsMount(mount)) = self.objects.objects[index]
                && mount.source == "vertexfs"
            {
                return mount.id.raw();
            }
            index += 1;
        }
        0
    }

    fn vertexfs_page_cache_inode_key(&self, backing: usize) -> Result<u32, IpcError> {
        if backing >= self.vertexfs_file_count {
            return Err(IpcError::VfsBadHandle);
        }
        let inode_id = self.vertexfs_files[backing].inode_id;
        if inode_id != 0 {
            Ok(inode_id)
        } else {
            Ok(u32::MAX.saturating_sub(backing as u32))
        }
    }

    fn vertexfs_page_cache_lookup(
        &self,
        backing: usize,
        mount_id: u64,
        inode_id: u32,
        page_offset: u64,
    ) -> Option<usize> {
        let mut index = 0;
        while index < self.vfs_page_cache.len() {
            let page = self.vfs_page_cache[index];
            if page.valid
                && page.backing == backing
                && page.mount_id == mount_id
                && page.inode_id == inode_id
                && page.page_offset == page_offset
            {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn vertexfs_page_cache_dirty_bytes(&self) -> usize {
        let mut bytes = 0usize;
        let mut index = 0;
        while index < self.vfs_page_cache.len() {
            if self.vfs_page_cache[index].valid && self.vfs_page_cache[index].dirty {
                bytes = bytes.saturating_add(VERTEXFS_SECTOR_SIZE);
            }
            index += 1;
        }
        bytes
    }

    fn vertexfs_page_cache_dirty_bytes_for_mount(&self, mount_id: u64) -> usize {
        let mut bytes = 0usize;
        let mut index = 0;
        while index < self.vfs_page_cache.len() {
            if self.vfs_page_cache[index].valid
                && self.vfs_page_cache[index].dirty
                && self.vfs_page_cache[index].mount_id == mount_id
            {
                bytes = bytes.saturating_add(VERTEXFS_SECTOR_SIZE);
            }
            index += 1;
        }
        bytes
    }

    fn update_vertexfs_page_cache_high_water(&mut self) {
        let dirty_bytes = self.vertexfs_page_cache_dirty_bytes();
        if dirty_bytes > self.vfs_page_cache_stats.high_water_dirty_bytes {
            self.vfs_page_cache_stats.high_water_dirty_bytes = dirty_bytes;
        }
    }

    fn vertexfs_page_cache_victim(&mut self, inode_id: u32, page_offset: u64) -> Result<usize, IpcError> {
        let mut index = 0;
        while index < self.vfs_page_cache.len() {
            if !self.vfs_page_cache[index].valid {
                return Ok(index);
            }
            index += 1;
        }

        index = 0;
        while index < self.vfs_page_cache.len() {
            let page = self.vfs_page_cache[index];
            if page.valid && !page.dirty && !page.pinned && !page.writeback {
                self.vfs_page_cache_stats.clean_evictions =
                    self.vfs_page_cache_stats.clean_evictions.saturating_add(1);
                serial::write_str("VertexFS vnode page cache clean eviction: inode=");
                serial::write_u64_dec(page.inode_id as u64);
                serial::write_str(" page=");
                serial::write_u64_dec(page.page_offset);
                serial::write_str("\n");
                return Ok(index);
            }
            index += 1;
        }

        self.vfs_page_cache_stats.dirty_eviction_blocks =
            self.vfs_page_cache_stats.dirty_eviction_blocks.saturating_add(1);
        serial::write_str("VertexFS vnode page cache dirty page not evicted under pressure: inode=");
        serial::write_u64_dec(inode_id as u64);
        serial::write_str(" page=");
        serial::write_u64_dec(page_offset);
        serial::write_str("\n");
        Err(IpcError::VfsNoSpace)
    }

    fn ensure_vertexfs_page_cache_page(
        &mut self,
        backing: usize,
        page_offset: u64,
    ) -> Result<(usize, bool), IpcError> {
        if backing >= self.vertexfs_file_count {
            return Err(IpcError::VfsBadHandle);
        }
        let mount_id = self.vertexfs_page_cache_mount_id();
        let inode_id = self.vertexfs_page_cache_inode_key(backing)?;
        if let Some(index) =
            self.vertexfs_page_cache_lookup(backing, mount_id, inode_id, page_offset)
        {
            return Ok((index, true));
        }

        let slot = self.vertexfs_page_cache_victim(inode_id, page_offset)?;
        let file = self.vertexfs_files[backing];
        let page_start = usize::try_from(page_offset)
            .map_err(|_| IpcError::VfsNoSpace)?
            .checked_mul(VERTEXFS_SECTOR_SIZE)
            .ok_or(IpcError::VfsNoSpace)?;
        let mut bytes = [0u8; VERTEXFS_SECTOR_SIZE];
        let len = if page_start < file.len {
            min(VERTEXFS_SECTOR_SIZE, file.len - page_start)
        } else {
            0
        };
        let mut cursor = 0;
        while cursor < len {
            bytes[cursor] = file.bytes[page_start + cursor];
            cursor += 1;
        }
        self.vfs_page_cache[slot] = VfsPageCachePage {
            valid: true,
            dirty: file.dirty,
            pinned: false,
            writeback: false,
            writeback_error: false,
            mount_id,
            inode_id,
            backing,
            page_offset,
            len,
            bytes,
        };
        self.update_vertexfs_page_cache_high_water();
        Ok((slot, false))
    }

    fn ensure_vertexfs_page_cache_dirty(&mut self, page_index: usize) -> Result<(), IpcError> {
        if page_index >= self.vfs_page_cache.len() || !self.vfs_page_cache[page_index].valid {
            return Err(IpcError::VfsBadHandle);
        }
        if !self.vfs_page_cache[page_index].dirty {
            let page = self.vfs_page_cache[page_index];
            let next_global_dirty_bytes = self
                .vertexfs_page_cache_dirty_bytes()
                .checked_add(VERTEXFS_SECTOR_SIZE)
                .ok_or(IpcError::VfsNoSpace)?;
            let next_mount_dirty_bytes = self
                .vertexfs_page_cache_dirty_bytes_for_mount(page.mount_id)
                .checked_add(VERTEXFS_SECTOR_SIZE)
                .ok_or(IpcError::VfsNoSpace)?;
            if next_global_dirty_bytes > VERTEXFS_PAGE_CACHE_DIRTY_BYTE_LIMIT
                || next_mount_dirty_bytes > VERTEXFS_PAGE_CACHE_PER_MOUNT_DIRTY_BYTE_LIMIT
            {
                self.vfs_page_cache_stats.dirty_limit_rejections = self
                    .vfs_page_cache_stats
                    .dirty_limit_rejections
                    .saturating_add(1);
                serial::write_str("VertexFS vnode page cache dirty limit rejects write: inode=");
                serial::write_u64_dec(page.inode_id as u64);
                serial::write_str(" page=");
                serial::write_u64_dec(page.page_offset);
                serial::write_str(" global_dirty_bytes=");
                serial::write_u64_dec(next_global_dirty_bytes as u64);
                serial::write_str(" global_limit=");
                serial::write_u64_dec(VERTEXFS_PAGE_CACHE_DIRTY_BYTE_LIMIT as u64);
                serial::write_str(" mount_dirty_bytes=");
                serial::write_u64_dec(next_mount_dirty_bytes as u64);
                serial::write_str(" mount_limit=");
                serial::write_u64_dec(VERTEXFS_PAGE_CACHE_PER_MOUNT_DIRTY_BYTE_LIMIT as u64);
                serial::write_str("\n");
                serial::write_str(
                    "VertexFS vnode page cache dirty page not evicted under pressure: inode=",
                );
                serial::write_u64_dec(page.inode_id as u64);
                serial::write_str(" page=");
                serial::write_u64_dec(page.page_offset);
                serial::write_str("\n");
                return Err(IpcError::VfsNoSpace);
            }
        }
        Ok(())
    }

    pub(super) fn read_vertexfs_page_cache(
        &mut self,
        backing: usize,
        offset: u64,
        destination: *mut u8,
        max_len: usize,
    ) -> Result<(usize, u64), IpcError> {
        if backing >= self.vertexfs_file_count {
            return Err(IpcError::VfsBadHandle);
        }
        let file_len = self.vertexfs_files[backing].len;
        let start = min(usize::try_from(offset).unwrap_or(usize::MAX), file_len);
        let remaining = file_len - start;
        let copy_len = min(remaining, max_len);
        if copy_len == 0 {
            return Ok((0, offset));
        }
        let page_offset = (start / VERTEXFS_SECTOR_SIZE) as u64;
        let page_start = start % VERTEXFS_SECTOR_SIZE;
        let (page_index, hit) = self.ensure_vertexfs_page_cache_page(backing, page_offset)?;
        let page = self.vfs_page_cache[page_index];
        if hit {
            self.vfs_page_cache_stats.read_hits =
                self.vfs_page_cache_stats.read_hits.saturating_add(1);
            serial::write_str("VertexFS vnode page cache hit: mount=");
        } else {
            self.vfs_page_cache_stats.read_misses =
                self.vfs_page_cache_stats.read_misses.saturating_add(1);
            serial::write_str("VertexFS vnode page cache miss: mount=");
        }
        serial::write_u64_dec(page.mount_id);
        serial::write_str(" inode=");
        serial::write_u64_dec(page.inode_id as u64);
        serial::write_str(" page=");
        serial::write_u64_dec(page.page_offset);
        serial::write_str(" no_service_ipc=yes\n");

        if self.vfs_page_cache_stats.last_read_valid
            && self.vfs_page_cache_stats.last_read_backing == backing
            && self.vfs_page_cache_stats.last_read_next_offset == start as u64
            && self.vfs_page_cache_stats.last_read_page_offset == page_offset
        {
            self.vfs_page_cache_stats.readahead_hits =
                self.vfs_page_cache_stats.readahead_hits.saturating_add(1);
            serial::write_str("VertexFS vnode page cache readahead hit: inode=");
            serial::write_u64_dec(page.inode_id as u64);
            serial::write_str(" page=");
            serial::write_u64_dec(page.page_offset);
            serial::write_str("\n");
        }

        usercopy::copy_to_user(
            UserPtr::new(destination as u64),
            &page.bytes[page_start..page_start + copy_len],
        )
        .map_err(|_| IpcError::InvalidUserBuffer)?;
        let next_offset = offset
            .checked_add(copy_len as u64)
            .ok_or(IpcError::VfsUnsupported)?;
        self.vfs_page_cache_stats.last_read_valid = true;
        self.vfs_page_cache_stats.last_read_backing = backing;
        self.vfs_page_cache_stats.last_read_next_offset = next_offset;
        self.vfs_page_cache_stats.last_read_page_offset = page_offset;
        Ok((copy_len, next_offset))
    }

    pub(super) fn write_vertexfs_page_cache(
        &mut self,
        backing: usize,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(usize, u64), IpcError> {
        if bytes.len() > MAX_VERTEXFS_FILE_BYTES {
            return Err(IpcError::VfsNoSpace);
        }
        let start = usize::try_from(offset).map_err(|_| IpcError::VfsNoSpace)?;
        let end = start.checked_add(bytes.len()).ok_or(IpcError::VfsNoSpace)?;
        if end > MAX_VERTEXFS_FILE_BYTES {
            return Err(IpcError::VfsNoSpace);
        }
        if backing >= self.vertexfs_file_count {
            return Err(IpcError::VfsBadHandle);
        }
        if bytes.is_empty() {
            return Ok((0, offset));
        }
        let page_offset = (start / VERTEXFS_SECTOR_SIZE) as u64;
        let page_start = start % VERTEXFS_SECTOR_SIZE;
        let (page_index, _) = self.ensure_vertexfs_page_cache_page(backing, page_offset)?;
        self.ensure_vertexfs_page_cache_dirty(page_index)?;
        {
            let page = &mut self.vfs_page_cache[page_index];
            let mut cursor = 0;
            while cursor < bytes.len() {
                page.bytes[page_start + cursor] = bytes[cursor];
                cursor += 1;
            }
            let written_len = page_start + bytes.len();
            if written_len > page.len {
                page.len = written_len;
            }
            page.dirty = true;
            page.pinned = false;
            page.writeback = false;
            page.writeback_error = false;
        }
        {
            let file = &mut self.vertexfs_files[backing];
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
        self.update_vertexfs_page_cache_high_water();
        serial::write_str("VertexFS vnode page cache writepage dirty: inode=");
        serial::write_u64_dec(self.vfs_page_cache[page_index].inode_id as u64);
        serial::write_str(" page=");
        serial::write_u64_dec(page_offset);
        serial::write_str("\n");
        Ok((bytes.len(), end as u64))
    }

    pub(super) fn truncate_vertexfs_page_cache(
        &mut self,
        backing: usize,
        len: usize,
    ) -> Result<(), IpcError> {
        if len > MAX_VERTEXFS_FILE_BYTES {
            return Err(IpcError::VfsNoSpace);
        }
        if backing >= self.vertexfs_file_count {
            return Err(IpcError::VfsBadHandle);
        }
        let old_len = self.vertexfs_files[backing].len;
        if old_len == len {
            return Ok(());
        }
        let (page_index, _) = self.ensure_vertexfs_page_cache_page(backing, 0)?;
        self.ensure_vertexfs_page_cache_dirty(page_index)?;
        let clear_start = min(old_len, len);
        let clear_end = if old_len > len { old_len } else { len };
        {
            let page = &mut self.vfs_page_cache[page_index];
            let mut cursor = clear_start;
            while cursor < clear_end {
                page.bytes[cursor] = 0;
                cursor += 1;
            }
            page.len = len;
            page.dirty = true;
            page.pinned = false;
            page.writeback = false;
            page.writeback_error = false;
        }
        {
            let file = &mut self.vertexfs_files[backing];
            let mut cursor = clear_start;
            while cursor < clear_end {
                file.bytes[cursor] = 0;
                cursor += 1;
            }
            file.len = len;
            file.dirty = true;
            file.checksum = vertexfs_checksum32(&file.bytes[..file.len]);
        }
        self.update_vertexfs_page_cache_high_water();
        serial::write_str("VertexFS vnode page cache truncate dirty: inode=");
        serial::write_u64_dec(self.vfs_page_cache[page_index].inode_id as u64);
        serial::write_str(" len=");
        serial::write_u64_dec(len as u64);
        serial::write_str("\n");
        Ok(())
    }

    pub(super) fn begin_vertexfs_page_cache_writeback(
        &mut self,
        backing: usize,
    ) -> Result<(), IpcError> {
        if backing >= self.vertexfs_file_count {
            return Err(IpcError::VfsBadHandle);
        }
        let mut dirty = false;
        let mut inode_id = self.vertexfs_files[backing].inode_id;
        let mut index = 0;
        while index < self.vfs_page_cache.len() {
            if self.vfs_page_cache[index].valid
                && self.vfs_page_cache[index].backing == backing
                && self.vfs_page_cache[index].dirty
            {
                dirty = true;
                inode_id = self.vfs_page_cache[index].inode_id;
                self.vfs_page_cache[index].pinned = true;
                self.vfs_page_cache[index].writeback = true;
            }
            index += 1;
        }
        if dirty {
            self.vfs_page_cache_stats.writeback_started =
                self.vfs_page_cache_stats.writeback_started.saturating_add(1);
            serial::write_str("VertexFS vnode page cache ordered writeback started: inode=");
            serial::write_u64_dec(inode_id as u64);
            serial::write_str("\n");
        }
        Ok(())
    }

    pub(super) fn finish_vertexfs_page_cache_writeback(
        &mut self,
        backing: usize,
    ) -> Result<(), IpcError> {
        if backing >= self.vertexfs_file_count {
            return Err(IpcError::VfsBadHandle);
        }
        let mut completed = false;
        let mut inode_id = self.vertexfs_files[backing].inode_id;
        let mut index = 0;
        while index < self.vfs_page_cache.len() {
            if self.vfs_page_cache[index].valid && self.vfs_page_cache[index].backing == backing {
                completed = completed || self.vfs_page_cache[index].dirty;
                inode_id = self.vfs_page_cache[index].inode_id;
                self.vfs_page_cache[index].dirty = false;
                self.vfs_page_cache[index].pinned = false;
                self.vfs_page_cache[index].writeback = false;
                self.vfs_page_cache[index].writeback_error = false;
                self.vfs_page_cache[index].len = self.vertexfs_files[backing].len;
            }
            index += 1;
        }
        if completed {
            self.vfs_page_cache_stats.writeback_completed =
                self.vfs_page_cache_stats.writeback_completed.saturating_add(1);
            serial::write_str("VertexFS vnode page cache ordered writeback clean: inode=");
            serial::write_u64_dec(inode_id as u64);
            serial::write_str("\n");
        }
        Ok(())
    }

    pub(super) fn record_vertexfs_page_cache_writeback_error(
        &mut self,
        backing: usize,
        status: u64,
    ) {
        if backing >= self.vertexfs_file_count {
            return;
        }
        self.vertexfs_files[backing].dirty = true;
        let mut inode_id = self.vertexfs_files[backing].inode_id;
        let mut recorded = false;
        let mut index = 0;
        while index < self.vfs_page_cache.len() {
            if self.vfs_page_cache[index].valid && self.vfs_page_cache[index].backing == backing {
                inode_id = self.vfs_page_cache[index].inode_id;
                self.vfs_page_cache[index].dirty = true;
                self.vfs_page_cache[index].pinned = false;
                self.vfs_page_cache[index].writeback = false;
                self.vfs_page_cache[index].writeback_error = true;
                recorded = true;
            }
            index += 1;
        }
        if recorded {
            self.vfs_page_cache_stats.writeback_errors =
                self.vfs_page_cache_stats.writeback_errors.saturating_add(1);
            self.update_vertexfs_page_cache_high_water();
            serial::write_str(
                "VertexFS vnode page cache writeback error recorded: inode=",
            );
            serial::write_u64_dec(inode_id as u64);
            serial::write_str(" status=");
            serial::write_u64_dec(status);
            serial::write_str(" dirty_retained=yes\n");
        }
    }

    fn invalidate_vertexfs_page_cache(&mut self, backing: usize) {
        let mut index = 0;
        while index < self.vfs_page_cache.len() {
            if self.vfs_page_cache[index].valid && self.vfs_page_cache[index].backing == backing {
                self.vfs_page_cache[index] = VfsPageCachePage::empty();
            }
            index += 1;
        }
        if self.vfs_page_cache_stats.last_read_backing == backing {
            self.vfs_page_cache_stats.last_read_valid = false;
        }
    }

    pub(super) fn release_vertexfs_file(&mut self, file_index: usize) -> Result<(), IpcError> {
        if file_index >= self.vertexfs_files.len() || self.vertexfs_file_in_use(file_index) {
            return Err(IpcError::BadCapability);
        }
        self.invalidate_vertexfs_page_cache(file_index);
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
        index = 0;
        while index < self.vfs_page_cache.len() {
            self.vfs_page_cache[index] = VfsPageCachePage::empty();
            index += 1;
        }
        self.vfs_page_cache_stats = VfsPageCacheStats::empty();
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
        self.begin_vertexfs_page_cache_writeback(backing)?;
        let file = self.vertexfs_files[backing];
        let checksum = vertexfs_checksum32(&file.bytes[..file.len]);
        if file.inode_id == 0 {
            self.finish_vertexfs_sync_file(backing, checksum)?;
            return Ok(VertexFsSyncResult::Cached { checksum });
        }
        if !self.vertexfs_image_loaded {
            self.record_vertexfs_page_cache_writeback_error(backing, STATUS_VFS_UNSUPPORTED);
            return Err(IpcError::VfsUnsupported);
        }
        let write_count = match self.commit_vertexfs_file_to_image(file, checksum) {
            Ok(write_count) => write_count,
            Err(error) => {
                self.record_vertexfs_page_cache_writeback_error(backing, STATUS_VFS_UNSUPPORTED);
                return Err(error);
            }
        };
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
        self.finish_vertexfs_page_cache_writeback(backing)?;
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
        let journal_sector = vertexfs_journal_sector(&self.vertexfs_image)?;
        write_vertexfs_journal_pending(
            &mut self.vertexfs_image,
            file.inode_id,
            &file.bytes[..file.len],
        )?;
        self.record_vertexfs_sync_sector(journal_sector)?;
        write_vertexfs_file_extent(&mut self.vertexfs_image, file)?;
        let mut sector = 0;
        while sector < file.sector_count {
            self.record_vertexfs_sync_sector(file.first_sector + sector as u64)?;
            sector += 1;
        }
        if vertexfs_image_has_inode(&self.vertexfs_image, file.inode_id)? {
            let (inode_sector, inode_sectors) = vertexfs_inode_table_section(&self.vertexfs_image)?;
            write_vertexfs_inode_record(&mut self.vertexfs_image, file, checksum)?;
            self.record_vertexfs_sync_section(inode_sector, inode_sectors)?;
        } else {
            let (inode_sector, inode_sectors) = vertexfs_inode_table_section(&self.vertexfs_image)?;
            let (directory_sector, directory_sectors) =
                vertexfs_directory_section(&self.vertexfs_image)?;
            let free_map_sector = vertexfs_free_map_sector(&self.vertexfs_image)?;
            write_vertexfs_dynamic_metadata(&mut self.vertexfs_image, file, checksum)?;
            self.record_vertexfs_sync_section(inode_sector, inode_sectors)?;
            self.record_vertexfs_sync_section(directory_sector, directory_sectors)?;
            self.record_vertexfs_sync_sector(free_map_sector)?;
        }
        write_vertexfs_journal_clean(&mut self.vertexfs_image)?;
        self.record_vertexfs_sync_sector(journal_sector)?;
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
            VfsBacking::VertexFsFile(backing) => self.vertexfs_file_link_count(backing),
            _ => 1,
        }
    }

    pub(super) fn vfs_node_stat_identity(&self, node: VfsNode) -> u64 {
        match node.backing {
            VfsBacking::MemoryFile(backing) => self.vfs_memory_file_identity(backing, node.id),
            VfsBacking::VertexFsFile(backing) => self.vertexfs_file_identity(backing, node.id),
            _ => node.id.raw(),
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
        match node.backing {
            VfsBacking::MemoryFile(backing) => {
                let _ = self.remove_vfs_node(node.id);
                let _ = self.release_vfs_memory_file(backing);
            }
            VfsBacking::VertexFsFile(backing) => {
                let _ = self.remove_vfs_node(node.id);
                let _ = self.release_vertexfs_file(backing);
            }
            _ => {}
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
        index = 0;
        while index < self.open_file_descriptions.len() {
            if let Some(description) = self.open_file_descriptions[index].as_mut()
                && description.watch_cursor > 0
            {
                description.watch_cursor -= 1;
            }
            index += 1;
        }
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
