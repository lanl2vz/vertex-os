use core::arch::{asm, global_asm};
use core::cell::UnsafeCell;

use crate::{gdt, ipc, serial, timer};

const IDT_ENTRY_COUNT: usize = 256;
const IDT_PRESENT_INTERRUPT_GATE: u16 = 0x8e00;

const VECTOR_INVALID_OPCODE: usize = 6;
const VECTOR_GENERAL_PROTECTION: usize = 13;
const VECTOR_PAGE_FAULT: usize = 14;
const VECTOR_TIMER_IRQ: usize = 32;

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    options: u16,
    offset_mid: u16,
    offset_high: u32,
    zero: u32,
}

impl IdtEntry {
    const fn missing() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            options: 0,
            offset_mid: 0,
            offset_high: 0,
            zero: 0,
        }
    }

    fn set_handler(&mut self, handler: u64) {
        self.offset_low = handler as u16;
        self.selector = gdt::KERNEL_CODE_SELECTOR;
        self.options = IDT_PRESENT_INTERRUPT_GATE;
        self.offset_mid = (handler >> 16) as u16;
        self.offset_high = (handler >> 32) as u32;
        self.zero = 0;
    }
}

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

struct Global<T>(UnsafeCell<T>);

unsafe impl<T> Sync for Global<T> {}

static IDT: Global<[IdtEntry; IDT_ENTRY_COUNT]> =
    Global(UnsafeCell::new([IdtEntry::missing(); IDT_ENTRY_COUNT]));

unsafe extern "C" {
    fn krust_invalid_opcode_entry();
    fn krust_general_protection_entry();
    fn krust_page_fault_entry();
    fn krust_timer_entry();
}

global_asm!(
    r#"
    .macro push_user_frame
    push rax
    push rbx
    push rcx
    push rdx
    push rbp
    push rdi
    push rsi
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    .endm

    .macro pop_user_frame
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
    .endm

    .global krust_invalid_opcode_entry
krust_invalid_opcode_entry:
    xor rsi, rsi
    xor rdx, rdx
    mov rdi, 6
    call krust_exception_dispatch
1:
    hlt
    jmp 1b

    .global krust_general_protection_entry
krust_general_protection_entry:
    mov rsi, [rsp]
    xor rdx, rdx
    mov rdi, 13
    call krust_exception_dispatch
2:
    hlt
    jmp 2b

    .global krust_page_fault_entry
krust_page_fault_entry:
    mov r14, [rsp]
    mov r15, cr2
    mov rax, [rsp + 16]
    test rax, 3
    jz 4f
    add rsp, 8
    push_user_frame
    mov rdi, r14
    mov rsi, r15
    mov rdx, rsp
    call krust_page_fault_user_dispatch
    pop_user_frame
    iretq
4:
    mov rdi, 14
    mov rsi, r14
    mov rdx, r15
    call krust_exception_dispatch
3:
    hlt
    jmp 3b

    .global krust_timer_entry
krust_timer_entry:
    push rax
    mov rax, [rsp + 16]
    test rax, 3
    pop rax
    jz 5f
    push_user_frame
    mov rdi, rsp
    call krust_timer_user_dispatch
    pop_user_frame
    iretq
5:
    push_user_frame
    call krust_timer_kernel_dispatch
    pop_user_frame
    iretq
"#
);

pub fn init() {
    let idt = unsafe { &mut *IDT.0.get() };
    idt[VECTOR_INVALID_OPCODE].set_handler(krust_invalid_opcode_entry as *const () as u64);
    idt[VECTOR_GENERAL_PROTECTION].set_handler(krust_general_protection_entry as *const () as u64);
    idt[VECTOR_PAGE_FAULT].set_handler(krust_page_fault_entry as *const () as u64);
    idt[VECTOR_TIMER_IRQ].set_handler(krust_timer_entry as *const () as u64);

    let pointer = DescriptorTablePointer {
        limit: (core::mem::size_of_val(idt) - 1) as u16,
        base: idt.as_ptr() as u64,
    };

    unsafe {
        asm!("lidt [{}]", in(reg) &pointer, options(readonly, nostack, preserves_flags));
    }

    serial::write_str("IDT initialized: #UD #GP #PF IRQ0\n");
}

#[unsafe(no_mangle)]
extern "C" fn krust_timer_user_dispatch(frame: &mut ipc::SyscallFrame) {
    timer::handle_tick();
    let result = ipc::preempt_current_process(frame);
    timer::eoi();
    if let ipc::ScheduleResult::Halt { ok } = result {
        print_halt_status(ok);
        halt_loop();
    }
}

#[unsafe(no_mangle)]
extern "C" fn krust_timer_kernel_dispatch() {
    timer::handle_tick();
    ipc::wake_timed_from_interrupt();
    timer::eoi();
}

#[unsafe(no_mangle)]
extern "C" fn krust_page_fault_user_dispatch(
    error_code: u64,
    cr2: u64,
    frame: &mut ipc::SyscallFrame,
) {
    serial::write_str("User page fault: proc=");
    serial::write_str(ipc::current_process_name());
    serial::write_str(" cr2=");
    serial::write_u64_hex(cr2);
    serial::write_str(" error=");
    serial::write_u64_hex(error_code);
    serial::write_str("\n");

    let result = ipc::fault_current_process("page-fault", cr2, error_code, frame);
    if let ipc::ScheduleResult::Halt { ok } = result {
        print_halt_status(ok);
        halt_loop();
    }
}

#[unsafe(no_mangle)]
extern "C" fn krust_exception_dispatch(vector: u64, error_code: u64, cr2: u64) -> ! {
    serial::write_str("Krust exception: vector=");
    serial::write_u64_dec(vector);
    serial::write_str(" error=");
    serial::write_u64_hex(error_code);

    if vector == VECTOR_PAGE_FAULT as u64 {
        serial::write_str(" cr2=");
        serial::write_u64_hex(cr2);
    }

    serial::write_str("\n");

    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

fn print_halt_status(ok: bool) {
    if ok {
        serial::write_str("Native service activation ok\n");
    } else {
        serial::write_str("Native service activation failed\n");
    }
}

fn halt_loop() -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}
