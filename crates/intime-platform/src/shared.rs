use crate::error::PlatformError;
use intime_core::models::Event;
use std::{
    collections::VecDeque,
    sync::{Condvar, Mutex, OnceLock},
};

// Global queue bridging platform-native hook/event threads to our instance runtime
// This is not nice
pub(crate) static EVENTS: OnceLock<(Mutex<VecDeque<Event>>, Condvar)> = OnceLock::new();

fn queue() -> &'static (Mutex<VecDeque<Event>>, Condvar) {
    EVENTS.get_or_init(|| (Mutex::new(VecDeque::new()), Condvar::new()))
}

pub(crate) fn push_event(event: Event) {
    let (lock, cvar) = queue();
    if let Ok(mut guard) = lock.lock() {
        guard.push_back(event);
        cvar.notify_one();
    }
}

pub(crate) fn pop_event_blocking() -> Result<Event, PlatformError> {
    let (lock, cvar) = queue();

    let mut guard = lock.lock().map_err(|_| {
        PlatformError::SynchronizationError("Global events mutex poisoned".to_string())
    })?;

    while guard.is_empty() {
        guard = cvar.wait(guard).map_err(|_| {
            PlatformError::SynchronizationError("Global condvar wait failed".to_string())
        })?;
    }

    Ok(guard.pop_front().ok_or_else(|| {
        PlatformError::SynchronizationError("event queue empty after wait".to_string())
    })?)
}

#[allow(dead_code)] // used by unit tests and optional non-blocking callers
pub(crate) fn try_pop_event() -> Option<Event> {
    let (lock, _) = queue();
    lock.lock().ok()?.pop_front()
}

#[cfg(test)]
mod tests {
    use super::*;
    use intime_core::{
        models::{EventData, EventMetadata},
        time::Timestamp,
    };
    use std::{
        sync::{Arc, Barrier},
        thread,
        time::Duration,
    };

    fn drain() {
        while try_pop_event().is_some() {}
    }

    #[test]
    fn event_queue_preserves_fifo_order() {
        drain();
        for i in 0..5u32 {
            push_event(Event {
                timestamp: Timestamp::now(),
                data: EventData::Gap,
                metadata: EventMetadata {
                    process_id: Some(i),
                    ..Default::default()
                },
            });
        }
        for i in 0..5u32 {
            let event = try_pop_event().expect("queued event");
            assert_eq!(event.metadata.process_id, Some(i));
        }
        assert!(try_pop_event().is_none());
    }

    #[test]
    fn blocking_pop_wakes_on_push() {
        drain();
        let barrier = Arc::new(Barrier::new(2));
        let barrier2 = barrier.clone();
        let handle = thread::spawn(move || {
            barrier2.wait();
            pop_event_blocking().expect("blocked pop")
        });

        barrier.wait();
        thread::sleep(Duration::from_millis(50));
        push_event(Event {
            timestamp: Timestamp::now(),
            data: EventData::IdleEnd,
            metadata: Default::default(),
        });

        let event = handle.join().unwrap();
        assert_eq!(event.data.name(), "idle_end");
    }
}
