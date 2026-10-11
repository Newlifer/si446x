//! Raw response helpers and typed command responses.

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
