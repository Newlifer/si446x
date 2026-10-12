//! Operating states used by the radio command API.

/// Target state for `CHANGE_STATE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum State {
    /// Retain the current operating state.
    NoChange = 0,
    /// Sleep or standby, according to the 32 kHz clock configuration.
    Sleep = 1,
    /// SPI command processing with the boot clock active.
    SpiActive = 2,
    /// Crystal oscillator running and ready for tuning.
    Ready = 3,
    /// Tune for transmission.
    TxTune = 5,
    /// Tune for reception.
    RxTune = 6,
    /// Transmit using previously configured parameters.
    Tx = 7,
    /// Receive using previously configured parameters.
    Rx = 8,
}

impl State {
    /// Decodes a current-state value, accepting READY2 as READY.
    ///
    /// Unknown values (including zero, which is only a target) return `None`.
    pub const fn from_current(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Sleep),
            2 => Some(Self::SpiActive),
            3 | 4 => Some(Self::Ready),
            5 => Some(Self::TxTune),
            6 => Some(Self::RxTune),
            7 => Some(Self::Tx),
            8 => Some(Self::Rx),
            _ => None,
        }
    }
}
