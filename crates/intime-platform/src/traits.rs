use intime_core::models::Event;

use crate::{CapturedImage, error::PlatformError};

/// Platform activity event producer (window focus, title changes, etc.).
pub trait EventSource: Send {
    fn poll(&mut self) -> Result<Event, PlatformError>;
    fn new() -> Self
    where
        Self: Sized;
}

/// Captures a window (or best-effort screen region) identified by `handle`.
pub trait ScreenshotSource: Send {
    fn capture(&mut self, handle: u64) -> Result<CapturedImage, PlatformError>;
}
