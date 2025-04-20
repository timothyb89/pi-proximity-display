use std::{
    ffi::OsStr, fs, path::{Path, PathBuf}, process::{Command, Output}
};

use color_eyre::{eyre::eyre, Result};
use serde_derive::Deserialize;
use tracing::{info, warn};

#[derive(Debug)]
pub struct Display {
    pub sysfs_path: PathBuf,
    pub name: String,
    pub brightness: u32,
    pub max_brightness: u32,
}

impl Display {
    pub fn try_from_path(p: impl AsRef<Path>) -> Result<Display> {
        info!("trying display: {}", p.as_ref().display());
        let p = p.as_ref();

        let name = fs::read_to_string(p.join("display_name"))?.trim().to_string();
        let brightness: u32 = fs::read_to_string(p.join("brightness"))?.trim().parse()?;
        let max_brightness: u32 = fs::read_to_string(p.join("max_brightness"))?.trim().parse()?;

        Ok(Display {
            name,
            brightness,
            max_brightness,
            sysfs_path: p.to_path_buf(),
        })
    }

    /// Rereads the display from disk.
    pub fn reload(&self) -> Result<Display> {
        Display::try_from_path(&self.sysfs_path)
    }

    pub fn set_brightness(&mut self, brightness: u32) -> Result<()> {
        let range = 0..=self.max_brightness;
        if !range.contains(&brightness) {
            return Err(eyre!(
                "brightness out of range: {brightness} not within 0 <= v <= {}",
                self.max_brightness
            ));
        }

        fs::write(self.sysfs_path.join("brightness"), brightness.to_string())?;
        self.brightness = brightness;

        Ok(())
    }

    pub fn set_power(&mut self, mode: DisplayPowerMode) -> Result<()> {
        match mode {
            DisplayPowerMode::On => wlopm_on(&self.name),
            DisplayPowerMode::Off => wlopm_off(&self.name),
        }
    }
}

pub fn list_displays() -> Result<Vec<Display>> {
    let mut displays = Vec::new();

    for child in fs::read_dir("/sys/class/backlight")? {
        let child = child?;

        match Display::try_from_path(child.path()) {
            Ok(display) => displays.push(display),
            Err(e) => warn!(
                "could not read display at {}, skipping: {e}",
                child.path().display()
            ),
        }
    }

    Ok(displays)
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum DisplayPowerMode {
    On,
    Off
}

// #[derive(Debug, Deserialize)]
// struct WLOPMOutput {
//     output: String,
//     power_mode: DisplayPowerMode,
// }

fn wlopm<I, S>(args: I) -> Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("wlopm")
        .args(args)
        .output()?;

    Ok(output)
}

fn wlopm_check_error(stderr: &[u8]) -> Result<()> {
    // annoyingly, wlopm doesn't return nonzero exit codes if an error occurs :/
    if !stderr.is_empty() {
        let err = String::from_utf8_lossy(&stderr);
        if err.starts_with("ERROR") {
            return Err(eyre!("wlopm returned an error: {err}"));
        }
    }

    Ok(())
}

fn wlopm_on(display_name: impl AsRef<str>) -> Result<()> {
    let output = wlopm(&["--on", display_name.as_ref()])?;
    wlopm_check_error(&output.stderr)?;

    Ok(())
}

fn wlopm_off(display_name: impl AsRef<str>) -> Result<()> {
    let output = wlopm(&["--off", display_name.as_ref()])?;
    wlopm_check_error(&output.stderr)?;

    Ok(())
}
