use super::KernelObjectId;
use super::{IpcError, ProcessId};

pub const MAX_MESSAGE_BYTES: usize = 512;
pub const MESSAGE_QUEUE_CAPACITY: usize = 4;

#[derive(Clone, Copy)]
pub(crate) struct IpcMessage {
    pub(crate) sender: ProcessId,
    pub(crate) len: usize,
    pub(crate) bytes: [u8; MAX_MESSAGE_BYTES],
}

impl IpcMessage {
    pub(crate) const fn empty() -> Self {
        Self {
            sender: ProcessId::empty(),
            len: 0,
            bytes: [0; MAX_MESSAGE_BYTES],
        }
    }

    pub(crate) fn u64_le_at(self, offset: usize) -> Option<u64> {
        if self.len < offset.saturating_add(8) {
            return None;
        }
        Some(
            (self.bytes[offset] as u64)
                | ((self.bytes[offset + 1] as u64) << 8)
                | ((self.bytes[offset + 2] as u64) << 16)
                | ((self.bytes[offset + 3] as u64) << 24)
                | ((self.bytes[offset + 4] as u64) << 32)
                | ((self.bytes[offset + 5] as u64) << 40)
                | ((self.bytes[offset + 6] as u64) << 48)
                | ((self.bytes[offset + 7] as u64) << 56),
        )
    }
}

#[derive(Clone, Copy)]
pub(crate) struct MessageQueue {
    messages: [IpcMessage; MESSAGE_QUEUE_CAPACITY],
    len: usize,
}

impl MessageQueue {
    pub(crate) const fn empty() -> Self {
        Self {
            messages: [IpcMessage::empty(); MESSAGE_QUEUE_CAPACITY],
            len: 0,
        }
    }

    pub(crate) fn len(self) -> usize {
        self.len
    }

    pub(crate) fn enqueue(
        &mut self,
        sender: ProcessId,
        bytes: &[u8; MAX_MESSAGE_BYTES],
        len: usize,
    ) -> Result<(), IpcError> {
        if self.len == MESSAGE_QUEUE_CAPACITY {
            return Err(IpcError::MessageTooLarge);
        }

        let mut message = IpcMessage::empty();
        message.sender = sender;
        message.len = len;
        message.bytes[..len].copy_from_slice(&bytes[..len]);
        self.messages[self.len] = message;
        self.len += 1;
        Ok(())
    }

    pub(crate) fn has_message_for(self, receiver: ProcessId) -> bool {
        let mut index = 0;
        while index < self.len {
            if self.messages[index].sender != receiver {
                return true;
            }
            index += 1;
        }
        false
    }

    pub(crate) fn dequeue_for(&mut self, receiver: ProcessId) -> Option<IpcMessage> {
        let mut index = 0;
        while index < self.len {
            if self.messages[index].sender != receiver {
                return Some(self.remove_at(index));
            }
            index += 1;
        }
        None
    }

    pub(crate) fn has_transaction_reply_for(
        self,
        receiver: ProcessId,
        transaction_id: u64,
    ) -> bool {
        let mut index = 0;
        while index < self.len {
            if self.messages[index].sender != receiver
                && self.messages[index].u64_le_at(0) == Some(transaction_id)
            {
                return true;
            }
            index += 1;
        }
        false
    }

    pub(crate) fn dequeue_transaction_reply_for(
        &mut self,
        receiver: ProcessId,
        transaction_id: u64,
    ) -> Option<IpcMessage> {
        let mut index = 0;
        while index < self.len {
            if self.messages[index].sender != receiver
                && self.messages[index].u64_le_at(0) == Some(transaction_id)
            {
                return Some(self.remove_at(index));
            }
            index += 1;
        }
        None
    }

    pub(crate) fn remove_transaction_from_sender(
        &mut self,
        sender: ProcessId,
        transaction_id: u64,
    ) -> bool {
        let mut index = 0;
        while index < self.len {
            if self.messages[index].sender == sender
                && self.messages[index].u64_le_at(0) == Some(transaction_id)
            {
                self.remove_at(index);
                return true;
            }
            index += 1;
        }
        false
    }

    pub(crate) fn remove_all_from_sender(&mut self, sender: ProcessId) -> usize {
        let mut removed = 0;
        let mut index = 0;
        while index < self.len {
            if self.messages[index].sender == sender {
                self.remove_at(index);
                removed += 1;
            } else {
                index += 1;
            }
        }
        removed
    }

    pub(crate) fn dequeue_fifo(&mut self) -> Option<IpcMessage> {
        if self.len == 0 {
            return None;
        }
        Some(self.remove_at(0))
    }

    fn remove_at(&mut self, index: usize) -> IpcMessage {
        let message = self.messages[index];
        let mut shift = index;
        while shift + 1 < self.len {
            self.messages[shift] = self.messages[shift + 1];
            shift += 1;
        }
        self.len -= 1;
        self.messages[self.len] = IpcMessage::empty();
        message
    }
}

#[derive(Clone, Copy)]
pub(crate) struct IpcEndpoint {
    pub(crate) id: KernelObjectId,
    pub(crate) name: &'static str,
    pub(crate) owner: ProcessId,
    queue: MessageQueue,
}

impl IpcEndpoint {
    pub(crate) const fn new(id: KernelObjectId, name: &'static str, owner: ProcessId) -> Self {
        Self {
            id,
            name,
            owner,
            queue: MessageQueue::empty(),
        }
    }

    pub(crate) fn enqueue(
        &mut self,
        sender: ProcessId,
        bytes: &[u8; MAX_MESSAGE_BYTES],
        len: usize,
    ) -> Result<(), IpcError> {
        self.queue.enqueue(sender, bytes, len)
    }

    pub(crate) fn has_message_for(&self, receiver: ProcessId) -> bool {
        self.queue.has_message_for(receiver)
    }

    pub(crate) fn dequeue_for(&mut self, receiver: ProcessId) -> Option<IpcMessage> {
        self.queue.dequeue_for(receiver)
    }

    pub(crate) fn has_vfs_state_reply_for(&self, receiver: ProcessId, transaction_id: u64) -> bool {
        self.queue
            .has_transaction_reply_for(receiver, transaction_id)
    }

    pub(crate) fn dequeue_vfs_state_reply_for(
        &mut self,
        receiver: ProcessId,
        transaction_id: u64,
    ) -> Option<IpcMessage> {
        self.queue
            .dequeue_transaction_reply_for(receiver, transaction_id)
    }

    pub(crate) fn remove_vfs_state_request(
        &mut self,
        sender: ProcessId,
        transaction_id: u64,
    ) -> bool {
        self.queue
            .remove_transaction_from_sender(sender, transaction_id)
    }

    pub(crate) fn remove_all_from_sender(&mut self, sender: ProcessId) -> usize {
        self.queue.remove_all_from_sender(sender)
    }
}
