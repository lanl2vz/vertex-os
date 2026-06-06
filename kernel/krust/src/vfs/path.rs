use crate::kernel::{InitError, IpcError};

pub(crate) const MAX_VFS_PATH_BYTES: usize = 128;
pub(crate) const MAX_VFS_NAME_BYTES: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct VfsNodeId(u64);

impl VfsNodeId {
    pub(crate) const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub(crate) const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy)]
pub(crate) struct VfsName {
    pub(crate) bytes: [u8; MAX_VFS_NAME_BYTES],
    pub(crate) len: usize,
}

impl VfsName {
    pub(crate) const fn empty() -> Self {
        Self {
            bytes: [0; MAX_VFS_NAME_BYTES],
            len: 0,
        }
    }

    pub(crate) fn from_static(value: &'static str) -> Result<Self, InitError> {
        Self::from_bytes(value.as_bytes()).map_err(|_| InitError::InvalidBootManifest)
    }

    pub(crate) fn from_user_component(value: &[u8]) -> Result<Self, IpcError> {
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

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

#[derive(Clone, Copy)]
pub(crate) struct VfsPath {
    pub(crate) bytes: [u8; MAX_VFS_PATH_BYTES],
    pub(crate) len: usize,
}

impl VfsPath {
    pub(crate) const fn empty() -> Self {
        Self {
            bytes: [0; MAX_VFS_PATH_BYTES],
            len: 0,
        }
    }

    pub(crate) fn from_root_path(path: &[u8]) -> Result<Self, IpcError> {
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

    pub(crate) fn from_boot_root_path(path: &'static str) -> Result<Self, InitError> {
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

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

pub(crate) fn valid_vfs_root_path(path: &[u8]) -> bool {
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
