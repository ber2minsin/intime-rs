
use intime_core::models::Event;

use crate::{CapturedImage, error::PlatformError};

/// This is a trait that defines how EventSource
/// should be implemented across different OS mods
pub trait EventSource {
    fn poll(&mut self) -> Result<Event, PlatformError>;
}

pub trait ScreenshotSource {
    fn capture(&mut self, handle: u64) -> Result<CapturedImage, PlatformError>;
}