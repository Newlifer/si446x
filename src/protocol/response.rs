//! Helpers for raw command responses.

/// Value returned by `READ_CMD_BUFF` when the radio is ready.
pub const CTS_READY: u8 = 0xff;

/// Returns whether a command-buffer status byte signals readiness.
pub const fn is_cts_ready(status: u8) -> bool {
    status == CTS_READY
}
