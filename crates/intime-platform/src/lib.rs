pub mod error;
pub mod frame_buffer;

pub mod traits;
pub use traits::EventSource;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

use crate::{error::PlatformError, traits::ScreenshotSource};

#[cfg(target_os = "windows")]
pub fn create_event_source() -> Result<Box<dyn EventSource>, PlatformError> {
    use windows::event_source::WindowsEventSource;
    Ok(Box::new(WindowsEventSource::new()))
}

#[cfg(target_os = "linux")]
pub fn create_event_source() -> Result<Box<dyn EventSource>, PlatformError> {
    todo!()
}

#[cfg(target_os = "windows")]
pub fn create_capture_engine() -> Result<Box<dyn ScreenshotSource>, PlatformError> {
        use windows::screenshot;

        return Ok(Box::new(screenshot::DxgiCapture::new()?));
}

#[cfg(target_os = "linux")]
pub fn create_capture_engine() -> Result<Box<dyn ScreenshotSource>, PlatformError> {
    todo!()
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

        #[cfg(target_os="windows")]
        {
            use crate::windows::ui::install_hooks;
            install_hooks().unwrap();
            loop {}
        }

        #[cfg(target_os="linux")]
        todo!();
    }
}
