const MAX_INSPECT_REPORT_BYTES: usize = 128 * 1024;

pub(crate) struct InspectReport {
    bytes: [u8; MAX_INSPECT_REPORT_BYTES],
    len: usize,
    truncated: bool,
}

impl InspectReport {
    pub(crate) const fn new() -> Self {
        Self {
            bytes: [0; MAX_INSPECT_REPORT_BYTES],
            len: 0,
            truncated: false,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.len = 0;
        self.truncated = false;
    }

    pub(crate) fn push_byte(&mut self, byte: u8) {
        if self.len == self.bytes.len() {
            self.truncated = true;
            return;
        }
        self.bytes[self.len] = byte;
        self.len += 1;
    }

    pub(crate) fn push_str(&mut self, value: &str) {
        self.push_bytes(value.as_bytes());
    }

    pub(crate) fn push_bytes(&mut self, value: &[u8]) {
        let mut index = 0;
        while index < value.len() {
            self.push_byte(value[index]);
            index += 1;
        }
    }

    pub(crate) fn push_u64_dec(&mut self, mut value: u64) {
        if value == 0 {
            self.push_byte(b'0');
            return;
        }

        let mut digits = [0u8; 20];
        let mut len = 0;
        while value > 0 {
            digits[len] = b'0' + (value % 10) as u8;
            value /= 10;
            len += 1;
        }
        while len > 0 {
            len -= 1;
            self.push_byte(digits[len]);
        }
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn is_truncated(&self) -> bool {
        self.truncated
    }
}
