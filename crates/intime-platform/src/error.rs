use thiserror::Error;

// #[derive(Error, Debug)]
// pub enum PlatformError {
//     #[error("")]
//     WindowsError(#[from] windows::core::Error),
//     #[error(transparent)]
//     Other(#[from] anyhow::Error),
// }

#[derive(Error, Debug)]
pub enum PlatformError {
    #[error("Windows API error: {0}")]
    WindowsError(#[from] windows::core::Error),
 
 
    /// All adapters were enumerated but not a single output could be initialized.
    #[error("No monitors found or all outputs failed to initialize")]
    NoMonitors,
 
    /// Caller asked for monitor index N but only `available` monitors exist.
    #[error("Monitor index {index} is out of range ({available} monitors available)")]
    MonitorOutOfRange { index: u32, available: usize },
 
    /// HWND was invalid, the window was destroyed, or its rect is zero-sized.
    #[error("Window handle is invalid or the window no longer exists")]
    InvalidWindow,
 
    /// Minimized windows have no on-screen pixels; capture is not possible.
    #[error("Window is minimized — restore it before capturing")]
    WindowMinimized,
 
    /// The window's monitor is not among the outputs tracked by this instance.
    #[error("Window is not visible on any tracked monitor")]
    WindowNotOnTrackedMonitor,
 
    /// The DXGI duplication session was invalidated (resolution change, a
    /// fullscreen exclusive app took over, monitor was unplugged, etc.).
    /// Recover by calling [`DxgiCapture::reinitialize`].
    #[error(
        "DXGI output was lost (display change or exclusive fullscreen app) \
         — call DxgiCapture::reinitialize"
    )]
    OutputLost,

    #[error("DXGI is initiated but it captured nothing")]
    EmptyFrame,

    #[error(transparent)]
    Other(#[from] anyhow::Error),

    /// Internal threading, Mutex poisoning, or Condvar synchronization failures.
    #[error("Internal synchronization error: {0}")]
    SynchronizationError(String),
}
 