use crate::error::PlatformError;

/// Start the Linux activity hooks.
///
/// Prefers Sway IPC when `SWAYSOCK` is set (reliable window focus/title/handle on
/// wlroots). Sway events are enriched with AT-SPI Document URL + focused UI.
/// Without Sway, AT-SPI alone drives the event stream.
pub fn install_hooks() -> Result<(), PlatformError> {
    if std::env::var_os("SWAYSOCK").is_some() {
        tracing::info!("Using Sway IPC for window events (AT-SPI enrichment for URL/focus)");
        return crate::linux::sway::run_event_loop();
    }

    tracing::info!("Using AT-SPI for window events");
    crate::linux::atspi_hooks::run_event_loop()
}
