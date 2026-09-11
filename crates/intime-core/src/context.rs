//! Heuristics that turn window titles / product hints into document context.
//!
//! Browser URLs are **not** invented from titles — they come from the a11y
//! Document interface (AT-SPI / UI Automation). This module only copies a URL
//! when it is literally present in the title string, and parses editor paths.

use crate::models::EventMetadata;

/// Best-effort enrichment of document fields from a window title.
///
/// Does not invent site URLs from page names (e.g. "Home / X"). Only accepts a
/// URL that already appears as `http(s)://…` in the title.
pub fn enrich_from_window_title(meta: &mut EventMetadata, product_hint: Option<&str>) {
    let Some(title) = meta.window_title.clone() else {
        return;
    };
    if title.trim().is_empty() {
        return;
    }

    let hint = product_hint
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    if looks_like_browser(&hint, &title) {
        if meta.url.is_none() {
            if let Some(url) = extract_browser_url(&title) {
                meta.url = Some(url);
            }
        }
        if meta.document_name.is_none() {
            meta.document_name = Some(strip_browser_suffix(&title));
        }
        sanitize_stale_url_with_hint(meta, product_hint);
        return;
    }

    // Non-browser: never keep a leaked document URL on this event.
    sanitize_stale_url_with_hint(meta, product_hint);

    if looks_like_editor(&hint, &title) {
        if let Some((file, workspace)) = parse_editor_title(&title) {
            if meta.document_name.is_none() {
                meta.document_name = Some(file.clone());
            }
            if meta.document_path.is_none() && file.contains('/') {
                meta.document_path = Some(file);
            }
            if meta.workspace_path.is_none() {
                meta.workspace_path = workspace;
            }
        }
    }
}

/// Drop a document URL that clearly disagrees with the window / app.
///
/// AT-SPI sometimes returns a stale DocumentWeb URL from another window
/// (system trays, dialogs, editors). Document URLs only belong on browsers
/// or MPRIS player events.
pub fn sanitize_stale_url(meta: &mut EventMetadata) {
    sanitize_stale_url_with_hint(meta, None);
}

/// Drop leaked / conflicting document URLs.
pub fn sanitize_stale_url_with_hint(meta: &mut EventMetadata, product_hint: Option<&str>) {
    if meta.url.is_none() {
        return;
    }

    let hint = product_hint.unwrap_or("").to_ascii_lowercase();
    let path = meta
        .executable_path
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let title = meta.window_title.as_deref().unwrap_or("");
    let is_mpris = meta.focused_control_type.as_deref() == Some("mpris");

    // System trays / settings applets are never document surfaces.
    if is_system_utility_app(&hint, &path, title) {
        meta.url = None;
        return;
    }

    let url_ok_here = is_mpris
        || looks_like_browser(&hint, title)
        || path_looks_like_browser(&path);

    if !url_ok_here {
        meta.url = None;
        return;
    }

    let Some(url) = meta.url.as_deref() else {
        return;
    };
    if url_conflicts_with_title(url, title) {
        meta.url = None;
    }
}

/// System trays / connection dialogs — persist events, but never own a session.
pub fn is_system_utility_app(product_hint: &str, executable_path: &str, title: &str) -> bool {
    let blob = format!(
        "{} {} {}",
        product_hint.to_ascii_lowercase(),
        executable_path.to_ascii_lowercase(),
        title.to_ascii_lowercase()
    );
    const MARKERS: &[&str] = &[
        "blueman",
        "bluetooth",
        "pavucontrol",
        "pulseaudio",
        "volume control",
        "gnome-control-center",
        "nm-connection-editor",
        "networkmanager",
        "kdeconnect",
        "notify-osd",
        "xdg-desktop-portal",
        "polkit",
        "fwupd",
    ];
    MARKERS.iter().any(|m| blob.contains(m))
}

