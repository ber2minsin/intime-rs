use crate::error::PlatformError;

/// Start the Linux activity hooks.
///
/// Prefers Sway IPC when `SWAYSOCK` is set (reliable window focus/title on
/// wlroots). Otherwise falls back to AT-SPI2, which covers GNOME and other
/// desktop environments that expose the accessibility bus.
pub fn install_hooks() -> Result<(), PlatformError> {
    if std::env::var_os("SWAYSOCK").is_some() {
        tracing::info!("Using Sway IPC for window events");
        return crate::linux::sway::run_event_loop();
    }

    tracing::info!("Using AT-SPI for window events");
    crate::linux::atspi_hooks::run_event_loop()
}
