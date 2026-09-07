//! Explicit idle detection: emit `idle_start` / `idle_end` and pause capture.

use intime_core::models::{Event, EventData};
use std::time::{Duration, Instant};

/// Transition produced by [`IdleChecker`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleTransition {
    BecameIdle,
    BecameActive,
}

/// Tracks wall-clock silence between real user events (not heartbeat captures).
///
/// While idle, the daemon should stop screenshot heartbeats and close the open
/// session with `ended_reason = idle` via the normal `idle_start` event path.
#[derive(Debug)]
pub struct IdleChecker {
    timeout: Duration,
    last_activity: Instant,
    idle: bool,
}

impl IdleChecker {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            last_activity: Instant::now(),
            idle: false,
        }
    }

    /// Default idle after 5 minutes; override with `INTIME_IDLE_AFTER_SECS`.
    pub fn from_env() -> Self {
        let secs = std::env::var("INTIME_IDLE_AFTER_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5 * 60);
        Self::new(Duration::from_secs(secs.max(1)))
    }

    pub fn is_idle(&self) -> bool {
        self.idle
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Call on every platform event. Returns `BecameActive` when leaving idle.
    pub fn observe_event(&mut self, event: &Event) -> Option<IdleTransition> {
        if !counts_as_user_activity(event) {
            return None;
        }
        self.last_activity = Instant::now();
        if self.idle {
            self.idle = false;
            Some(IdleTransition::BecameActive)
        } else {
            None
        }
    }

    /// Poll on a timer (e.g. heartbeat). Returns `BecameIdle` once when timed out.
    pub fn poll(&mut self) -> Option<IdleTransition> {
        if self.idle {
            return None;
        }
        if self.last_activity.elapsed() < self.timeout {
            return None;
        }
        self.idle = true;
        Some(IdleTransition::BecameIdle)
    }
}

fn counts_as_user_activity(event: &Event) -> bool {
    match &event.data {
        EventData::IdleStart | EventData::IdleEnd | EventData::Gap | EventData::Background { .. } => {
            false
        }
        EventData::WindowFocus { .. }
        | EventData::TitleChange { .. }
        | EventData::TextChanged { .. }
        | EventData::AppSeen { .. }
        | EventData::UiAction { .. } => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intime_core::{models::EventMetadata, time::Timestamp};
    use std::thread;

    fn focus_event() -> Event {
        Event {
            timestamp: Timestamp::now(),
            data: EventData::WindowFocus {
                fingerprint: blake3::hash(b"test"),
                window_handle: 1,
            },
            metadata: EventMetadata::default(),
        }
    }

    #[test]
    fn becomes_idle_after_timeout_without_activity() {
        let mut idle = IdleChecker::new(Duration::from_millis(40));
        assert!(idle.poll().is_none());
        thread::sleep(Duration::from_millis(50));
        assert_eq!(idle.poll(), Some(IdleTransition::BecameIdle));
        assert!(idle.is_idle());
        // sticky until activity
        assert!(idle.poll().is_none());
    }

    #[test]
    fn activity_clears_idle() {
        let mut idle = IdleChecker::new(Duration::from_millis(20));
        thread::sleep(Duration::from_millis(30));
        assert_eq!(idle.poll(), Some(IdleTransition::BecameIdle));
        assert_eq!(
            idle.observe_event(&focus_event()),
            Some(IdleTransition::BecameActive)
        );
        assert!(!idle.is_idle());
    }

    #[test]
    fn idle_events_do_not_count_as_activity() {
        let mut idle = IdleChecker::new(Duration::from_secs(60));
        idle.idle = true;
        let start = Event {
            timestamp: Timestamp::now(),
            data: EventData::IdleStart,
            metadata: Default::default(),
        };
        assert!(idle.observe_event(&start).is_none());
        assert!(idle.is_idle());
    }
}