fn url_conflicts_with_title(url: &str, title: &str) -> bool {
    let Some(url_host) = url_host(url) else {
        return false;
    };
    let title_l = title.to_ascii_lowercase();
    let host_l = url_host.to_ascii_lowercase();

    // Site tokens in titles → expected host fragments.
    const HINTS: &[(&str, &[&str])] = &[
        ("netflix", &["netflix.com"]),
        ("youtube", &["youtube.com", "youtu.be"]),
        ("prime video", &["primevideo.com", "amazon."]),
        ("disney+", &["disneyplus.com", "disney."]),
        ("disney plus", &["disneyplus.com", "disney."]),
        ("hulu", &["hulu.com"]),
        ("twitch", &["twitch.tv"]),
        ("instagram", &["instagram.com"]),
        ("reddit", &["reddit.com"]),
        ("linkedin", &["linkedin.com"]),
        ("twitter", &["twitter.com", "x.com"]),
        (" / x", &["x.com", "twitter.com"]),
        ("gmail", &["mail.google.com", "gmail.com"]),
        ("google search", &["google."]),
        ("github", &["github.com"]),
    ];

    let mut title_expects: Option<&[&str]> = None;
    for (token, hosts) in HINTS {
        if title_l.contains(token) {
            title_expects = Some(hosts);
            break;
        }
    }
    let Some(expected) = title_expects else {
        return false;
    };
    !expected.iter().any(|h| host_l.contains(h))
}

fn path_looks_like_browser(path_l: &str) -> bool {
    ["brave", "chrome", "chromium", "firefox", "msedge", "opera", "vivaldi"]
        .iter()
        .any(|b| path_l.contains(b))
}

fn url_host(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split('/').next()?.trim();
    if host.is_empty() {
        None
    } else {
        Some(host.trim_start_matches("www.").to_string())
    }
}

fn looks_like_browser(hint: &str, title: &str) -> bool {
    const BROWSERS: &[&str] = &[
        "brave",
        "chrome",
        "chromium",
        "firefox",
        "edge",
        "safari",
        "vivaldi",
        "opera",
    ];
    BROWSERS.iter().any(|b| hint.contains(b))
        || BROWSERS
            .iter()
            .any(|b| title.to_ascii_lowercase().contains(b))
}

fn looks_like_editor_app(hint: &str) -> bool {
    const EDITORS: &[&str] = &[
        "code",
        "visual studio code",
        "cursor",
        "zed",
        "sublime",
        "jetbrains",
        "idea",
        "goland",
        "pycharm",
        "webstorm",
        "nvim",
        "vim",
        "emacs",
        "kate",
        "gedit",
    ];
    EDITORS.iter().any(|e| hint.contains(e))
}

fn looks_like_editor(hint: &str, title: &str) -> bool {
    if looks_like_editor_app(hint) {
        return true;
    }
    // Avoid classifying browser tabs that mention "Cursor" / "VS Code" as editors.
    if looks_like_browser(hint, title) {
        return false;
    }
    const EDITORS: &[&str] = &[
        "visual studio code",
        " - cursor",
        " — cursor",
        "nvim",
        "vim",
        "emacs",
        "jetbrains",
    ];
    let title_l = title.to_ascii_lowercase();
    EDITORS.iter().any(|e| title_l.contains(e))
}

fn looks_like_terminal(hint: &str, title: &str) -> bool {
    const TERMS: &[&str] = &[
        "alacritty",
        "kitty",
        "wezterm",
        "foot",
        "gnome-terminal",
        "konsole",
        "terminal",
        "wt.exe",
        "windows terminal",
    ];
    let title_l = title.to_ascii_lowercase();
    TERMS.iter().any(|t| hint.contains(t) || title_l.contains(t))
}

fn looks_like_database(hint: &str, title: &str) -> bool {
    const DB: &[&str] = &[
        "dbeaver",
        "datagrip",
        "pgadmin",
        "mysql workbench",
        "tableplus",
        "beekeeper",
        "azure data studio",
    ];
    let title_l = title.to_ascii_lowercase();
    DB.iter().any(|t| hint.contains(t) || title_l.contains(t))
}

