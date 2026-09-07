use std::time::{Duration, Instant};

use anyhow::Result;
use blake3::Hash;
use intime_core::{
    category::{
        context_key_with_workspace, event_is_high_value, event_is_meaningful_for, is_media_category,
        is_social_category, is_strong_media_context_key, media_context_equivalent,
        pick_richer_media_title, session_summary_line, CategoryHit, CategoryMatchInput,
    },
    features::FeatureFlags,
    models::{Event, EventData},
    session::{SessionEndReason, SessionPromotionPolicy, SessionSource},
    time::Timestamp,
};
use intime_storage::storage::Storage;

/// Tracks open sessions by category + context. Discrete verbs stay raw events.
pub struct SessionTracker {
    flags: FeatureFlags,
    policy: SessionPromotionPolicy,
    current_session: Option<i64>,
    last_category_id: Option<i64>,
    last_category_slug: Option<String>,
    last_context_key: Option<String>,
    last_app_id: Option<i64>,
    /// Fingerprint of the app/window that owns the open or pending session.
    session_fingerprint: Option<Hash>,
    last_title: Option<String>,
    /// Candidate not yet promoted to a DB session.
    pending: Option<PendingSession>,
    last_product_hint: Option<String>,
}

#[derive(Debug, Clone)]
struct PendingSession {
    category_id: i64,
    category_slug: String,
    context_key: String,
    app_id: Option<i64>,
    title: Option<String>,
    fingerprint: Option<Hash>,
    started_at: Timestamp,
    first_seen: Instant,
    meaningful_events: u32,
}

impl SessionTracker {
    pub fn new(flags: FeatureFlags) -> Self {
        Self {
            flags,
            policy: SessionPromotionPolicy::default(),
            current_session: None,
            last_category_id: None,
            last_category_slug: None,
            last_context_key: None,
            last_app_id: None,
            session_fingerprint: None,
            last_title: None,
            pending: None,
            last_product_hint: None,
        }
    }

    pub fn current_session_id(&self) -> Option<i64> {
        self.current_session
    }

