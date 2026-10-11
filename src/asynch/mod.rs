//! Asynchronous Si446x API based on `embedded-hal-async` 1.0.

use embedded_hal::{digital::OutputPin, spi::Operation};
use embedded_hal_async::{delay::DelayNs, spi::SpiDevice};

use crate::{
    CtsConfig, FuncInfo, PartInfo, PowerUp,
    device::Device,
    error::Error,
    protocol::{command, opcode::READ_CMD_BUFF, response::is_cts_ready},
};

/// Asynchronous Si446x driver.
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
    /// Boots the selected firmware image after hardware reset.
    ///
    /// Returns `InvalidConfig` without SPI I/O for an oscillator frequency
    /// outside 25–32 MHz. Waits for command completion, but does not configure
    /// radio properties or download a patch. See [`Self::command`] for error
    /// recovery and cancellation behavior.
    pub async fn power_up<D: DelayNs>(
        &mut self,
        config: PowerUp,
        delay: &mut D,
    ) -> Result<(), Error<SPI::Error, SDN::Error>> {
        let request = config.encode().ok_or(Error::InvalidConfig)?;
        self.command(&request, &mut [], delay).await
    }

    /// Reads chip identification information.
    ///
    /// Has the same cancellation behavior as [`Self::command`].
    pub async fn part_info<D: DelayNs>(
        &mut self,
        delay: &mut D,
    ) -> Result<PartInfo, Error<SPI::Error, SDN::Error>> {
        let mut bytes = [0; PartInfo::LEN];
        self.command(&command::PartInfo.encode(), &mut bytes, delay)
            .await?;
        Ok(PartInfo::from_bytes(bytes))
    }

    /// Reads the currently loaded firmware information.
    ///
    /// Has the same cancellation behavior as [`Self::command`].
    pub async fn func_info<D: DelayNs>(
        &mut self,
        delay: &mut D,
    ) -> Result<FuncInfo, Error<SPI::Error, SDN::Error>> {
        let mut bytes = [0; FuncInfo::LEN];
        self.command(&command::FuncInfo.encode(), &mut bytes, delay)
            .await?;
        Ok(FuncInfo::from_bytes(bytes))
    }

    /// Performs a hardware reset through the SDN pin.
    ///
    /// If cancelled, the reset sequence may be incomplete (including SDN left
    /// high). Complete a new reset before using the radio again.
    pub async fn reset<D: DelayNs>(
        &mut self,
        delay: &mut D,
    ) -> Result<(), Error<SPI::Error, SDN::Error>> {
        self.inner.sdn.set_high().map_err(Error::Sdn)?;
        delay.delay_us(20).await;
        self.inner.sdn.set_low().map_err(Error::Sdn)?;
        delay.delay_ms(20).await;
        Ok(())
    }

    /// Sends a raw command and reads its response after CTS becomes ready.
    ///
    /// Waits for CTS before writing the request and again before reading its
    /// response, including for commands with no response bytes. The attempt
    /// limit applies independently to each wait. Invalid inputs cause no SPI I/O.
    /// After a timeout, a subsequent call checks CTS again before sending.
    /// An error does not undo a command already sent, and response contents are
    /// unspecified on error.
    ///
    /// # Cancellation
    ///
    /// Cancellation does not undo a request already sent. If cancelled during
    /// a polling delay, the next call waits for CTS before sending, but the
    /// previous response is abandoned. Cancellation during SPI I/O depends on
    /// the HAL's cancellation guarantees: the transfer may be partial or CS may
    /// remain asserted. Restore the SPI device as required by the HAL, then
    /// reset and reinitialize the radio if its state is uncertain. The same
    /// recovery applies after SPI errors. This driver does not cache readiness.
    pub async fn command<D: DelayNs>(
        &mut self,
        request: &[u8],
        response: &mut [u8],
        delay: &mut D,
    ) -> Result<(), Error<SPI::Error, SDN::Error>> {
        if request.is_empty() {
            return Err(Error::InvalidCommand);
        }
        self.wait_cts(delay).await?;
        self.write_request(request).await?;
        self.wait_cts(delay).await?;
        if !response.is_empty() {
            self.read_response(response).await?;
        }
        Ok(())
    }

    /// Checks command readiness once.
    pub async fn poll_cts(&mut self) -> Result<bool, Error<SPI::Error, SDN::Error>> {
        let command = [READ_CMD_BUFF];
        let mut status = [0];
        self.inner
            .spi
            .transaction(&mut [Operation::Write(&command), Operation::Read(&mut status)])
            .await
            .map_err(Error::Spi)?;
        Ok(is_cts_ready(status[0]))
    }

    /// Polls command readiness until CTS is ready or the configured limit is reached.
    pub async fn wait_cts<D: DelayNs>(
        &mut self,
        delay: &mut D,
    ) -> Result<(), Error<SPI::Error, SDN::Error>> {
        let config = self.inner.cts;
        if config.max_attempts == 0 {
            return Err(Error::InvalidConfig);
        }
        for attempt in 0..config.max_attempts {
            if self.poll_cts().await? {
                return Ok(());
            }
            if attempt + 1 < config.max_attempts {
                delay.delay_ns(config.poll_interval_ns).await;
            }
        }
        Err(Error::CtsTimeout)
    }

    async fn write_request(&mut self, request: &[u8]) -> Result<(), Error<SPI::Error, SDN::Error>> {
        self.inner.spi.write(request).await.map_err(Error::Spi)
    }

    async fn read_response(
        &mut self,
        response: &mut [u8],
    ) -> Result<(), Error<SPI::Error, SDN::Error>> {
        let command = [READ_CMD_BUFF];
        let mut status = [0];
        self.inner
            .spi
            .transaction(&mut [
                Operation::Write(&command),
                Operation::Read(&mut status),
                Operation::Read(response),
            ])
            .await
            .map_err(Error::Spi)?;
        if is_cts_ready(status[0]) {
            Ok(())
        } else {
            Err(Error::InvalidResponse)
        }
    }
}
