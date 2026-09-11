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

/// IDE / coding categories that should merge on workspace or repo identity.
pub fn is_coding_category(slug: &str) -> bool {
    matches!(
        slug,
        "code_editing"
            | "ai_coding"
            | "terminal"
            | "database"
            | "devops"
            | "api_testing"
            | "code_collaboration"
    )
}

pub fn is_design_category(slug: &str) -> bool {
    matches!(slug, "design_2d" | "vfx_3d" | "video_editing")
}

pub fn is_gaming_category(slug: &str) -> bool {
    matches!(slug, "gaming")
}

/// Build a stable session context key for merge decisions.
///
/// Prefer project-level identity when available so LLM summaries can group
/// "all work on repo X" or "design file Y" without inventing joins:
/// - media → URL content id / title
/// - social → site host
/// - coding → `ws:{workspace}` or `repo:github:{owner}/{repo}`
/// - design → `figma:{file_key}` when present
/// - gaming → `game:{title}` (not launcher chrome)
pub fn context_key(
    category_slug: &str,
    app_id: Option<i64>,
    document: Option<&str>,
    media_title: Option<&str>,
) -> String {
    context_key_with_workspace(category_slug, app_id, document, media_title, None)
}

/// Like [`context_key`] but prefers IDE workspace / project name when set.
pub fn context_key_with_workspace(
    category_slug: &str,
    app_id: Option<i64>,
    document: Option<&str>,
    media_title: Option<&str>,
    workspace: Option<&str>,
) -> String {
    if is_media_category(category_slug) {
        return media_context_key(document, media_title);
    }
    if is_social_category(category_slug) {
        return social_context_key(document, media_title);
    }

    // Repo / design / project keys win over raw document paths.
    if let Some(project) = project_context_key(category_slug, document, workspace, media_title) {
        return project;
    }

    if is_gaming_category(category_slug) {
        if let Some(game) = gaming_context_key(document, media_title) {
            return game;
        }
    }

    let doc = document.map(str::trim).filter(|s| !s.is_empty());
    match (app_id, doc) {
        (Some(app), Some(d)) => format!("app:{app}|doc:{}", normalize_key(d)),
        (Some(app), None) => format!("app:{app}"),
        (None, Some(d)) => format!("doc:{}", normalize_key(d)),
        (None, None) => format!("cat:{category_slug}"),
    }
}

/// Project-level identity for coding / design / VCS browsing.
pub fn project_context_key(
    category_slug: &str,
    document_or_url: Option<&str>,
    workspace: Option<&str>,
    title: Option<&str>,
) -> Option<String> {
    let hint = document_or_url.or(title);
    if let Some(repo) = parse_git_host_repo(hint) {
        // Issues/projects boards stay page-level via caller category; still
        // group by repo for summarization.
        return Some(format!("repo:{repo}"));
    }
    if is_design_category(category_slug) || category_slug == "browsing" || category_slug == "content_creation"
    {
        if let Some(key) = parse_figma_file_key(hint) {
            return Some(format!("figma:{key}"));
        }
    }
    if is_coding_category(category_slug) {
        if let Some(ws) = workspace.map(str::trim).filter(|s| !s.is_empty()) {
            return Some(format!("ws:{}", normalize_key(ws)));
        }
        // Editor title "file — workspace — Cursor" without enriched workspace.
        if let Some(ws) = workspace_from_editor_title(title) {
            return Some(format!("ws:{}", normalize_key(&ws)));
        }
        // Terminal/nvim titles: "nvim ~/D/w/r/intime-rs" or "cargo run … ~/…/intime-rs".
        if let Some(ws) = workspace_from_path_in_title(title) {
            return Some(format!("ws:{}", normalize_key(&ws)));
        }
    }
    None
}

fn gaming_context_key(document: Option<&str>, title: Option<&str>) -> Option<String> {
    let raw = title.or(document)?.trim();
    if raw.is_empty() {
        return None;
    }
    let n = normalize_media_title(raw);
    if is_weak_launcher_title(&n) {
        return None;
    }
    Some(format!("game:{n}"))
}

