/// Configuration for polling the command-ready (CTS) byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CtsConfig {
    /// Maximum number of `READ_CMD_BUFF` attempts.
    pub max_attempts: u16,
    /// Delay between attempts, in nanoseconds.
    pub poll_interval_ns: u32,
}

impl Default for CtsConfig {
    fn default() -> Self {
        Self {
            max_attempts: 1000,
            poll_interval_ns: 100_000,
        }
    }
}

pub(crate) struct Device<SPI, SDN> {
    pub(crate) spi: SPI,
    pub(crate) sdn: SDN,
    pub(crate) cts: CtsConfig,
}

impl<SPI, SDN> Device<SPI, SDN> {
    pub(crate) fn new(spi: SPI, sdn: SDN) -> Self {
        Self {
            spi,
            sdn,
            cts: CtsConfig::default(),
        }
    }

    pub(crate) fn release(self) -> (SPI, SDN) {
        (self.spi, self.sdn)
    }
}
