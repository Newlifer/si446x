#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! An allocation-free driver for Silicon Labs Si446x radios.

#[cfg(feature = "async")]
pub mod asynch;
pub mod blocking;
mod device;
pub mod error;
pub mod protocol;

pub use device::CtsConfig;
pub use error::Error;
pub use protocol::{
    command::{BootFunction, PowerUp},
    response::{FuncInfo, PartInfo},
};