fn is_weak_launcher_title(normalized: &str) -> bool {
    matches!(
        normalized,
        "steam"
            | "steam big picture"
            | "epic games launcher"
            | "epic games"
            | "riot client"
            | "battle.net"
            | "battle.net launcher"
            | "ubisoft connect"
            | "ea app"
            | "ea desktop"
            | "gog galaxy"
            | "lutris"
            | "heroic games launcher"
            | "heroic"
            | "xbox"
            | "geforce now"
    )
}

/// Parse `github.com/owner/repo` (and GitLab/Bitbucket) into `host:owner/repo`.
pub fn parse_git_host_repo(hint: Option<&str>) -> Option<String> {
    let h = hint?.trim();
    if h.is_empty() {
        return None;
    }
    let lower = h.to_ascii_lowercase();
    for host in ["github.com", "gitlab.com", "bitbucket.org", "codeberg.org"] {
        let Some(idx) = lower.find(host) else {
            continue;
        };
        let after = h.get(idx + host.len()..)?;
        let after = after.trim_start_matches('/');
        let mut parts = after.split('/');
        let owner = parts.next()?.trim();
        let repo_raw = parts.next()?.trim();
        let repo = repo_raw
            .trim_end_matches(".git")
            .split('?')
            .next()?
            .split('#')
            .next()?
            .trim();
        if owner.is_empty() || repo.is_empty() {
            continue;
        }
        if matches!(
            owner,
            "features"
                | "pricing"
                | "enterprise"
                | "about"
                | "login"
                | "signup"
                | "security"
                | "settings"
                | "notifications"
                | "marketplace"
                | "explore"
                | "topics"
                | "orgs"
                | "pulls"
                | "issues"
                | "new"
                | "organizations"
        ) {
            continue;
        }
        if !owner
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
            || !repo
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            continue;
        }
        return Some(format!("{host}:{owner}/{repo}"));
    }
    None
}

pub fn parse_figma_file_key(hint: Option<&str>) -> Option<String> {
    let h = hint?.trim();
    let lower = h.to_ascii_lowercase();
    for marker in ["/file/", "/design/", "/proto/", "/board/"] {
        if let Some(idx) = lower.find(marker) {
            let start = idx + marker.len();
            let rest = h.get(start..)?;
            let key = rest
                .split('/')
                .next()?
                .split('?')
                .next()?
                .trim();
            if key.len() >= 6
                && key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Some(key.to_string());
            }
        }
    }
    None
}

fn workspace_from_editor_title(title: Option<&str>) -> Option<String> {
    let t = title?.trim();
    for sep in [" — ", " - "] {
        let parts: Vec<&str> = t.split(sep).map(str::trim).filter(|p| !p.is_empty()).collect();
        // "file — workspace — Cursor" or "file — workspace — Visual Studio Code"
        if parts.len() >= 3 {
            let last = parts.last()?.to_ascii_lowercase();
            if last.contains("cursor")
                || last.contains("code")
                || last.contains("zed")
                || last.contains("idea")
                || last.contains("jetbrains")
                || last.contains("sublime")
                || last.contains("neovim")
                || last.contains("vim")
            {
                return Some(parts[1].to_string());
            }
        }
    }
    None
}

/// Pull a workspace folder name from terminal/editor titles that embed a path.
///
/// Examples: `nvim ~/D/w/r/intime-rs`, `nvim . ~/proj/foo`, `cargo run -p x ~/proj/foo`.
fn workspace_from_path_in_title(title: Option<&str>) -> Option<String> {
    let t = title?.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    // Prefer titles that look like an editor/shell in a project dir.
    let looks_coding = lower.contains("nvim")
        || lower.contains("vim")
        || lower.contains("cargo ")
        || lower.contains("make ")
        || lower.starts_with('~')
        || lower.contains(" ~/")
        || lower.contains(" /home/")
        || lower.contains(" /users/");
    if !looks_coding {
        return None;
    }
    let mut best: Option<&str> = None;
    for token in t.split_whitespace() {
        let tok = token.trim_matches(|c: char| c == ',' || c == ';' || c == ')');
        if !(tok.starts_with('~') || tok.starts_with('/')) {
            continue;
        }
        if tok.len() < 2 {
            continue;
        }
        best = Some(tok);
    }
    let path = best?;
    let name = path
        .trim_end_matches('/')
        .rsplit('/')
        .next()?
        .trim();
    if name.is_empty() || name == "~" || name == "." || name == ".." {
        return None;
    }
    // Avoid treating home dir itself as the workspace.
    if matches!(name, "home" | "Users" | "users") {
        return None;
    }
    Some(name.to_string())
}

