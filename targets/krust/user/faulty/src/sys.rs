use core::arch::asm;

pub const STATUS_OK: u64 = 0;
pub const STATUS_VFS_BUSY: u64 = u64::MAX - 37;

const SYS_EXIT: u64 = 2;
const SYS_LOG_WRITE: u64 = 7;
const SYS_PROCESS_ATTEMPT: u64 = 20;
const SYS_VFS_OPEN: u64 = 48;
const SYS_VFS_CLOSE: u64 = 50;
const SYS_VFS_LOCK: u64 = 61;
const SYS_VFS_UNLOCK: u64 = 62;

const VFS_OPEN_READ: u64 = 1;
const VFS_OPEN_WRITE: u64 = 1 << 1;
const VFS_LOCK_EXCLUSIVE: u64 = 2;

pub fn log(cap_slot: u64, message: &[u8]) -> u64 {
    syscall3(
        SYS_LOG_WRITE,
        cap_slot,
        message.as_ptr() as u64,
        message.len() as u64,
    )
}

pub fn process_attempt() -> u64 {
    syscall3(SYS_PROCESS_ATTEMPT, 0, 0, 0)
}

pub fn vfs_open_path_readwrite(cap_slot: u64, path: &[u8]) -> u64 {
    syscall3(
        SYS_VFS_OPEN,
        cap_slot,
        path.as_ptr() as u64,
        ((VFS_OPEN_READ | VFS_OPEN_WRITE) << 32) | path.len() as u64,
    )
}

pub fn vfs_lock_exclusive(handle: u64) -> u64 {
    syscall3(SYS_VFS_LOCK, handle, VFS_LOCK_EXCLUSIVE, 0)
}

pub fn vfs_unlock(handle: u64) -> u64 {
    syscall3(SYS_VFS_UNLOCK, handle, 0, 0)
}

pub fn vfs_close(handle: u64) -> u64 {
    syscall3(SYS_VFS_CLOSE, handle, 0, 0)
}

pub fn pause() {
    unsafe {
        asm!("pause", options(nomem, nostack, preserves_flags));
    }
}

pub fn exit(status: u64) -> ! {
    syscall3(SYS_EXIT, status, 0, 0);
    loop {
        pause();
    }
}

fn syscall3(number: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    let result: u64;

    unsafe {
        asm!(
            "syscall",
            inlateout("rax") number => result,
            in("rdi") arg0,
            in("rsi") arg1,
            in("rdx") arg2,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
    }

    result
}
