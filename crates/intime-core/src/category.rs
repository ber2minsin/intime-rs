//! Category taxonomy and activity-rule matching.
//!
//! Rules are seeded comprehensively and stored in SQLite so users (and later a
//! local LLM) can add/adjust mappings without code changes.

use serde::{Deserialize, Serialize};

/// How a rule was introduced — seeds ship with the app; `User`/`Llm` are edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleSource {
    Seed,
    User,
    Llm,
}

impl RuleSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Seed => "seed",
            Self::User => "user",
            Self::Llm => "llm",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "user" => Self::User,
            "llm" => Self::Llm,
            _ => Self::Seed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub occupation_tags: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityRule {
    pub id: i64,
    pub category_id: i64,
    pub match_field: String,
    pub match_op: String,
    pub pattern: String,
    pub priority: i64,
    pub enabled: bool,
    pub source: String,
    pub notes: Option<String>,
}

/// Fields available for rule matching (from app identity + event metadata).
#[derive(Debug, Clone, Default)]
pub struct CategoryMatchInput {
    pub aumid: Option<String>,
    pub product_name: Option<String>,
    pub display_name: Option<String>,
    pub company: Option<String>,
    pub executable_path: Option<String>,
    pub window_title: Option<String>,
    pub url: Option<String>,
    pub focused_control_type: Option<String>,
    pub automation_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CategoryHit {
    pub category_id: i64,
    pub rule_id: i64,
    pub priority: i64,
}

/// First enabled rule by descending priority that matches `input`.
pub fn match_category(rules: &[ActivityRule], input: &CategoryMatchInput) -> Option<CategoryHit> {
    let mut ranked: Vec<&ActivityRule> = rules.iter().filter(|r| r.enabled).collect();
    ranked.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(&b.id)));

    for rule in ranked {
        if rule_matches(rule, input) {
            return Some(CategoryHit {
                category_id: rule.category_id,
                rule_id: rule.id,
                priority: rule.priority,
            });
        }
    }
    None
}

fn field_value<'a>(rule: &ActivityRule, input: &'a CategoryMatchInput) -> Option<&'a str> {
    let v = match rule.match_field.as_str() {
        "aumid" => input.aumid.as_deref(),
        "product_name" => input.product_name.as_deref(),
        "display_name" => input.display_name.as_deref(),
        "company" => input.company.as_deref(),
        "executable_path" => input.executable_path.as_deref(),
        "window_title" => input.window_title.as_deref(),
        "url" => input.url.as_deref(),
        "focused_control_type" => input.focused_control_type.as_deref(),
        "automation_id" => input.automation_id.as_deref(),
        _ => None,
    }?;
    let t = v.trim();
    if t.is_empty() { None } else { Some(t) }
}

fn rule_matches(rule: &ActivityRule, input: &CategoryMatchInput) -> bool {
    let Some(value) = field_value(rule, input) else {
        return false;
    };
    let pattern = rule.pattern.trim();
    if pattern.is_empty() {
        return false;
    }
    let value_l = value.to_ascii_lowercase();
    let pattern_l = pattern.to_ascii_lowercase();
    match rule.match_op.as_str() {
        "equals" => value_l == pattern_l,
        "contains" => value_l.contains(&pattern_l),
        "prefix" => value_l.starts_with(&pattern_l),
        "regex" => regex_is_match(pattern, value),
        _ => false,
    }
}

fn regex_is_match(pattern: &str, value: &str) -> bool {
    // Avoid pulling regex crate: simple case-insensitive substring fallback for
    // malformed patterns; full regex when pattern looks plain enough.
    // For seed data we use contains/equals/prefix; regex is for user/LLM rules.
    match regex_lite_match(pattern, value) {
        Some(ok) => ok,
        None => value.to_ascii_lowercase().contains(&pattern.to_ascii_lowercase()),
    }
}

