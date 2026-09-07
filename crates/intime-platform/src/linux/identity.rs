//! Best-effort Linux app identity: company / version / publisher.
//!
//! Mirrors Windows version-resource + Authenticode fields using:
//! - `.desktop` files (Name, StartupWMClass, X-Flatpak)
//! - AppStream / Flatpak metainfo (`developer`, `release version`)
//! - Path / reverse-DNS heuristics when metadata is thin

use std::fs;
use std::path::{Path, PathBuf};

use intime_core::models::{AppDetails, SignatureInfo, VersionInfo};

/// Fill `company_name`, `version_info`, `signature_info`, and prefer a stable `aumid`
/// when missing, without changing an already-computed fingerprint caller may hold.
///
/// Callers should compute `fingerprint()` **after** this enrichment.
pub fn enrich_app_details(mut details: AppDetails) -> AppDetails {
    let desktop = find_desktop_entry(
        details.aumid.as_deref(),
        details.product_name.as_deref(),
        &details.file_path,
    );

    if details.aumid.is_none() {
        if let Some(id) = desktop
            .as_ref()
            .and_then(|d| d.flatpak_id.clone().or(d.desktop_id.clone()))
        {
            details.aumid = Some(id);
        }
    }

    if details.product_name.as_deref().map(str::trim).filter(|s| !s.is_empty()).is_none() {
        if let Some(name) = desktop.as_ref().and_then(|d| d.name.clone()) {
            details.product_name = Some(name);
        }
    }

    let metainfo = desktop
        .as_ref()
        .and_then(|d| d.flatpak_id.clone().or(d.desktop_id.clone()))
        .as_deref()
        .and_then(find_metainfo)
        .or_else(|| {
            details
                .aumid
                .as_deref()
                .and_then(find_metainfo)
        });

    if details.company_name.is_none() {
        details.company_name = metainfo
            .as_ref()
            .and_then(|m| m.developer.clone())
            .or_else(|| desktop.as_ref().and_then(|d| company_from_desktop_id(d)))
            .or_else(|| company_from_path(&details.file_path))
            .or_else(|| {
                details
                    .aumid
                    .as_deref()
                    .and_then(company_from_reverse_dns)
            });
    }

    let version = metainfo
        .as_ref()
        .and_then(|m| m.version.clone())
        .or_else(|| version_from_path(&details.file_path));

    if details.version_info.is_none() && (version.is_some() || desktop.is_some()) {
        details.version_info = Some(VersionInfo {
            file_description: desktop.as_ref().and_then(|d| d.comment.clone()),
            product_version: version.clone(),
            file_version: version,
            original_filename: Path::new(&details.file_path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned()),
            internal_name: desktop
                .as_ref()
                .and_then(|d| d.startup_wm_class.clone().or(d.desktop_id.clone())),
            legal_copyright: None,
        });
    }

    if details.signature_info.is_none() {
        if let Some(publisher) = details.company_name.clone().or_else(|| {
            metainfo.as_ref().and_then(|m| m.developer.clone())
        }) {
            details.signature_info = Some(SignatureInfo {
                publisher: Some(publisher),
                subject_full: metainfo.as_ref().and_then(|m| m.developer_id.clone()),
                issuer: None,
                serial_number: None,
            });
        }
    }

    details
}

#[derive(Debug, Clone, Default)]
struct DesktopEntry {
    #[allow(dead_code)]
    path: PathBuf,
    desktop_id: Option<String>,
    name: Option<String>,
    comment: Option<String>,
    startup_wm_class: Option<String>,
    flatpak_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct Metainfo {
    developer: Option<String>,
    developer_id: Option<String>,
    version: Option<String>,
}

fn find_desktop_entry(
    aumid: Option<&str>,
    product: Option<&str>,
    file_path: &str,
) -> Option<DesktopEntry> {
    let exe_stem = Path::new(file_path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned());

    let mut candidates: Vec<String> = Vec::new();
    for key in [aumid, product, exe_stem.as_deref()].into_iter().flatten() {
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        candidates.push(format!("{key}.desktop"));
        // Flatpak / reverse-DNS ids are already full desktop ids.
        if key.contains('.') {
            candidates.push(format!("{key}.desktop"));
        }
    }

    for dir in desktop_search_dirs() {
        // Exact filename matches first.
        for name in &candidates {
            let path = dir.join(name);
            if path.is_file() {
                if let Some(entry) = parse_desktop_file(&path) {
                    return Some(entry);
                }
            }
        }

        // Scan for StartupWMClass / Name / X-Flatpak matches.
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Some(parsed) = parse_desktop_file(&path) else {
                continue;
            };
            if desktop_matches(&parsed, aumid, product, exe_stem.as_deref()) {
                return Some(parsed);
            }
        }
    }
    None
}

