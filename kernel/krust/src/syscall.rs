use core::arch::{asm, global_asm};

use crate::{gdt, ipc, serial};

const IA32_EFER: u32 = 0xc000_0080;
const IA32_STAR: u32 = 0xc000_0081;
const IA32_LSTAR: u32 = 0xc000_0082;
const IA32_FMASK: u32 = 0xc000_0084;

const EFER_SYSCALL_ENABLE: u64 = 1;
const RFLAGS_INTERRUPT_ENABLE: u64 = 1 << 9;
const RFLAGS_DIRECTION_FLAG: u64 = 1 << 10;

const SYSCALL_STACK_SIZE: usize = 256 * 1024;
const SYS_EXIT: u64 = 2;
const SYS_IPC_SEND: u64 = 3;
const SYS_IPC_RECV: u64 = 4;
const SYS_YIELD: u64 = 5;
const SYS_BOOT_READ: u64 = 6;
const SYS_LOG_WRITE: u64 = 7;
const SYS_ACTIVATE_GENERATION: u64 = 8;
const SYS_PROCESS_CREATE: u64 = 9;
const SYS_CAP_DERIVE: u64 = 10;
const SYS_CAP_DROP: u64 = 11;
const SYS_CAP_TRANSFER: u64 = 12;
const SYS_OBJECT_READ: u64 = 13;
const SYS_SLEEP_MS: u64 = 16;
const SYS_PROCESS_WAIT: u64 = 17;
const SYS_ROLLBACK_GENERATION: u64 = 18;
const SYS_IPC_RECV_TIMEOUT: u64 = 19;
const SYS_PROCESS_ATTEMPT: u64 = 20;
const SYS_CAP_REVOKE: u64 = 21;
const SYS_CAP_INSPECT: u64 = 22;
const SYS_CAP_MOVE: u64 = 23;
const SYS_CAP_COPY: u64 = 24;
const SYS_ENDPOINT_CREATE: u64 = 25;
const SYS_QUOTA_DELEGATE: u64 = 26;
const SYS_IO_READ: u64 = 27;
const SYS_IO_WRITE: u64 = 28;
const SYS_IRQ_WAIT: u64 = 29;
const SYS_MMIO_MAP: u64 = 30;
const SYS_RUNTIME_INSPECT: u64 = 31;
const SYS_DMA_MAP: u64 = 32;
const SYS_IO_READ16: u64 = 33;
const SYS_IO_WRITE16: u64 = 34;
const SYS_IO_READ32: u64 = 35;
const SYS_IO_WRITE32: u64 = 36;
const SYS_PROCESS_START: u64 = 37;
const SYS_PROCESS_KILL: u64 = 38;
const SYS_SECRET_READ: u64 = 39;
const SYS_VIRTIO_DEVICE_PROBE: u64 = 40;
const SYS_VIRTIO_RNG_READ: u64 = 41;
const SYS_VIRTIO_NET_TX: u64 = 42;
const SYS_VIRTIO_NET_RX: u64 = 43;
const SYS_NETWORK_SEND_UDP: u64 = 44;
const SYS_NAMESPACE_RESOLVE: u64 = 45;
const SYS_NETWORK_RECV_UDP: u64 = 46;
const SYS_VIRTIO_DEVICE_REPORT: u64 = 47;
const SYS_VFS_OPEN: u64 = 48;
const SYS_VFS_READ: u64 = 49;
const SYS_VFS_CLOSE: u64 = 50;
const SYS_VFS_STAT: u64 = 51;
const SYS_VFS_SEEK: u64 = 52;
const SYS_VFS_PREAD: u64 = 53;
const SYS_VFS_WRITE: u64 = 54;
const SYS_VFS_PWRITE: u64 = 55;
const SYS_VFS_SYNC: u64 = 56;
const SYS_VFS_DUP: u64 = 57;
const SYS_VFS_CREATE: u64 = 58;
const SYS_VFS_UNLINK: u64 = 59;
const SYS_VFS_DERIVE_ROOT: u64 = 60;
const SYS_VFS_LOCK: u64 = 61;
const SYS_VFS_UNLOCK: u64 = 62;
const SYS_VFS_READDIR: u64 = 63;
const SYS_VFS_MOUNT: u64 = 64;
const SYS_VFS_UNMOUNT: u64 = 65;
const SYS_VFS_RENAME: u64 = 66;
const SYS_VFS_MKDIR: u64 = 67;
const SYS_VFS_RMDIR: u64 = 68;
const SYS_VFS_LINK: u64 = 69;

