use crate::kernel::InitError;

use super::MAX_VFS_NAME_BYTES;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum VfsStateOperation {
    Read,
    Stat,
    Write,
    Control,
    ServiceRead,
}

pub(crate) fn state_volume_mount_component(id: &'static str) -> Result<&'static str, InitError> {
    let Some(component) = id.strip_prefix("state:") else {
        return Err(InitError::InvalidBootManifest);
    };
    if component.is_empty() || component.len() > MAX_VFS_NAME_BYTES {
        return Err(InitError::InvalidBootManifest);
    }
    let mut index = 0;
    while index < component.len() {
        let byte = component.as_bytes()[index];
        if byte == b'/' || byte == 0 {
            return Err(InitError::InvalidBootManifest);
        }
        index += 1;
    }
    Ok(component)
}
