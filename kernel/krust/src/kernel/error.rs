#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitError {
    ObjectTableFull,
    ProcessTableFull,
    CapabilityTableFull,
    InvalidBootManifest,
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
