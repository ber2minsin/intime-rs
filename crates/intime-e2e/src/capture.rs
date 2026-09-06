use std::{
    collections::{HashMap, VecDeque},
    io::Cursor,
    path::{Path, PathBuf},
};

use image::DynamicImage;
use intime_platform::{CapturedImage, error::PlatformError, traits::ScreenshotSource};

/// Screenshot source that replays staged fixture images by window handle.
///
/// Each handle has a queue of fixture paths; every `capture(handle)` pops the
/// next image. This models recorded desktop frames for scenario replay.
/// Live grim/portal capture belongs in the Linux desktop Docker profile.
pub struct FixtureCapture {
    root: PathBuf,
    queues: HashMap<u64, VecDeque<PathBuf>>,
    captures: Vec<(u64, PathBuf)>,
}

impl FixtureCapture {
    pub fn new(root: impl Into<PathBuf>, queues: HashMap<u64, VecDeque<PathBuf>>) -> Self {
        Self {
            root: root.into(),
            queues,
            captures: Vec::new(),
        }
    }

    pub fn captures(&self) -> &[(u64, PathBuf)] {
        &self.captures
    }

    fn load_rgb(path: &Path) -> Result<CapturedImage, PlatformError> {
        let bytes = std::fs::read(path).map_err(|e| {
            PlatformError::Other(anyhow::anyhow!("fixture read {}: {e}", path.display()))
        })?;
        let img = image::load(Cursor::new(&bytes), image::ImageFormat::Png)
            .or_else(|_| image::load_from_memory(&bytes))
            .map_err(|e| {
                PlatformError::Other(anyhow::anyhow!("fixture decode {}: {e}", path.display()))
            })?;
        dynamic_to_captured(img)
    }
}

impl ScreenshotSource for FixtureCapture {
    fn capture(&mut self, handle: u64) -> Result<CapturedImage, PlatformError> {
        let rel = self
            .queues
            .get_mut(&handle)
            .and_then(|q| q.pop_front())
            .ok_or_else(|| {
                PlatformError::Other(anyhow::anyhow!(
                    "no remaining fixture for window handle {handle}"
                ))
            })?;
        let path = self.root.join(&rel);
        self.captures.push((handle, path.clone()));
        Self::load_rgb(&path)
    }
}

fn dynamic_to_captured(img: DynamicImage) -> Result<CapturedImage, PlatformError> {
    let rgb = img.to_rgb8();
    let width = rgb.width();
    let height = rgb.height();
    if width == 0 || height == 0 {
        return Err(PlatformError::EmptyFrame);
    }
    Ok(CapturedImage {
        width,
        height,
        pixels: rgb.into_raw(),
    })
}