const STATUS_OK: u64 = 0;
const STATUS_BAD_CAPABILITY: u64 = u64::MAX - 1;
const STATUS_BAD_BUFFER: u64 = u64::MAX - 2;
const STATUS_TOO_LARGE: u64 = u64::MAX - 3;
const STATUS_EMPTY: u64 = u64::MAX - 4;
const STATUS_VFS_PERMISSION: u64 = u64::MAX - 32;
const STATUS_VFS_BAD_PATH: u64 = u64::MAX - 33;
const STATUS_VFS_NOT_FOUND: u64 = u64::MAX - 34;
const STATUS_VFS_NOT_DIRECTORY: u64 = u64::MAX - 35;
const STATUS_VFS_NOT_FILE: u64 = u64::MAX - 36;
const STATUS_VFS_BUSY: u64 = u64::MAX - 37;
const STATUS_VFS_BAD_HANDLE: u64 = u64::MAX - 38;
const STATUS_VFS_UNSUPPORTED: u64 = u64::MAX - 39;
const STATUS_VFS_NO_SPACE: u64 = u64::MAX - 40;
const STATUS_VFS_EXISTS: u64 = u64::MAX - 41;

#[repr(C, align(16))]
pub struct SyscallStack([u8; SYSCALL_STACK_SIZE]);

#[unsafe(no_mangle)]
static mut KRUST_SYSCALL_STACK: SyscallStack = SyscallStack([0; SYSCALL_STACK_SIZE]);
#[unsafe(no_mangle)]
static mut KRUST_SYSCALL_USER_RSP: u64 = 0;

unsafe extern "C" {
    fn krust_syscall_entry();
}

global_asm!(
    r#"
    .global krust_syscall_entry
krust_syscall_entry:
    mov [rip + KRUST_SYSCALL_USER_RSP], rsp
    lea rsp, [rip + KRUST_SYSCALL_STACK + {syscall_stack_size}]
    sub rsp, {frame_size}
    mov [rsp + {r15}], r15
    mov [rsp + {r14}], r14
    mov [rsp + {r13}], r13
    mov [rsp + {r12}], r12
    mov [rsp + {r11}], r11
    mov [rsp + {r10}], r10
    mov [rsp + {r9}], r9
    mov [rsp + {r8}], r8
    mov [rsp + {rsi}], rsi
    mov [rsp + {rdi}], rdi
    mov [rsp + {rbp}], rbp
    mov [rsp + {rdx}], rdx
    mov [rsp + {rcx}], rcx
    mov [rsp + {rbx}], rbx
    mov [rsp + {rax}], rax
    mov [rsp + {user_rip}], rcx
    mov qword ptr [rsp + {user_cs}], {user_code}
    mov [rsp + {user_rflags}], r11
    mov r10, [rip + KRUST_SYSCALL_USER_RSP]
    mov [rsp + {user_rsp}], r10
    mov qword ptr [rsp + {user_ss}], {user_data}
    mov rdi, [rsp + {rax}]
    mov rsi, [rsp + {rdi}]
    mov rdx, [rsp + {rsi}]
    mov rcx, [rsp + {rdx}]
    mov r8, rsp
    cld
    call krust_syscall_dispatch
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rsi
    pop rdi
    pop rbp
    pop rdx
    pop rcx
    pop rbx
    pop rax
    iretq
"#,
    syscall_stack_size = const SYSCALL_STACK_SIZE,
    frame_size = const ipc::FRAME_SIZE,
    r15 = const ipc::FRAME_R15,
    r14 = const ipc::FRAME_R14,
    r13 = const ipc::FRAME_R13,
    r12 = const ipc::FRAME_R12,
    r11 = const ipc::FRAME_R11,
    r10 = const ipc::FRAME_R10,
    r9 = const ipc::FRAME_R9,
    r8 = const ipc::FRAME_R8,
    rsi = const ipc::FRAME_RSI,
    rdi = const ipc::FRAME_RDI,
    rbp = const ipc::FRAME_RBP,
    rdx = const ipc::FRAME_RDX,
    rcx = const ipc::FRAME_RCX,
    rbx = const ipc::FRAME_RBX,
    rax = const ipc::FRAME_RAX,
    user_rip = const ipc::FRAME_USER_RIP,
    user_cs = const ipc::FRAME_USER_CS,
    user_rflags = const ipc::FRAME_USER_RFLAGS,
    user_rsp = const ipc::FRAME_USER_RSP,
    user_ss = const ipc::FRAME_USER_SS,
    user_code = const gdt::USER_CODE_SELECTOR as u64,
    user_data = const gdt::USER_DATA_SELECTOR as u64,
);

