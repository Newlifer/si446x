//! Shared raw and typed Si446x command protocol definitions.
//!
//! Boot and identification formats follow Silicon Labs' [revC2A command API]
//! (archived copy of the vendor documentation).
//!
//! [revC2A command API]: https://motla.github.io/EZRadioPRO_REVC2/Si4463/revC2A/index_bootloader.html
//!
//! Property, GPIO and state formats follow the vendor's [application API].
//!
//! [application API]: https://motla.github.io/EZRadioPRO_REVC2/Si4463/revC2A/index_ezradiopro.html

pub mod command;
pub mod gpio;
pub mod opcode;
pub mod response;
pub mod state;
