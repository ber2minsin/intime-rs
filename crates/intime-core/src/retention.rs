//! Screenshot retention / lifecycle policy.
//!
//! UX model (predictable age tiers + optional disk budget):
//! 1. **Full quality** for `full_quality_days` (default 2).
//! 2. **Compact** (re-encode JPEG + downscale) until `retain_days` (default 7).
//! 3. **Delete** the file after `retain_days` (embeddings + event row remain).
//! 4. Sessions marked `important` skip all of the above.
//! 5. Optional `target_bytes`: when the screenshot directory is over budget,
//!    oldest non-important files are compacted/deleted early.

use serde::{Deserialize, Serialize};

/// Defaults tuned for light desktop use (~week of full→compact→delete).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenshotRetentionPolicy {
    pub enabled: bool,
    /// Keep capture quality this long (days).
    pub full_quality_days: u32,
    /// Delete screenshot files older than this (days). Embeddings stay.
    pub retain_days: u32,
    /// JPEG quality (1–100) used when compacting.
    pub compact_jpeg_quality: u8,
    /// Longest edge in pixels after compact (0 = no resize).
    pub compact_max_edge: u32,
    /// Soft cap on total screenshot bytes (`None` / 0 = disabled).
    pub target_bytes: Option<u64>,
    /// How often the daemon sweeps (seconds).
    pub interval_secs: u64,
}

impl Default for ScreenshotRetentionPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            full_quality_days: 2,
            retain_days: 7,
            compact_jpeg_quality: 40,
            compact_max_edge: 1280,
            target_bytes: Some(512 * 1024 * 1024),
            interval_secs: 3600,
        }
    }
}

impl ScreenshotRetentionPolicy {
    pub fn from_env() -> Self {
        let mut p = Self::default();
        p.enabled = env_bool("INTIME_SCREENSHOT_RETENTION_ENABLED", p.enabled);
        p.full_quality_days =
            env_u32("INTIME_SCREENSHOT_FULL_QUALITY_DAYS", p.full_quality_days);
        p.retain_days = env_u32("INTIME_SCREENSHOT_RETAIN_DAYS", p.retain_days);
        p.compact_jpeg_quality =
            env_u32("INTIME_SCREENSHOT_COMPACT_QUALITY", p.compact_jpeg_quality as u32)
                .clamp(1, 100) as u8;
        p.compact_max_edge = env_u32("INTIME_SCREENSHOT_COMPACT_MAX_EDGE", p.compact_max_edge);
        p.target_bytes = match std::env::var("INTIME_SCREENSHOT_TARGET_MB") {
            Ok(v) => {
                let mb: u64 = v.trim().parse().unwrap_or(512);
                if mb == 0 {
                    None
                } else {
                    Some(mb.saturating_mul(1024 * 1024))
                }
            }
            Err(_) => p.target_bytes,
        };
        p.interval_secs = env_u64("INTIME_SCREENSHOT_RETENTION_INTERVAL_SECS", p.interval_secs);
        if p.full_quality_days > p.retain_days {
            p.full_quality_days = p.retain_days;
        }
        p
    }

    /// Decide what to do given age and current tier (`full` / `compact` / other).
    pub fn action_for_age(&self, age_days: f64, tier: Option<&str>) -> RetentionAction {
        if age_days >= self.retain_days as f64 {
            return RetentionAction::Delete;
        }
        let tier = tier.unwrap_or("full");
        if age_days >= self.full_quality_days as f64 && tier != "compact" {
            return RetentionAction::Compact;
        }
        RetentionAction::Keep
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionAction {
    Keep,
    Compact,
    Delete,
}

fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

fn env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_progress_full_compact_delete() {
        let p = ScreenshotRetentionPolicy::default();
        assert_eq!(p.action_for_age(0.5, Some("full")), RetentionAction::Keep);
        assert_eq!(p.action_for_age(2.0, Some("full")), RetentionAction::Compact);
        assert_eq!(p.action_for_age(3.0, Some("compact")), RetentionAction::Keep);
        assert_eq!(p.action_for_age(7.0, Some("compact")), RetentionAction::Delete);
    }
}
