pub mod error;
pub mod frame_buffer;
pub mod shared;
pub mod traits;

pub use traits::EventSource;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

use crate::{error::PlatformError, traits::ScreenshotSource};

/// RGB8 screenshot pixels (`width * height * 3` bytes).
pub struct CapturedImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[cfg(target_os = "windows")]
pub fn create_event_source() -> Result<Box<dyn EventSource>, PlatformError> {
    use windows::event_source::WindowsEventSource;
    Ok(Box::new(WindowsEventSource::new()))
}

#[cfg(target_os = "linux")]
pub fn create_event_source() -> Result<Box<dyn EventSource>, PlatformError> {
    use linux::event_source::LinuxEventSource;
    Ok(Box::new(LinuxEventSource::new()))
}

#[cfg(target_os = "windows")]
pub fn create_capture_engine() -> Result<Box<dyn ScreenshotSource>, PlatformError> {
    use windows::screenshot::DxgiCapture;
    Ok(Box::new(DxgiCapture::new()?))
}

#[cfg(target_os = "linux")]
pub fn create_capture_engine() -> Result<Box<dyn ScreenshotSource>, PlatformError> {
    use linux::screenshot::LinuxCapture;
    Ok(Box::new(LinuxCapture::new()?))
}

/// Fill `url` / focused UI fields from AT-SPI for a process (Sway event enrichment).
#[cfg(target_os = "linux")]
pub fn enrich_metadata_from_atspi(pid: u32, meta: &mut intime_core::models::EventMetadata) {
    linux::atspi_hooks::enrich_metadata_from_atspi(pid, meta);
}
