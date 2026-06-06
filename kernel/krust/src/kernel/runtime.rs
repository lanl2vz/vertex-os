#[derive(Clone, Copy)]
pub enum ScheduleResult {
    Continue,
    Switched,
    Halt { ok: bool },
}
