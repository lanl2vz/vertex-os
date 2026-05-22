use core::ptr;

use crate::{limine, paging};

#[derive(Clone, Copy)]
pub struct UserPtr(u64);

impl UserPtr {
    pub const fn new(address: u64) -> Self {
        Self(address)
    }

    fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserCopyError {
    HhdmUnavailable,
    BufferTooSmall,
    InvalidRange(paging::UserRangeError),
}

pub fn copy_from_user(dst: &mut [u8], user_ptr: UserPtr, len: usize) -> Result<(), UserCopyError> {
    if len > dst.len() {
        return Err(UserCopyError::BufferTooSmall);
    }

    validate(user_ptr, len, paging::UserAccess::Read)?;

    unsafe {
        ptr::copy_nonoverlapping(user_ptr.raw() as *const u8, dst.as_mut_ptr(), len);
    }
    Ok(())
}

pub fn copy_to_user(user_ptr: UserPtr, src: &[u8]) -> Result<(), UserCopyError> {
    validate(user_ptr, src.len(), paging::UserAccess::Write)?;

    unsafe {
        ptr::copy_nonoverlapping(src.as_ptr(), user_ptr.raw() as *mut u8, src.len());
    }
    Ok(())
}

pub fn validate_user_buffer(
    user_ptr: UserPtr,
    len: usize,
    access: paging::UserAccess,
) -> Result<(), UserCopyError> {
    validate(user_ptr, len, access)
}

fn validate(
    user_ptr: UserPtr,
    len: usize,
    access: paging::UserAccess,
) -> Result<(), UserCopyError> {
    let hhdm_offset = limine::hhdm_offset().ok_or(UserCopyError::HhdmUnavailable)?;
    paging::validate_user_range(
        hhdm_offset,
        paging::active_root_table_physical(),
        user_ptr.raw(),
        len,
        access,
    )
    .map_err(UserCopyError::InvalidRange)
}
