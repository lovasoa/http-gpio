use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::RwLock;

use gpio_cdev::{Chip, LineHandle, LineRequestFlags};
use gpio_cdev::errors::Error;

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
    pins: RwLock<HashMap<GpioPath, LineHandle>>,
}

impl State {
    pub fn new() -> Self {
        let active_pins = HashMap::<GpioPath, LineHandle>::new();
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
        // Get a line handle that was created before
        let pins = self.pins.read().unwrap();
        if let Some(handle) = pins.get(&gpio_path) {
            if let Ok(r) = action(handle) {
                return Ok(r); // Happy path, no write lock
            }
        }
        let device_path = format!("/dev/{}", gpio_path.chip); // Sad path, open a new line handle
        let mut chip = Chip::new(device_path)?;
        let line = chip.get_line(gpio_path.pin)?;
        let handle = line.request(flags, 0, "http-gpio")?;
        let mut pins = self.pins.write().unwrap();
        let result = action(&handle)?;
        pins.insert(gpio_path, handle);
        Ok(result)
    }

    pub fn read(&self, gpio_path: GpioPath) -> AppResult<u8> {
        self.do_with_handle(gpio_path, LineRequestFlags::INPUT, |line| line.get_value())
    }

    pub fn write(&self, gpio_path: GpioPath, value: u8) -> AppResult<()> {
        self.do_with_handle(gpio_path, LineRequestFlags::OUTPUT, |line| line.set_value(value))
    }
}