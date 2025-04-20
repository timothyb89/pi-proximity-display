use std::path::Path;

use bitfield_struct::bitfield;
use i2cdev::{core::*, linux::LinuxI2CDevice};

mod error;

use error::{Error, Result};

pub const ADDR: u16 = 0x13; // hard-coded
pub const REG_COMMAND: u8 = 0x80;
pub const REG_PRODUCT_ID: u8 = 0x81;
pub const REG_PROX_RATE: u8 = 0x82; // note: not supported on vcnl4000
pub const REG_LED_CURRENT: u8 = 0x83;
pub const REG_AMBIENT_LIGHT: u8 = 0x84;
pub const REG_AMBIENT_LIGHT_RESULT_HIGH: u8 = 0x85; // (2 bytes)
pub const REG_AMBIENT_LIGHT_RESULT_LOW: u8 = 0x86; // (2 bytes)
pub const REG_PROXIMITY_RESULT_HIGH: u8 = 0x87; // (2 bytes)
pub const REG_PROXIMITY_RESULT_LOW: u8 = 0x88; // (2 bytes)

#[bitfield(u8)]
pub struct SensorCommand {
    /// If set, enables self timed measurements. No measurements will be
    /// available unless either this is set, or an on demand measurement is
    /// triggered. `proximity_enabled` and/or `ambient_light_enabled` are also
    /// required for initial measurements.
    pub self_timed_enabled: bool,

    /// If set, enables proximity measurements.
    pub proximity_enabled: bool,

    /// If set, enables ambient light measurements.
    pub ambient_light_enabled: bool,

    /// If set, starts an on-demand proximity measurement. Cleared once the
    /// value has been read.
    pub proximity_on_demand: bool,

    /// When set, starts an on-demand ambient light measurement. Cleared once
    /// the value is read from the result registers.
    pub ambient_light_on_demand: bool,

    /// If set, proximity data is available for reading. Read only; any set
    /// values will be ignored.
    pub proximity_data_ready: bool,

    /// If set, ambient light data is available for reading. Read only; any set
    /// values will be ignored.
    pub ambient_light_data_ready: bool,

