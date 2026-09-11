use std::time::{Duration, Instant};

use anyhow::Result;
use blake3::Hash;
use intime_core::{
    category::{
        context_key_with_workspace, event_is_high_value, event_is_meaningful_for, is_concrete_content_key,
        is_media_category, is_noise_session_identity, is_strong_media_context_key, is_weak_media_title,
        is_weak_social_title, normalize_media_title, pick_richer_media_title, prefer_media_category_slug,
        prefer_media_context_key, prefer_session_title, session_summary_line,
        should_continue_media_session, CategoryHit, CategoryMatchInput,
    },
    features::FeatureFlags,
    models::{Event, EventData, UiActionKind},
    session::{SessionEndReason, SessionPromotionPolicy, SessionSource},
    time::Timestamp,
};
use intime_storage::storage::Storage;
use tracing::info;

/// Tracks open sessions by category + context.
pub struct SessionTracker {
    flags: FeatureFlags,
    policy: SessionPromotionPolicy,
    current_session: Option<i64>,
    last_category_id: Option<i64>,
    last_category_slug: Option<String>,
    last_context_key: Option<String>,
    last_app_id: Option<i64>,
    session_fingerprint: Option<Hash>,
    last_title: Option<String>,
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

struct Identity {
    ctx: String,
    title: Option<String>,
    category_id: i64,
    category_slug: String,
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

    /// Heartbeat / dwell tick: promote pending by wall time even when AT-SPI only
    /// re-emits the same title (those duplicates never reach `observe`).
    pub async fn tick(&mut self, storage: &Storage, now: Timestamp) -> Result<Option<i64>> {
        if !self.flags.session_grouping {
            return Ok(None);
        }
        self.try_promote(storage, now, Instant::now(), false).await?;
        Ok(self.current_session)
    }

    pub async fn observe(
        &mut self,
        storage: &Storage,
        event: &Event,
        app_id: Option<i64>,
        hit: Option<&CategoryHit>,
        category_slug: &str,
        _product_hint: Option<&str>,
    ) -> Result<Option<i64>> {
        if !self.flags.session_grouping {
            return Ok(None);
        }

        let now = event.timestamp;
        let now_inst = Instant::now();
        let event_fp = event.data.fingerprint();

        if let Some(reason) = end_reason_for_idle(event) {
            // Flush pending, then close *all* open sessions (parallel activities end on idle).
            self.try_promote(storage, now, now_inst, false).await?;
            self.close_open(storage, now, reason).await?;
            storage
                .session_repository
                .close_orphaned_open_sessions(now.as_datetime(), None, reason.as_str())
                .await?;
            self.pending = None;
            self.session_fingerprint = None;
            return Ok(None);
        }

        if let EventData::AppSeen { details, .. } = &event.data {
            self.on_app_seen(storage, details, app_id, hit, category_slug, event)
                .await?;
            return Ok(self.current_session);
        }

        let app_id = app_id.or(self.last_app_id);
        let Some(hit) = hit else {
            return Ok(self.current_session);
        };

        let identity = self.identity_from_event(event, hit, category_slug, app_id);
        let prev_title = self.last_title.clone();
        self.apply_identity(storage, &identity, app_id, event_fp, now, now_inst)
            .await?;

        let ui_kind = match &event.data {
            EventData::UiAction { kind, .. } => Some(kind.as_str()),
            _ => None,
        };
        // Compare against title *before* identity apply — otherwise the first
        // title_change looks like a duplicate of itself and never promotes.
        let meaningful = event_is_meaningful_for(
            event.data.name(),
            prev_title.as_deref(),
            identity.title.as_deref(),
        );
        self.note_activity(identity.title.clone(), app_id, event_fp, meaningful);

        let high_value = event_is_high_value(event.data.name(), ui_kind);
        self.try_promote(storage, now, now_inst, high_value).await?;

        if let Some(id) = app_id {
            self.last_app_id = Some(id);
            if let Some(session_id) = self.current_session {
                storage
                    .session_repository
                    .set_session_app_id(session_id, id)
                    .await?;
            }
        }

        self.maybe_finish_close(storage, event, now).await?;
        Ok(self.current_session)
    }