    pub async fn observe(
        &mut self,
        storage: &Storage,
        event: &Event,
        app_id: Option<i64>,
        hit: Option<&CategoryHit>,
        category_slug: &str,
        product_hint: Option<&str>,
    ) -> Result<Option<i64>> {
        if !self.flags.session_grouping {
            return Ok(None);
        }

        let now_inst = Instant::now();
        let now = event.timestamp;
        let event_fp = event.data.fingerprint();

        // Explicit idle / gap: close any open session and clear pending.
        if let Some(reason) = end_reason_for_idle_event(event) {
            self.close_open(storage, now, reason).await?;
            self.pending = None;
            self.session_fingerprint = None;
            return Ok(None);
        }

        // AppSeen enriches identity but must not open/split sessions by itself.
        // Still attach to the current session when one is already open, and
        // backfill app_id when media started from MPRIS before the browser was seen.
        if matches!(event.data, EventData::AppSeen { .. }) {
            if let EventData::AppSeen { details, .. } = &event.data {
                self.last_product_hint = details
                    .product_name
                    .clone()
                    .or_else(|| (!details.file_path.is_empty()).then(|| details.display_name()));
            }
            if let Some(id) = app_id {
                self.last_app_id = Some(id);
                if let Some(session_id) = self.current_session {
                    storage
                        .session_repository
                        .set_session_app_id(session_id, id)
                        .await?;
                }
            }
            return Ok(self.current_session);
        }

        let _ = product_hint.or(self.last_product_hint.as_deref());
        let app_id = app_id.or(self.last_app_id);

        let url = event.metadata.url.clone();
        let workspace = event.metadata.workspace_path.clone();
        let document = event
            .metadata
            .document_path
            .clone()
            .or_else(|| event.metadata.document_name.clone())
            .or_else(|| url.clone());

        let label = match &event.data {
            EventData::UiAction { label, .. } => label.as_deref(),
            _ => None,
        };
        let media_title = if is_media_category(category_slug) {
            pick_richer_media_title(event.metadata.window_title.as_deref(), label)
        } else if is_social_category(category_slug) {
            event.metadata.window_title.clone()
        } else {
            None
        };

        let Some(hit) = hit else {
            // Uncategorized: do not open sessions.
            return Ok(self.current_session);
        };

        // Prefer URL (for media/repo ids) over document_name when both exist.
        let document_for_key = url.as_deref().or(document.as_deref());
        let mut ctx = context_key_with_workspace(
            category_slug,
            app_id,
            document_for_key,
            media_title
                .as_deref()
                .or(event.metadata.window_title.as_deref()),
            workspace.as_deref(),
        );
        let title = event
            .metadata
            .window_title
            .clone()
            .or(document)
            .or(media_title.clone());

        // Keep URL-id keys when a later focus has the same strong title but no URL.
        if is_media_category(category_slug) {
            if let Some(prev) = self.last_context_key.as_deref() {
                let prev_title = self
                    .pending
                    .as_ref()
                    .and_then(|p| p.title.as_deref())
                    .or(self.last_title.as_deref());
                if media_context_equivalent(prev, &ctx, prev_title, media_title.as_deref()) {
                    // Prefer the stronger (URL-id) key when either side has one.
                    if is_strong_media_context_key(prev)
                        && (!is_strong_media_context_key(&ctx) || prev.starts_with("media:yt:") || prev.starts_with("media:nf:"))
                    {
                        ctx = prev.to_string();
                    }
                }
            }
        }

        let category_changed = self.last_category_id != Some(hit.category_id);
        let context_changed = self.last_context_key.as_deref() != Some(ctx.as_str());

        if category_changed || context_changed {
            self.close_open(storage, now, SessionEndReason::ContextChange)
                .await?;
            self.pending = Some(PendingSession {
                category_id: hit.category_id,
                category_slug: category_slug.to_string(),
                context_key: ctx.clone(),
                app_id,
                title: title.clone(),
                fingerprint: event_fp,
                started_at: now,
                first_seen: now_inst,
                meaningful_events: 0,
            });
            self.last_category_id = Some(hit.category_id);
            self.last_category_slug = Some(category_slug.to_string());
            self.last_context_key = Some(ctx);
            self.last_app_id = app_id;
            self.session_fingerprint = event_fp;
        }

        let ui_kind = match &event.data {
            EventData::UiAction { kind, .. } => Some(kind.as_str()),
            _ => None,
        };
        let meaningful = event_is_meaningful_for(
            event.data.name(),
            self.last_title.as_deref(),
            title.as_deref(),
        );
        let high_value = event_is_high_value(event.data.name(), ui_kind);

        if let Some(pending) = self.pending.as_mut() {
            if meaningful {
                pending.meaningful_events = pending.meaningful_events.saturating_add(1);
            }
            if title.is_some() {
                pending.title = title.clone().or(pending.title.clone());
            }
            if pending.app_id.is_none() {
                pending.app_id = app_id;
            }
            if pending.fingerprint.is_none() {
                pending.fingerprint = event_fp;
            }
        }
        if title.is_some() {
            self.last_title = title.clone().or(self.last_title.clone());
        }
        if event_fp.is_some() {
            self.session_fingerprint = event_fp.or(self.session_fingerprint);
        }

        if self.current_session.is_none() {
            let should_promote = high_value
                || self.pending.as_ref().is_some_and(|p| {
                    p.meaningful_events >= self.policy.min_meaningful_events
                        || now_inst.duration_since(p.first_seen)
                            >= Duration::from_secs(self.policy.min_duration_secs)
                });
            if should_promote {
                if let Some(pending) = self.pending.take() {
                    // Reuse the same context instead of opening a duplicate row
                    // (e.g. play → leave briefly → return to the same video).
                    let session_id = if let Some(existing) = storage
                        .session_repository
                        .find_session_by_context(
                            &pending.context_key,
                            Some(pending.category_id),
                        )
                        .await?
                    {
                        let reuse_window_secs =
                            reuse_window_secs(&pending.category_slug, &pending.context_key);
                        let within_reuse = existing
                            .ended_at
                            .map(|ended| {
                                now.as_datetime()
                                    .signed_duration_since(ended)
                                    .num_seconds()
                                    < reuse_window_secs
                            })
                            .unwrap_or(true); // still open somehow
                        if within_reuse {
                            if existing.ended_at.is_some() {
                                storage
                                    .session_repository
                                    .reopen_session(existing.id)
                                    .await?;
                            }
                            existing.id
                        } else {
                            storage
                                .session_repository
                                .open_session(
                                    pending.started_at.as_datetime(),
                                    Some(&pending.category_slug),
                                    pending.title.as_deref(),
                                    SessionSource::Heuristic.as_str(),
                                    Some(pending.category_id),
                                    Some(&pending.context_key),
                                    pending.app_id,
                                )
                                .await?
                        }
                    } else {
                        storage
                            .session_repository
                            .open_session(
                                pending.started_at.as_datetime(),
                                Some(&pending.category_slug),
                                pending.title.as_deref(),
                                SessionSource::Heuristic.as_str(),
                                Some(pending.category_id),
                                Some(&pending.context_key),
                                pending.app_id,
                            )
                            .await?
                    };
                    if let Some(id) = pending.app_id {
                        storage
                            .session_repository
                            .set_session_app_id(session_id, id)
                            .await?;
                    }
                    self.current_session = Some(session_id);
                    self.last_category_id = Some(pending.category_id);
                    self.last_category_slug = Some(pending.category_slug);
                    self.last_context_key = Some(pending.context_key);
                    self.last_app_id = pending.app_id.or(self.last_app_id);
                    self.session_fingerprint = pending.fingerprint.or(self.session_fingerprint);
                    self.last_title = pending.title.or(self.last_title.clone());
                }
            }
        }

        if let Some(id) = app_id {
            self.last_app_id = Some(id);
            if let Some(session_id) = self.current_session {
                storage
                    .session_repository
                    .set_session_app_id(session_id, id)
                    .await?;
            }
        }

        // Finish/close of *this* window can end the session — ignore Finish from
        // unrelated dialogs (Bluetooth, volume) that would wipe media pending.
        if let EventData::UiAction { kind, fingerprint, .. } = &event.data {
            if matches!(
                kind,
                intime_core::models::UiActionKind::Finish
                    | intime_core::models::UiActionKind::Close
            ) {
                let owns_session = self
                    .session_fingerprint
                    .map(|fp| fp == *fingerprint)
                    .unwrap_or(false)
                    || self
                        .pending
                        .as_ref()
                        .and_then(|p| p.fingerprint)
                        .map(|fp| fp == *fingerprint)
                        .unwrap_or(false);
                // Also allow when we have no fingerprint yet (first events).
                let no_owner = self.session_fingerprint.is_none()
                    && self
                        .pending
                        .as_ref()
                        .map(|p| p.fingerprint.is_none())
                        .unwrap_or(true);
                if owns_session || (no_owner && self.current_session.is_some()) {
                    let reason = if matches!(kind, intime_core::models::UiActionKind::Finish) {
                        SessionEndReason::Finish
                    } else {
                        SessionEndReason::Close
                    };
                    self.close_open(storage, now, reason).await?;
                    self.pending = None;
                    self.session_fingerprint = None;
                }
            }
        }

        Ok(self.current_session)
    }

