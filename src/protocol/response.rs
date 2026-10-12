//! Raw response helpers and typed command responses.

use super::{gpio::DriveStrength, state::State};

/// Mode and sampled logic level returned for one radio pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinStatus {
    /// Current mode code, preserved even if unknown.
    pub mode: u8,
    /// Sampled logic level (bit 7); bit 6 does not report pull-up configuration.
    pub high: bool,
}

impl PinStatus {
    /// Decodes a pin reply, ignoring its undefined bit 6.
    pub const fn from_byte(byte: u8) -> Self {
        Self {
            mode: byte & 0x3f,
            high: byte & 0x80 != 0,
        }
    }
}

/// GPIO configuration and levels returned by `GPIO_PIN_CFG`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpioPinStatus {
    /// GPIO0 through GPIO3 modes and levels.
    pub gpio: [PinStatus; 4],
    /// NIRQ mode and level.
    pub nirq: PinStatus,
    /// SDO mode and level.
    pub sdo: PinStatus,
    /// Common output drive strength.
    pub drive_strength: DriveStrength,
}

impl GpioPinStatus {
    /// Response length excluding CTS.
    pub const LEN: usize = 7;

    /// Decodes a reply excluding CTS, ignoring undefined bits.
    pub const fn from_bytes(bytes: [u8; Self::LEN]) -> Self {
        Self {
            gpio: [
                PinStatus::from_byte(bytes[0]),
                PinStatus::from_byte(bytes[1]),
                PinStatus::from_byte(bytes[2]),
                PinStatus::from_byte(bytes[3]),
            ],
            nirq: PinStatus::from_byte(bytes[4]),
            sdo: PinStatus::from_byte(bytes[5]),
            drive_strength: DriveStrength::from_bits(bytes[6]),
        }
    }
}

/// Current operating state and tuned channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceState {
    /// Raw main-state code (low nibble), preserved for unknown states.
    pub main_state: u8,
    /// Currently tuned channel.
    pub channel: u8,
}

impl DeviceState {
    /// Response length excluding CTS.
    pub const LEN: usize = 2;

    /// Decodes reply data, ignoring the undefined high nibble of CURR_STATE.
    pub const fn from_bytes(bytes: [u8; Self::LEN]) -> Self {
        Self {
            main_state: bytes[0] & 0x0f,
            channel: bytes[1],
        }
    }

    /// Returns a known state, normalizing READY2 to READY.
    pub const fn state(self) -> Option<State> {
        State::from_current(self.main_state)
    }
}

/// Chip identification returned by `PART_INFO`.
///
/// Values are preserved as reported, including unknown part and revision IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartInfo {
    /// Chip mask revision (`CHIPREV`).
    pub chip_revision: u8,
    /// Part number (`PART`), e.g. `0x4460` for Si4460.
    pub part: u16,
    /// Part build (`PBUILD`).
    pub part_build: u8,
    /// Device ID (`ID`).
    pub id: u16,
    /// Customer ID (`CUSTOMER`).
    pub customer: u8,
    /// ROM ID (`ROMID`).
    pub rom_id: u8,
}

impl PartInfo {
    /// Number of response data bytes, excluding CTS.
    pub const LEN: usize = 8;

    /// Decodes response data without CTS; length is enforced by the array type.
    pub const fn from_bytes(bytes: [u8; Self::LEN]) -> Self {
        Self {
            chip_revision: bytes[0],
            part: u16::from_be_bytes([bytes[1], bytes[2]]),
            part_build: bytes[3],
            id: u16::from_be_bytes([bytes[4], bytes[5]]),
            customer: bytes[6],
            rom_id: bytes[7],
        }
    }
}

/// Currently loaded firmware information returned by `FUNC_INFO`.
///
/// Function and revision values are preserved without restricting them to
/// known firmware versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuncInfo {
    /// External revision number (`REVEXT`).
    pub revision_external: u8,
    /// Branch revision number (`REVBRANCH`).
    pub revision_branch: u8,
    /// Internal revision number (`REVINT`).
    pub revision_internal: u8,
    /// Applied patch ID (`PATCH`); zero means no patch.
    pub patch: u16,
    /// Current functional mode (`FUNC`).
    pub function: u8,
}

impl FuncInfo {
    /// Number of response data bytes, excluding CTS.
    pub const LEN: usize = 6;

    /// Decodes response data without CTS; length is enforced by the array type.
    pub const fn from_bytes(bytes: [u8; Self::LEN]) -> Self {
        Self {
            revision_external: bytes[0],
            revision_branch: bytes[1],
            revision_internal: bytes[2],
            patch: u16::from_be_bytes([bytes[3], bytes[4]]),
            function: bytes[5],
        }
    }
}

/// Value returned by `READ_CMD_BUFF` when the radio is ready.
pub const CTS_READY: u8 = 0xff;

/// Returns whether a command-buffer status byte signals readiness.
pub const fn is_cts_ready(status: u8) -> bool {
    status == CTS_READY
}