    async fn on_app_seen(
        &mut self,
        storage: &Storage,
        details: &intime_core::models::AppDetails,
        app_id: Option<i64>,
        hit: Option<&CategoryHit>,
        category_slug: &str,
        event: &Event,
    ) -> Result<()> {
        self.last_product_hint = details
            .product_name
            .clone()
            .or_else(|| (!details.file_path.is_empty()).then(|| details.display_name()));
        if let Some(id) = app_id {
            self.last_app_id = Some(id);
            if let Some(session_id) = self.current_session {
                storage
                    .session_repository
                    .set_session_app_id(session_id, id)
                    .await?;
            }
        }
        // Refine open media/social with a better title from AppSeen (e.g. show name).
        let Some(hit) = hit else {
            return Ok(());
        };
        if !(is_media_category(category_slug) || category_slug.starts_with("social_")) {
            return Ok(());
        }
        if self.current_session.is_none() && self.pending.is_none() {
            return Ok(());
        }
        let identity = self.identity_from_event(event, hit, category_slug, app_id.or(self.last_app_id));
        if self.same_or_continuable(&identity) {
            self.refine(storage, &identity, app_id.or(self.last_app_id), event.data.fingerprint())
                .await?;
        }
        Ok(())
    }

    fn identity_from_event(
        &self,
        event: &Event,
        hit: &CategoryHit,
        category_slug: &str,
        app_id: Option<i64>,
    ) -> Identity {
        let label = match &event.data {
            EventData::UiAction { label, .. } => label.as_deref(),
            _ => None,
        };
        let url = event.metadata.url.as_deref();
        let workspace = event.metadata.workspace_path.as_deref();
        let content_title = if is_media_category(category_slug) {
            pick_richer_media_title(event.metadata.window_title.as_deref(), label)
        } else {
            event.metadata.window_title.clone()
        };

        let mut ctx = context_key_with_workspace(
            category_slug,
            app_id,
            url,
            content_title
                .as_deref()
                .or(event.metadata.window_title.as_deref()),
            workspace,
        );

        if is_media_category(category_slug) {
            if let Some(prev) = self.last_context_key.as_deref() {
                if should_continue_media_session(
                    prev,
                    &ctx,
                    self.last_title.as_deref(),
                    content_title.as_deref(),
                ) {
                    ctx = prefer_media_context_key(prev, &ctx);
                }
            }
        }

        // Display title: concrete content first; never invent chrome as the label.
        let raw_title = content_title
            .as_deref()
            .or(event.metadata.window_title.as_deref());
        let title = prefer_session_title(None, raw_title).and_then(|t| {
            let n = normalize_media_title(&t);
            if is_weak_media_title(&n) || is_weak_social_title(&n) {
                None
            } else {
                Some(t)
            }
        });

        Identity {
            ctx,
            title,
            category_id: hit.category_id,
            category_slug: category_slug.to_string(),
        }
    }

    fn same_or_continuable(&self, id: &Identity) -> bool {
        let same_cat = self.last_category_id == Some(id.category_id);
        let same_ctx = self.last_context_key.as_deref() == Some(id.ctx.as_str());
        if same_cat && same_ctx {
            return true;
        }
        let Some(prev_slug) = self.last_category_slug.as_deref() else {
            return false;
        };
        let Some(prev_ctx) = self.last_context_key.as_deref() else {
            return false;
        };
        is_media_category(prev_slug)
            && is_media_category(&id.category_slug)
            && should_continue_media_session(
                prev_ctx,
                &id.ctx,
                self.last_title.as_deref(),
                id.title.as_deref(),
            )
    }

    async fn apply_identity(
        &mut self,
        storage: &Storage,
        id: &Identity,
        app_id: Option<i64>,
        event_fp: Option<Hash>,
        now: Timestamp,
        now_inst: Instant,
    ) -> Result<()> {
        if self.same_or_continuable(id) {
            self.refine(storage, id, app_id, event_fp).await?;
            return Ok(());
        }

        // Promote earned pending for the previous context, then switch focus.
        // Do NOT close the previous session — users multitask (meeting + code, music + game).
        self.try_promote(storage, now, now_inst, false).await?;
        if let Some(prev) = self.current_session.take() {
            info!(
                session_id = prev,
                context_key = self.last_context_key.as_deref(),
                "session detach (multitask)"
            );
        }

        self.pending = Some(PendingSession {
            category_id: id.category_id,
            category_slug: id.category_slug.clone(),
            context_key: id.ctx.clone(),
            app_id,
            title: id.title.clone(),
            fingerprint: event_fp,
            started_at: now,
            first_seen: now_inst,
            meaningful_events: 0,
        });
        self.last_category_id = Some(id.category_id);
        self.last_category_slug = Some(id.category_slug.clone());
        self.last_context_key = Some(id.ctx.clone());
        self.last_app_id = app_id.or(self.last_app_id);
        self.session_fingerprint = event_fp.or(self.session_fingerprint);
        if let Some(t) = &id.title {
            self.last_title = prefer_session_title(self.last_title.as_deref(), Some(t));
        }
        Ok(())
    }

