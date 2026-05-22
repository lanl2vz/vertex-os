use core::{arch::asm, cell::UnsafeCell};

pub const KERNEL_CODE_SELECTOR: u16 = 0x08;
pub const USER_SELECTOR_BASE: u16 = 0x10;
pub const USER_DATA_SELECTOR: u16 = 0x18 | 3;
pub const USER_CODE_SELECTOR: u16 = 0x20 | 3;
const TSS_SELECTOR: u16 = 0x28;
const INTERRUPT_STACK_SIZE: usize = 64 * 1024;

struct Global<T>(UnsafeCell<T>);

unsafe impl<T> Sync for Global<T> {}

#[repr(C, align(16))]
struct InterruptStack([u8; INTERRUPT_STACK_SIZE]);

#[repr(C, packed)]
struct TaskStateSegment {
    reserved0: u32,
    rsp: [u64; 3],
    reserved1: u64,
    ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    io_map_base: u16,
}

static GDT: Global<[u64; 7]> = Global(UnsafeCell::new([0; 7]));
static TSS: Global<TaskStateSegment> = Global(UnsafeCell::new(TaskStateSegment {
    reserved0: 0,
    rsp: [0; 3],
    reserved1: 0,
    ist: [0; 7],
    reserved2: 0,
    reserved3: 0,
    io_map_base: core::mem::size_of::<TaskStateSegment>() as u16,
}));
static INTERRUPT_STACK: Global<InterruptStack> =
    Global(UnsafeCell::new(InterruptStack([0; INTERRUPT_STACK_SIZE])));

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

pub fn init() {
    let gdt = unsafe { &mut *GDT.0.get() };
    let tss = unsafe { &mut *TSS.0.get() };
    let interrupt_stack = unsafe { &mut *INTERRUPT_STACK.0.get() };
    let stack_top = interrupt_stack.0.as_ptr() as u64 + INTERRUPT_STACK_SIZE as u64;
    tss.rsp[0] = stack_top;

    gdt[0] = 0;
    gdt[1] = 0x00af_9b00_0000_ffff;
    gdt[2] = 0x00cf_9300_0000_ffff;
    gdt[3] = 0x00cf_f300_0000_ffff;
    gdt[4] = 0x00af_fb00_0000_ffff;
    let (tss_low, tss_high) = tss_descriptor(tss as *const TaskStateSegment as u64);
    gdt[5] = tss_low;
    gdt[6] = tss_high;

    let pointer = DescriptorTablePointer {
        limit: (core::mem::size_of_val(gdt) - 1) as u16,
        base: gdt.as_ptr() as u64,
    };

    unsafe {
        asm!("lgdt [{}]", in(reg) &pointer, options(readonly, nostack, preserves_flags));
        asm!("ltr ax", in("ax") TSS_SELECTOR, options(nostack, preserves_flags));
    }
}

pub unsafe fn enter_user_mode(cr3: u64, entry: u64, stack_top: u64) -> ! {
    unsafe {
        asm!(
            "mov ds, ax",
            "mov es, ax",
            in("ax") USER_DATA_SELECTOR,
            options(nostack, preserves_flags)
        );

        asm!(
            "mov cr3, {cr3}",
            "push {user_data}",
            "push {stack_top}",
            "push {rflags}",
            "push {user_code}",
            "push {entry}",
            "iretq",
            cr3 = in(reg) cr3,
            user_data = in(reg) USER_DATA_SELECTOR as u64,
            stack_top = in(reg) stack_top,
            rflags = in(reg) 0x202u64,
            user_code = in(reg) USER_CODE_SELECTOR as u64,
            entry = in(reg) entry,
            options(noreturn)
        );
    }
}

pub unsafe fn switch_address_space(cr3: u64) {
    unsafe {
        asm!(
            "mov cr3, {cr3}",
            cr3 = in(reg) cr3,
            options(nostack, preserves_flags)
        );
    }
}

fn tss_descriptor(base: u64) -> (u64, u64) {
    let limit = (core::mem::size_of::<TaskStateSegment>() - 1) as u64;
    let low = (limit & 0xffff)
        | ((base & 0x00ff_ffff) << 16)
        | (0x89u64 << 40)
        | (((limit >> 16) & 0x0f) << 48)
        | (((base >> 24) & 0xff) << 56);
    let high = base >> 32;
    (low, high)
}