    /// Read only; any set values will be ignored.
    pub config_lock: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ProximityMeasurementFrequency {
    /// 1.95 samples/sec
    M1_95,

    /// 3.90625 samples/sec
    M3_90625,

    /// 7.8125 samples/sec
    M7_8125,

    /// 16.625 samples/sec
    M16_625,

    /// 31.25 samples/sec
    M31_25,

    /// 62.5 samples/sec
    M62_5,

    /// 125 samples/sec
    M125,

    /// 250 samples/sec
    M250
}

impl ProximityMeasurementFrequency {
    pub fn value(self) -> u8 {
        match self {
            ProximityMeasurementFrequency::M1_95 => 0,
            ProximityMeasurementFrequency::M3_90625 => 1,
            ProximityMeasurementFrequency::M7_8125 => 2,
            ProximityMeasurementFrequency::M16_625 => 3,
            ProximityMeasurementFrequency::M31_25 => 4,
            ProximityMeasurementFrequency::M62_5 => 5,
            ProximityMeasurementFrequency::M125 => 6,
            ProximityMeasurementFrequency::M250 => 7,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AmbientLightMeasurementFrequency {
    /// 1 sample/sec
    M1,

    /// 2 samples/sec
    M2,

    /// 3 samples/sec
    M3,

    /// 4 samples/sec
    M4,

    /// 5 samples/sec
    M5,

    /// 6 samples/sec
    M6,

    /// 8 samples/sec
    M8,

    /// 10 samples/sec
    M10
}

impl AmbientLightMeasurementFrequency {
    pub fn value(self) -> u8 {
        match self {
            AmbientLightMeasurementFrequency::M1 => 0,
            AmbientLightMeasurementFrequency::M2 => 1,
            AmbientLightMeasurementFrequency::M3 => 2,
            AmbientLightMeasurementFrequency::M4 => 3,
            AmbientLightMeasurementFrequency::M5 => 4,
            AmbientLightMeasurementFrequency::M6 => 5,
            AmbientLightMeasurementFrequency::M8 => 6,
            AmbientLightMeasurementFrequency::M10 => 7,
        }
    }
}

#[bitfield(u8)]
pub struct ProductInfo {
    #[bits(4)]
    pub revision: u8,

    #[bits(4)]
    pub product: u8,
}

impl ProductInfo {
    /// Checks that this ProductInfo matches the documented constant values.
    pub fn verify(self) -> Result<Self> {
        if !(self.product() == 2 && self.revision() == 1) {
            return Err(Error::InvalidProduct {
                product: self.product(),
                revision: self.revision()
            })
        }

        Ok(self)
    }
}

pub struct ProximitySensor {
    device: LinuxI2CDevice,
}

#[bitfield(u8)]
pub struct LEDCurrent {
    #[bits(6)]
    current: u8,

    #[bits(2)]
    fuse_prog_id: u8
}

impl LEDCurrent {
    pub fn verify(self) -> Result<Self> {
        if self.current() > 20 {
            return Err(Error::InvalidLEDCurrent(self.current()));
        }

        Ok(self)
    }

    pub fn with_current_ma(self, ma: u16) -> Self {
        let val = (ma / 10).clamp(0, 20) as u8;

        self.with_current(val)
    }

    pub fn to_milliamps(self) -> u16 {
        self.current() as u16 * 10
    }
}

impl ProximitySensor {
    pub fn try_new(i2c_device: impl AsRef<Path>) -> Result<Self> {
        let device = LinuxI2CDevice::new(i2c_device, ADDR)?;

        Ok(ProximitySensor {
            device,
        })
    }

    pub fn read_command_register(&mut self) -> Result<SensorCommand> {
        let byte = self.device.smbus_read_byte_data(REG_COMMAND)?;
        let parsed = SensorCommand::from_bits(byte);

        Ok(parsed)
    }

    pub fn set_command_register(&mut self, command: SensorCommand) -> Result<()> {
        self.device.smbus_write_byte_data(REG_COMMAND, command.into_bits())?;

        Ok(())
    }

    /// Reads the product ID and revision, returning the result as a tuple of
    /// (id, rev).
    pub fn read_product(&mut self) -> Result<ProductInfo> {
        let byte = self.device.smbus_read_byte_data(REG_PRODUCT_ID)?;
        let pi = ProductInfo::from_bits(byte);

        Ok(pi)
    }

    pub fn read_ambient_light(&mut self) -> Result<u16> {
        let high = self.device.smbus_read_byte_data(REG_AMBIENT_LIGHT_RESULT_HIGH)?;
        let low = self.device.smbus_read_byte_data(REG_AMBIENT_LIGHT_RESULT_LOW)?;

        let val = ((high as u16) << 8) | (low as u16);

        Ok(val)
    }

    /// Reads the latest proximity value. This is unitless and depends on the
    /// configured LED current, among other factors.
    pub fn read_proximity(&mut self) -> Result<u16> {
        let high = self.device.smbus_read_byte_data(REG_PROXIMITY_RESULT_HIGH)?;
        let low = self.device.smbus_read_byte_data(REG_PROXIMITY_RESULT_LOW)?;

        let val = ((high as u16) << 8) | (low as u16);

        Ok(val)
    }

    pub fn read_led_current(&mut self) -> Result<LEDCurrent> {
        let byte = self.device.smbus_read_byte_data(REG_LED_CURRENT)?;
        let c = LEDCurrent::from_bits(byte);

        Ok(c)
    }

    /// Sets current in milliamps. Values are clamped to 0-200mA.
    pub fn set_led_current_ma(&mut self, current_ma: u16) -> Result<()> {
        let c = LEDCurrent::new().with_current_ma(current_ma);
        self.device.smbus_write_byte_data(REG_LED_CURRENT, c.into_bits())?;

        Ok(())
    }
}
