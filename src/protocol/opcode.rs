//! Command opcodes defined by the Si446x command API (AN633).
//!
//! Revision- and part-specific commands are gated behind cargo features:
//!
//! - `rev-c2`: commands added in firmware revC2A / A2A.
//! - `si4467-68`: commands specific to the Si4467 and Si4468.

// Boot commands

/// Power up the device and select the operational mode and functionality.
pub const POWER_UP: u8 = 0x02;
/// Download a firmware patch image (revC2A / A2A only).
#[cfg(feature = "rev-c2")]
pub const PATCH_IMAGE: u8 = 0x04;

// Common commands

/// Perform no operation; may be used to check communication with the device.
pub const NOP: u8 = 0x00;
/// Return part identification information.
pub const PART_INFO: u8 = 0x01;
/// Return firmware and patch information.
pub const FUNC_INFO: u8 = 0x10;
/// Write one or more radio properties.
pub const SET_PROPERTY: u8 = 0x11;
/// Read one or more radio properties.
pub const GET_PROPERTY: u8 = 0x12;
/// Configure the GPIO pins.
pub const GPIO_PIN_CFG: u8 = 0x13;
/// Read or reset FIFO information.
pub const FIFO_INFO: u8 = 0x15;
/// Read and optionally clear pending interrupts.
pub const GET_INT_STATUS: u8 = 0x20;
/// Request the current device state and channel.
pub const REQUEST_DEVICE_STATE: u8 = 0x33;
/// Manually switch the chip to a desired operating state.
pub const CHANGE_STATE: u8 = 0x34;
/// Switch to high- or low-temperature settings and recalibrate (revC2A / A2A only).
#[cfg(feature = "rev-c2")]
pub const OFFLINE_RECAL: u8 = 0x38;
/// Read CTS and the response of the preceding command.
pub const READ_CMD_BUFF: u8 = 0x44;
/// Read the fast response registers starting with FRR_A (no CTS polling needed).
pub const FRR_A_READ: u8 = 0x50;
/// Read the fast response registers starting with FRR_B (no CTS polling needed).
pub const FRR_B_READ: u8 = 0x51;
/// Read the fast response registers starting with FRR_C (no CTS polling needed).
pub const FRR_C_READ: u8 = 0x53;
/// Read the fast response registers starting with FRR_D (no CTS polling needed).
pub const FRR_D_READ: u8 = 0x57;

// Image rejection calibration commands

/// Perform image rejection calibration.
pub const IRCAL: u8 = 0x17;
/// Perform manual image rejection calibration.
pub const IRCAL_MANUAL: u8 = 0x1A;

// TX commands

/// Start packet transmission.
pub const START_TX: u8 = 0x31;
/// Start transmission in MFSK mode (Si4467 / Si4468 revA2A only).
#[cfg(feature = "si4467-68")]
pub const START_MFSK: u8 = 0x35;
/// Hop to a new frequency while in TX.
pub const TX_HOP: u8 = 0x37;
/// Write data bytes to the TX FIFO.
pub const WRITE_TX_FIFO: u8 = 0x66;

// RX commands

/// Return the length of the variable field in the last received packet.
pub const PACKET_INFO: u8 = 0x16;
/// Read and optionally clear modem interrupts.
pub const GET_MODEM_STATUS: u8 = 0x22;
/// Start packet reception.
pub const START_RX: u8 = 0x32;
/// Manually hop to a new frequency while in RX.
pub const RX_HOP: u8 = 0x36;
/// Read data bytes from the RX FIFO.
pub const READ_RX_FIFO: u8 = 0x77;

// Advanced commands

/// Perform a conversion with the auxiliary ADC (temperature, battery, GPIO).
pub const GET_ADC_READING: u8 = 0x14;
/// Configure the chip for a specific protocol, e.g. IEEE 802.15.4g
/// (Si4467 / Si4468 revA2A only).
#[cfg(feature = "si4467-68")]
pub const PROTOCOL_CFG: u8 = 0x18;
/// Read and optionally clear packet handler interrupts.
pub const GET_PH_STATUS: u8 = 0x21;
/// Read and optionally clear chip interrupts.
pub const GET_CHIP_STATUS: u8 = 0x23;
