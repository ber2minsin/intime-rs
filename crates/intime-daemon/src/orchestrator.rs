use anyhow::Error;
use blake3::Hash as Blake3Hash;
use image::ImageFormat;
use intime_core::time::Timestamp;
use intime_platform::traits::ScreenshotSource;
use std::path::PathBuf;
use tracing::info;

pub struct ScreenshotOrchestrator<T: ScreenshotSource> {
    source: T,
}

impl<T: ScreenshotSource> ScreenshotOrchestrator<T> {
    pub fn new(source: T) -> Self {
        Self { source }
    }

    pub fn process(&mut self, handle: u64) -> Result<PathBuf, Error> {
        let captured_image = self.source.capture(handle)?;
        let safe_date = Timestamp::now().to_filename_readable();

        // TODO CRITICAL change this, just for testing
        let mut image_path = PathBuf::from("data/screenshots");
        std::fs::create_dir_all(&image_path)?;
        image_path.push(format!("{}_{}.jpg", handle, safe_date));

        info!("Trying to save image over at {:?}", image_path);

        let img = image::RgbImage::from_raw(
            captured_image.width,
            captured_image.height,
            captured_image.pixels,
        )
        .expect("Failed to create RGB image buffer from raw data");

        // Save image as Jpeg (JPEG natively supports RGB8)
        img.save_with_format(&image_path, ImageFormat::Jpeg)
            .expect("Failed to save image file");

        Ok(image_path)
    }
}
