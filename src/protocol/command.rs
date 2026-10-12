//! Raw and typed command requests.

use super::opcode;

/// Maximum number of properties written in one command.
pub const MAX_SET_PROPERTIES: usize = 12;
/// Maximum number of properties read in one command.
pub const MAX_GET_PROPERTIES: usize = 16;

pub(crate) fn valid_property_range(start: u8, count: usize, maximum: usize) -> bool {
    count > 0 && count <= maximum && usize::from(start) + count <= 256
}

/// A contiguous property write within one group.
#[derive(Debug, Clone, Copy)]
pub struct SetProperty<'a> {
    /// Property group ID.
    pub group: u8,
    /// Offset of the first property within the group.
    pub start: u8,
    /// One through twelve property values.
    pub values: &'a [u8],
}

impl SetProperty<'_> {
    /// Encodes into a fixed buffer and returns it with its used length.
    ///
    /// Rejects empty or oversized writes and ranges crossing the group boundary.
    /// Only the first returned `length` bytes are part of the request.
    pub fn encode(self) -> Option<([u8; 16], usize)> {
        if !valid_property_range(self.start, self.values.len(), MAX_SET_PROPERTIES) {
            return None;
        }
        let mut bytes = [0; 16];
        bytes[..4].copy_from_slice(&[
            opcode::SET_PROPERTY,
            self.group,
            self.values.len() as u8,
            self.start,
        ]);
        let length = 4 + self.values.len();
        bytes[4..length].copy_from_slice(self.values);
        Some((bytes, length))
    }
}

/// A contiguous property read within one group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetProperty {
    /// Property group ID.
    pub group: u8,
    /// Offset of the first property within the group.
    pub start: u8,
    /// Number of properties to read (one through sixteen).
    pub count: u8,
}

impl GetProperty {
    /// Encodes the request, rejecting invalid counts or group-boundary crossing.
    pub fn encode(self) -> Option<[u8; 4]> {
        if !valid_property_range(self.start, usize::from(self.count), MAX_GET_PROPERTIES) {
            return None;
        }
        Some([opcode::GET_PROPERTY, self.group, self.count, self.start])
    }
}

/// Request to switch operating state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeState(pub super::state::State);

impl ChangeState {
    /// Encodes the request with reserved bits clear.
    pub const fn encode(self) -> [u8; 2] {
        [opcode::CHANGE_STATE, self.0 as u8]
    }
}

/// Request for the current state and tuned channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestDeviceState;

impl RequestDeviceState {
    /// Encodes the opcode-only request.
    pub const fn encode(self) -> [u8; 1] {
        [opcode::REQUEST_DEVICE_STATE]
    }
}

/// Firmware image selected by `POWER_UP`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BootFunction {
    /// Remain in the bootloader.
    Bootloader = 0,
    /// Boot the main radio application.
    MainApplication = 1,
}

/// Parameters for the `POWER_UP` command.
///
/// The reserved oscillator-options byte is always zero, as required by the
/// revC2A command API. Legacy firmware options such as nonzero `XTAL_OPTIONS`
/// can be sent through the raw command API instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerUp {
    /// Firmware image to boot.
    pub function: BootFunction,
    /// Whether a compatible firmware patch has already been downloaded.
    ///
    /// This flag does not download a patch; the caller must do so before boot.
    pub patch: bool,
    /// Crystal or external oscillator frequency in Hz (25 MHz through 32 MHz).
    pub xo_frequency_hz: u32,
}

impl Default for PowerUp {
    fn default() -> Self {
        Self {
            function: BootFunction::MainApplication,
            patch: false,
            xo_frequency_hz: 30_000_000,
        }
    }
}

impl PowerUp {
    /// Encodes the request, including the opcode, without allocating.
    ///
    /// Returns `None` if the oscillator frequency is outside 25–32 MHz.
    pub const fn encode(self) -> Option<[u8; 7]> {
        if self.xo_frequency_hz < 25_000_000 || self.xo_frequency_hz > 32_000_000 {
            return None;
        }
        let frequency = self.xo_frequency_hz.to_be_bytes();
        Some([
            opcode::POWER_UP,
            self.function as u8 | if self.patch { 0x80 } else { 0 },
            0,
            frequency[0],
            frequency[1],
            frequency[2],
            frequency[3],
        ])
    }
}

/// Request for chip identification information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartInfo;

impl PartInfo {
    /// Encodes the request, including its opcode.
    pub const fn encode(self) -> [u8; 1] {
        [opcode::PART_INFO]
    }
}

/// Request for the currently loaded firmware information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuncInfo;

impl FuncInfo {
    /// Encodes the request, including its opcode.
    pub const fn encode(self) -> [u8; 1] {
        [opcode::FUNC_INFO]
    }
}

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