fn regex_lite_match(pattern: &str, value: &str) -> Option<bool> {
    // Minimal support: (?i) prefix and literal-ish patterns with .*
    let mut pat = pattern;
    let mut case_insensitive = false;
    if let Some(rest) = pat.strip_prefix("(?i)") {
        case_insensitive = true;
        pat = rest;
    }
    if pat.chars().any(|c| matches!(c, '[' | ']' | '(' | ')' | '{' | '}' | '|' | '+' | '?' | '\\')) {
        return None;
    }
    let hay = if case_insensitive {
        value.to_ascii_lowercase()
    } else {
        value.to_string()
    };
    let needle = if case_insensitive {
        pat.to_ascii_lowercase()
    } else {
        pat.to_string()
    };
    if let Some((a, b)) = needle.split_once(".*") {
        return Some(hay.contains(a) && hay.contains(b));
    }
    Some(hay.contains(&needle))
}

/// Categories where the media title (not just app) defines the merge context.
pub fn is_media_category(slug: &str) -> bool {
    matches!(
        slug,
        "media_watching"
            | "media_streaming_official"
            | "media_streaming_unofficial"
            | "media_local"
            | "music_listening"
            | "live_streaming"
    )
}

pub fn is_social_category(slug: &str) -> bool {
    matches!(
        slug,
        "social_long" | "social_short" | "social_messaging" | "content_creation"
    )
}

/// Build a stable session context key for merge decisions.
pub fn context_key(
    category_slug: &str,
    app_id: Option<i64>,
    document: Option<&str>,
    media_title: Option<&str>,
) -> String {
    if is_media_category(category_slug) {
        if let Some(title) = media_title.map(str::trim).filter(|s| !s.is_empty()) {
            return format!("media:{}", normalize_media_title(title));
        }
    }
    // Social sites: merge on site identity, not every post/tweet title.
    if is_social_category(category_slug) {
        if let Some(site) = social_site_key(document.or(media_title)) {
            return format!("social:{site}");
        }
        return format!("social:{category_slug}");
    }
    let doc = document.map(str::trim).filter(|s| !s.is_empty());
    match (app_id, doc) {
        (Some(app), Some(d)) => format!("app:{app}|doc:{}", normalize_key(d)),
        (Some(app), None) => format!("app:{app}"),
        (None, Some(d)) => format!("doc:{}", normalize_key(d)),
        (None, None) => format!("cat:{category_slug}"),
    }
}

fn social_site_key(hint: Option<&str>) -> Option<String> {
    let h = hint?.to_ascii_lowercase();
    if h.contains("x.com") || h.contains("twitter") || h.contains(" / x") || h.contains(" on x:") {
        return Some("x.com".into());
    }
    if h.contains("instagram") {
        return Some("instagram.com".into());
    }
    if h.contains("linkedin") {
        return Some("linkedin.com".into());
    }
    if h.contains("reddit") {
        return Some("reddit.com".into());
    }
    if h.contains("facebook") || h.contains("fb.com") {
        return Some("facebook.com".into());
    }
    if h.contains("tiktok") {
        return Some("tiktok.com".into());
    }
    if h.contains("youtube") {
        return Some("youtube.com".into());
    }
    if h.starts_with("http://") || h.starts_with("https://") {
        // host only
        let rest = h.split("://").nth(1)?;
        let host = rest.split('/').next()?.trim();
        if !host.is_empty() {
            return Some(host.trim_start_matches("www.").to_string());
        }
    }
    None
}