fn looks_like_media(hint: &str, title: &str) -> bool {
    const MEDIA: &[&str] = &[
        "spotify",
        "vlc",
        "mpv",
        "rhythmbox",
        "youtube music",
        "youtube",
        "prime video",
        "netflix",
        "hulu",
        "disney",
        "twitch",
        "foobar2000",
        "jellyfin",
        "plex",
    ];
    let title_l = title.to_ascii_lowercase();
    MEDIA.iter().any(|t| hint.contains(t) || title_l.contains(t))
}

fn looks_like_meeting(hint: &str, title: &str) -> bool {
    const MEET: &[&str] = &[
        "zoom",
        "teams",
        "meet.google",
        "webex",
        "jitsi",
    ];
    let title_l = title.to_ascii_lowercase();
    MEET.iter().any(|t| hint.contains(t) || title_l.contains(t))
}

fn extract_browser_url(title: &str) -> Option<String> {
    // Only when the address is literally in the title, e.g. "Page - https://example.com - Brave"
    for part in title.split(" - ").map(str::trim) {
        if part.starts_with("http://") || part.starts_with("https://") {
            return Some(part.to_string());
        }
    }
    for token in title.split_whitespace() {
        let t = token.trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | '.' | ')' | '('));
        if t.starts_with("https://") || t.starts_with("http://") {
            return Some(t.to_string());
        }
    }
    None
}

fn strip_browser_suffix(title: &str) -> String {
    let mut parts: Vec<&str> = title.split(" - ").map(str::trim).collect();
    if parts.len() >= 2 {
        let last = parts.last().unwrap().to_ascii_lowercase();
        if looks_like_browser(&last, last.as_str()) {
            parts.pop();
        }
    }
    parts.join(" - ")
}

fn parse_editor_title(title: &str) -> Option<(String, Option<String>)> {
    // "main.rs — intime-rs — Cursor" or "main.rs - intime-rs - Visual Studio Code"
    let separators = [" — ", " - "];
    for sep in separators {
        let parts: Vec<&str> = title.split(sep).map(str::trim).filter(|p| !p.is_empty()).collect();
        if parts.len() >= 2 {
            let file = parts[0].to_string();
            let workspace = if parts.len() >= 3 {
                Some(parts[1].to_string())
            } else {
                None
            };
            return Some((file, workspace));
        }
    }
    None
}

/// Coarse activity intent guess used by heuristic session grouping.
pub fn guess_intent(meta: &EventMetadata, product_hint: Option<&str>) -> &'static str {
    let hint = product_hint.unwrap_or("").to_ascii_lowercase();
    let title = meta.window_title.as_deref().unwrap_or("").to_ascii_lowercase();

    if looks_like_meeting(&hint, &title) {
        return "meeting";
    }
    if looks_like_media(&hint, &title) {
        return "media";
    }
    if looks_like_database(&hint, &title) {
        return "database";
    }
    if looks_like_terminal(&hint, &title) {
        return "terminal";
    }

    // Prefer browser over editor when the product is a browser (e.g. "Cursor … - Brave").
    if looks_like_browser(&hint, &title) {
        if title.contains("figma") || hint.contains("figma") {
            return "design";
        }
        if title.contains("mail")
            || title.contains("outlook")
            || title.contains("slack")
            || title.contains("discord")
            || title.contains("gmail")
        {
            return "communication";
        }
        return "browsing";
    }

    if looks_like_editor(&hint, &title) || meta.document_path.is_some() {
        return "coding";
    }
    if looks_like_browser(&hint, &title) || meta.url.is_some() {
        return "browsing";
    }
    if hint.contains("figma") || hint.contains("inkscape") || hint.contains("gimp") {
        return "design";
    }
    if hint.contains("slack") || hint.contains("discord") || hint.contains("teams") {
        return "communication";
    }
    "unknown"
}

