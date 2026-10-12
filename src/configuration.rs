//! Validation of prepared, length-prefixed radio command streams.

use crate::protocol::{command, gpio, opcode};

/// Structural error in a prepared command stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigurationError {
    /// No final zero-length terminator was found.
    MissingTerminator,
    /// A record exceeds the 16-byte command buffer.
    InvalidLength {
        /// Byte offset of the record's length prefix.
        offset: usize,
        /// Encoded command length.
        length: u8,
    },
    /// A record's payload extends beyond the provided slice.
    Truncated {
        /// Byte offset of the record's length prefix.
        offset: usize,
    },
    /// Bytes follow the zero terminator.
    TrailingData {
        /// Byte offset immediately after the terminator.
        offset: usize,
    },
    /// A recognized command has an invalid length or parameter structure.
    InvalidCommand {
        /// Byte offset of the record's length prefix.
        offset: usize,
    },
}

/// A fully validated borrowed configuration stream, without allocation.
///
/// Format: `[length, command bytes..., length, command bytes..., 0]`, matching
/// WDS command arrays. Each command is 1–16 bytes; the final zero is mandatory
/// and must be the last byte. `[0]` is a valid empty configuration.
///
/// Known typed commands are checked for length, reserved bits, counts and
/// ranges. Other command payloads are opaque: framing validation cannot check
/// their opcodes, property IDs/values, firmware compatibility or RF settings.
/// Direct FIFO/FRR operations and READ_CMD_BUFF are rejected because they do
/// not use the normal command/CTS transport. Firmware patch streams should be
/// sent through the raw API separately, before booting the application.
///
/// ```
/// use si446x::{Configuration, ConfigurationError};
///
/// // Set one property in group 0x20, then request READY state.
/// let bytes = [5, 0x11, 0x20, 1, 0, 3, 2, 0x34, 3, 0];
/// let config = Configuration::new(&bytes)?;
/// assert_eq!(config.commands().count(), 2);
/// # Ok::<(), ConfigurationError>(())
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Configuration<'a> {
    bytes: &'a [u8],
}

impl<'a> Configuration<'a> {
    /// Validates the entire stream without touching hardware.
    pub fn new(bytes: &'a [u8]) -> Result<Self, ConfigurationError> {
        let mut offset = 0;
        while offset < bytes.len() {
            let length = bytes[offset];
            if length == 0 {
                if offset + 1 != bytes.len() {
                    return Err(ConfigurationError::TrailingData { offset: offset + 1 });
                }
                return Ok(Self { bytes });
            }
            if length > 16 {
                return Err(ConfigurationError::InvalidLength { offset, length });
            }
            let start = offset + 1;
            let count = usize::from(length);
            if count > bytes.len() - start {
                return Err(ConfigurationError::Truncated { offset });
            }
            let end = start + count;
            if !valid_command(&bytes[start..end]) {
                return Err(ConfigurationError::InvalidCommand { offset });
            }
            offset = end;
        }
        Err(ConfigurationError::MissingTerminator)
    }

    /// Returns the original stream, including prefixes and terminator.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Iterates over command payloads, excluding length prefixes and terminator.
    pub fn commands(self) -> impl Iterator<Item = &'a [u8]> {
        let mut bytes = self.bytes;
        core::iter::from_fn(move || {
            let length = usize::from(bytes[0]);
            if length == 0 {
                return None;
            }
            let payload = &bytes[1..1 + length];
            bytes = &bytes[1 + length..];
            Some(payload)
        })
    }
}

fn valid_command(bytes: &[u8]) -> bool {
    match bytes[0] {
        opcode::SET_PROPERTY => {
            bytes.len() >= 5
                && usize::from(bytes[2]) == bytes.len() - 4
                && command::valid_property_range(
                    bytes[3],
                    usize::from(bytes[2]),
                    command::MAX_SET_PROPERTIES,
                )
        }
        opcode::GET_PROPERTY => {
            bytes.len() == 4
                && command::valid_property_range(
                    bytes[3],
                    usize::from(bytes[2]),
                    command::MAX_GET_PROPERTIES,
                )
        }
        opcode::GPIO_PIN_CFG => gpio::valid_request(bytes),
        opcode::CHANGE_STATE => bytes.len() == 2 && matches!(bytes[1], 0..=3 | 5..=8),
        opcode::NOP | opcode::PART_INFO | opcode::FUNC_INFO | opcode::REQUEST_DEVICE_STATE => {
            bytes.len() == 1
        }
        opcode::POWER_UP => {
            if bytes.len() != 7 || !matches!(bytes[1], 0 | 1 | 0x80 | 0x81) || bytes[2] != 0 {
                return false;
            }
            let frequency = u32::from_be_bytes([bytes[3], bytes[4], bytes[5], bytes[6]]);
            (25_000_000..=32_000_000).contains(&frequency)
        }
        opcode::READ_CMD_BUFF
        | opcode::WRITE_TX_FIFO
        | opcode::READ_RX_FIFO
        | opcode::FRR_A_READ
        | opcode::FRR_B_READ
        | opcode::FRR_C_READ
        | opcode::FRR_D_READ => false,
        _ => true,
    }
}
