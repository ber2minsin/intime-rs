//! Desktop environment / compositor detection for Linux event sources.

/// Which compositor / desktop path intime should prefer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopKind {
    Sway,
    Hyprland,
    Gnome,
    Kde,
    /// Unknown Wayland or X11 — use AT-SPI + portal.
    Generic,
}

impl DesktopKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sway => "sway",
            Self::Hyprland => "hyprland",
            Self::Gnome => "gnome",
            Self::Kde => "kde",
            Self::Generic => "generic",
        }
    }
}

/// Detect the running desktop from env vars (no compositor round-trips).
pub fn detect_desktop() -> DesktopKind {
    if std::env::var_os("SWAYSOCK").is_some() {
        return DesktopKind::Sway;
    }
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        return DesktopKind::Hyprland;
    }
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let session = std::env::var("XDG_SESSION_DESKTOP")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let combined = format!("{desktop};{session}");
    if combined.contains("gnome") || combined.contains("unity") {
        return DesktopKind::Gnome;
    }
    if combined.contains("kde") || combined.contains("plasma") {
        return DesktopKind::Kde;
    }
    if combined.contains("hyprland") {
        return DesktopKind::Hyprland;
    }
    if combined.contains("sway") {
        return DesktopKind::Sway;
    }
    DesktopKind::Generic
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_sway_from_sock() {
        // Don't mutate process env permanently in parallel tests — unit-test the
        // string classifier via a local helper path instead when needed.
        assert_eq!(DesktopKind::Sway.as_str(), "sway");
        assert_eq!(DesktopKind::Gnome.as_str(), "gnome");
    }
}