    async fn close_open(
        &mut self,
        storage: &Storage,
        ended_at: Timestamp,
        reason: SessionEndReason,
    ) -> Result<()> {
        let ended = ended_at.as_datetime();
        if let Some(session_id) = self.current_session.take() {
            let summary = match (
                self.last_category_slug.as_deref(),
                self.last_context_key.as_deref(),
            ) {
                (Some(slug), Some(ctx)) => {
                    Some(session_summary_line(slug, self.last_title.as_deref(), ctx))
                }
                _ => None,
            };
            storage
                .session_repository
                .close_session(
                    session_id,
                    ended,
                    summary.as_deref(),
                    Some(reason.as_str()),
                )
                .await?;
        }
        self.last_category_id = None;
        self.last_category_slug = None;
        self.last_context_key = None;
        Ok(())
    }
}

fn reuse_window_secs(category_slug: &str, context_key: &str) -> i64 {
    if is_media_category(category_slug) {
        if is_strong_media_context_key(context_key) {
            // Same show/video resumes even after a longer detour.
            2 * 60 * 60
        } else {
            // Generic "Netflix" / weak keys must not reopen hours later.
            2 * 60
        }
    } else {
        5 * 60
    }
}

fn end_reason_for_idle_event(event: &Event) -> Option<SessionEndReason> {
    match event.data.name() {
        "idle_start" => Some(SessionEndReason::Idle),
        "gap" => Some(SessionEndReason::Gap),
        // idle_end means the user returned; session already closed on idle_start.
        _ => None,
    }
}

/// Convenience: build matcher input from event + optional product hints.
pub fn match_input_from_event(
    event: &Event,
    aumid: Option<String>,
    product_name: Option<String>,
    display_name: Option<String>,
    company: Option<String>,
) -> CategoryMatchInput {
    CategoryMatchInput {
        aumid,
        product_name: product_name.or_else(|| event.metadata.executable_path.clone()),
        display_name,
        company,
        executable_path: event.metadata.executable_path.clone(),
        window_title: event.metadata.window_title.clone(),
        url: event.metadata.url.clone(),
        focused_control_type: event.metadata.focused_control_type.clone(),
        automation_id: event.metadata.automation_id.clone(),
    }
}
