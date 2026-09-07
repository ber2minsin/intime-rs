use crate::error::PlatformError;
use crate::linux::desktop::{detect_desktop, DesktopKind};

/// Start the Linux activity hooks.
///
/// Preference order:
/// 1. **Sway** (`SWAYSOCK`) — IPC window events + AT-SPI enrichment; also starts a
///    background AT-SPI **document** listener so SPA URL changes are not missed.
/// 2. **Hyprland / GNOME / KDE / generic** — AT-SPI primary event stream + portal
///    screenshots (Wayland-friendly, compositor-agnostic).
///
/// MPRIS always runs from `event_source` regardless of desktop.
pub fn install_hooks() -> Result<(), PlatformError> {
    let kind = detect_desktop();
    tracing::info!(desktop = kind.as_str(), "Detected Linux desktop environment");

    match kind {
        DesktopKind::Sway => {
            tracing::info!(
                "Using Sway IPC for window events; AT-SPI for URL/focus + document updates"
            );
            // Document/attribute stream fills URL gaps when the window title is stable.
            std::thread::Builder::new()
                .name("intime-atspi-docs".into())
                .spawn(|| {
                    if let Err(e) = crate::linux::atspi_hooks::run_document_event_loop() {
                        tracing::warn!("AT-SPI document loop exited: {e}");
                    }
                })
                .map_err(|e| PlatformError::SynchronizationError(e.to_string()))?;
            crate::linux::sway::run_event_loop()
        }
        DesktopKind::Hyprland => {
            tracing::info!(
                "Hyprland detected — using AT-SPI (+ portal screenshots). \
                 Hyprland IPC toplevels are not wired yet; AT-SPI covers focus/title/URL."
            );
            crate::linux::atspi_hooks::run_event_loop()
        }
        DesktopKind::Gnome => {
            tracing::info!(
                "GNOME detected — using AT-SPI for window/URL events and xdg-desktop-portal \
                 for screenshots (install xdg-desktop-portal-gnome)"
            );
            crate::linux::atspi_hooks::run_event_loop()
        }
        DesktopKind::Kde => {
            tracing::info!(
                "KDE/Plasma detected — using AT-SPI + xdg-desktop-portal-kde screenshots"
            );
            crate::linux::atspi_hooks::run_event_loop()
        }
        DesktopKind::Generic => {
            tracing::info!("Using AT-SPI for window events (generic Wayland/X11 session)");
            crate::linux::atspi_hooks::run_event_loop()
        }
    }
}