pub fn init() {
    let entry = krust_syscall_entry as *const () as usize as u64;
    let star =
        ((gdt::USER_SELECTOR_BASE as u64) << 48) | ((gdt::KERNEL_CODE_SELECTOR as u64) << 32);

    unsafe {
        write_msr(IA32_STAR, star);
        write_msr(IA32_LSTAR, entry);
        write_msr(IA32_FMASK, RFLAGS_INTERRUPT_ENABLE | RFLAGS_DIRECTION_FLAG);
        write_msr(IA32_EFER, read_msr(IA32_EFER) | EFER_SYSCALL_ENABLE);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn krust_syscall_dispatch(
    number: u64,
    arg0: u64,
    arg1: u64,
    arg2: u64,
    frame: &mut ipc::SyscallFrame,
) {
    match number {
        SYS_EXIT => exit_current_process(arg0, frame),
        SYS_IPC_SEND => match ipc::send(
            arg0,
            arg1 as *const u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_IPC_SEND", error),
        },
        SYS_IPC_RECV => match ipc::receive(
            arg0,
            arg1 as *mut u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
            frame,
        ) {
            Ok(()) => {}
            Err(error) => frame.rax = ipc_error_status("SYS_IPC_RECV", error),
        },
        SYS_YIELD => schedule_yield(frame),
        SYS_BOOT_READ => match ipc::read_boot_module(
            arg0,
            arg1 as *mut u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(len) => frame.rax = len as u64,
            Err(error) => frame.rax = ipc_error_status("SYS_BOOT_READ", error),
        },
        SYS_LOG_WRITE => match ipc::log_write(
            arg0,
            arg1 as *const u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_LOG_WRITE", error),
        },
        SYS_ACTIVATE_GENERATION => match ipc::activate_generation(
            arg0,
            arg1 as *const u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
            frame,
        ) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_ACTIVATE_GENERATION", error),
        },
        SYS_PROCESS_CREATE => match ipc::create_process(arg0, arg1) {
            Ok(pid) => frame.rax = pid,
            Err(error) => frame.rax = ipc_error_status("SYS_PROCESS_CREATE", error),
        },
        SYS_CAP_DERIVE => match ipc::cap_derive(arg0, arg1, arg2) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_CAP_DERIVE", error),
        },
        SYS_CAP_DROP => match ipc::cap_drop(arg0) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_CAP_DROP", error),
        },
        SYS_CAP_TRANSFER => match ipc::cap_transfer(arg0, arg1, arg2) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_CAP_TRANSFER", error),
        },
        SYS_OBJECT_READ => match ipc::legacy_object_read(
            arg0,
            arg1 as *mut u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(len) => frame.rax = len as u64,
            Err(error) => frame.rax = ipc_error_status("SYS_OBJECT_READ", error),
        },
        SYS_SLEEP_MS => match ipc::sleep_ms(arg0, arg1, frame) {
            Ok(()) => {}
            Err(error) => frame.rax = ipc_error_status("SYS_SLEEP_MS", error),
        },
        SYS_PROCESS_WAIT => match ipc::process_wait(arg0, arg1) {
            Ok(status) => frame.rax = status,
            Err(error) => frame.rax = ipc_error_status("SYS_PROCESS_WAIT", error),
        },
        SYS_ROLLBACK_GENERATION => match ipc::rollback_generation(
            arg0,
            arg1 as *const u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
            frame,
        ) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_ROLLBACK_GENERATION", error),
        },
        SYS_IPC_RECV_TIMEOUT => {
            let max_len = usize::try_from(arg2 & 0xffff_ffff).unwrap_or(usize::MAX);
            let timeout_ms = arg2 >> 32;
            match ipc::receive_timeout(arg0, arg1 as *mut u8, max_len, timeout_ms, frame) {
                Ok(()) => {}
                Err(error) => frame.rax = ipc_error_status("SYS_IPC_RECV_TIMEOUT", error),
            }
        }
        SYS_PROCESS_ATTEMPT => match ipc::process_attempt() {
            Ok(attempt) => frame.rax = attempt,
            Err(error) => frame.rax = ipc_error_status("SYS_PROCESS_ATTEMPT", error),
        },
        SYS_CAP_REVOKE => match ipc::cap_revoke(arg0) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_CAP_REVOKE", error),
        },
        SYS_CAP_INSPECT => match ipc::cap_inspect(arg0) {
            Ok(parent_cap_id) => frame.rax = parent_cap_id,
            Err(error) => frame.rax = ipc_error_status("SYS_CAP_INSPECT", error),
        },
        SYS_CAP_MOVE => match ipc::cap_move(arg0, arg1) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_CAP_MOVE", error),
        },
        SYS_CAP_COPY => match ipc::cap_copy(arg0, arg1, arg2) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_CAP_COPY", error),
        },
        SYS_ENDPOINT_CREATE => match ipc::endpoint_create(arg0, arg1) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_ENDPOINT_CREATE", error),
        },
        SYS_QUOTA_DELEGATE => match ipc::quota_delegate(arg0, arg1, arg2) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_QUOTA_DELEGATE", error),
        },
        SYS_IO_READ => match ipc::io_read(arg0, arg1) {
            Ok(value) => frame.rax = value,
            Err(error) => frame.rax = ipc_error_status("SYS_IO_READ", error),
        },
        SYS_IO_WRITE => match ipc::io_write(arg0, arg1, arg2) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_IO_WRITE", error),
        },
        SYS_IO_READ16 => match ipc::io_read16(arg0, arg1) {
            Ok(value) => frame.rax = value,
            Err(error) => frame.rax = ipc_error_status("SYS_IO_READ16", error),
        },
        SYS_IO_WRITE16 => match ipc::io_write16(arg0, arg1, arg2) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_IO_WRITE16", error),
        },
        SYS_IO_READ32 => match ipc::io_read32(arg0, arg1) {
            Ok(value) => frame.rax = value,
            Err(error) => frame.rax = ipc_error_status("SYS_IO_READ32", error),
        },
        SYS_IO_WRITE32 => match ipc::io_write32(arg0, arg1, arg2) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_IO_WRITE32", error),
        },
        SYS_PROCESS_START => match ipc::start_process(arg0, arg1) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_PROCESS_START", error),
        },
        SYS_PROCESS_KILL => match ipc::kill_process(arg0, arg1) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_PROCESS_KILL", error),
        },
        SYS_SECRET_READ => match ipc::secret_read(
            arg0,
            arg1 as *mut u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(len) => frame.rax = len as u64,
            Err(error) => frame.rax = ipc_error_status("SYS_SECRET_READ", error),
        },
        SYS_VIRTIO_DEVICE_PROBE => match ipc::virtio_device_probe(arg0) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_VIRTIO_DEVICE_PROBE", error),
        },
        SYS_VIRTIO_DEVICE_REPORT => match ipc::virtio_device_report(
            arg0,
            arg1 as *const u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_VIRTIO_DEVICE_REPORT", error),
        },
        SYS_VFS_OPEN => match ipc::vfs_open(arg0, arg1 as *const u8, arg2) {
            Ok(handle) => frame.rax = handle,
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_OPEN", error),
        },
        SYS_VFS_READ => match ipc::vfs_read(
            arg0,
            arg1 as *mut u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
            frame,
        ) {
            Ok(()) => {}
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_READ", error),
        },
        SYS_VFS_CLOSE => match ipc::vfs_close(arg0) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_CLOSE", error),
        },
        SYS_VFS_STAT => match ipc::vfs_stat(
            arg0,
            arg1 as *mut u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
            frame,
        ) {
            Ok(()) => {}
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_STAT", error),
        },
        SYS_VFS_SEEK => match ipc::vfs_seek(arg0, arg1, arg2) {
            Ok(offset) => frame.rax = offset,
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_SEEK", error),
        },
        SYS_VFS_PREAD => match ipc::vfs_pread(arg0, arg1 as *mut u8, arg2, frame) {
            Ok(()) => {}
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_PREAD", error),
        },
        SYS_VFS_WRITE => match ipc::vfs_write(
            arg0,
            arg1 as *const u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
            frame,
        ) {
            Ok(()) => {}
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_WRITE", error),
        },
        SYS_VFS_PWRITE => match ipc::vfs_pwrite(arg0, arg1 as *const u8, arg2, frame) {
            Ok(()) => {}
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_PWRITE", error),
        },
        SYS_VFS_SYNC => match ipc::vfs_sync(arg0, frame) {
            Ok(()) => {}
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_SYNC", error),
        },
        SYS_VFS_DUP => match ipc::vfs_dup(arg0, arg1) {
            Ok(handle) => frame.rax = handle,
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_DUP", error),
        },
        SYS_VFS_CREATE => match ipc::vfs_create(arg0, arg1 as *const u8, arg2) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_CREATE", error),
        },
        SYS_VFS_UNLINK => match ipc::vfs_unlink(
            arg0,
            arg1 as *const u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_UNLINK", error),
        },
        SYS_VFS_DERIVE_ROOT => match ipc::vfs_derive_root(arg0, arg1 as *const u8, arg2) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_DERIVE_ROOT", error),
        },
        SYS_VFS_LOCK => match ipc::vfs_lock(arg0, arg1) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_LOCK", error),
        },
        SYS_VFS_UNLOCK => match ipc::vfs_unlock(arg0) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_UNLOCK", error),
        },
        SYS_VFS_READDIR => match ipc::vfs_readdir(
            arg0,
            arg1 as *mut u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(len) => frame.rax = len as u64,
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_READDIR", error),
        },
        SYS_VFS_MOUNT => match ipc::vfs_mount(arg0, arg1 as *const u8, arg2) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_MOUNT", error),
        },
        SYS_VFS_UNMOUNT => match ipc::vfs_unmount(
            arg0,
            arg1 as *const u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_UNMOUNT", error),
        },
        SYS_VFS_RENAME => {
            match ipc::vfs_rename(
                arg0,
                arg1 as *const u8,
                usize::try_from(arg2).unwrap_or(usize::MAX),
            ) {
                Ok(()) => frame.rax = STATUS_OK,
                Err(error) => frame.rax = ipc_error_status("SYS_VFS_RENAME", error),
            }
        }
        SYS_VFS_MKDIR => match ipc::vfs_mkdir(arg0, arg1 as *const u8, arg2) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_MKDIR", error),
        },
        SYS_VFS_RMDIR => match ipc::vfs_rmdir(
            arg0,
            arg1 as *const u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_VFS_RMDIR", error),
        },
        SYS_VFS_LINK => {
            match ipc::vfs_link(
                arg0,
                arg1 as *const u8,
                usize::try_from(arg2).unwrap_or(usize::MAX),
            ) {
                Ok(()) => frame.rax = STATUS_OK,
                Err(error) => frame.rax = ipc_error_status("SYS_VFS_LINK", error),
            }
        }
        SYS_VIRTIO_RNG_READ => match ipc::virtio_rng_read(
            arg0,
            arg1 as *mut u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(len) => frame.rax = len as u64,
            Err(error) => frame.rax = ipc_error_status("SYS_VIRTIO_RNG_READ", error),
        },
        SYS_VIRTIO_NET_TX => match ipc::virtio_net_tx(
            arg0,
            arg1 as *const u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_VIRTIO_NET_TX", error),
        },
        SYS_VIRTIO_NET_RX => match ipc::virtio_net_rx(
            arg0,
            arg1 as *mut u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(len) => frame.rax = len as u64,
            Err(error) => frame.rax = ipc_error_status("SYS_VIRTIO_NET_RX", error),
        },
        SYS_NETWORK_SEND_UDP => match ipc::network_send_udp(
            arg0,
            arg1 as *const u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_NETWORK_SEND_UDP", error),
        },
        SYS_NETWORK_RECV_UDP => match ipc::network_recv_udp(
            arg0,
            arg1 as *mut u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
            frame,
        ) {
            Ok(()) => {}
            Err(error) => frame.rax = ipc_error_status("SYS_NETWORK_RECV_UDP", error),
        },
        SYS_IRQ_WAIT => match ipc::irq_wait(arg0, arg1, frame) {
            Ok(()) => {}
            Err(error) => frame.rax = ipc_error_status("SYS_IRQ_WAIT", error),
        },
        SYS_MMIO_MAP => match ipc::mmio_map(arg0) {
            Ok(base) => frame.rax = base,
            Err(error) => frame.rax = ipc_error_status("SYS_MMIO_MAP", error),
        },
        SYS_RUNTIME_INSPECT => match ipc::runtime_inspect(
            arg0,
            arg1 as *mut u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(len) => frame.rax = len as u64,
            Err(error) => frame.rax = ipc_error_status("SYS_RUNTIME_INSPECT", error),
        },
        SYS_DMA_MAP => match ipc::dma_map(
            arg0,
            arg1 as *mut u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_DMA_MAP", error),
        },
        SYS_NAMESPACE_RESOLVE => {
            let path_len = usize::try_from(arg2 & 0xffff_ffff).unwrap_or(usize::MAX);
            let target_slot = arg2 >> 32;
            match ipc::namespace_resolve(arg0, arg1 as *const u8, path_len, target_slot) {
                Ok(()) => frame.rax = STATUS_OK,
                Err(error) => frame.rax = ipc_error_status("SYS_NAMESPACE_RESOLVE", error),
            }
        }
        _ => {
            serial::write_str("Unknown userspace syscall: ");
            serial::write_u64_dec(number);
            serial::write_str("\n");
            frame.rax = u64::MAX;
        }
    }
}

