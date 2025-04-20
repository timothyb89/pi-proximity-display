use std::time::Instant;
use std::{ops::Range, path::PathBuf, thread, time::Duration};
use std::str::FromStr;

use clap::Parser;
use color_eyre::eyre::eyre;
use color_eyre::Result;
use display::{Display, DisplayPowerMode};
use tracing::info;
use vcnl4010::{ProximitySensor, SensorCommand};

mod display;

fn parse_range(s: &str) -> Result<Range<u32>, String> {
    let parts: Vec<&str> = s.split("..").collect();
    if parts.len() != 2 {
        return Err(format!("Invalid range format: {}", s));
    }

    let start = u32::from_str(parts[0])
        .map_err(|_| format!("Invalid start value: {}", parts[0]))?;
    let end = u32::from_str(parts[1])
        .map_err(|_| format!("Invalid end value: {}", parts[1]))?;

    Ok(start..end)
}

#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// An alternate I2C device to use
    #[arg(short = 'i', long, default_value = "/dev/i2c-1")]
    i2c_device: PathBuf,

    /// A current value for the proximity sensor LED in mA between 0 and 200.
    #[arg(short = 'c', long, default_value = "200")]
    proximity_led_current: u16,

    /// An explicit display to manage, otherwise the first display with a
    /// controllable backlight is used.
    #[arg(long)]
    display_name: Option<String>,

    /// A range of proximity values in the format `min..max` such that 'min' is
    /// the proximity value below which the display should turn off, and 'max'
    /// is the value above which the display should turn on. The range in
    /// between is used for hysteresis.
    #[arg(long, value_parser = parse_range)]
    proximity_range: Range<u32>,

    /// Amount of time to keep the display on once detected and then cleared.
    #[arg(long, value_parser = humantime::parse_duration, default_value = "20s")]
    proximity_hold: Duration,

    /// A range of ambient light levels in the form 'min..max', inclusive.
    /// Values in this range will be linearly mapped to the values in
    /// `--brightness-range` to calculate the desired brightness. If unset,
    /// brightness control is disabled.
    #[arg(long, value_parser = parse_range)]
    ambient_light_range: Option<Range<u32>>,

    /// A range of display brightness to output in the form of min..max
    /// (inclusive). Ambient light values will be linearly mapped to this scale.
    /// Values may be excluded to set effective minimum and maximum brightness
    /// levels. If unset, brightness control is disabled.
    #[arg(long, value_parser = parse_range)]
    brightness_range: Option<Range<u32>>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum State {
    Detected,
    Cleared,
    ClearedTransitioning(Instant),
}

impl State {
    fn update(&self, args: &Args, proximity: u32) -> Option<State> {
        // if the detection threshold is exceeded, it's always detected
        if self != &State::Detected && proximity >= args.proximity_range.end {
            return Some(State::Detected);
        }

        match self {
            State::Detected if proximity <= args.proximity_range.start => {
                return Some(State::ClearedTransitioning(Instant::now()));
            },
            State::ClearedTransitioning(i) if i.elapsed() > args.proximity_hold => {
                return Some(State::Cleared);
            },
            _ => (),
        }

        None
    }

    fn transition(&self, display: &mut Display) -> Result<()> {
        match self {
            State::Detected => display.set_power(DisplayPowerMode::On)?,
            State::Cleared => display.set_power(DisplayPowerMode::Off)?,
            _ => (),
        }

        Ok(())
    }
}

fn select_display(args: &Args) -> Result<Display> {
    let displays = display::list_displays()?;
    info!("found displays: {:?}", displays);

    if let Some(display_name) = &args.display_name {
        let display = displays
            .into_iter()
            .find(|d| d.name.eq_ignore_ascii_case(display_name));

        if let Some(display) = display {
            Ok(display)
        } else {
            return Err(eyre!("requested display {display_name} not found"));
        }
    } else {
        if let Some(display) = displays.into_iter().next() {
            Ok(display)
        } else {
            return Err(eyre!("no displays found"));
        }
    }
}

fn map_ambient_to_display_brightness(
    ambient: u32,
    ambient_light_range: &Range<u32>,
    brightness_range: &Range<u32>
) -> u32 {
    let ambient_start = ambient_light_range.start as f32;
    let brightness_start = brightness_range.start as f32;
    let ambient_span = (ambient_light_range.end - ambient_light_range.start) as f32;
    let brightness_span = (brightness_range.end - brightness_range.start) as f32;

    if ambient_span == 0.0 || brightness_span == 0.0 {
        return brightness_range.start;
    }

    let mapped = brightness_start + ((ambient as f32 - ambient_start) * brightness_span) / ambient_span;

    (mapped.round() as u32).clamp(brightness_range.start, brightness_range.end)
}

fn main() -> Result<()> {
    install_tracing();
    color_eyre::install()?;

    let args = Args::parse();

    let mut selected_display = select_display(&args)?;
    info!("selected display: {selected_display:?}");

    let mut sensor = ProximitySensor::try_new(&args.i2c_device)?;
    let product = sensor.read_product()?.verify()?;
    info!("product: {product:?}");

    let command = sensor.read_command_register()?;
    info!("command: {command:?}");

    sensor.set_command_register(
        SensorCommand::new()
            .with_self_timed_enabled(true)
            .with_proximity_enabled(true)
            .with_ambient_light_enabled(true),
    )?;

    sensor.set_led_current_ma(args.proximity_led_current)?;
    info!("current: {:?}", sensor.read_led_current()?);

    let command = sensor.read_command_register()?;
    info!("updated command: {command:?}");

    let mut count: usize = 0;
    let mut state = State::Cleared;
    let mut brightness: u32 = 0;

    loop {
        let proximity_val = sensor.read_proximity()? as u32;
        let ambient_light_val = sensor.read_ambient_light()? as u32;

        if let Some(new) = state.update(&args, proximity_val) {
            info!("new state: {new:?}");
            state = new;
            state.transition(&mut selected_display)?;
        }

        if let (Some(ambient), Some(display)) = (&args.ambient_light_range, &args.brightness_range) {
            let new_brightness = map_ambient_to_display_brightness(ambient_light_val, ambient, display);
            if new_brightness != brightness {
                brightness = new_brightness;
                selected_display.set_brightness(brightness)?;
                info!("set brightness to {brightness} (ambient: {ambient_light_val})")
            }
        }

        count += 1;
        if count % 20 == 0 {
            // log the current data every 5s
            info!("proximity: {proximity_val} | ambient: {ambient_light_val}");
        }

        thread::sleep(Duration::from_millis(250));
    }
}

pub fn install_tracing() {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};

    let fmt_layer = fmt::layer().with_target(true).with_writer(std::io::stderr);

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(ErrorLayer::default())
        .init();
}
