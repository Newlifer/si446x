//! Blocking Si446x API

use embedded_hal::{
    delay::DelayNs,
    digital::OutputPin,
    spi::{Operation, SpiDevice},
};

use crate::{
    CtsConfig,
    device::Device,
    error::Error,
    protocol::{opcode::READ_CMD_BUFF, response::is_cts_ready},
};

/// Blocking Si446x driver.
pub struct Si446x<SPI, SDN> {
    inner: Device<SPI, SDN>,
}

impl<SPI, SDN> Si446x<SPI, SDN> {
    /// Creates a driver with the default CTS polling configuration.
    pub fn new(spi: SPI, sdn: SDN) -> Self {
        Self {
            inner: Device::new(spi, sdn),
        }
    }

    /// Releases the owned SPI device and SDN pin.
    pub fn release(self) -> (SPI, SDN) {
        self.inner.release()
    }

    /// Returns the active CTS polling configuration.
    pub const fn cts_config(&self) -> CtsConfig {
        self.inner.cts
    }

    /// Replaces the CTS polling configuration.
    pub fn set_cts_config(&mut self, config: CtsConfig) {
        self.inner.cts = config;
    }
}

impl<SPI, SDN> Si446x<SPI, SDN>
where
    SPI: SpiDevice<u8>,
    SDN: OutputPin,
{
    /// Performs a hardware reset through the SDN pin.
    pub fn reset<D: DelayNs>(
        &mut self,
        delay: &mut D,
    ) -> Result<(), Error<SPI::Error, SDN::Error>> {
        self.inner.sdn.set_high().map_err(Error::Sdn)?;
        delay.delay_us(20);
        self.inner.sdn.set_low().map_err(Error::Sdn)?;
        delay.delay_ms(20);
        Ok(())
    }

    /// Sends a raw command and reads its response after CTS becomes ready.
    pub fn command<D: DelayNs>(
        &mut self,
        request: &[u8],
        response: &mut [u8],
        delay: &mut D,
    ) -> Result<(), Error<SPI::Error, SDN::Error>> {
        if request.is_empty() {
            return Err(Error::InvalidCommand);
        }
        self.write_request(request)?;
        self.wait_cts(delay)?;
        if !response.is_empty() {
            self.read_response(response)?;
        }
        Ok(())
    }

    /// Checks command readiness once.
    pub fn poll_cts(&mut self) -> Result<bool, Error<SPI::Error, SDN::Error>> {
        let command = [READ_CMD_BUFF];
        let mut status = [0];
        self.inner
            .spi
            .transaction(&mut [Operation::Write(&command), Operation::Read(&mut status)])
            .map_err(Error::Spi)?;
        Ok(is_cts_ready(status[0]))
    }

    /// Polls command readiness until CTS is ready or the configured limit is reached.
    pub fn wait_cts<D: DelayNs>(
        &mut self,
        delay: &mut D,
    ) -> Result<(), Error<SPI::Error, SDN::Error>> {
        let config = self.inner.cts;
        if config.max_attempts == 0 {
            return Err(Error::InvalidConfig);
        }
        for attempt in 0..config.max_attempts {
            if self.poll_cts()? {
                return Ok(());
            }
            if attempt + 1 < config.max_attempts {
                delay.delay_ns(config.poll_interval_ns);
            }
        }
        Err(Error::CtsTimeout)
    }

    fn write_request(&mut self, request: &[u8]) -> Result<(), Error<SPI::Error, SDN::Error>> {
        self.inner.spi.write(request).map_err(Error::Spi)
    }

    fn read_response(&mut self, response: &mut [u8]) -> Result<(), Error<SPI::Error, SDN::Error>> {
        let command = [READ_CMD_BUFF];
        let mut status = [0];
        self.inner
            .spi
            .transaction(&mut [
                Operation::Write(&command),
                Operation::Read(&mut status),
                Operation::Read(response),
            ])
            .map_err(Error::Spi)?;
        if is_cts_ready(status[0]) {
            Ok(())
        } else {
            Err(Error::InvalidResponse)
        }
    }
}
