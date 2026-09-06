use crate::{
    error::PlatformError, linux::native::install_hooks, shared::pop_event_blocking,
    traits::EventSource,
};

pub struct LinuxEventSource;

impl LinuxEventSource {
    pub fn new() -> Self {
        std::thread::spawn(|| {
            if let Err(e) = install_hooks() {
                tracing::error!("Linux platform hook thread exited: {e:?}");
            }
        });
        Self
    }
}

impl EventSource for LinuxEventSource {
    fn poll(&mut self) -> Result<intime_core::models::Event, PlatformError> {
        pop_event_blocking()
    }

    fn new() -> Self
    where
        Self: Sized,
    {
        LinuxEventSource::new()
    }
}
