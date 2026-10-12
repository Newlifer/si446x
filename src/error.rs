//! Driver error types.

/// An error produced by the driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error<SpiError, SdnError> {
    /// The SPI device returned an error.
    Spi(SpiError),
    /// The shutdown pin returned an error.
    Sdn(SdnError),
    /// CTS did not become ready within the configured number of attempts.
    CtsTimeout,
    /// The radio returned a malformed or unexpected response.
    InvalidResponse,
    /// The command buffer was empty or otherwise invalid.
    InvalidCommand,
    /// The driver configuration is invalid.
    InvalidConfig,
    /// A prepared command stream failed structural validation before SPI I/O.
    InvalidConfiguration(crate::configuration::ConfigurationError),
}
