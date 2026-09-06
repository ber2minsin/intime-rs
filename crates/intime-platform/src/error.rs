use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlatformError {
    #[cfg(target_os = "windows")]
    #[error("Windows API error: {0}")]
    WindowsError(#[from] windows::core::Error),

    /// All adapters were enumerated but not a single output could be initialized.
    #[error("No monitors found or all outputs failed to initialize")]
    NoMonitors,

    /// Caller asked for monitor index N but only `available` monitors exist.
    #[error("Monitor index {index} is out of range ({available} monitors available)")]
    MonitorOutOfRange { index: u32, available: usize },

    /// Window handle was invalid, the window was destroyed, or its rect is zero-sized.
    #[error("Window handle is invalid or the window no longer exists")]
    InvalidWindow,

    /// Minimized windows have no on-screen pixels; capture is not possible.
    #[error("Window is minimized — restore it before capturing")]
    WindowMinimized,

    /// The window's monitor is not among the outputs tracked by this instance.
    #[error("Window is not visible on any tracked monitor")]
    WindowNotOnTrackedMonitor,

    /// Display capture session was invalidated (resolution change, exclusive
    /// fullscreen app, monitor unplug, etc.). Recover by reinitializing.
    #[error("Display output was lost — reinitialize the capture engine")]
    OutputLost,

    #[error("Capture produced an empty frame")]
    EmptyFrame,

    #[error(transparent)]
    Other(#[from] anyhow::Error),

    /// Internal threading, Mutex poisoning, or Condvar synchronization failures.
    #[error("Internal synchronization error: {0}")]
    SynchronizationError(String),
}
