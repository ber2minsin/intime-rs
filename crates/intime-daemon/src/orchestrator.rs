use anyhow::{Context, Result};
use image::ImageFormat;
use intime_core::{
    models::{Event, EventData},
    time::Timestamp,
};
use intime_platform::traits::ScreenshotSource;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tracing::info;

pub struct ScreenshotOrchestrator {
    source: Box<dyn ScreenshotSource>,
    policy: ScreenshotPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScreenshotKey {
    handle: u64,
    fingerprint: Option<blake3::Hash>,
    title: Option<String>,
}

#[derive(Debug)]
struct ScreenshotPolicy {
    minimum_interval: Duration,
    last_capture: Option<(Instant, ScreenshotKey)>,
}

impl ScreenshotPolicy {
    fn new(minimum_interval: Duration) -> Self {
        Self {
            minimum_interval,
            last_capture: None,
        }
    }

    fn should_capture(&self, now: Instant, key: &ScreenshotKey) -> bool {
        let Some((captured_at, previous)) = &self.last_capture else {
            return true;
        };

        if previous.handle != key.handle || previous.fingerprint != key.fingerprint {
            return true;
        }

        now.duration_since(*captured_at) >= self.minimum_interval && previous.title != key.title
    }

    fn record_capture(&mut self, now: Instant, key: ScreenshotKey) {
        self.last_capture = Some((now, key));
    }
}

impl ScreenshotOrchestrator {
    pub fn new(source: Box<dyn ScreenshotSource>) -> Self {
        Self::with_minimum_interval(source, Duration::from_millis(750))
    }

    pub fn with_minimum_interval(
        source: Box<dyn ScreenshotSource>,
        minimum_interval: Duration,
    ) -> Self {
        Self {
            source,
            policy: ScreenshotPolicy::new(minimum_interval),
        }
    }

    pub fn process_event(&mut self, event: &Event) -> Result<Option<PathBuf>> {
        let Some(handle) = event.data.window_handle() else {
            return Ok(None);
        };

        let key = ScreenshotKey {
            handle,
            fingerprint: event.data.fingerprint(),
            title: match &event.data {
                EventData::TitleChange { new_title, .. } => Some(new_title.clone()),
                _ => event.metadata.window_title.clone(),
            },
        };
        let now = Instant::now();

        if !self.policy.should_capture(now, &key) {
            return Ok(None);
        }

        let path = self.capture_and_save(handle)?;
        self.policy.record_capture(now, key);
        Ok(Some(path))
    }

    fn capture_and_save(&mut self, handle: u64) -> Result<PathBuf> {
        let captured_image = self
            .source
            .capture(handle)
            .context("screenshot capture failed")?;

        let safe_date = Timestamp::now().to_filename_readable();
        let mut image_path = PathBuf::from("data/screenshots");
        std::fs::create_dir_all(&image_path).context("failed to create screenshot directory")?;
        image_path.push(format!("{}_{}.jpg", handle, safe_date));

        info!("Saving screenshot to {:?}", image_path);

        let img = image::RgbImage::from_raw(
            captured_image.width,
            captured_image.height,
            captured_image.pixels,
        )
        .ok_or_else(|| anyhow::anyhow!("invalid RGB buffer for captured image"))?;

        img.save_with_format(&image_path, ImageFormat::Jpeg)
            .with_context(|| format!("failed to write screenshot to {}", image_path.display()))?;

        Ok(image_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intime_platform::CapturedImage;
    use std::sync::{Arc, Mutex};

    struct FakeCapture {
        handles: Arc<Mutex<Vec<u64>>>,
    }

    impl ScreenshotSource for FakeCapture {
        fn capture(
            &mut self,
            handle: u64,
        ) -> Result<CapturedImage, intime_platform::error::PlatformError> {
            self.handles.lock().unwrap().push(handle);
            Ok(CapturedImage {
                width: 1,
                height: 1,
                pixels: vec![0, 0, 0],
            })
        }
    }

    #[test]
    fn suppresses_duplicate_events_inside_interval() {
        let now = Instant::now();
        let key = ScreenshotKey {
            handle: 7,
            fingerprint: None,
            title: Some("same".into()),
        };
        let mut policy = ScreenshotPolicy::new(Duration::from_secs(1));

        assert!(policy.should_capture(now, &key));
        policy.record_capture(now, key.clone());
        assert!(!policy.should_capture(now + Duration::from_millis(500), &key));
    }

    #[test]
    fn captures_new_window_immediately() {
        let now = Instant::now();
        let first = ScreenshotKey {
            handle: 7,
            fingerprint: None,
            title: None,
        };
        let second = ScreenshotKey {
            handle: 8,
            ..first.clone()
        };
        let mut policy = ScreenshotPolicy::new(Duration::from_secs(10));
        policy.record_capture(now, first);
        assert!(policy.should_capture(now, &second));
    }

    #[test]
    fn captures_meaningful_title_change_after_debounce() {
        let now = Instant::now();
        let first = ScreenshotKey {
            handle: 7,
            fingerprint: None,
            title: Some("Inbox".into()),
        };
        let changed = ScreenshotKey {
            title: Some("Writing to Alice".into()),
            ..first.clone()
        };
        let mut policy = ScreenshotPolicy::new(Duration::from_millis(750));
        policy.record_capture(now, first);

        assert!(!policy.should_capture(now + Duration::from_millis(100), &changed));
        assert!(policy.should_capture(now + Duration::from_millis(750), &changed));
    }

    #[test]
    fn orchestrator_does_not_capture_repeated_focus() {
        let handles = Arc::new(Mutex::new(Vec::new()));
        let mut orchestrator = ScreenshotOrchestrator::with_minimum_interval(
            Box::new(FakeCapture {
                handles: handles.clone(),
            }),
            Duration::from_secs(60),
        );
        let event = Event {
            timestamp: Timestamp::now(),
            data: EventData::WindowFocus {
                fingerprint: blake3::hash(b"app"),
                window_handle: 42,
            },
            metadata: Default::default(),
        };

        let first = orchestrator.process_event(&event).unwrap();
        let second = orchestrator.process_event(&event).unwrap();
        assert!(first.is_some());
        assert!(second.is_none());
        assert_eq!(&*handles.lock().unwrap(), &[42]);
    }
}
