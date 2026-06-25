use std::{
    collections::VecDeque,
    sync::{Condvar, Mutex, OnceLock},
};

use intime_core::models::Event;

use crate::{
    error::PlatformError, 
    traits::EventSource, 
    windows::native::install_hooks
};

// Global queue bridging global Win32 hook functions to our instance runtime
pub(crate) static EVENTS: OnceLock<(Mutex<VecDeque<Event>>, Condvar)> = OnceLock::new();

fn queue() -> &'static (Mutex<VecDeque<Event>>, Condvar) {
    EVENTS.get_or_init(|| (Mutex::new(VecDeque::new()), Condvar::new()))
}

pub struct WindowsEventSource {}

impl WindowsEventSource {
    pub fn new() -> Self {
        // TODO can be moved to separate start and can be defined in shared trait
        let _hook_thread = std::thread::spawn(|| unsafe {
            let _ = install_hooks();
        });

        Self {}
    }
}

impl EventSource for WindowsEventSource {
    fn poll(&mut self) -> Result<Event, PlatformError> {
        let (lock, cvar) = queue();
        
        // Safely lock the inner vector, converting poisoning errors to our PlatformError type
        let mut guard = lock.lock().map_err(|_| {
            PlatformError::SynchronizationError("Global events mutex poisoned".to_string())
        })?;
        
        // Loop to shield against spurious thread wakeups
        while guard.is_empty() {
            guard = cvar.wait(guard).map_err(|_| {
                PlatformError::SynchronizationError("Global condvar wait failed".to_string())
            })?;
        }
        
        // FIFO push_back on the producer, pop_front here
        Ok(guard.pop_front().expect("non-empty after wait"))
    }
    
    fn new() -> Self
    where
        Self: Sized {
        WindowsEventSource {  }
    }
}

pub(crate) fn push_event(event: Event) {
    let (lock, cvar) = queue();
    if let Ok(mut guard) = lock.lock() {
        guard.push_back(event);
        cvar.notify_one();
    }
}