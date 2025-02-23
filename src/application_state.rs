use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::BitXor;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use gpio_cdev::{Chip, chips, LineDirection, LineHandle, LineInfo, LineRequestFlags};
use gpio_cdev::errors::Error;
use log::{debug, error, info};
use serde::Serialize;

#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Debug, Clone)]
pub struct GpioPath {
    chip: String,
    pin: u32,
}

impl GpioPath {
    pub fn new(chip: String, pin: u32) -> Self {
        Self { chip, pin }
    }
}

#[derive(Debug)]
pub enum AppError {
    Gpio(gpio_cdev::errors::Error),
}

pub type AppResult<O> = Result<O, AppError>;

impl From<gpio_cdev::errors::Error> for AppError {
    fn from(e: Error) -> Self {
        Self::Gpio(e)
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Gpio(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for AppError {}

pub struct State {
    pins: RwLock<HashMap<GpioPath, Arc<LineHandle>>>,
}

impl State {
    pub fn new() -> Self {
        let active_pins = HashMap::<GpioPath, Arc<LineHandle>>::new();
        Self {
            pins: RwLock::new(active_pins),
        }
    }

    /// Given an action to perform on a pin, try to perform it on an already opened line handle, and
    /// if the action fails, retries it with a freshly opened handle.
    /// (this gracefully handles writing to a pin that was previously opened as input)
    fn do_with_handle<F, O, E>(
        &self,
        gpio_path: GpioPath,
        flags: LineRequestFlags,
        action: F,
    ) -> AppResult<O>
    where
        F: Fn(&LineHandle) -> Result<O, E>,
        AppError: From<E>,
        E: Display,
    {
        debug!("Trying to acquire a read lock on pins");
        {
            // Read lock
            let pins = self.pins.read().unwrap();
            if let Some(handle) = pins.get(&gpio_path) {
                match action(handle) {
                    Ok(res) => {
                        debug!("Action succeeded with pre-existing pin handle");
                        return Ok(res); // Happy path, no write lock
                    }
                    Err(e) => {
                        debug!(
                            "Action failed with pre-existing pin handle ({}); freeing it",
                            e
                        );
                    }
                }
            } else {
                debug!("No pre-existing pin handle")
            }
        }
        // slow path, application state is locked
        let mut pins = self.pins.write().unwrap();
        // drop the old line handle if it exists
        pins.remove(&gpio_path);
        info!("Opening device {}", gpio_path.chip);
        let device_path = format!("/dev/{}", gpio_path.chip); // Sad path, open a new line handle
        let mut chip = Chip::new(device_path)?;
        info!("Getting pin {}", gpio_path.pin);
        let line = chip.get_line(gpio_path.pin)?;
        info!("Making an {:?} request", flags);
        let handle = line.request(flags, 0, "http-gpio")?;
        let arc_handle = Arc::new(handle);
        debug!("Saving the pin handle for later");
        pins.insert(gpio_path, Arc::clone(&arc_handle));
        // Release the lock
        drop(pins);
        debug!("Performing action");
        let result = action(&arc_handle)?;
        Ok(result)
    }

    pub fn read(&self, gpio_path: GpioPath) -> AppResult<u8> {
        self.do_with_handle(gpio_path, LineRequestFlags::INPUT, |line| line.get_value())
    }

    pub fn write(&self, gpio_path: GpioPath, value: u8) -> AppResult<()> {
        self.do_with_handle(gpio_path, LineRequestFlags::OUTPUT, |line| {
            line.set_value(value)
        })
    }

    pub fn write_schedule(&self, gpio_path: GpioPath, schedule: Vec<u16>) -> AppResult<u8> {
        let pin = gpio_path.pin;
        info!("Will blink {:?} for a total of {} milliseconds", gpio_path, schedule.iter().sum::<u16>());
        self.do_with_handle(gpio_path, LineRequestFlags::OUTPUT, |line| -> AppResult<u8>{
            let mut value = 0;
            for &time in &schedule {
                debug!("Setting {} to {}", pin, value);
                line.set_value(value)?;
                value = value.bitxor(1);
                std::thread::sleep(Duration::from_millis(time.into()))
            }
            Ok(value)
        })
    }
}

pub fn list_chips() -> AppResult<Vec<GpioDeviceDescription>> {
    Ok(chips()?
        .enumerate()
        .flat_map(|(num, c)| match c {
            Ok(chip) => Some(GpioDeviceDescription::new(chip)),
            Err(e) => {
                error!("Unable to access chip {} in list: {}", num, e);
                None
            }
        })
        .collect())
}

pub fn list_pins(chip_name: String) -> AppResult<Vec<GpioPinDescription>> {
    let chip = Chip::new(format!("/dev/{}", chip_name))?;
    Ok(chip
        .lines()
        .flat_map(|line| match line.info() {
            Ok(info) => Some(GpioPinDescription::new(info)),
            Err(e) => {
                error!("Unable to access line info for line {:?}: {}", line, e);
                None
            }
        })
        .collect())
}

pub fn single_pin_description(gpio_pin: GpioPath) -> AppResult<GpioPinDescription> {
    let mut chip = Chip::new(format!("/dev/{}", gpio_pin.chip))?;
    let line = chip.get_line(gpio_pin.pin)?;
    let info = line.info()?;
    Ok(GpioPinDescription::new(info))
}

#[derive(Serialize)]
pub struct GpioDeviceDescription {
    name: String,
    label: String,
    num_lines: u32,
}

impl GpioDeviceDescription {
    pub fn new(chip: Chip) -> Self {
        Self {
            name: chip.name().to_string(),
            label: chip.label().to_string(),
            num_lines: chip.num_lines(),
        }
    }
}

#[derive(Serialize)]
pub struct GpioPinDescription {
    name: Option<String>,
    currently_used_by: Option<String>,
    is_used: bool,
    is_kernel: bool,
    is_output: bool,
    is_active_low: bool,
    offset: u32,
}

impl GpioPinDescription {
    pub fn new(line: LineInfo) -> Self {
        Self {
            name: line.name().map(ToString::to_string),
            currently_used_by: line.consumer().map(ToString::to_string),
            is_output: line.direction() == LineDirection::Out,
            is_used: line.is_used(),
            is_kernel: line.is_kernel(),
            is_active_low: line.is_active_low(),
            offset: line.line().offset(),
        }
    }
}
