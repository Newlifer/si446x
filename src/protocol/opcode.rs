//! Command opcodes defined by the Si446x command API.

/// Return part identification information.
pub const PART_INFO: u8 = 0x01;
/// Boot the radio after power-on reset.
pub const POWER_UP: u8 = 0x02;
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
/// Start packet transmission.
pub const START_TX: u8 = 0x31;
/// Start packet reception.
pub const START_RX: u8 = 0x32;
/// Read CTS and the response of the preceding command.
pub const READ_CMD_BUFF: u8 = 0x44;
