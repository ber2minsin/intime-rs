use std::time::{Duration, Instant};

use anyhow::Result;
use intime_core::{
    category::{
        context_key, event_is_high_value, event_is_meaningful, is_media_category,
        is_social_category, CategoryHit, CategoryMatchInput,
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

        // Explicit idle / gap: close any open session and clear pending.
        if let Some(reason) = end_reason_for_idle_event(event) {
            self.close_open(storage, now, reason).await?;
            self.pending = None;
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

        let document = event
            .metadata
            .document_path
            .clone()
            .or_else(|| event.metadata.document_name.clone())
            .or_else(|| event.metadata.url.clone());

        let media_title = if is_media_category(category_slug) {
            event
                .metadata
                .window_title
                .as_deref()
                .or(match &event.data {
                    EventData::UiAction { label, .. } => label.as_deref(),
                    _ => None,
                })
        } else if is_social_category(category_slug) {
            event.metadata.window_title.as_deref()
        } else {
            None
        };

        let Some(hit) = hit else {
            // Uncategorized: do not open sessions.
            return Ok(self.current_session);
        };

        let ctx = context_key(category_slug, app_id, document.as_deref(), media_title);
        let title = event
            .metadata
            .window_title
            .clone()
            .or(document)
            .or_else(|| media_title.map(|s| s.to_string()));

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
                started_at: now,
                first_seen: now_inst,
                meaningful_events: 0,
            });
            self.last_category_id = Some(hit.category_id);
            self.last_category_slug = Some(category_slug.to_string());
            self.last_context_key = Some(ctx);
            self.last_app_id = app_id;
        }

        let ui_kind = match &event.data {
            EventData::UiAction { kind, .. } => Some(kind.as_str()),
            _ => None,
        };
        let meaningful = event_is_meaningful(event.data.name());
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
                        let reuse_window_secs = if is_media_category(&pending.category_slug) {
                            // Same show/video resumes even after a longer detour.
                            2 * 60 * 60
                        } else {
                            5 * 60
                        };
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

        // Finish/close of window can end the session for that context.
        if let EventData::UiAction { kind, .. } = &event.data {
            if matches!(
                kind,
                intime_core::models::UiActionKind::Finish
                    | intime_core::models::UiActionKind::Close
            ) {
                let reason = if matches!(kind, intime_core::models::UiActionKind::Finish) {
                    SessionEndReason::Finish
                } else {
                    SessionEndReason::Close
                };
                self.close_open(storage, now, reason).await?;
                self.pending = None;
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
            storage
                .session_repository
                .close_session(session_id, ended, None, Some(reason.as_str()))
                .await?;
        }
        self.last_category_id = None;
        self.last_category_slug = None;
        self.last_context_key = None;
        Ok(())
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
