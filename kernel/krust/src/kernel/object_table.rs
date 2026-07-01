use crate::{
    device::{
        DmaRegionObject, FramebufferObject, InterruptLineObject, IoPortRangeObject,
        MmioRegionObject, NetworkPortObject, PciDeviceObject, TimerObject, VirtioDeviceObject,
    },
    vfs::{
        MAX_NAMESPACE_ENTRIES, NamespaceEntry, NamespaceObject, VfsMountObject, VfsNodeId, VfsPath,
        VfsRootObject, vfs_authority_path_covers,
    },
};

use super::{
    BOOT_ENDPOINT_ID, BootModuleObject, InitError, IpcEndpoint, IpcError, KernelObject,
    KernelObjectId, MAX_OBJECTS, ProcessControlObject, ProcessId, SecretObject, StateVolumeObject,
    StoreObject,
};

pub(crate) struct ObjectTable {
    pub(crate) objects: [Option<KernelObject>; MAX_OBJECTS],
    pub(crate) count: usize,
    next_id: u64,
}

impl ObjectTable {
    pub(crate) const fn new() -> Self {
        Self {
            objects: [None; MAX_OBJECTS],
            count: 0,
            next_id: BOOT_ENDPOINT_ID,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.count = 0;
        self.next_id = BOOT_ENDPOINT_ID;
    }

    pub(crate) fn add_endpoint(&mut self, name: &'static str) -> Result<KernelObjectId, InitError> {
        self.add_endpoint_owned(name, ProcessId::empty())
    }

    pub(crate) fn add_endpoint_owned(
        &mut self,
        name: &'static str,
        owner: ProcessId,
    ) -> Result<KernelObjectId, InitError> {
        let id = KernelObjectId::new(self.next_id);
        self.insert_object(KernelObject::IpcEndpoint(IpcEndpoint::new(id, name, owner)))?;
        self.next_id += 1;
        Ok(id)
    }

    pub(crate) fn add_boot_module(
        &mut self,
        name: &'static str,
        base: u64,
        length: u64,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId::new(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::BootModule(BootModuleObject::new(
            id, name, base, length,
        )));
        self.count += 1;
        Ok(id)
    }

    pub(crate) fn add_store_object(
        &mut self,
        name: &'static str,
        base: u64,
        length: u64,
        hash: &'static str,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId::new(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::StoreObject(StoreObject::new(
            id, name, base, length, hash,
        )));
        self.count += 1;
        Ok(id)
    }

    pub(crate) fn add_state_volume(
        &mut self,
        name: &'static str,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId::new(self.next_id);
        self.next_id += 1;
        self.objects[self.count] =
            Some(KernelObject::StateVolume(StateVolumeObject::new(id, name)));
        self.count += 1;
        Ok(id)
    }

    pub(crate) fn add_timer(&mut self, name: &'static str) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId::new(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::Timer(TimerObject::new(id, name)));
        self.count += 1;
        Ok(id)
    }

    pub(crate) fn add_network_port(
        &mut self,
        name: &'static str,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId::new(self.next_id);
        self.next_id += 1;
        self.objects[self.count] =
            Some(KernelObject::NetworkPort(NetworkPortObject::new(id, name)));
        self.count += 1;
        Ok(id)
    }

    pub(crate) fn add_io_port(
        &mut self,
        name: &'static str,
        base: u64,
        length: u64,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId::new(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::IoPortRange(IoPortRangeObject::new(
            id, name, base, length,
        )));
        self.count += 1;
        Ok(id)
    }

    pub(crate) fn add_mmio_region(
        &mut self,
        name: &'static str,
        base: u64,
        length: u64,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId::new(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::MmioRegion(MmioRegionObject::new(
            id, name, base, length,
        )));
        self.count += 1;
        Ok(id)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add_framebuffer(
        &mut self,
        name: &'static str,
        physical_base: u64,
        length: u64,
        width: u64,
        height: u64,
        pitch: u64,
        bpp: u16,
        red_mask_size: u8,
        red_mask_shift: u8,
        green_mask_size: u8,
        green_mask_shift: u8,
        blue_mask_size: u8,
        blue_mask_shift: u8,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId::new(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::Framebuffer(FramebufferObject::new(
            id,
            name,
            physical_base,
            length,
            width,
            height,
            pitch,
            bpp,
            red_mask_size,
            red_mask_shift,
            green_mask_size,
            green_mask_shift,
            blue_mask_size,
            blue_mask_shift,
        )));
        self.count += 1;
        Ok(id)
    }

    pub(crate) fn add_interrupt_line(
        &mut self,
        name: &'static str,
        line: u64,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId::new(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::InterruptLine(InterruptLineObject::new(
            id, name, line,
        )));
        self.count += 1;
        Ok(id)
    }

    pub(crate) fn add_dma_region(
        &mut self,
        name: &'static str,
        base: u64,
        length: u64,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId::new(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::DmaRegion(DmaRegionObject::new(
            id, name, base, length,
        )));
        self.count += 1;
        Ok(id)
    }

    pub(crate) fn add_pci_device(
        &mut self,
        name: &'static str,
        kind: &'static str,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId::new(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::PciDevice(PciDeviceObject::new(
            id, name, kind,
        )));
        self.count += 1;
        Ok(id)
    }

    pub(crate) fn add_virtio_device(
        &mut self,
        name: &'static str,
        transport: &'static str,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId::new(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::VirtioDevice(VirtioDeviceObject::new(
            id, name, transport,
        )));
        self.count += 1;
        Ok(id)
    }

    pub(crate) fn add_namespace(
        &mut self,
        name: &'static str,
        entries: [Option<NamespaceEntry>; MAX_NAMESPACE_ENTRIES],
        entry_count: usize,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId::new(self.next_id);
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

    pub(crate) fn add_vfs_root(
        &mut self,
        name: &'static str,
        root_path: &'static str,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }
        let root_path = VfsPath::from_boot_root_path(root_path)?;

        let id = KernelObjectId::new(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::VfsRoot(VfsRootObject::new(
            id, name, root_path, false,
        )));
        self.count += 1;
        Ok(id)
    }

    pub(crate) fn add_derived_vfs_root(
        &mut self,
        root_path: VfsPath,
    ) -> Result<KernelObjectId, IpcError> {
        if self.count == self.objects.len() {
            return Err(IpcError::BadCapability);
        }

        let id = KernelObjectId::new(self.next_id);
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

    pub(crate) fn add_vfs_mount(
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

        let id = KernelObjectId::new(self.next_id);
        self.next_id += 1;
        self.insert_object(KernelObject::VfsMount(VfsMountObject::new(
            id, name, root_node, root_path, source, flags, dynamic, owner,
        )))?;
        Ok(id)
    }

    pub(crate) fn add_process_control(
        &mut self,
        name: &'static str,
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId::new(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::ProcessControl(ProcessControlObject::new(
            id, name,
        )));
        self.count += 1;
        Ok(id)
    }

    pub(crate) fn add_secret(
        &mut self,
        name: &'static str,
        value: &'static [u8],
    ) -> Result<KernelObjectId, InitError> {
        if self.count == self.objects.len() {
            return Err(InitError::ObjectTableFull);
        }

        let id = KernelObjectId::new(self.next_id);
        self.next_id += 1;
        self.objects[self.count] = Some(KernelObject::Secret(SecretObject::new(id, name, value)));
        self.count += 1;
        Ok(id)
    }

    pub(crate) fn endpoint_count(&self) -> usize {
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

    pub(crate) fn remove_owned_endpoints(&mut self, owner: ProcessId) -> u64 {
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

    pub(crate) fn remove_owned_endpoint(&mut self, id: KernelObjectId, owner: ProcessId) -> bool {
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

    pub(crate) fn remove_derived_vfs_root(&mut self, id: KernelObjectId) -> bool {
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

    pub(crate) fn remove_dynamic_vfs_mount(
        &mut self,
        root_node: VfsNodeId,
    ) -> Option<KernelObjectId> {
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

    pub(crate) fn remove_dynamic_vfs_mount_by_path(
        &mut self,
        root_path: &[u8],
    ) -> Option<KernelObjectId> {
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

    pub(crate) fn live_count(&self) -> usize {
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

    pub(crate) fn trim_empty_tail(&mut self) {
        while self.count > 0 && self.objects[self.count - 1].is_none() {
            self.count -= 1;
        }
    }

    pub(crate) fn get_endpoint_mut(&mut self, id: KernelObjectId) -> Option<&mut IpcEndpoint> {
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

    pub(crate) fn get_endpoint(&self, id: KernelObjectId) -> Option<IpcEndpoint> {
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

    pub(crate) fn get_boot_module(&self, id: KernelObjectId) -> Option<BootModuleObject> {
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

    pub(crate) fn get_store_object(&self, id: KernelObjectId) -> Option<StoreObject> {
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

    pub(crate) fn get_state_volume(&self, id: KernelObjectId) -> Option<StateVolumeObject> {
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

    pub(crate) fn get_network_port(&self, id: KernelObjectId) -> Option<NetworkPortObject> {
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

    pub(crate) fn get_network_port_mut(
        &mut self,
        id: KernelObjectId,
    ) -> Option<&mut NetworkPortObject> {
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

    pub(crate) fn get_timer(&self, id: KernelObjectId) -> Option<TimerObject> {
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

    pub(crate) fn get_io_port(&self, id: KernelObjectId) -> Option<IoPortRangeObject> {
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

    pub(crate) fn get_mmio_region(&self, id: KernelObjectId) -> Option<MmioRegionObject> {
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

    pub(crate) fn get_framebuffer(&self, id: KernelObjectId) -> Option<FramebufferObject> {
        let mut index = 0;
        while index < self.count {
            if let Some(KernelObject::Framebuffer(framebuffer)) = self.objects[index]
                && framebuffer.id == id
            {
                return Some(framebuffer);
            }
            index += 1;
        }

        None
    }

    pub(crate) fn get_interrupt_line(&self, id: KernelObjectId) -> Option<InterruptLineObject> {
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

    pub(crate) fn get_interrupt_line_mut(
        &mut self,
        id: KernelObjectId,
    ) -> Option<&mut InterruptLineObject> {
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

    pub(crate) fn get_interrupt_line_by_number(
        &self,
        irq_line: u64,
    ) -> Option<InterruptLineObject> {
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

    pub(crate) fn get_dma_region(&self, id: KernelObjectId) -> Option<DmaRegionObject> {
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

    pub(crate) fn get_dma_region_mut(
        &mut self,
        id: KernelObjectId,
    ) -> Option<&mut DmaRegionObject> {
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

    pub(crate) fn get_virtio_device(&self, id: KernelObjectId) -> Option<VirtioDeviceObject> {
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

    pub(crate) fn get_virtio_device_mut(
        &mut self,
        id: KernelObjectId,
    ) -> Option<&mut VirtioDeviceObject> {
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

    pub(crate) fn get_namespace(&self, id: KernelObjectId) -> Option<NamespaceObject> {
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

    pub(crate) fn get_vfs_root(&self, id: KernelObjectId) -> Option<VfsRootObject> {
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

    pub(crate) fn get_vfs_mount_by_root_node(
        &self,
        root_node: VfsNodeId,
    ) -> Option<VfsMountObject> {
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

    pub(crate) fn get_vfs_mount_by_path(&self, path: &[u8]) -> Option<VfsMountObject> {
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

    pub(crate) fn get_vfs_mount_by_exact_path(&self, path: &[u8]) -> Option<VfsMountObject> {
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

    pub(crate) fn get_process_control(&self, id: KernelObjectId) -> Option<ProcessControlObject> {
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

    pub(crate) fn get_secret(&self, id: KernelObjectId) -> Option<SecretObject> {
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