fn normalize_key(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

/// Strip browser / site chrome so MPRIS titles and window titles share one key.
pub fn normalize_media_title(title: &str) -> String {
    let mut t = title.trim().to_string();
    // Drop leading unread counts: "(1) Title"
    if t.starts_with('(') {
        if let Some(end) = t.find(')') {
            let rest = t[end + 1..].trim_start();
            if !rest.is_empty() {
                t = rest.to_string();
            }
        }
    }
    let lower = t.to_ascii_lowercase();
    const SUFFIXES: &[&str] = &[
        " - youtube - brave",
        " - youtube - google chrome",
        " - youtube - chromium",
        " - youtube - mozilla firefox",
        " - youtube - microsoft edge",
        " - youtube - firefox",
        " - youtube - chrome",
        " - youtube - edge",
        " - youtube - safari",
        " - youtube - opera",
        " - youtube - vivaldi",
        " | youtube",
        " - youtube",
        " — youtube",
        " – youtube",
        // MPRIS used to append " - brave" / " - firefox" to the track title.
        " - brave",
        " - firefox",
        " - chrome",
        " - chromium",
        " - edge",
        " - safari",
        " - opera",
        " - vivaldi",
        " - spotify",
    ];
    for suffix in SUFFIXES {
        if lower.ends_with(suffix) {
            t = t[..t.len() - suffix.len()].trim_end().to_string();
            break;
        }
    }
    normalize_key(&t)
}

/// Whether an event type is "meaningful" enough to count toward opening a session.
pub fn event_is_meaningful(event_type: &str) -> bool {
    matches!(
        event_type,
        "window_focus"
            | "title_change"
            | "text_changed"
            | "ui_action"
            | "idle_end"
    )
}

/// High-value events that can open a session immediately (no duration wait).
///
/// `pause_media` is intentionally excluded — pausing alone must not open a new
/// session when the user already left the media context.
pub fn event_is_high_value(event_type: &str, ui_kind: Option<&str>) -> bool {
    if matches!(event_type, "ui_action") {
        return matches!(
            ui_kind,
            Some("play_media" | "meeting_join" | "form_submit" | "save")
        );
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: i64, cat: i64, field: &str, op: &str, pat: &str, pri: i64) -> ActivityRule {
        ActivityRule {
            id,
            category_id: cat,
            match_field: field.into(),
            match_op: op.into(),
            pattern: pat.into(),
            priority: pri,
            enabled: true,
            source: "seed".into(),
            notes: None,
        }
    }

    #[test]
    fn higher_priority_media_beats_browsing() {
        let rules = vec![
            rule(1, 1, "product_name", "contains", "brave", 60),
            rule(2, 2, "window_title", "contains", "prime video", 130),
        ];
        let input = CategoryMatchInput {
            product_name: Some("brave-browser".into()),
            window_title: Some("Prime Video: Legally Blonde - Brave".into()),
            ..Default::default()
        };
        let hit = match_category(&rules, &input).unwrap();
        assert_eq!(hit.category_id, 2);
    }

    #[test]
    fn cursor_product_is_ai_coding_not_browser_title() {
        let rules = vec![
            rule(1, 10, "window_title", "contains", "cursor", 50),
            rule(2, 11, "product_name", "contains", "cursor", 115),
            rule(3, 12, "product_name", "contains", "brave", 60),
        ];
        let input = CategoryMatchInput {
            product_name: Some("Cursor".into()),
            window_title: Some("main.rs — intime-rs — Cursor".into()),
            ..Default::default()
        };
        assert_eq!(match_category(&rules, &input).unwrap().category_id, 11);
    }

    #[test]
    fn context_key_media_uses_title() {
        assert_eq!(
            context_key(
                "media_streaming_official",
                Some(1),
                None,
                Some("GABBAGOOBLINS - TV INTRO")
            ),
            "media:gabbagooblins - tv intro"
        );
        assert_eq!(
            context_key("media_local", None, None, Some("Track A")),
            "media:track a"
        );
        // MPRIS title and browser tab title must merge.
        assert_eq!(
            context_key(
                "media_streaming_official",
                None,
                None,
                Some("Stop Playing Kayle Reroll, Play This Instead")
            ),
            context_key(
                "media_streaming_official",
                Some(1),
                None,
                Some("(1) Stop Playing Kayle Reroll, Play This Instead - YouTube - Brave")
            )
        );
        assert_eq!(
            normalize_media_title("Stop Playing Kayle Reroll, Play This Instead - brave"),
            normalize_media_title(
                "(1) Stop Playing Kayle Reroll, Play This Instead - YouTube - Brave"
            )
        );
        assert_eq!(
            context_key("social_long", Some(1), Some("Home / X"), None),
            "social:x.com"
        );
        assert_eq!(
            context_key(
                "social_long",
                Some(1),
                Some("https://x.com/home"),
                Some("Home / X - Brave")
            ),
            "social:x.com"
        );
    }
}
