//! Age-tiered screenshot retention: full → compact → delete.
//! Important sessions are skipped. Optional disk budget accelerates aging.

use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use chrono::Utc;
use image::{DynamicImage, imageops::FilterType};
use intime_core::retention::{RetentionAction, ScreenshotRetentionPolicy};
use intime_storage::storage::Storage;
use tracing::{debug, info, warn};

/// Background loop: sleep `interval`, run one sweep, repeat.
pub async fn run_screenshot_retention_loop(storage: Storage, policy: ScreenshotRetentionPolicy) {
    if !policy.enabled {
        info!("Screenshot retention disabled");
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        }
    }

    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(policy.interval_secs.max(60)));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // Run once shortly after startup so a week-old pile is not waiting an hour.
    ticker.tick().await;

    loop {
        ticker.tick().await;
        match run_retention_sweep(&storage, &policy).await {
            Ok(stats) => {
                if stats.compacted > 0 || stats.deleted > 0 || stats.budget_actions > 0 {
                    info!(
                        compacted = stats.compacted,
                        deleted = stats.deleted,
                        budget_actions = stats.budget_actions,
                        bytes_after = stats.bytes_after,
                        "Screenshot retention sweep finished"
                    );
                } else {
                    debug!(
                        bytes_after = stats.bytes_after,
                        "Screenshot retention sweep: nothing to do"
                    );
                }
            }
            Err(e) => warn!("Screenshot retention sweep failed: {e:#}"),
        }
    }
}

#[derive(Debug, Default)]
pub struct RetentionStats {
    pub compacted: u64,
    pub deleted: u64,
    pub budget_actions: u64,
    pub bytes_after: u64,
    pub skipped_important: u64,
}

pub async fn run_retention_sweep(
    storage: &Storage,
    policy: &ScreenshotRetentionPolicy,
) -> Result<RetentionStats> {
    let mut stats = RetentionStats::default();
    let now = Utc::now();
    let candidates = storage
        .event_repository
        .list_screenshot_candidates(5_000)
        .await
        .context("list screenshot candidates")?;

    for c in &candidates {
        if c.session_important != 0 {
            stats.skipped_important += 1;
            continue;
        }
        let age_days = (now - c.occured_at).num_milliseconds() as f64 / 86_400_000.0;
        match policy.action_for_age(age_days, c.tier.as_deref()) {
            RetentionAction::Keep => {}
            RetentionAction::Compact => {
                if compact_screenshot(storage, c.event_id, &c.path, policy).await? {
                    stats.compacted += 1;
                }
            }
            RetentionAction::Delete => {
                if delete_screenshot(storage, c.event_id, &c.path).await? {
                    stats.deleted += 1;
                }
            }
        }
    }

    // Budget pass: if over target, age the oldest remaining non-important files early.
    if let Some(target) = policy.target_bytes {
        let mut bytes = screenshot_dir_bytes(Path::new("data/screenshots"));
        if bytes > target {
            let again = storage
                .event_repository
                .list_screenshot_candidates(5_000)
                .await
                .context("list for budget")?;
            for c in again {
                if bytes <= target {
                    break;
                }
                if c.session_important != 0 {
                    continue;
                }
                let tier = c.tier.as_deref().unwrap_or("full");
                if tier != "compact" {
                    if compact_screenshot(storage, c.event_id, &c.path, policy).await? {
                        stats.compacted += 1;
                        stats.budget_actions += 1;
                        bytes = screenshot_dir_bytes(Path::new("data/screenshots"));
                        continue;
                    }
                }
                if delete_screenshot(storage, c.event_id, &c.path).await? {
                    stats.deleted += 1;
                    stats.budget_actions += 1;
                    bytes = screenshot_dir_bytes(Path::new("data/screenshots"));
                }
            }
        }
        stats.bytes_after = screenshot_dir_bytes(Path::new("data/screenshots"));
    } else {
        stats.bytes_after = screenshot_dir_bytes(Path::new("data/screenshots"));
    }

    Ok(stats)
}

async fn compact_screenshot(
    storage: &Storage,
    event_id: i64,
    path: &str,
    policy: &ScreenshotRetentionPolicy,
) -> Result<bool> {
    let path_buf = resolve_screenshot_path(path);
    if !path_buf.is_file() {
        // Stale DB pointer — clear it.
        storage.event_repository.clear_screenshot(event_id).await?;
        return Ok(false);
    }

    let path_owned = path_buf.clone();
    let quality = policy.compact_jpeg_quality;
    let max_edge = policy.compact_max_edge;
    let compacted = tokio::task::spawn_blocking(move || {
        reencode_jpeg_in_place(&path_owned, quality, max_edge)
    })
    .await
    .context("compact join")?
    .context("compact jpeg")?;

    if compacted {
        storage
            .event_repository
            .set_screenshot_tier(event_id, "compact", None)
            .await?;
    }
    Ok(compacted)
}

async fn delete_screenshot(storage: &Storage, event_id: i64, path: &str) -> Result<bool> {
    let path_buf = resolve_screenshot_path(path);
    if path_buf.is_file() {
        if let Err(e) = fs::remove_file(&path_buf) {
            warn!(path = %path_buf.display(), error = %e, "failed to delete screenshot file");
        }
    }
    storage.event_repository.clear_screenshot(event_id).await?;
    Ok(true)
}

fn resolve_screenshot_path(path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        p
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

/// Re-encode JPEG with lower quality / optional downscale. Returns true if rewritten.
pub fn reencode_jpeg_in_place(path: &Path, quality: u8, max_edge: u32) -> Result<bool> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let img = image::load_from_memory(&bytes).context("decode screenshot")?;
    let img = if max_edge > 0 {
        downscale_max_edge(img, max_edge)
    } else {
        img
    };

    let mut out = Cursor::new(Vec::new());
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    let rgb = img.to_rgb8();
    encoder
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .context("jpeg encode")?;
    let encoded = out.into_inner();

    // Only write if we actually saved space (or at least replaced).
    if encoded.len() >= bytes.len() && max_edge == 0 {
        // Rare: already small; still mark compact by rewriting so we don't retry forever.
    }
    let tmp = path.with_extension("jpg.tmp");
    fs::write(&tmp, &encoded).context("write tmp")?;
    fs::rename(&tmp, path).context("rename tmp")?;
    Ok(true)
}

fn downscale_max_edge(img: DynamicImage, max_edge: u32) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let long = w.max(h);
    if long <= max_edge {
        return img;
    }
    let scale = max_edge as f32 / long as f32;
    let nw = ((w as f32) * scale).round().max(1.0) as u32;
    let nh = ((h as f32) * scale).round().max(1.0) as u32;
    img.resize(nw, nh, FilterType::Triangle)
}

pub fn screenshot_dir_bytes(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    #[test]
    fn reencode_shrinks_solid_image() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shot.jpg");
        let img: ImageBuffer<Rgb<u8>, _> =
            ImageBuffer::from_fn(800, 600, |x, y| Rgb([(x % 256) as u8, (y % 256) as u8, 40]));
        img.save(&path).unwrap();
        let before = fs::metadata(&path).unwrap().len();
        assert!(reencode_jpeg_in_place(&path, 30, 400).unwrap());
        let after = fs::metadata(&path).unwrap().len();
        assert!(after < before, "after={after} before={before}");
    }
}
