use crate::kernel::{InitError, IpcError};

pub(crate) const MAX_VFS_MEM_FILE_BYTES: usize = 512;
pub(crate) const MAX_VFS_PIPE_BYTES: usize = 128;

#[derive(Clone, Copy)]
pub(crate) struct VfsPipeBuffer {
    pub(crate) bytes: [u8; MAX_VFS_PIPE_BYTES],
    pub(crate) len: usize,
}

impl VfsPipeBuffer {
    pub(crate) const fn empty() -> Self {
        Self {
            bytes: [0; MAX_VFS_PIPE_BYTES],
            len: 0,
        }
    }

    pub(crate) fn enqueue(&mut self, bytes: &[u8]) -> Result<usize, IpcError> {
        if !self.is_empty() {
            return Err(IpcError::VfsBusy);
        }
        if bytes.len() > self.bytes.len() {
            return Err(IpcError::VfsNoSpace);
        }
        let mut index = 0;
        while index < bytes.len() {
            self.bytes[index] = bytes[index];
            index += 1;
        }
        self.len = bytes.len();
        Ok(bytes.len())
    }

    pub(crate) fn is_empty(self) -> bool {
        self.len == 0
    }
}

#[derive(Clone, Copy)]
pub(crate) struct VfsMemoryFile {
    pub(crate) name: &'static str,
    pub(crate) bytes: [u8; MAX_VFS_MEM_FILE_BYTES],
    pub(crate) len: usize,
}

impl VfsMemoryFile {
    pub(crate) const fn empty() -> Self {
        Self {
            name: "",
            bytes: [0; MAX_VFS_MEM_FILE_BYTES],
            len: 0,
        }
    }

    pub(crate) fn new(name: &'static str, initial: &[u8]) -> Result<Self, InitError> {
        if initial.len() > MAX_VFS_MEM_FILE_BYTES {
            return Err(InitError::InvalidBootManifest);
        }
        let mut file = Self::empty();
        file.name = name;
        let mut index = 0;
        while index < initial.len() {
            file.bytes[index] = initial[index];
            index += 1;
        }
        file.len = initial.len();
        Ok(file)
    }
}
