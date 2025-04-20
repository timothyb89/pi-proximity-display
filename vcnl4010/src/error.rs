use std::backtrace::Backtrace;

use i2cdev::linux::LinuxI2CError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {

  #[error("i2c error: {0}")]
  I2CError(#[from] LinuxI2CError),

  #[error("unsupported product: product={product}, revision={revision}")]
  InvalidProduct {
    product: u8,
    revision: u8,
  },

  #[error("invalid LED current value: {0}")]
  InvalidLEDCurrent(u8),
}

pub type Result<T> = std::result::Result<T, Error>;