/// Prefer official streaming over generic local when MPRIS + browser tab collide.
pub fn prefer_media_category_slug(prev: &str, new: &str) -> String {
    const RANK: &[(&str, u8)] = &[
        ("media_streaming_official", 5),
        ("media_streaming_unofficial", 4),
        ("live_streaming", 4),
        ("media_watching", 3),
        ("music_listening", 3),
        ("media_local", 1),
    ];
    let rank = |s: &str| RANK.iter().find(|(n, _)| *n == s).map(|(_, r)| *r).unwrap_or(0);
    if rank(new) >= rank(prev) {
        new.to_string()
    } else {
        prev.to_string()
    }
}

/// Prefer URL-id / strong title keys when refining an open media session.
pub fn prefer_media_context_key(prev: &str, new: &str) -> String {
    if prev == new {
        return prev.to_string();
    }
    let score = |k: &str| -> u8 {
        if k.starts_with("media:yt:") || k.starts_with("media:nf:") {
            4
        } else if is_strong_media_context_key(k) {
            3
        } else if k.starts_with("media:url:") {
            2
        } else if k.starts_with("media:weak:") {
            1
        } else if k.starts_with("media:") {
            3
        } else {
            0
        }
    };
    if score(new) > score(prev) {
        new.to_string()
    } else {
        prev.to_string()
    }
}

/// Whether a media session should continue instead of splitting on category/key churn.
pub fn should_continue_media_session(
    prev_key: &str,
    new_key: &str,
    prev_title: Option<&str>,
    new_title: Option<&str>,
) -> bool {
    if prev_key == new_key {
        return true;
    }
    if media_context_equivalent(prev_key, new_key, prev_title, new_title) {
        return true;
    }
    // Browse chrome → concrete show/id for the same site.
    let prev_weak = prev_key.starts_with("media:weak:");
    let new_strong = is_strong_media_context_key(new_key) || new_key.starts_with("media:yt:") || new_key.starts_with("media:nf:");
    if prev_weak && new_strong {
        return true;
    }
    false
}

/// One-line session summary for closed rows (LLM / UI friendly).
pub fn session_summary_line(
    category_slug: &str,
    title: Option<&str>,
    context_key: &str,
) -> String {
    let title_bit = title
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            let n = normalize_media_title(s);
            if n.len() > 80 {
                format!("{}…", &n[..77])
            } else {
                n
            }
        })
        .unwrap_or_else(|| context_key.to_string());
    format!("{category_slug} · {title_bit} · {context_key}")
}

fn media_context_key(document_or_url: Option<&str>, media_title: Option<&str>) -> String {
    if let Some(id) = media_content_id(document_or_url) {
        return id;
    }
    let normalized = media_title
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(normalize_media_title)
        .filter(|s| !s.is_empty());
    if let Some(title) = normalized {
        if !is_weak_media_title(&title) {
            return format!("media:{title}");
        }
        // Weak chrome title: still try URL path as a last resort identity.
        if let Some(url) = document_or_url.map(str::trim).filter(|u| looks_like_url(u)) {
            return format!("media:url:{}", normalize_key(url));
        }
        return format!("media:weak:{title}");
    }
    if let Some(url) = document_or_url.map(str::trim).filter(|u| looks_like_url(u)) {
        return format!("media:url:{}", normalize_key(url));
    }
    "media:unknown".into()
}

/// Extract a stable content id from a media URL when possible.
pub fn media_content_id(url_or_doc: Option<&str>) -> Option<String> {
    let raw = url_or_doc?.trim();
    if raw.is_empty() {
        return None;
    }
    // YouTube watch / short / embed / youtu.be
    if let Some(id) = youtube_video_id(raw) {
        return Some(format!("media:yt:{id}"));
    }
    // Netflix title / watch paths: /title/80100172 or /watch/80100172
    let lower = raw.to_ascii_lowercase();
    if lower.contains("netflix.com") {
        if let Some(id) = path_id_after(raw, &["/title/", "/watch/", "/Title/", "/Watch/"]) {
            return Some(format!("media:nf:{id}"));
        }
    }
    None
}