    async fn refine(
        &mut self,
        storage: &Storage,
        id: &Identity,
        app_id: Option<i64>,
        event_fp: Option<Hash>,
    ) -> Result<()> {
        let slug = match self.last_category_slug.as_deref() {
            Some(prev) if is_media_category(prev) && is_media_category(&id.category_slug) => {
                prefer_media_category_slug(prev, &id.category_slug)
            }
            _ => id.category_slug.clone(),
        };
        let ctx = match self.last_context_key.as_deref() {
            Some(prev) if is_media_category(&id.category_slug) => {
                prefer_media_context_key(prev, &id.ctx)
            }
            _ => id.ctx.clone(),
        };
        let category_id = if self.last_category_slug.as_deref() == Some(slug.as_str()) {
            self.last_category_id.unwrap_or(id.category_id)
        } else {
            id.category_id
        };
        let title = prefer_session_title(self.last_title.as_deref(), id.title.as_deref());

        if let Some(pending) = self.pending.as_mut() {
            pending.category_id = category_id;
            pending.category_slug = slug.clone();
            pending.context_key = ctx.clone();
            pending.title = prefer_session_title(pending.title.as_deref(), title.as_deref());
            if pending.app_id.is_none() {
                pending.app_id = app_id;
            }
            if pending.fingerprint.is_none() {
                pending.fingerprint = event_fp;
            }
        }

        if let Some(session_id) = self.current_session {
            let store_title = title.as_deref().and_then(|t| {
                let n = normalize_media_title(t);
                if is_weak_media_title(&n) || is_weak_social_title(&n) {
                    None
                } else {
                    Some(t)
                }
            });
            storage
                .session_repository
                .refine_session(
                    session_id,
                    Some(&slug),
                    store_title,
                    Some(category_id),
                    Some(&ctx),
                )
                .await?;
        }

        self.last_category_id = Some(category_id);
        self.last_category_slug = Some(slug);
        self.last_context_key = Some(ctx);
        if let Some(t) = title {
            self.last_title = Some(t);
        }
        if let Some(id) = app_id {
            self.last_app_id = Some(id);
        }
        if event_fp.is_some() {
            self.session_fingerprint = event_fp.or(self.session_fingerprint);
        }
        Ok(())
    }

    fn note_activity(
        &mut self,
        title: Option<String>,
        app_id: Option<i64>,
        event_fp: Option<Hash>,
        meaningful: bool,
    ) {
        if let Some(pending) = self.pending.as_mut() {
            if meaningful {
                pending.meaningful_events = pending.meaningful_events.saturating_add(1);
            }
            pending.title = prefer_session_title(pending.title.as_deref(), title.as_deref());
            if pending.app_id.is_none() {
                pending.app_id = app_id;
            }
            if pending.fingerprint.is_none() {
                pending.fingerprint = event_fp;
            }
        }
        if let Some(t) = title {
            self.last_title = prefer_session_title(self.last_title.as_deref(), Some(&t));
        }
        if event_fp.is_some() {
            self.session_fingerprint = event_fp.or(self.session_fingerprint);
        }
    }

    fn pending_ready(&self, now_inst: Instant, high_value: bool) -> bool {
        let Some(p) = self.pending.as_ref() else {
            return false;
        };
        // Drop noise pendings so they don't linger.
        if is_noise_session_identity(&p.context_key, p.title.as_deref()) {
            return false;
        }
        if high_value {
            return true;
        }
        if p.meaningful_events >= self.policy.min_meaningful_events {
            return true;
        }
        if now_inst.duration_since(p.first_seen)
            >= Duration::from_secs(self.policy.min_duration_secs)
        {
            return true;
        }
        is_concrete_content_key(&p.context_key) && p.meaningful_events >= 1
    }

