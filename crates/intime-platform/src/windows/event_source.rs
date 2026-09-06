use intime_core::models::Event;

use crate::{
    error::PlatformError, shared::pop_event_blocking, traits::EventSource,
    windows::native::install_hooks,
};

pub struct WindowsEventSource;

impl WindowsEventSource {
    pub fn new() -> Self {
        std::thread::spawn(|| unsafe {
            install_hooks();
        });
        Self
    }
}

impl EventSource for WindowsEventSource {
    fn poll(&mut self) -> Result<Event, PlatformError> {
        pop_event_blocking()
    }

    fn new() -> Self
    where
        Self: Sized,
    {
        WindowsEventSource::new()
    }
}
