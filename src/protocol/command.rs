//! Helpers for raw command requests.

/// A borrowed, non-empty raw command request.
#[derive(Debug, Clone, Copy)]
pub struct Command<'a>(&'a [u8]);

impl<'a> Command<'a> {
    /// Validates and wraps a raw command byte slice.
    pub fn new(bytes: &'a [u8]) -> Option<Self> {
        if bytes.is_empty() {
            None
        } else {
            Some(Self(bytes))
        }
    }

    /// Returns the complete request, including its opcode byte.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.0
    }
}