    async fn try_promote(
        &mut self,
        storage: &Storage,
        now: Timestamp,
        now_inst: Instant,
        high_value: bool,
    ) -> Result<()> {
        if self.current_session.is_some() || !self.pending_ready(now_inst, high_value) {
            // Drop noise pendings so they don't linger.
            if self.pending.as_ref().is_some_and(|p| {
                is_noise_session_identity(&p.context_key, p.title.as_deref())
            }) {
                let p = self.pending.take().unwrap();
                info!(
                    context_key = %p.context_key,
                    category = %p.category_slug,
                    "session skip noise"
                );
            }
            return Ok(());
        }
        let Some(pending) = self.pending.take() else {
            return Ok(());
        };

        let find_category = if is_media_category(&pending.category_slug) {
            None
        } else {
            Some(pending.category_id)
        };

        let (session_id, action) = match storage
            .session_repository
            .find_session_by_context(&pending.context_key, find_category)
            .await?
        {
            Some(existing) if within_reuse(&existing, &pending, now) => {
                let action = if existing.ended_at.is_some() {
                    storage
                        .session_repository
                        .reopen_session(existing.id)
                        .await?;
                    "reopen"
                } else {
                    "adopt"
                };
                storage
                    .session_repository
                    .refine_session(
                        existing.id,
                        Some(&pending.category_slug),
                        pending.title.as_deref(),
                        Some(pending.category_id),
                        Some(&pending.context_key),
                    )
                    .await?;
                (existing.id, action)
            }
            _ => {
                let id = storage
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
                    .await?;
                (id, "open")
            }
        };

        if let Some(id) = pending.app_id {
            storage
                .session_repository
                .set_session_app_id(session_id, id)
                .await?;
        }

        info!(
            action,
            session_id,
            context_key = %pending.context_key,
            category = %pending.category_slug,
            "session promote"
        );

        self.current_session = Some(session_id);
        self.last_category_id = Some(pending.category_id);
        self.last_category_slug = Some(pending.category_slug);
        self.last_context_key = Some(pending.context_key);
        self.last_app_id = pending.app_id.or(self.last_app_id);
        self.session_fingerprint = pending.fingerprint.or(self.session_fingerprint);
        self.last_title = pending.title.or(self.last_title.clone());
        Ok(())
    }

    async fn maybe_finish_close(
        &mut self,
        storage: &Storage,
        event: &Event,
        now: Timestamp,
    ) -> Result<()> {
        let EventData::UiAction {
            kind, fingerprint, ..
        } = &event.data
        else {
            return Ok(());
        };
        if !matches!(kind, UiActionKind::Finish | UiActionKind::Close) {
            return Ok(());
        }
        let owns = self.session_fingerprint == Some(*fingerprint)
            || self
                .pending
                .as_ref()
                .and_then(|p| p.fingerprint)
                .is_some_and(|fp| fp == *fingerprint);
        let no_owner = self.session_fingerprint.is_none()
            && self
                .pending
                .as_ref()
                .is_none_or(|p| p.fingerprint.is_none());
        if !(owns || (no_owner && self.current_session.is_some())) {
            return Ok(());
        }
        let reason = if matches!(kind, UiActionKind::Finish) {
            SessionEndReason::Finish
        } else {
            SessionEndReason::Close
        };
        self.close_open(storage, now, reason).await?;
        self.pending = None;
        self.session_fingerprint = None;
        Ok(())
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
            info!(
                session_id,
                reason = reason.as_str(),
                context_key = self.last_context_key.as_deref(),
                "session close"
            );
        }
        self.last_category_id = None;
        self.last_category_slug = None;
        self.last_context_key = None;
        Ok(())
    }
}

fn within_reuse(
    existing: &intime_storage::db::models::SessionRecord,
    pending: &PendingSession,
    now: Timestamp,
) -> bool {
    let window = reuse_window_secs(&pending.category_slug, &pending.context_key);
    existing
        .ended_at
        .map(|ended| now.as_datetime().signed_duration_since(ended).num_seconds() < window)
        .unwrap_or(true)
}

fn reuse_window_secs(category_slug: &str, context_key: &str) -> i64 {
    if is_media_category(category_slug) {
        if is_strong_media_context_key(context_key) {
            2 * 60 * 60
        } else {
            2 * 60
        }
    } else {
        5 * 60
    }
}

fn end_reason_for_idle(event: &Event) -> Option<SessionEndReason> {
    match event.data.name() {
        "idle_start" => Some(SessionEndReason::Idle),
        "gap" => Some(SessionEndReason::Gap),
        _ => None,
    }
}

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