fn exit_current_process(status: u64, frame: &mut ipc::SyscallFrame) {
    serial::write_str("Process exited: proc=");
    serial::write_str(ipc::current_process_name());
    serial::write_str(" status=");
    serial::write_u64_dec(status);
    serial::write_str("\n");

    match ipc::exit_current_process(status, frame) {
        ipc::ScheduleResult::Continue | ipc::ScheduleResult::Switched => {}
        ipc::ScheduleResult::Halt { ok } => {
            if ok {
                serial::write_str("Native service activation ok\n");
            } else {
                serial::write_str("Native service activation failed\n");
            }
            halt_loop()
        }
    }
}

fn schedule_yield(frame: &mut ipc::SyscallFrame) {
    match ipc::yield_current_process(frame) {
        ipc::ScheduleResult::Continue | ipc::ScheduleResult::Switched => {}
        ipc::ScheduleResult::Halt { ok } => {
            if ok {
                serial::write_str("Native service activation ok\n");
            } else {
                serial::write_str("Native service activation failed\n");
            }
            halt_loop()
        }
    }
}

fn ipc_error_status(operation: &str, error: ipc::IpcError) -> u64 {
    match error {
        ipc::IpcError::BadCapability => {
            serial::write_str(operation);
            serial::write_str(" rejected: bad capability\n");
            STATUS_BAD_CAPABILITY
        }
        ipc::IpcError::InvalidUserBuffer => {
            serial::write_str("Bad pointer test: ");
            serial::write_str(operation);
            serial::write_str(" returned STATUS_BAD_BUFFER\n");
            STATUS_BAD_BUFFER
        }
        ipc::IpcError::MessageTooLarge => {
            serial::write_str("IPC syscall rejected: message too large\n");
            STATUS_TOO_LARGE
        }
        ipc::IpcError::Empty => {
            serial::write_str("IPC syscall rejected: endpoint empty\n");
            STATUS_EMPTY
        }
        ipc::IpcError::VfsPermission => {
            vfs_error_status(operation, "STATUS_VFS_PERMISSION", STATUS_VFS_PERMISSION)
        }
        ipc::IpcError::VfsBadPath => {
            vfs_error_status(operation, "STATUS_VFS_BAD_PATH", STATUS_VFS_BAD_PATH)
        }
        ipc::IpcError::VfsNotFound => {
            vfs_error_status(operation, "STATUS_VFS_NOT_FOUND", STATUS_VFS_NOT_FOUND)
        }
        ipc::IpcError::VfsNotDirectory => vfs_error_status(
            operation,
            "STATUS_VFS_NOT_DIRECTORY",
            STATUS_VFS_NOT_DIRECTORY,
        ),
        ipc::IpcError::VfsNotFile => {
            vfs_error_status(operation, "STATUS_VFS_NOT_FILE", STATUS_VFS_NOT_FILE)
        }
        ipc::IpcError::VfsBusy => vfs_error_status(operation, "STATUS_VFS_BUSY", STATUS_VFS_BUSY),
        ipc::IpcError::VfsBadHandle => {
            vfs_error_status(operation, "STATUS_VFS_BAD_HANDLE", STATUS_VFS_BAD_HANDLE)
        }
        ipc::IpcError::VfsUnsupported => {
            vfs_error_status(operation, "STATUS_VFS_UNSUPPORTED", STATUS_VFS_UNSUPPORTED)
        }
        ipc::IpcError::VfsNoSpace => {
            vfs_error_status(operation, "STATUS_VFS_NO_SPACE", STATUS_VFS_NO_SPACE)
        }
        ipc::IpcError::VfsExists => {
            vfs_error_status(operation, "STATUS_VFS_EXISTS", STATUS_VFS_EXISTS)
        }
    }
}

fn vfs_error_status(operation: &str, status_name: &str, status: u64) -> u64 {
    serial::write_str(operation);
    serial::write_str(" returned ");
    serial::write_str(status_name);
    serial::write_str("\n");
    status
}

fn halt_loop() -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

unsafe fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;

    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags)
        );
    }

    ((high as u64) << 32) | low as u64
}

unsafe fn write_msr(msr: u32, value: u64) {
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") value as u32,
            in("edx") (value >> 32) as u32,
            options(nomem, nostack, preserves_flags)
        );
    }
}