fn youtube_video_id(url: &str) -> Option<String> {
    let lower = url.to_ascii_lowercase();
    if let Some(idx) = lower.find("youtu.be/") {
        let start = idx + "youtu.be/".len();
        let id = extract_id_at(url, start)?;
        if is_plausible_yt_id(&id) {
            return Some(id);
        }
    }
    for marker in ["watch?v=", "watch?vi=", "&v=", "?v=", "/embed/", "/shorts/"] {
        if let Some(idx) = lower.find(marker) {
            let start = idx + marker.len();
            let id = extract_id_at(url, start)?;
            if is_plausible_yt_id(&id) {
                return Some(id);
            }
        }
    }
    None
}

fn extract_id_at(url: &str, start: usize) -> Option<String> {
    let rest = url.get(start..)?;
    let id = rest
        .split(|c| c == '?' || c == '/' || c == '&' || c == '#')
        .next()?
        .trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn is_plausible_yt_id(id: &str) -> bool {
    let len = id.len();
    (6..=20).contains(&len) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn path_id_after(url: &str, markers: &[&str]) -> Option<String> {
    let lower = url.to_ascii_lowercase();
    for marker in markers {
        let marker_l = marker.to_ascii_lowercase();
        if let Some(idx) = lower.find(&marker_l) {
            let start = idx + marker.len();
            if let Some(id) = extract_id_at(url, start) {
                if id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                    return Some(id);
                }
            }
        }
    }
    None
}

fn looks_like_url(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    l.starts_with("http://") || l.starts_with("https://")
}

/// Titles that are only site/browser chrome — not a specific video/show.
pub fn is_weak_media_title(normalized: &str) -> bool {
    matches!(
        normalized,
        "youtube"
            | "netflix"
            | "home - netflix"
            | "home"
            | "prime video"
            | "amazon prime video"
            | "disney+"
            | "disney plus"
            | "hulu"
            | "twitch"
            | "spotify"
            | "youtube music"
            | "media"
            | "unknown"
            | "stopped"
    ) || normalized.starts_with("home - ")
        && matches!(
            normalized.strip_prefix("home - ").unwrap_or(""),
            "netflix" | "youtube" | "prime video" | "hulu" | "disney+" | "disney plus"
        )
        || normalized.ends_with(": stopped")
}

/// Prefer URL-id keys (`media:yt:…` / `media:nf:…`) over title keys for reopen.
pub fn is_strong_media_context_key(key: &str) -> bool {
    key.starts_with("media:yt:")
        || key.starts_with("media:nf:")
        || (key.starts_with("media:")
            && !key.starts_with("media:weak:")
            && !key.starts_with("media:url:")
            && !key.starts_with("media:unknown")
            && !is_weak_media_title(key.trim_start_matches("media:")))
}

/// Whether two media context keys refer to the same content.
///
/// Allows merging a URL-id key with a strong title key when the normalized
/// titles match (MPRIS play with watch URL + later browser focus without URL).
pub fn media_context_equivalent(
    prev_key: &str,
    new_key: &str,
    prev_title: Option<&str>,
    new_title: Option<&str>,
) -> bool {
    if prev_key == new_key {
        return true;
    }
    if !(prev_key.starts_with("media:") && new_key.starts_with("media:")) {
        return false;
    }
    let prev_n = prev_title
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(normalize_media_title);
    let new_n = new_title
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(normalize_media_title);
    match (prev_n, new_n) {
        (Some(a), Some(b)) if a == b && !is_weak_media_title(&a) => true,
        _ => false,
    }
}

/// Pick the richer of window title vs MPRIS/UI label for media identity.
pub fn pick_richer_media_title(window_title: Option<&str>, label: Option<&str>) -> Option<String> {
    let win = window_title.map(str::trim).filter(|s| !s.is_empty());
    let lab = label.map(str::trim).filter(|s| !s.is_empty());
    match (win, lab) {
        (Some(w), Some(l)) => {
            let nw = normalize_media_title(w);
            let nl = normalize_media_title(l);
            let w_weak = is_weak_media_title(&nw);
            let l_weak = is_weak_media_title(&nl);
            if w_weak && !l_weak {
                Some(l.to_string())
            } else if l_weak && !w_weak {
                Some(w.to_string())
            } else if nl.len() > nw.len() {
                Some(l.to_string())
            } else {
                Some(w.to_string())
            }
        }
        (Some(w), None) => Some(w.to_string()),
        (None, Some(l)) => Some(l.to_string()),
        (None, None) => None,
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
        let rest = h.split("://").nth(1)?;
        let host = rest.split('/').next()?.trim();
        if !host.is_empty() {
            return Some(host.trim_start_matches("www.").to_string());
        }
    }
    None
}

/// Social identity: status id > concrete post title > bare site feed.
fn social_context_key(document: Option<&str>, title: Option<&str>) -> String {
    let site = social_site_key(document.or(title)).unwrap_or_else(|| "unknown".into());
    if let Some(id) = parse_x_status_id(document) {
        return format!("social:{site}:status:{id}");
    }
    if let Some(t) = title.map(normalize_media_title).filter(|t| !t.is_empty()) {
        if !is_weak_social_title(&t) {
            let short = if t.chars().count() > 72 {
                let s: String = t.chars().take(69).collect();
                format!("{s}…")
            } else {
                t
            };
            return format!("social:{site}:{short}");
        }
    }
    format!("social:{site}")
}

fn parse_x_status_id(url: Option<&str>) -> Option<String> {
    let raw = url?.trim();
    let lower = raw.to_ascii_lowercase();
    for marker in ["/status/", "/statuses/"] {
        if let Some(idx) = lower.find(marker) {
            let start = idx + marker.len();
            let id = extract_id_at(raw, start)?;
            if id.chars().all(|c| c.is_ascii_digit()) && id.len() >= 6 {
                return Some(id);
            }
        }
    }
    None
}

/// Feed chrome titles that must not replace a concrete post/article title.
pub fn is_weak_social_title(normalized: &str) -> bool {
    matches!(
        normalized,
        "x"
            | "twitter"
            | "home / x"
            | "home"
            | "explore / x"
            | "explore"
            | "notifications / x"
            | "notifications"
            | "messages / x"
            | "messages"
            | "grok / x"
            | "bookmarks / x"
            | "bookmarks"
            | "instagram"
            | "linkedin"
            | "reddit"
            | "facebook"
            | "tiktok"
    ) || normalized.ends_with(" / x")
        && matches!(
            normalized.strip_suffix(" / x").unwrap_or(""),
            "home" | "explore" | "notifications" | "messages" | "bookmarks" | "grok" | "search"
        )
}

/// Prefer a concrete title over browser/site chrome when refining sessions.
pub fn prefer_session_title(prev: Option<&str>, new: Option<&str>) -> Option<String> {
    let new = new.map(str::trim).filter(|s| !s.is_empty())?;
    let Some(prev) = prev.map(str::trim).filter(|s| !s.is_empty()) else {
        return Some(new.to_string());
    };
    let prev_n = normalize_media_title(prev);
    let new_n = normalize_media_title(new);
    let prev_weak = is_weak_media_title(&prev_n) || is_weak_social_title(&prev_n);
    let new_weak = is_weak_media_title(&new_n) || is_weak_social_title(&new_n);
    if new_weak && !prev_weak {
        return Some(prev.to_string());
    }
    if prev_weak && !new_weak {
        return Some(new.to_string());
    }
    // Prefer the richer (usually longer) concrete title.
    if new_n.len() >= prev_n.len() {
        Some(new.to_string())
    } else {
        Some(prev.to_string())
    }
}

/// Context keys / titles that must never open a session (player chrome, noise).
pub fn is_noise_session_identity(context_key: &str, title: Option<&str>) -> bool {
    if context_key.starts_with("media:weak:")
        || context_key == "media:unknown"
        || context_key.starts_with("media:url:")
    {
        return true;
    }
    if let Some(rest) = context_key.strip_prefix("media:") {
        if is_weak_media_title(rest) || rest.ends_with(": stopped") {
            return true;
        }
    }
    if let Some(t) = title {
        let n = normalize_media_title(t);
        if is_weak_media_title(&n) || n.ends_with(": stopped") {
            return true;
        }
    }
    false
}

/// Context keys that identify concrete content (promote after one meaningful event).
pub fn is_concrete_content_key(key: &str) -> bool {
    if key.starts_with("media:yt:") || key.starts_with("media:nf:") {
        return true;
    }
    if key.starts_with("media:weak:") || key == "media:unknown" || key.starts_with("media:url:") {
        return false;
    }
    if key.starts_with("media:") {
        return is_strong_media_context_key(key);
    }
    if let Some(rest) = key.strip_prefix("social:") {
        // social:x.com:status:… or social:x.com:post title — not bare social:x.com
        return rest.contains(':');
    }
    key.starts_with("ws:")
        || key.starts_with("repo:")
        || key.starts_with("figma:")
        || key.starts_with("game:")
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
        " - netflix - brave",
        " - netflix - google chrome",
        " - netflix - firefox",
        " - netflix - chrome",
        " - netflix - edge",
        " - netflix - safari",
        " - netflix",
        " - prime video - brave",
        " - prime video",
        " - disney+ - brave",
        " - disney+",
        " - hulu - brave",
        " - hulu",
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
///
/// Callers that already know a `title_change` is a duplicate of the current
/// title should skip counting it (see [`event_is_meaningful_for`]).
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

/// `title_change` is only meaningful when the normalized title actually changed.
pub fn event_is_meaningful_for(
    event_type: &str,
    prev_title: Option<&str>,
    new_title: Option<&str>,
) -> bool {
    if event_type == "title_change" {
        let prev = prev_title.map(normalize_media_title).unwrap_or_default();
        let new = new_title.map(normalize_media_title).unwrap_or_default();
        return !new.is_empty() && prev != new;
    }
    event_is_meaningful(event_type)
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
        assert!(
            context_key(
                "social_long",
                Some(1),
                None,
                Some("Hunter Biden Announces $LAPTOP Memecoin Launch on September 9 / X - Brave")
            )
            .starts_with("social:x.com:hunter biden")
        );
        assert_eq!(
            context_key(
                "social_long",
                None,
                Some("https://x.com/user/status/1234567890123456789"),
                Some("Some post / X")
            ),
            "social:x.com:status:1234567890123456789"
        );
        assert_eq!(
            prefer_session_title(Some("Community - Netflix - Brave"), Some("Netflix - Brave")),
            Some("Community - Netflix - Brave".into())
        );
        assert!(is_concrete_content_key("media:community"));
        assert!(!is_concrete_content_key("media:weak:netflix"));
        assert!(is_concrete_content_key("social:x.com:hunter biden announces"));
        assert!(!is_concrete_content_key("social:x.com"));
    }

    #[test]
    fn context_key_prefers_youtube_and_netflix_ids() {
        assert_eq!(
            context_key(
                "media_streaming_official",
                None,
                Some("https://www.youtube.com/watch?v=abc123XYZ"),
                Some("Some Video - YouTube - Brave")
            ),
            "media:yt:abc123XYZ"
        );
        assert_eq!(
            context_key(
                "media_streaming_official",
                None,
                Some("https://www.netflix.com/title/80100172"),
                Some("Community - Netflix - Brave")
            ),
            "media:nf:80100172"
        );
        assert_eq!(
            context_key(
                "media_streaming_official",
                None,
                None,
                Some("Netflix - Brave")
            ),
            "media:weak:netflix"
        );
        assert!(is_weak_media_title(&normalize_media_title("Home - Netflix - Brave")));
        assert!(!is_weak_media_title(&normalize_media_title(
            "Community - Netflix - Brave"
        )));
    }

    #[test]
    fn media_context_equivalent_merges_url_and_title_keys() {
        assert!(media_context_equivalent(
            "media:yt:abc123",
            "media:stop playing kayle reroll, play this instead",
            Some("Stop Playing Kayle Reroll, Play This Instead"),
            Some("(1) Stop Playing Kayle Reroll, Play This Instead - YouTube - Brave"),
        ));
        assert!(!media_context_equivalent(
            "media:yt:abc123",
            "media:other video",
            Some("Video A"),
            Some("Video B"),
        ));
        assert!(!is_strong_media_context_key("media:weak:netflix"));
        assert!(is_strong_media_context_key("media:yt:abc123"));
        assert!(is_strong_media_context_key(
            "media:stop playing kayle reroll, play this instead"
        ));
    }

    #[test]
    fn duplicate_title_change_is_not_meaningful() {
        assert!(!event_is_meaningful_for(
            "title_change",
            Some("Same - YouTube - Brave"),
            Some("Same - YouTube - Brave"),
        ));
        assert!(event_is_meaningful_for(
            "title_change",
            Some("Video A - YouTube - Brave"),
            Some("Video B - YouTube - Brave"),
        ));
        assert!(event_is_meaningful_for("window_focus", None, None));
    }

    #[test]
    fn repo_and_workspace_context_keys() {
        assert_eq!(
            parse_git_host_repo(Some("https://github.com/ber2minsin/intime-rs/tree/main")),
            Some("github.com:ber2minsin/intime-rs".into())
        );
        assert_eq!(
            context_key_with_workspace(
                "ai_coding",
                Some(1),
                Some("pipeline.rs"),
                Some("pipeline.rs — intime-rs — Cursor"),
                Some("intime-rs"),
            ),
            "ws:intime-rs"
        );
        assert_eq!(
            context_key_with_workspace(
                "code_editing",
                Some(1),
                Some("https://github.com/foo/bar/pull/12"),
                None,
                None,
            ),
            "repo:github.com:foo/bar"
        );
        assert_eq!(
            parse_figma_file_key(Some("https://www.figma.com/file/AbCdEf123456/My-Design")),
            Some("AbCdEf123456".into())
        );
        assert_eq!(
            context_key_with_workspace(
                "design_2d",
                Some(1),
                Some("https://www.figma.com/design/AbCdEf123456/Foo"),
                None,
                None,
            ),
            "figma:AbCdEf123456"
        );
        assert_eq!(
            context_key_with_workspace(
                "gaming",
                Some(1),
                None,
                Some("Counter-Strike 2"),
                None,
            ),
            "game:counter-strike 2"
        );
        assert!(
            context_key_with_workspace("gaming", Some(1), None, Some("Steam"), None)
                .starts_with("app:")
                || context_key_with_workspace("gaming", Some(1), None, Some("Steam"), None)
                    .starts_with("cat:")
        );
    }

    #[test]
    fn nvim_terminal_title_yields_workspace_key() {
        assert_eq!(
            context_key_with_workspace(
                "terminal",
                Some(1),
                None,
                Some("nvim ~/D/w/r/intime-rs"),
                None,
            ),
            "ws:intime-rs"
        );
        assert_eq!(
            context_key_with_workspace(
                "code_editing",
                Some(1),
                None,
                Some("nvim . ~/Documents/workspace/rust/intime-rs"),
                None,
            ),
            "ws:intime-rs"
        );
    }

    #[test]
    fn media_session_continues_across_weak_and_local_streaming() {
        assert!(should_continue_media_session(
            "media:weak:netflix",
            "media:nf:70251221",
            Some("Netflix - Brave"),
            Some("Community - Netflix - Brave"),
        ));
        assert!(should_continue_media_session(
            "media:example track title",
            "media:yt:exampleVideoId",
            Some("Example Track Title"),
            Some("(1) Example Track Title - YouTube - Brave"),
        ));
        assert!(!should_continue_media_session(
            "media:friends",
            "media:community",
            Some("Friends - Netflix"),
            Some("Community - Netflix"),
        ));
        assert_eq!(
            prefer_media_category_slug("media_local", "media_streaming_official"),
            "media_streaming_official"
        );
        assert_eq!(
            prefer_media_context_key("media:weak:netflix", "media:nf:1"),
            "media:nf:1"
        );
    }
}
