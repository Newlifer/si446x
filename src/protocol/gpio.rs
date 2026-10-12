//! GPIO, NIRQ and SDO pin configuration and readback.
//!
//! Configure GPIO0 as a direct TX data input, GPIO1 as CTS, and NIRQ as the
//! interrupt output using the blocking API. The microcontroller's `tx_data`
//! output must be physically connected to GPIO0 on the radio.
//!
//! ```no_run
//! use embedded_hal::digital::OutputPin;
//! use si446x::{GpioPinConfig, PinConfig};
//! # use embedded_hal::{delay::DelayNs, spi::SpiDevice};
//! # use si446x::blocking::Si446x;
//! # fn example<SPI, SDN, D, TX, E>(
//! #     radio: &mut Si446x<SPI, SDN>,
//! #     mut delay: D,
//! #     mut tx_data: TX,
//! # ) -> Result<(), E>
//! # where
//! #     SPI: SpiDevice<u8>,
//! #     SDN: OutputPin,
//! #     D: DelayNs,
//! #     TX: OutputPin,
//! #     E: From<si446x::Error<SPI::Error, SDN::Error>> + From<TX::Error>,
//! # {
//! // radio, delay and tx_data are created using the microcontroller HAL.
//! // The radio must already be powered up.
//! radio.gpio_pin_cfg(
//!     GpioPinConfig {
//!         gpio: [
//!             PinConfig::INPUT,
//!             PinConfig::CTS,
//!             PinConfig::UNCHANGED,
//!             PinConfig::UNCHANGED,
//!         ],
//!         nirq: PinConfig::NIRQ,
//!         sdo: PinConfig::SDO,
//!         ..GpioPinConfig::default()
//!     },
//!     &mut delay,
//! )?;
//!
//! // Drive the microcontroller pin connected to GPIO0 on the radio.
//! tx_data.set_high()?;
//! tx_data.set_low()?;
//! # Ok(())
//! # }
//! ```
//!
//! This command assigns pin functions only. OOK transmission also requires
//! configuring the radio properties for Direct Mode and entering TX state.

use super::opcode;

/// Mode and pull-up control for one radio pin.
///
/// Mode codes depend on the pin: mode 39 means interrupt output on NIRQ,
/// but hop-table-wrap output on GPIO0–3. The encoder validates each pin's set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PinConfig {
    /// Mode code from the GPIO_PIN_CFG command table (six bits).
    pub mode: u8,
    /// Enable the weak pull-up resistor.
    pub pull_up: bool,
}

impl PinConfig {
    /// Preserve the pin's current behavior.
    pub const UNCHANGED: Self = Self {
        mode: 0,
        pull_up: false,
    };
    /// Disable input and output drivers.
    pub const TRISTATE: Self = Self {
        mode: 1,
        pull_up: false,
    };
    /// Drive the pin low.
    pub const LOW: Self = Self {
        mode: 2,
        pull_up: false,
    };
    /// Drive the pin high.
    pub const HIGH: Self = Self {
        mode: 3,
        pull_up: false,
    };
    /// Configure a CMOS input, e.g. for direct TX data.
    pub const INPUT: Self = Self {
        mode: 4,
        pull_up: false,
    };
    /// Output the command-ready signal.
    pub const CTS: Self = Self {
        mode: 8,
        pull_up: false,
    };
    /// Output SPI serial data; intended for the SDO pin.
    pub const SDO: Self = Self {
        mode: 11,
        pull_up: false,
    };
    /// Output the active-low interrupt signal; intended for NIRQ.
    pub const NIRQ: Self = Self {
        mode: 39,
        pull_up: false,
    };

    const fn encode(self) -> u8 {
        self.mode | if self.pull_up { 0x40 } else { 0 }
    }
}

/// Common output drive strength for GPIO, NIRQ and SDO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum DriveStrength {
    /// Highest drive strength.
    #[default]
    High = 0,
    /// Medium-high drive strength.
    MediumHigh = 1,
    /// Medium-low drive strength.
    MediumLow = 2,
    /// Lowest drive strength.
    Low = 3,
}

impl DriveStrength {
    pub(crate) const fn from_bits(value: u8) -> Self {
        match (value >> 5) & 3 {
            0 => Self::High,
            1 => Self::MediumHigh,
            2 => Self::MediumLow,
            _ => Self::Low,
        }
    }
}

/// Parameters for `GPIO_PIN_CFG`.
///
/// Default preserves pin modes and selects highest output drive strength.
/// Changing SDO away from its SPI function may prevent further SPI readback;
/// restore it or reset the radio before relying on the command transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GpioPinConfig {
    /// Configurations for GPIO0 through GPIO3.
    pub gpio: [PinConfig; 4],
    /// Interrupt pin configuration.
    pub nirq: PinConfig,
    /// SPI serial data output pin configuration.
    pub sdo: PinConfig,
    /// Drive strength shared by all output pins.
    pub drive_strength: DriveStrength,
}

impl GpioPinConfig {
    /// Encodes the request or rejects a mode unsupported on its selected pin.
    ///
    /// Checks the documented mode sets, but not the availability of a signal
    /// on a particular chip revision or its required property configuration.
    pub fn encode(self) -> Option<[u8; 8]> {
        let bytes = [
            opcode::GPIO_PIN_CFG,
            self.gpio[0].encode(),
            self.gpio[1].encode(),
            self.gpio[2].encode(),
            self.gpio[3].encode(),
            self.nirq.encode(),
            self.sdo.encode(),
            (self.drive_strength as u8) << 5,
        ];
        if self.gpio.iter().any(|pin| pin.mode > 63)
            || self.nirq.mode > 63
            || self.sdo.mode > 63
            || !valid_request(&bytes)
        {
            None
        } else {
            Some(bytes)
        }
    }
}

pub(crate) fn valid_request(bytes: &[u8]) -> bool {
    bytes.len() == 8
        && bytes[1..7].iter().all(|byte| byte & 0x80 == 0)
        && bytes[1..5]
            .iter()
            .all(|byte| matches!(byte & 0x3f, 0..=29 | 31..=39))
        && matches!(bytes[5] & 0x3f, 0..=4 | 7 | 8 | 11 | 12 | 15..=27 | 29 | 31 | 39)
        && matches!(bytes[6] & 0x3f, 0..=5 | 7 | 8 | 11 | 12 | 14..=27)
        && bytes[7] & !0x60 == 0
}
