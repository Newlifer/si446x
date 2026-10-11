//! Shared raw and typed Si446x command protocol definitions.
//!
//! Boot and identification formats follow Silicon Labs' [revC2A command API]
//! (archived copy of the vendor documentation).
//!
//! [revC2A command API]: https://motla.github.io/EZRadioPRO_REVC2/Si4463/revC2A/index_bootloader.html

pub mod command;
pub mod opcode;
pub mod response;