fn desktop_matches(
    entry: &DesktopEntry,
    aumid: Option<&str>,
    product: Option<&str>,
    exe_stem: Option<&str>,
) -> bool {
    let keys: Vec<&str> = [aumid, product, exe_stem]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if keys.is_empty() {
        return false;
    }

    for key in &keys {
        let key_l = key.to_ascii_lowercase();
        if entry
            .desktop_id
            .as_deref()
            .is_some_and(|id| id.eq_ignore_ascii_case(key))
            || entry
                .flatpak_id
                .as_deref()
                .is_some_and(|id| id.eq_ignore_ascii_case(key))
            || entry
                .startup_wm_class
                .as_deref()
                .is_some_and(|c| c.eq_ignore_ascii_case(key))
            || entry
                .name
                .as_deref()
                .is_some_and(|n| n.eq_ignore_ascii_case(key))
        {
            return true;
        }
        // "brave-browser" vs desktop id "com.brave.Browser"
        if entry
            .desktop_id
            .as_deref()
            .or(entry.flatpak_id.as_deref())
            .is_some_and(|id| id.to_ascii_lowercase().contains(&key_l.replace('-', "."))
                || key_l.contains(&id.to_ascii_lowercase().rsplit('.').next().unwrap_or("")))
        {
            return true;
        }
    }
    false
}

fn desktop_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(&home).join(".local/share/applications"));
        dirs.push(PathBuf::from(&home).join(".local/share/flatpak/exports/share/applications"));
    }
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    dirs.push(PathBuf::from("/usr/local/share/applications"));
    dirs.push(PathBuf::from("/usr/share/applications"));
    if let Ok(xdg) = std::env::var("XDG_DATA_DIRS") {
        for part in xdg.split(':').filter(|s| !s.is_empty()) {
            dirs.push(PathBuf::from(part).join("applications"));
        }
    }
    dirs
}

fn parse_desktop_file(path: &Path) -> Option<DesktopEntry> {
    let text = fs::read_to_string(path).ok()?;
    let desktop_id = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned());

    let mut entry = DesktopEntry {
        path: path.to_path_buf(),
        desktop_id,
        ..Default::default()
    };

    let mut in_desktop_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        // Skip localized keys like Name[de]=...
        if key.contains('[') {
            continue;
        }
        match key {
            "Name" if entry.name.is_none() => entry.name = Some(value.to_string()),
            "Comment" if entry.comment.is_none() => entry.comment = Some(value.to_string()),
            "StartupWMClass" => entry.startup_wm_class = Some(value.to_string()),
            "X-Flatpak" => entry.flatpak_id = Some(value.to_string()),
            _ => {}
        }
    }
    Some(entry)
}

fn find_metainfo(app_id: &str) -> Option<Metainfo> {
    let names = [
        format!("{app_id}.metainfo.xml"),
        format!("{app_id}.appdata.xml"),
    ];
    for dir in metainfo_search_dirs() {
        for name in &names {
            let path = dir.join(name);
            if path.is_file() {
                if let Some(info) = parse_metainfo(&path) {
                    return Some(info);
                }
            }
        }
    }
    None
}

fn metainfo_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(&home).join(".local/share/metainfo"));
        dirs.push(PathBuf::from(&home).join(".local/share/appdata"));
        dirs.push(
            PathBuf::from(&home).join(".local/share/flatpak/exports/share/metainfo"),
        );
    }
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/metainfo"));
    dirs.push(PathBuf::from("/usr/share/metainfo"));
    dirs.push(PathBuf::from("/usr/share/appdata"));
    if let Ok(xdg) = std::env::var("XDG_DATA_DIRS") {
        for part in xdg.split(':').filter(|s| !s.is_empty()) {
            dirs.push(PathBuf::from(part).join("metainfo"));
            dirs.push(PathBuf::from(part).join("appdata"));
        }
    }
    dirs
}

fn parse_metainfo(path: &Path) -> Option<Metainfo> {
    let text = fs::read_to_string(path).ok()?;
    let developer = extract_xml_tag_block(&text, "developer")
        .and_then(|block| extract_xml_tag(&block, "name"))
        .or_else(|| extract_xml_tag(&text, "developer_name"));
    let developer_id = extract_attr(&text, "developer", "id");
    let version = extract_release_version(&text);

    if developer.is_none() && version.is_none() {
        return None;
    }
    Some(Metainfo {
        developer,
        developer_id,
        version,
    })
}

