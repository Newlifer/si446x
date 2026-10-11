//! Raw and typed command requests.

use super::opcode;

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
