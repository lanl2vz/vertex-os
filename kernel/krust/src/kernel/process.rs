use crate::gdt;

const INITIAL_USER_RFLAGS: u64 = 0x202;

pub const FRAME_R15: usize = 0;
pub const FRAME_R14: usize = 8;
pub const FRAME_R13: usize = 16;
pub const FRAME_R12: usize = 24;
pub const FRAME_R11: usize = 32;
pub const FRAME_R10: usize = 40;
pub const FRAME_R9: usize = 48;
pub const FRAME_R8: usize = 56;
pub const FRAME_RSI: usize = 64;
pub const FRAME_RDI: usize = 72;
pub const FRAME_RBP: usize = 80;
pub const FRAME_RDX: usize = 88;
pub const FRAME_RCX: usize = 96;
pub const FRAME_RBX: usize = 104;
pub const FRAME_RAX: usize = 112;
pub const FRAME_USER_RIP: usize = 120;
pub const FRAME_USER_CS: usize = 128;
pub const FRAME_USER_RFLAGS: usize = 136;
pub const FRAME_USER_RSP: usize = 144;
pub const FRAME_USER_SS: usize = 152;
pub const FRAME_SIZE: usize = 160;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessId(u64);

impl ProcessId {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy)]
pub struct ProcessContext {
    pub cr3: u64,
    pub entry: u64,
    pub stack_top: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SyscallFrame {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    pub rax: u64,
    pub user_rip: u64,
    pub user_cs: u64,
    pub user_rflags: u64,
    pub user_rsp: u64,
    pub user_ss: u64,
}

impl SyscallFrame {
    pub(crate) const fn empty() -> Self {
        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            rdx: 0,
            rcx: 0,
            rbx: 0,
            rax: 0,
            user_rip: 0,
            user_cs: 0,
            user_rflags: 0,
            user_rsp: 0,
            user_ss: 0,
        }
    }

    pub(crate) fn from_context(context: ProcessContext) -> Self {
        Self {
            user_rip: context.entry,
            user_cs: gdt::USER_CODE_SELECTOR as u64,
            user_rflags: INITIAL_USER_RFLAGS,
            user_rsp: context.stack_top,
            user_ss: gdt::USER_DATA_SELECTOR as u64,
            ..Self::empty()
        }
    }
}
