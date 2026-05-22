use core::arch::{asm, global_asm};

use crate::{gdt, ipc, serial};

const IA32_EFER: u32 = 0xc000_0080;
const IA32_STAR: u32 = 0xc000_0081;
const IA32_LSTAR: u32 = 0xc000_0082;
const IA32_FMASK: u32 = 0xc000_0084;

const EFER_SYSCALL_ENABLE: u64 = 1;
const RFLAGS_INTERRUPT_ENABLE: u64 = 1 << 9;

const SYSCALL_STACK_SIZE: usize = 16 * 1024;
const SYS_WRITE_SERIAL: u64 = 1;
const SYS_EXIT: u64 = 2;
const SYS_IPC_SEND: u64 = 3;
const SYS_IPC_RECV: u64 = 4;
const SYS_YIELD: u64 = 5;
const SYS_BOOT_READ: u64 = 6;
const SYS_LOG_WRITE: u64 = 7;
const SYS_ACTIVATE_GENERATION: u64 = 8;
const SYS_PROCESS_START: u64 = 9;
const SYS_CAP_DERIVE: u64 = 10;
const SYS_CAP_DROP: u64 = 11;
const SYS_CAP_TRANSFER: u64 = 12;
const SYS_OBJECT_READ: u64 = 13;
const SYS_STATE_WRITE: u64 = 14;
const SYS_STATE_READ: u64 = 15;
const SYS_SLEEP_MS: u64 = 16;
const SYS_PROCESS_STATUS: u64 = 17;

const STATUS_OK: u64 = 0;
const STATUS_BAD_CAPABILITY: u64 = u64::MAX - 1;
const STATUS_BAD_BUFFER: u64 = u64::MAX - 2;
const STATUS_TOO_LARGE: u64 = u64::MAX - 3;
const STATUS_EMPTY: u64 = u64::MAX - 4;

#[repr(C, align(16))]
pub struct SyscallStack([u8; SYSCALL_STACK_SIZE]);

#[unsafe(no_mangle)]
static mut KRUST_SYSCALL_STACK: SyscallStack = SyscallStack([0; SYSCALL_STACK_SIZE]);

unsafe extern "C" {
    fn krust_syscall_entry();
}

global_asm!(
    r#"
    .global krust_syscall_entry
krust_syscall_entry:
    mov r10, rsp
    lea rsp, [rip + KRUST_SYSCALL_STACK + 16384]
    sub rsp, 64
    mov [rsp + 0], r10
    mov [rsp + 8], rcx
    mov [rsp + 16], r11
    mov qword ptr [rsp + 24], 0
    mov [rsp + 32], rdi
    mov [rsp + 40], rsi
    mov [rsp + 48], rdx
    mov qword ptr [rsp + 56], 0
    mov rdi, rax
    mov rsi, [rsp + 32]
    mov rdx, [rsp + 40]
    mov rcx, [rsp + 48]
    mov r8, rsp
    call krust_syscall_dispatch
    mov r10, [rsp + 0]
    mov rcx, [rsp + 8]
    mov r11, [rsp + 16]
    mov rax, [rsp + 24]
    add rsp, 64
    mov rsp, r10
    sysretq
"#
);

pub fn init() {
    let entry = krust_syscall_entry as *const () as usize as u64;
    let star =
        ((gdt::USER_SELECTOR_BASE as u64) << 48) | ((gdt::KERNEL_CODE_SELECTOR as u64) << 32);

    unsafe {
        write_msr(IA32_STAR, star);
        write_msr(IA32_LSTAR, entry);
        write_msr(IA32_FMASK, RFLAGS_INTERRUPT_ENABLE);
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
        SYS_WRITE_SERIAL => {
            let _ = (arg0, arg1);
            serial::write_str("Legacy SYS_WRITE_SERIAL rejected: use SYS_LOG_WRITE\n");
            frame.rax = STATUS_BAD_CAPABILITY;
        }
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
        ) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_ACTIVATE_GENERATION", error),
        },
        SYS_PROCESS_START => match ipc::start_process(arg0, arg1) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_PROCESS_START", error),
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
        SYS_OBJECT_READ => match ipc::object_read(
            arg0,
            arg1 as *mut u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(len) => frame.rax = len as u64,
            Err(error) => frame.rax = ipc_error_status("SYS_OBJECT_READ", error),
        },
        SYS_STATE_WRITE => match ipc::state_write(
            arg0,
            arg1 as *const u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_STATE_WRITE", error),
        },
        SYS_STATE_READ => match ipc::state_read(
            arg0,
            arg1 as *mut u8,
            usize::try_from(arg2).unwrap_or(usize::MAX),
        ) {
            Ok(len) => frame.rax = len as u64,
            Err(error) => frame.rax = ipc_error_status("SYS_STATE_READ", error),
        },
        SYS_SLEEP_MS => match ipc::sleep_ms(arg0, arg1) {
            Ok(()) => frame.rax = STATUS_OK,
            Err(error) => frame.rax = ipc_error_status("SYS_SLEEP_MS", error),
        },
        SYS_PROCESS_STATUS => match ipc::process_status(arg0, arg1) {
            Ok(status) => frame.rax = status,
            Err(error) => frame.rax = ipc_error_status("SYS_PROCESS_STATUS", error),
        },
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
            serial::write_str("IPC syscall rejected: bad capability\n");
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
    }
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