fn extract_release_version(text: &str) -> Option<String> {
    // Match `<release …>` not the `<releases>` wrapper.
    let mut search = text;
    while let Some(idx) = search.find("<release") {
        let slice = &search[idx..];
        // `<releases` starts with `<release` — skip it.
        let next = slice.get(8..9).unwrap_or("");
        if next != " " && next != ">" && next != "\n" && next != "\t" && next != "/" {
            search = &slice[8..];
            continue;
        }
        let end = slice.find('>').unwrap_or(slice.len().min(240));
        let head = &slice[..end];
        let key = "version=\"";
        if let Some(a) = head.find(key) {
            let a = a + key.len();
            if let Some(rel) = head[a..].find('"') {
                let value = head[a..a + rel].trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        search = &slice[8..];
    }
    None
}

fn extract_xml_tag(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = text.find(&open)?;
    let after_open = text[start..].find('>')? + start + 1;
    let end = text[after_open..].find(&close)? + after_open;
    let value = text[after_open..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn extract_xml_tag_block(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = text.find(&open)?;
    let end = text[start..].find(&close)? + start + close.len();
    Some(text[start..end].to_string())
}

fn extract_attr(text: &str, tag: &str, attr: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = text.find(&open)?;
    let slice = &text[start..];
    let end = slice.find('>').unwrap_or(slice.len().min(300));
    let head = &slice[..end];
    let key = format!("{attr}=\"");
    let a = head.find(&key)? + key.len();
    let b = head[a..].find('"')? + a;
    let value = head[a..b].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn company_from_desktop_id(entry: &DesktopEntry) -> Option<String> {
    entry
        .flatpak_id
        .as_deref()
        .or(entry.desktop_id.as_deref())
        .and_then(company_from_reverse_dns)
}

fn company_from_reverse_dns(id: &str) -> Option<String> {
    // io.dbeaver.DBeaverCommunity → known map or "dbeaver"
    let lower = id.to_ascii_lowercase();
    const KNOWN: &[(&str, &str)] = &[
        ("brave", "Brave Software"),
        ("mozilla", "Mozilla"),
        ("google", "Google"),
        ("microsoft", "Microsoft"),
        ("dbeaver", "DBeaver Corporation"),
        ("jetbrains", "JetBrains"),
        ("gnome", "GNOME"),
        ("kde", "KDE"),
        ("canonical", "Canonical"),
        ("discord", "Discord"),
        ("slack", "Slack Technologies"),
        ("spotify", "Spotify"),
        ("valve", "Valve"),
        ("telegram", "Telegram"),
    ];
    for (needle, company) in KNOWN {
        if lower.contains(needle) {
            return Some((*company).to_string());
        }
    }

    // org.kde.dolphin → KDE-style: use second label capitalized when first is org/com/io/net
    let parts: Vec<&str> = id.split('.').collect();
    if parts.len() >= 2 {
        let vendor = parts[1];
        if !vendor.is_empty() && vendor.len() > 1 {
            let mut chars = vendor.chars();
            let first = chars.next()?.to_ascii_uppercase();
            return Some(format!("{first}{}", chars.as_str()));
        }
    }
    None
}

fn company_from_path(path: &str) -> Option<String> {
    let lower = path.to_ascii_lowercase();
    const RULES: &[(&str, &str)] = &[
        ("brave.com", "Brave Software"),
        ("google", "Google"),
        ("mozilla", "Mozilla"),
        ("microsoft", "Microsoft"),
        ("discord", "Discord"),
        ("slack", "Slack Technologies"),
        ("jetbrains", "JetBrains"),
        ("spotify", "Spotify"),
        ("dropbox", "Dropbox"),
        ("zoom", "Zoom"),
    ];
    for (needle, company) in RULES {
        if lower.contains(needle) {
            return Some((*company).to_string());
        }
    }
    None
}

fn version_from_path(path: &str) -> Option<String> {
    // Flatpak export paths sometimes embed nothing useful; skip.
    // Chromium-style installs may ship a sibling VERSION file.
    let dir = Path::new(path).parent()?;
    for name in ["VERSION", "version", "PRODUCT_VERSION"] {
        let candidate = dir.join(name);
        if let Ok(text) = fs::read_to_string(&candidate) {
            let v = text.lines().next().unwrap_or("").trim();
            if !v.is_empty() && v.len() < 64 {
                return Some(v.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_metainfo_developer_and_version() {
        let path = std::env::temp_dir().join("intime_test_dbeaver.metainfo.xml");
        let mut f = fs::File::create(&path).unwrap();
        write!(
            f,
            r#"<?xml version="1.0"?>
<component>
  <name>DBeaver Community</name>
  <developer id="io.dbeaver">
    <name>DBeaver Corporation</name>
  </developer>
  <releases>
    <release version="26.1.4" date="2026-08-02"/>
  </releases>
</component>"#
        )
        .unwrap();
        let info = parse_metainfo(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert_eq!(info.developer.as_deref(), Some("DBeaver Corporation"));
        assert_eq!(info.developer_id.as_deref(), Some("io.dbeaver"));
        assert_eq!(info.version.as_deref(), Some("26.1.4"));
    }

    #[test]
    fn company_from_brave_path() {
        assert_eq!(
            company_from_path("/opt/brave.com/brave/brave").as_deref(),
            Some("Brave Software")
        );
    }

    #[test]
    fn reverse_dns_dbeaver() {
        assert_eq!(
            company_from_reverse_dns("io.dbeaver.DBeaverCommunity").as_deref(),
            Some("DBeaver Corporation")
        );
    }

    #[test]
    fn enrich_fills_company_from_path_when_no_desktop() {
        let details = AppDetails {
            title: "Brave".into(),
            file_path: "/opt/brave.com/brave/brave".into(),
            aumid: Some("brave-browser".into()),
            company_name: None,
            product_name: Some("brave-browser".into()),
            version_info: None,
            signature_info: None,
        };
        let enriched = enrich_app_details(details);
        assert_eq!(enriched.company_name.as_deref(), Some("Brave Software"));
        assert!(enriched.signature_info.is_some());
    }
}