/// Infer a discrete UI action from an accessibility control name / role.
pub fn guess_ui_action_from_control(name: &str, role: Option<&str>) -> Option<crate::models::UiActionKind> {
    use crate::models::UiActionKind;
    let n = name.to_ascii_lowercase();
    let role = role.unwrap_or("").to_ascii_lowercase();

    if n.contains("cancel") || n.contains("dismiss") || n == "no" {
        return Some(UiActionKind::DialogCancel);
    }
    if n.contains("submit")
        || n.contains("sign in")
        || n.contains("log in")
        || n.contains("login")
        || n == "ok"
        || n == "okay"
        || (n.contains("send") && (role.contains("button") || role.contains("push")))
    {
        if n.contains("send") {
            return Some(UiActionKind::Send);
        }
        return Some(UiActionKind::FormSubmit);
    }
    if n.contains("save") {
        return Some(UiActionKind::Save);
    }
    if n.contains("download") {
        return Some(UiActionKind::Download);
    }
    if n.contains("upload") || n.contains("attach") {
        return Some(UiActionKind::Upload);
    }
    if n.contains("search") || n.contains("find") {
        return Some(UiActionKind::Search);
    }
    if n.contains("copy") {
        return Some(UiActionKind::Copy);
    }
    if n.contains("paste") {
        return Some(UiActionKind::Paste);
    }
    if n.contains("open") {
        return Some(UiActionKind::Open);
    }
    if n.contains("close") || n.contains("exit") || n.contains("quit") {
        return Some(UiActionKind::Close);
    }
    if n.contains("join") && (n.contains("meeting") || n.contains("call")) {
        return Some(UiActionKind::MeetingJoin);
    }
    if n.contains("play") || n.contains("pause") {
        return Some(UiActionKind::PlayMedia);
    }
    if role.contains("button") || role.contains("push") {
        // Generic activate — treat as dialog accept when named Yes/Confirm/Apply
        if n == "yes" || n.contains("confirm") || n.contains("apply") || n.contains("accept") {
            return Some(UiActionKind::DialogAccept);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::UiActionKind;

    #[test]
    fn parses_brave_youtube_title() {
        let mut meta = EventMetadata {
            window_title: Some(
                "(1) Inside The Rise - YouTube - Brave".into(),
            ),
            ..Default::default()
        };
        enrich_from_window_title(&mut meta, Some("brave-browser"));
        assert_eq!(
            meta.document_name.as_deref(),
            Some("(1) Inside The Rise - YouTube")
        );
    }

    #[test]
    fn parses_editor_title() {
        let mut meta = EventMetadata {
            window_title: Some("pipeline.rs — intime-rs — Cursor".into()),
            ..Default::default()
        };
        enrich_from_window_title(&mut meta, Some("cursor"));
        assert_eq!(meta.document_name.as_deref(), Some("pipeline.rs"));
        assert_eq!(meta.workspace_path.as_deref(), Some("intime-rs"));
        assert_eq!(guess_intent(&meta, Some("cursor")), "coding");
    }

    #[test]
    fn does_not_invent_urls_from_site_titles() {
        let mut home = EventMetadata {
            window_title: Some("Home / X - Brave".into()),
            ..Default::default()
        };
        enrich_from_window_title(&mut home, Some("brave-browser"));
        assert!(home.url.is_none());
        assert_eq!(home.document_name.as_deref(), Some("Home / X"));

        let mut ig = EventMetadata {
            window_title: Some("Instagram - Brave".into()),
            ..Default::default()
        };
        enrich_from_window_title(&mut ig, Some("brave-browser"));
        assert!(ig.url.is_none());
    }

    #[test]
    fn copies_literal_url_from_browser_title() {
        let mut meta = EventMetadata {
            window_title: Some("Docs - https://example.com/path - Brave".into()),
            ..Default::default()
        };
        enrich_from_window_title(&mut meta, Some("brave-browser"));
        assert_eq!(meta.url.as_deref(), Some("https://example.com/path"));
    }

    #[test]
    fn drops_stale_url_that_conflicts_with_title() {
        let mut meta = EventMetadata {
            window_title: Some("Community - Netflix - Brave".into()),
            url: Some("https://github.com/example/repo/tree/main".into()),
            ..Default::default()
        };
        sanitize_stale_url(&mut meta);
        assert!(meta.url.is_none());

        let mut ok = EventMetadata {
            window_title: Some("Community - Netflix - Brave".into()),
            url: Some("https://www.netflix.com/title/80100172".into()),
            ..Default::default()
        };
        sanitize_stale_url(&mut ok);
        assert_eq!(
            ok.url.as_deref(),
            Some("https://www.netflix.com/title/80100172")
        );
    }

    #[test]
    fn drops_document_url_on_non_browser_windows() {
        // AT-SPI can leak a DocumentWeb URL onto trays / dialogs / editors.
        let mut utility = EventMetadata {
            window_title: Some("Device-ABCD".into()),
            url: Some("https://www.youtube.com/watch?v=exampleVideoId".into()),
            executable_path: Some("/usr/bin/python3".into()),
            ..Default::default()
        };
        sanitize_stale_url_with_hint(&mut utility, Some("blueman-applet"));
        assert!(utility.url.is_none());
        assert!(is_system_utility_app(
            "blueman-applet",
            "/usr/bin/python3",
            "Device-ABCD"
        ));

        // No product hint: still drop — path/title are not a browser.
        let mut bare = EventMetadata {
            window_title: Some("Device-ABCD".into()),
            url: Some("https://www.youtube.com/watch?v=exampleVideoId".into()),
            executable_path: Some("/usr/bin/python3".into()),
            ..Default::default()
        };
        sanitize_stale_url(&mut bare);
        assert!(bare.url.is_none());

        let mut editor = EventMetadata {
            window_title: Some("main.rs — my-project — Cursor".into()),
            url: Some("https://www.netflix.com/watch/123".into()),
            executable_path: Some("/usr/share/cursor/cursor".into()),
            ..Default::default()
        };
        sanitize_stale_url_with_hint(&mut editor, Some("cursor"));
        assert!(editor.url.is_none());
    }

    #[test]
    fn keeps_mpris_url_with_bare_track_title() {
        let mut meta = EventMetadata {
            window_title: Some("Some Track Name".into()),
            url: Some("https://www.youtube.com/watch?v=exampleVideoId".into()),
            focused_control_type: Some("mpris".into()),
            automation_id: Some("brave".into()),
            ..Default::default()
        };
        sanitize_stale_url(&mut meta);
        assert_eq!(
            meta.url.as_deref(),
            Some("https://www.youtube.com/watch?v=exampleVideoId")
        );
    }

    #[test]
    fn browser_tab_about_cursor_is_browsing_not_coding() {
        let meta = EventMetadata {
            window_title: Some("Cursor - The best way to code with AI - Brave".into()),
            ..Default::default()
        };
        assert_eq!(guess_intent(&meta, Some("brave-browser")), "browsing");
    }

    #[test]
    fn terminal_and_database_intents() {
        let term = EventMetadata {
            window_title: Some("cargo run -p intime".into()),
            ..Default::default()
        };
        assert_eq!(guess_intent(&term, Some("Alacritty")), "terminal");

        let db = EventMetadata {
            window_title: Some("DBeaver 26.1.4 - event".into()),
            ..Default::default()
        };
        assert_eq!(guess_intent(&db, Some("DBeaver")), "database");
    }

    #[test]
    fn guesses_submit_from_button_name() {
        assert_eq!(
            guess_ui_action_from_control("Submit", Some("PushButton")),
            Some(UiActionKind::FormSubmit)
        );
        assert_eq!(
            guess_ui_action_from_control("Save Document", Some("Button")),
            Some(UiActionKind::Save)
        );
    }
}
