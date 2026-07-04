use crate::{
    device::{
        DmaRegionObject, FramebufferObject, InterruptLineObject, IoPortRangeObject,
        MmioRegionObject, NetworkPortObject, PciDeviceObject, TimerObject, VirtioDeviceObject,
    },
    vfs::{NamespaceObject, VfsMountObject, VfsRootObject},
};

use super::{runtime_config::BootStateVolumeConfig, transport::IpcEndpoint};

pub const BOOT_ENDPOINT_ID: u64 = 1;
pub(crate) const MAX_OBJECTS: usize = 128;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct KernelObjectId(u64);

impl KernelObjectId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy)]
pub(crate) struct BootModuleObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
    pub(crate) base: u64,
    pub(crate) length: u64,
}

impl BootModuleObject {
    pub(crate) const fn new(
        id: KernelObjectId,
        name: &'static str,
        base: u64,
        length: u64,
    ) -> Self {
        Self {
            id,
            name,
            base,
            length,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct StoreObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
    pub(crate) base: u64,
    pub(crate) length: u64,
    pub(crate) hash: &'static str,
}

impl StoreObject {
    pub(crate) const fn new(
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

#[derive(Clone, Copy)]
pub(crate) struct StateVolumeObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
    pub(crate) owner: &'static str,
    pub(crate) schema_version: &'static str,
    pub(crate) storage_class: &'static str,
    pub(crate) migration_policy: &'static str,
    pub(crate) retention_policy: &'static str,
    pub(crate) sharing_policy: &'static str,
}

impl StateVolumeObject {
    pub(crate) const fn new(id: KernelObjectId, state: BootStateVolumeConfig) -> Self {
        Self {
            id,
            name: state.id,
            owner: state.owner,
            schema_version: state.schema_version,
            storage_class: state.storage_class,
            migration_policy: state.migration_policy,
            retention_policy: state.retention_policy,
            sharing_policy: state.sharing_policy,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ProcessControlObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
}

impl ProcessControlObject {
    pub(crate) const fn new(id: KernelObjectId, name: &'static str) -> Self {
        Self { id, name }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SecretObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
    pub(crate) value: &'static [u8],
}

impl SecretObject {
    pub(crate) const fn new(id: KernelObjectId, name: &'static str, value: &'static [u8]) -> Self {
        Self { id, name, value }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum KernelObject {
    IpcEndpoint(IpcEndpoint),
    BootModule(BootModuleObject),
    StoreObject(StoreObject),
    StateVolume(StateVolumeObject),
    Timer(TimerObject),
    NetworkPort(NetworkPortObject),
    IoPortRange(IoPortRangeObject),
    MmioRegion(MmioRegionObject),
    Framebuffer(FramebufferObject),
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
    pub(crate) fn id(self) -> KernelObjectId {
        match self {
            Self::IpcEndpoint(object) => object.id,
            Self::BootModule(object) => object.id,
            Self::StoreObject(object) => object.id,
            Self::StateVolume(object) => object.id,
            Self::Timer(object) => object.id,
            Self::NetworkPort(object) => object.id,
            Self::IoPortRange(object) => object.id,
            Self::MmioRegion(object) => object.id,
            Self::Framebuffer(object) => object.id,
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
