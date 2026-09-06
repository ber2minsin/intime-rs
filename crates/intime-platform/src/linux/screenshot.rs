use std::io::Cursor;
use std::process::Command;

use ashpd::desktop::screenshot::Screenshot;
use image::DynamicImage;
use swayipc::Connection;

use crate::{
    CapturedImage, error::PlatformError, linux::sway::find_node_by_id, traits::ScreenshotSource,
};

/// Linux screenshot backend.
///
/// On Sway/wlroots, captures a window region with `grim` using geometry from
/// the Sway tree. Elsewhere, falls back to the XDG Desktop Portal screenshot
/// API (works on GNOME and other portal backends).
pub struct LinuxCapture {
    prefer_grim: bool,
}

impl LinuxCapture {
    pub fn new() -> Result<Self, PlatformError> {
        let prefer_grim = std::env::var_os("SWAYSOCK").is_some() && grim_available();
        if prefer_grim {
            tracing::info!("Screenshot backend: grim (wlroots)");
        } else {
            tracing::info!("Screenshot backend: XDG Desktop Portal");
        }
        Ok(Self { prefer_grim })
    }

    fn capture_with_grim(&self, handle: u64) -> Result<CapturedImage, PlatformError> {
        let mut conn = Connection::new()
            .map_err(|e| PlatformError::Other(anyhow::anyhow!("Sway IPC connect failed: {e}")))?;
        let tree = conn
            .get_tree()
            .map_err(|e| PlatformError::Other(anyhow::anyhow!("Sway get_tree failed: {e}")))?;

        let node = find_node_by_id(&tree, handle as i64).ok_or(PlatformError::InvalidWindow)?;

        let rect = &node.rect;
        if rect.width <= 0 || rect.height <= 0 {
            return Err(PlatformError::InvalidWindow);
        }

        let geometry = format!("{},{} {}x{}", rect.x, rect.y, rect.width, rect.height);
        let output = Command::new("grim")
            .args(["-g", &geometry, "-t", "png", "-"])
            .output()
            .map_err(|e| PlatformError::Other(anyhow::anyhow!("Failed to run grim: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PlatformError::Other(anyhow::anyhow!(
                "grim failed: {stderr}"
            )));
        }

        decode_png_to_rgb(&output.stdout)
    }

    fn capture_with_portal(&self) -> Result<CapturedImage, PlatformError> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| PlatformError::SynchronizationError(e.to_string()))?;

        rt.block_on(async {
            let response = Screenshot::request()
                .interactive(false)
                .modal(false)
                .send()
                .await
                .map_err(|e| {
                    PlatformError::Other(anyhow::anyhow!("Portal screenshot request: {e}"))
                })?
                .response()
                .map_err(|e| {
                    PlatformError::Other(anyhow::anyhow!("Portal screenshot response: {e}"))
                })?;

            let uri = response.uri().clone();
            let path = url_to_path(&uri)?;

            let bytes = std::fs::read(&path).map_err(|e| {
                PlatformError::Other(anyhow::anyhow!("Failed to read portal screenshot: {e}"))
            })?;
            let _ = std::fs::remove_file(&path);

            decode_image_bytes(&bytes)
        })
    }
}

impl ScreenshotSource for LinuxCapture {
    fn capture(&mut self, handle: u64) -> Result<CapturedImage, PlatformError> {
        if self.prefer_grim {
            match self.capture_with_grim(handle) {
                Ok(img) => return Ok(img),
                Err(e) => {
                    tracing::warn!("grim capture failed ({e:?}), falling back to portal");
                }
            }
        }
        self.capture_with_portal()
    }
}

fn url_to_path(uri: &url::Url) -> Result<std::path::PathBuf, PlatformError> {
    if uri.scheme() != "file" {
        return Err(PlatformError::Other(anyhow::anyhow!(
            "Screenshot URI is not a file path: {uri}"
        )));
    }
    let path = uri
        .to_file_path()
        .map_err(|_| PlatformError::Other(anyhow::anyhow!("Invalid file URI: {uri}")))?;
    Ok(path)
}

fn grim_available() -> bool {
    Command::new("grim")
        .arg("-h")
        .output()
        .map(|o| o.status.success() || !o.stdout.is_empty() || !o.stderr.is_empty())
        .unwrap_or(false)
}

fn decode_png_to_rgb(bytes: &[u8]) -> Result<CapturedImage, PlatformError> {
    decode_image_bytes(bytes)
}

pub(crate) fn decode_image_bytes(bytes: &[u8]) -> Result<CapturedImage, PlatformError> {
    let img = image::load(Cursor::new(bytes), image::ImageFormat::Png)
        .or_else(|_| image::load_from_memory(bytes))
        .map_err(|e| PlatformError::Other(anyhow::anyhow!("Failed to decode screenshot: {e}")))?;

    dynamic_to_captured(img)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_png_fixture_to_rgb() {
        let bytes = include_bytes!("../../tests/fixtures/sample.png");
        let img = decode_image_bytes(bytes).expect("decode");
        assert_eq!(img.width, 4);
        assert_eq!(img.height, 4);
        assert_eq!(img.pixels.len(), 4 * 4 * 3);
        assert_eq!(&img.pixels[0..3], &[10, 20, 30]);
    }

    #[test]
    fn rejects_empty_bytes() {
        assert!(decode_image_bytes(b"").is_err());
    }
}
