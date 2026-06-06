use crate::kernel::KernelObjectId;

use super::{VfsName, VfsNodeId};

#[derive(Clone, Copy)]
pub(crate) enum VfsNodeKind {
    RegularFile,
    Directory,
    DeviceNode,
    Pipe,
    SyntheticNode,
}

#[derive(Clone, Copy)]
pub(crate) enum VfsBacking {
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
pub(crate) struct VfsNode {
    pub(crate) id: VfsNodeId,
    pub(crate) name: VfsName,
    pub(crate) parent: Option<VfsNodeId>,
    pub(crate) kind: VfsNodeKind,
    pub(crate) backing: VfsBacking,
    pub(crate) mount_source: &'static str,
    pub(crate) metadata_version: u64,
}

impl VfsNode {
    pub(crate) fn with_name(
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
