use crate::kernel::{
    IpcError, IpcMessage, KernelObjectId, MAX_MESSAGE_BYTES, MessageQueue, ProcessId,
};

#[derive(Clone, Copy)]
pub(crate) struct NetworkPortObject {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
    queue: MessageQueue,
}

impl NetworkPortObject {
    pub(crate) const fn new(id: KernelObjectId, name: &'static str) -> Self {
        Self {
            id,
            name,
            queue: MessageQueue::empty(),
        }
    }

    pub(crate) fn enqueue_udp(
        &mut self,
        sender: ProcessId,
        bytes: &[u8; MAX_MESSAGE_BYTES],
        len: usize,
    ) -> Result<(), IpcError> {
        self.queue.enqueue(sender, bytes, len)
    }

    pub(crate) fn dequeue_udp(&mut self) -> Option<IpcMessage> {
        self.queue.dequeue_fifo()
    }

    pub(crate) fn has_pending_udp(self) -> bool {
        self.queue.len() > 0
    }
}
