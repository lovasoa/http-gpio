use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::{Arc, RwLock};

use gpio_cdev::{Chip, LineHandle, LineRequestFlags};
use gpio_cdev::errors::Error;
use log::{debug, info};

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
    Gpio(gpio_cdev::errors::Error)
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
            pins: RwLock::new(active_pins)
        }
    }


    /// Given an action to perform on a pin, try to perform it on an already opened line handle, and
    /// if the action fails, retries it with a freshly opened handle.
    /// (this gracefully handles writing to a pin that was previously opened as input)
    fn do_with_handle<F, O, E>(&self, gpio_path: GpioPath, flags: LineRequestFlags, action: F) -> AppResult<O>
        where F: Fn(&LineHandle) -> Result<O, E>,
              AppError: From<E> {
        debug!("Trying to acquire a read lock on pins");
        { // Read lock
            let pins = self.pins.read().unwrap();
            if let Some(handle) = pins.get(&gpio_path) {
                if let Ok(r) = action(handle) {
                    debug!("Action succeeded with pre-existing pin handle");
                    return Ok(r); // Happy path, no write lock
                } else {
                    debug!("Action failed with pre-existing pin handle");
                }
            } else {
                debug!("No pre-existing pin handle")
            }
        }
        info!("Opening device {}", gpio_path.chip);
        let device_path = format!("/dev/{}", gpio_path.chip); // Sad path, open a new line handle
        let mut chip = Chip::new(device_path)?;
        info!("Getting pin {}", gpio_path.pin);
        let line = chip.get_line(gpio_path.pin)?;
        info!("Making an {:?} request", flags);
        let handle = line.request(flags, 0, "http-gpio")?;
        let arc_handle = Arc::new(handle);
        { // Application state is locked
            let mut pins = self.pins.write().unwrap();
            debug!("Saving the pin handle for later");
            pins.insert(gpio_path, Arc::clone(&arc_handle));
        }
        debug!("Performing action");
        let result = action(&arc_handle)?;
        Ok(result)
    }

    pub fn read(&self, gpio_path: GpioPath) -> AppResult<u8> {
        self.do_with_handle(gpio_path, LineRequestFlags::INPUT, |line| line.get_value())
    }

    pub fn write(&self, gpio_path: GpioPath, value: u8) -> AppResult<()> {
        self.do_with_handle(gpio_path, LineRequestFlags::OUTPUT, |line| line.set_value(value))
    }
}