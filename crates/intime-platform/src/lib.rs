pub mod error;
pub mod frame_buffer;

// 1. Make the trait module public so external crates can use the trait definitions
pub mod traits; 

// 2. Export the trait itself explicitly at the root level if desired
pub use traits::EventSource as EventSourceTrait;

#[cfg(target_os = "windows")]
mod windows;

// 3. Keep your concrete struct export clean
#[cfg(target_os = "windows")]
pub use windows::event_source::WindowsEventSource as EventSource;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::LinuxEventSource as EventSource;

use crate::{error::PlatformError, traits::ScreenshotSource};

pub fn create_capture_engine() -> Result<impl ScreenshotSource, PlatformError> {
    #[cfg(target_os = "windows")]
    {
        use windows::screenshot;

        return Ok(screenshot::DxgiCapture::new()?);
    }
}

pub struct CapturedImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>
}


#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    #[cfg(test)]
    pub fn test_hooks() {
        use crate::windows::ui::install_hooks;

        install_hooks().unwrap();
        loop {}
    }
}