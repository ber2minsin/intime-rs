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
