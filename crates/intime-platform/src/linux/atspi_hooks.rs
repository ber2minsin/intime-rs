use crate::{
    error::PlatformError,
    linux::identity::enrich_app_details,
    shared::push_event,
};
use std::collections::VecDeque;
use std::path::Path;

use atspi::{
    AccessibilityConnection, Event as AtspiEvent, FocusEvents, MatchType, ObjectEvents,
    ObjectMatchRule, Role, SortOrder, State, WindowEvents,
    events::object::StateChangedEvent,
    proxy::{
        accessible::AccessibleProxy,
        proxy_ext::ProxyExt,
    },
    zbus::{self, fdo::DBusProxy, names::BusName},
};
use futures_lite::StreamExt;
use intime_core::{
    context::guess_ui_action_from_control,
    models::{AppDetails, Event, EventData, EventMetadata, UiActionKind},
    time::Timestamp,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use zbus::names::OwnedUniqueName;

pub fn run_event_loop() -> Result<(), PlatformError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| PlatformError::SynchronizationError(e.to_string()))?;

    rt.block_on(async_event_loop())
}

async fn async_event_loop() -> Result<(), PlatformError> {
    let connection = AccessibilityConnection::new()
        .await
        .map_err(|e| PlatformError::Other(anyhow::anyhow!("AT-SPI connection failed: {e}")))?;
    let conn = connection.connection().clone();

    // Focus + state changes cover most toolkits; name changes for title updates.
    let _ = connection
        .register_event::<atspi::events::focus::FocusEvent>()
        .await;
    let _ = connection.register_event::<StateChangedEvent>().await;
    let _ = connection
        .register_event::<atspi::events::object::PropertyChangeEvent>()
        .await;
    let _ = connection
        .register_event::<atspi::events::object::TextChangedEvent>()
        .await;
    let _ = connection
        .register_event::<atspi::events::document::LoadCompleteEvent>()
        .await;
    let _ = connection
        .register_event::<atspi::events::document::ReloadEvent>()
        .await;
    let _ = connection
        .register_event::<atspi::events::document::AttributesChangedEvent>()
        .await;
    let _ = connection
        .register_event::<atspi::events::document::PageChangedEvent>()
        .await;
    let _ = connection
        .register_event::<atspi::events::window::CloseEvent>()
        .await;
    let _ = connection
        .register_event::<atspi::events::window::ActivateEvent>()
        .await;

    let mut events = connection.event_stream();
    while let Some(item) = events.next().await {
        let event = match item {
            Ok(e) => e,
            Err(e) => {
                tracing::debug!("AT-SPI stream error: {e}");
                continue;
            }
        };

        if let Err(e) = handle_atspi_event(&conn, event).await {
            tracing::debug!("Failed to handle AT-SPI event: {e:?}");
        }
    }

    Ok(())
}

async fn handle_atspi_event(
    conn: &zbus::Connection,
    event: AtspiEvent,
) -> Result<(), PlatformError> {
    match event {
        AtspiEvent::Focus(FocusEvents::Focus(ev)) => {
            emit_focus(conn, &ev.item).await?;
        }
        AtspiEvent::Object(ObjectEvents::StateChanged(ev)) => {
            // Some toolkits emit focused=true instead of Focus events.
            if ev.state == atspi::State::Focused && ev.enabled {
                emit_focus(conn, &ev.item).await?;
            }
            // Button press / activate → discrete UI actions (submit, save, …).
            if ev.enabled
                && matches!(
                    ev.state,
                    atspi::State::Pressed | atspi::State::Checked | atspi::State::Active
                )
            {
                emit_control_action(conn, &ev.item).await?;
            }
        }
        AtspiEvent::Object(ObjectEvents::PropertyChange(ev)) => {
            if ev.property != "accessible-name" && ev.property != "name" {
                return Ok(());
            }
            emit_title_change(conn, &ev.item).await?;
        }
        AtspiEvent::Object(ObjectEvents::TextChanged(ev)) => {
            emit_text_changed(conn, &ev.item).await?;
        }
        AtspiEvent::Document(doc) => {
            let (item, emit_load_action) = match &doc {
                atspi::DocumentEvents::LoadComplete(e) => (&e.item, true),
                atspi::DocumentEvents::Reload(e) => (&e.item, true),
                atspi::DocumentEvents::AttributesChanged(e) => (&e.item, false),
                atspi::DocumentEvents::PageChanged(e) => (&e.item, false),
                _ => return Ok(()),
            };
            // Refresh app context with the real Document URL from a11y.
            emit_document_context(conn, item).await?;
            if emit_load_action {
                emit_ui_action(conn, item, UiActionKind::LoadComplete, None).await?;
            }
        }
        AtspiEvent::Window(WindowEvents::Close(ev)) => {
            emit_ui_action(conn, &ev.item, UiActionKind::Finish, Some("window closed")).await?;
        }
        AtspiEvent::Window(WindowEvents::Activate(ev)) => {
            emit_focus(conn, &ev.item).await?;
        }
        _ => {}
    }
    Ok(())
}

fn build_metadata(
    details: &AppDetails,
    process_id: u32,
    element_meta: &ElementMeta,
) -> EventMetadata {
    EventMetadata {
        window_title: details.title.clone().into(),
        executable_path: (!details.file_path.is_empty()).then(|| details.file_path.clone()),
        process_id: Some(process_id),
        focused_element: element_meta.name.clone(),
        focused_element_class: element_meta
            .class
            .clone()
            .or_else(|| element_meta.role.clone()),
        focused_control_type: element_meta.role_raw.clone(),
        automation_id: element_meta.automation_id.clone(),
        url: element_meta.url.clone(),
        ..Default::default()
    }
}

async fn emit_focus(
    conn: &zbus::Connection,
    item: &atspi::ObjectRefOwned,
) -> Result<(), PlatformError> {
    let Some((details, handle, process_id, element_meta)) = resolve_app(conn, item, true).await
    else {
        return Ok(());
    };
    let fingerprint = details.fingerprint();
    let metadata = build_metadata(&details, process_id, &element_meta);

    push_event(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint,
            details,
            window_handle: handle,
        },
        metadata: metadata.clone(),
    });
    push_event(Event {
        timestamp: Timestamp::now(),
        data: EventData::WindowFocus {
            fingerprint,
            window_handle: handle,
        },
        metadata,
    });
    Ok(())
}

async fn emit_title_change(
    conn: &zbus::Connection,
    item: &atspi::ObjectRefOwned,
) -> Result<(), PlatformError> {
    let Some((details, handle, process_id, element_meta)) = resolve_app(conn, item, true).await
    else {
        return Ok(());
    };
    let fingerprint = details.fingerprint();
    let metadata = build_metadata(&details, process_id, &element_meta);

    push_event(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint,
            details: details.clone(),
            window_handle: handle,
        },
        metadata: metadata.clone(),
    });
    push_event(Event {
        timestamp: Timestamp::now(),
        data: EventData::TitleChange {
            fingerprint,
            new_title: details.title.clone(),
            window_handle: handle,
        },
        metadata,
    });
    Ok(())
}

/// Document load / attribute changes: refresh title + real URL without inventing one.
async fn emit_document_context(
    conn: &zbus::Connection,
    item: &atspi::ObjectRefOwned,
) -> Result<(), PlatformError> {
    let Some((details, handle, process_id, element_meta)) = resolve_app(conn, item, true).await
    else {
        return Ok(());
    };
    let fingerprint = details.fingerprint();
    let metadata = build_metadata(&details, process_id, &element_meta);

    push_event(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint,
            details: details.clone(),
            window_handle: handle,
        },
        metadata: metadata.clone(),
    });
    push_event(Event {
        timestamp: Timestamp::now(),
        data: EventData::TitleChange {
            fingerprint,
            new_title: details.title,
            window_handle: handle,
        },
        metadata,
    });
    Ok(())
}

async fn emit_text_changed(
    conn: &zbus::Connection,
    item: &atspi::ObjectRefOwned,
) -> Result<(), PlatformError> {
    let Some((details, handle, process_id, element_meta)) = resolve_app(conn, item, false).await
    else {
        return Ok(());
    };
    let fingerprint = details.fingerprint();
    let mut metadata = build_metadata(&details, process_id, &element_meta);
    metadata.text_changed = true;
    push_event(Event {
        timestamp: Timestamp::now(),
        data: EventData::TextChanged {
            fingerprint,
            window_handle: handle,
        },
        metadata,
    });
    Ok(())
}

async fn emit_control_action(
    conn: &zbus::Connection,
    item: &atspi::ObjectRefOwned,
) -> Result<(), PlatformError> {
    let Some((details, handle, process_id, element_meta)) = resolve_app(conn, item, false).await
    else {
        return Ok(());
    };
    let name = element_meta.name.clone().unwrap_or_default();
    let role = element_meta.role.as_deref();
    // Only treat typical actionable roles as discrete actions.
    let role_l = role.unwrap_or("").to_ascii_lowercase();
    if !(role_l.contains("button")
        || role_l.contains("push")
        || role_l.contains("menu")
        || role_l.contains("link")
        || role_l.contains("check")
        || role_l.contains("radio"))
    {
        return Ok(());
    }
    let Some(kind) = guess_ui_action_from_control(&name, role) else {
        return Ok(());
    };
    let fingerprint = details.fingerprint();
    push_event(Event {
        timestamp: Timestamp::now(),
        data: EventData::UiAction {
            kind,
            fingerprint,
            window_handle: handle,
            label: Some(name).filter(|s| !s.is_empty()),
        },
        metadata: build_metadata(&details, process_id, &element_meta),
    });
    Ok(())
}

async fn emit_ui_action(
    conn: &zbus::Connection,
    item: &atspi::ObjectRefOwned,
    kind: UiActionKind,
    label: Option<&str>,
) -> Result<(), PlatformError> {
    let Some((details, handle, process_id, element_meta)) = resolve_app(conn, item, true).await
    else {
        return Ok(());
    };
    let fingerprint = details.fingerprint();
    push_event(Event {
        timestamp: Timestamp::now(),
        data: EventData::UiAction {
            kind,
            fingerprint,
            window_handle: handle,
            label: label
                .map(|s| s.to_string())
                .or_else(|| Some(details.title.clone())),
        },
        metadata: build_metadata(&details, process_id, &element_meta),
    });
    Ok(())
}

fn read_exe_path(pid: i32) -> Option<String> {
    std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

#[derive(Default, Clone)]
struct ElementMeta {
    name: Option<String>,
    role: Option<String>,
    role_raw: Option<String>,
    class: Option<String>,
    automation_id: Option<String>,
    url: Option<String>,
}

fn is_window_like(role: Role) -> bool {
    matches!(
        role,
        Role::Frame | Role::Window | Role::Application | Role::DesktopFrame | Role::RootPane
    )
}

async fn accessible_from_ref<'a>(
    conn: &'a zbus::Connection,
    item: &atspi::ObjectRefOwned,
) -> Option<AccessibleProxy<'a>> {
    let sender_ref = item.name()?;
    let sender: OwnedUniqueName = sender_ref.to_owned().into();
    let path = item.path().to_owned();
    AccessibleProxy::builder(conn)
        .destination(sender)
        .ok()?
        .path(path)
        .ok()?
        .build()
        .await
        .ok()
}

async fn element_meta_from(accessible: &AccessibleProxy<'_>) -> ElementMeta {
    let element_name = accessible.name().await.ok().filter(|s| !s.is_empty());
    let role = accessible.get_role().await.ok();
    let role_str = role.map(|r| format!("{r:?}"));
    let role_friendly = role.map(role_label);
    let attrs = accessible.get_attributes().await.unwrap_or_default();
    let automation_id = attrs
        .get("id")
        .or_else(|| attrs.get("xml-id"))
        .or_else(|| attrs.get("html-id"))
        .cloned()
        .filter(|s| !s.is_empty());
    let class = attrs
        .get("class")
        .or_else(|| attrs.get("className"))
        .or_else(|| attrs.get("tag"))
        .cloned()
        .filter(|s| !s.is_empty());

    ElementMeta {
        name: element_name,
        role: role_friendly,
        role_raw: role_str,
        class,
        automation_id,
        url: None,
    }
}

/// Prefer Frame/Window accessible-name as the window title (not the focused control).
async fn resolve_window_title(
    conn: &zbus::Connection,
    accessible: &AccessibleProxy<'_>,
    fallback: &str,
) -> String {
    let mut current = accessible.clone();
    for _ in 0..24 {
        if let Ok(role) = current.get_role().await {
            if matches!(role, Role::Frame | Role::Window | Role::DesktopFrame) {
                if let Ok(name) = current.name().await {
                    if !name.is_empty() {
                        return name;
                    }
                }
            }
        }
        let Ok(parent_ref) = current.parent().await else {
            break;
        };
        let Some(parent) = accessible_from_ref(conn, &parent_ref).await else {
            break;
        };
        current = parent;
    }
    if let Ok(name) = accessible.name().await {
        if !name.is_empty() {
            return name;
        }
    }
    fallback.to_string()
}

fn is_document_role(role: Role) -> bool {
    matches!(
        role,
        Role::DocumentWeb
            | Role::DocumentFrame
            | Role::DocumentSpreadsheet
            | Role::DocumentText
            | Role::DocumentPresentation
    )
}

fn pick_url_from_attrs(attrs: &std::collections::HashMap<String, String>) -> Option<String> {
    const KEYS: &[&str] = &[
        "DocURL",
        "URI",
        "Uri",
        "url",
        "URL",
        "doc-url",
        "document-url",
    ];
    for key in KEYS {
        if let Some(v) = attrs.get(*key) {
            let t = v.trim();
            if t.starts_with("http://")
                || t.starts_with("https://")
                || t.starts_with("file:")
                || t.starts_with("about:")
            {
                return Some(t.to_string());
            }
        }
    }
    for (k, v) in attrs {
        let kl = k.to_ascii_lowercase();
        if kl.contains("url") || kl == "uri" {
            let t = v.trim();
            if t.starts_with("http://")
                || t.starts_with("https://")
                || t.starts_with("file:")
                || t.starts_with("about:")
            {
                return Some(t.to_string());
            }
        }
    }
    None
}

async fn url_from_document_iface(accessible: &AccessibleProxy<'_>) -> Option<String> {
    let proxies = accessible.proxies().await.ok()?;
    let doc = proxies.document().await.ok()?;
    if let Ok(attrs) = doc.get_attributes().await {
        if let Some(url) = pick_url_from_attrs(&attrs) {
            return Some(url);
        }
    }
    for key in ["DocURL", "URI", "url", "URL"] {
        if let Ok(v) = doc.get_attribute_value(key).await {
            let t = v.trim();
            if t.starts_with("http://")
                || t.starts_with("https://")
                || t.starts_with("file:")
                || t.starts_with("about:")
            {
                return Some(t.to_string());
            }
        }
    }
    None
}

async fn url_from_accessible_attrs(accessible: &AccessibleProxy<'_>) -> Option<String> {
    let attrs = accessible.get_attributes().await.ok()?;
    pick_url_from_attrs(&attrs)
}

async fn non_empty_document_url(accessible: &AccessibleProxy<'_>) -> Option<String> {
    url_from_document_iface(accessible)
        .await
        .or(url_from_accessible_attrs(accessible).await)
}

/// Climb to the nearest Frame/Window proxy (browser chrome root).
async fn find_frame_ancestor<'a>(
    conn: &'a zbus::Connection,
    start: &AccessibleProxy<'a>,
) -> Option<AccessibleProxy<'a>> {
    let mut current = start.clone();
    for _ in 0..32 {
        if let Ok(role) = current.get_role().await {
            if matches!(role, Role::Frame | Role::Window | Role::DesktopFrame) {
                return Some(current);
            }
        }
        let Ok(parent_ref) = current.parent().await else {
            break;
        };
        current = accessible_from_ref(conn, &parent_ref).await?;
    }
    None
}

/// Chromium/Brave: real URL lives on Role::DocumentWeb under the frame.
/// Application/Frame expose Document with empty URI — those must be skipped.
async fn find_document_web(
    conn: &zbus::Connection,
    root: &AccessibleProxy<'_>,
) -> Option<(atspi::ObjectRefOwned, String)> {
    if let Ok(proxies) = root.proxies().await {
        if let Ok(collection) = proxies.collection().await {
            let rule = ObjectMatchRule::builder()
                .roles(&[Role::DocumentWeb, Role::DocumentFrame], MatchType::Any)
                .build();
            if let Ok(matches) = collection
                .get_matches(rule, SortOrder::Canonical, 16, true)
                .await
            {
                for obj in matches {
                    let Some(acc) = accessible_from_ref(conn, &obj).await else {
                        continue;
                    };
                    if let Some(url) = non_empty_document_url(&acc).await {
                        return Some((obj, url));
                    }
                }
            }
        }
    }

    let mut queue: VecDeque<(atspi::ObjectRefOwned, u8)> = VecDeque::new();
    if let Ok(children) = root.get_children().await {
        for child in children {
            queue.push_back((child, 0));
        }
    }
    let mut visited = 0usize;
    while let Some((obj, depth)) = queue.pop_front() {
        visited += 1;
        if visited > 400 || depth > 14 {
            continue;
        }
        let Some(acc) = accessible_from_ref(conn, &obj).await else {
            continue;
        };
        let role = acc.get_role().await.ok();
        if let Some(role) = role {
            if is_document_role(role) {
                if let Some(url) = non_empty_document_url(&acc).await {
                    return Some((obj, url));
                }
            }
            if matches!(
                role,
                Role::MenuBar
                    | Role::Menu
                    | Role::MenuItem
                    | Role::ToolBar
                    | Role::PageTabList
                    | Role::Notification
            ) {
                continue;
            }
        }
        if let Ok(children) = acc.get_children().await {
            let mut prioritized = Vec::new();
            let mut rest = Vec::new();
            for child in children.into_iter().take(40) {
                if let Some(c) = accessible_from_ref(conn, &child).await {
                    let r = c.get_role().await.ok();
                    if r.map(is_document_role).unwrap_or(false)
                        || matches!(
                            r,
                            Some(Role::ScrollPane)
                                | Some(Role::Panel)
                                | Some(Role::Filler)
                                | Some(Role::PageTab)
                        )
                    {
                        prioritized.push(child);
                    } else {
                        rest.push(child);
                    }
                }
            }
            for child in prioritized.into_iter().chain(rest) {
                queue.push_back((child, depth + 1));
            }
        }
    }
    None
}

async fn lookup_document_url(
    conn: &zbus::Connection,
    start: &AccessibleProxy<'_>,
) -> Option<String> {
    if let Some(url) = non_empty_document_url(start).await {
        return Some(url);
    }

    let mut current = start.clone();
    for _ in 0..32 {
        if let Ok(role) = current.get_role().await {
            if is_document_role(role) {
                if let Some(url) = non_empty_document_url(&current).await {
                    return Some(url);
                }
            }
        }
        if let Some(url) = non_empty_document_url(&current).await {
            return Some(url);
        }
        let Ok(parent_ref) = current.parent().await else {
            break;
        };
        let Some(parent) = accessible_from_ref(conn, &parent_ref).await else {
            break;
        };
        current = parent;
    }

    let root = find_frame_ancestor(conn, start)
        .await
        .unwrap_or_else(|| start.clone());
    find_document_web(conn, &root)
        .await
        .map(|(_, url)| url)
}

async fn find_focused_ref(
    conn: &zbus::Connection,
    start: &AccessibleProxy<'_>,
) -> Option<atspi::ObjectRefOwned> {
    if let Ok(proxies) = start.proxies().await {
        if let Ok(collection) = proxies.collection().await {
            let rule = ObjectMatchRule::builder()
                .states([State::Focused], MatchType::All)
                .build();
            if let Ok(matches) = collection
                .get_matches(rule, SortOrder::Canonical, 8, true)
                .await
            {
                for obj in matches {
                    if let Some(acc) = accessible_from_ref(conn, &obj).await {
                        if let Ok(role) = acc.get_role().await {
                            if !is_window_like(role) {
                                return Some(obj);
                            }
                        }
                    }
                }
            }
        }
    }

    let mut queue: VecDeque<(atspi::ObjectRefOwned, u8)> = VecDeque::new();
    if let Ok(children) = start.get_children().await {
        for child in children {
            queue.push_back((child, 0));
        }
    }
    let mut visited = 0usize;
    while let Some((obj, depth)) = queue.pop_front() {
        visited += 1;
        if visited > 250 || depth > 12 {
            continue;
        }
        let Some(acc) = accessible_from_ref(conn, &obj).await else {
            continue;
        };
        let role = acc.get_role().await.ok();
        if let Ok(states) = acc.get_state().await {
            if states.contains(State::Focused) {
                if role.map(|r| !is_window_like(r)).unwrap_or(true) {
                    return Some(obj);
                }
            }
        }
        if let Some(role) = role {
            if matches!(
                role,
                Role::MenuBar | Role::Menu | Role::ToolBar | Role::PageTabList
            ) {
                continue;
            }
        }
        if let Ok(children) = acc.get_children().await {
            for child in children.into_iter().take(32) {
                queue.push_back((child, depth + 1));
            }
        }
    }
    None
}

async fn resolve_app(
    conn: &zbus::Connection,
    item: &atspi::ObjectRefOwned,
    refine_focus: bool,
) -> Option<(AppDetails, u64, u32, ElementMeta)> {
    let mut accessible = accessible_from_ref(conn, item).await?;
    let mut known_url: Option<String> = None;

    let role = accessible.get_role().await.ok();
    if refine_focus {
        if role.map(is_window_like).unwrap_or(false) {
            if let Some(focused_ref) = find_focused_ref(conn, &accessible).await {
                if let Some(focused) = accessible_from_ref(conn, &focused_ref).await {
                    accessible = focused;
                }
            } else {
                // Window activate often has no Focused child — use the web document.
                let frame = find_frame_ancestor(conn, &accessible)
                    .await
                    .unwrap_or_else(|| accessible.clone());
                if let Some((doc_ref, url)) = find_document_web(conn, &frame).await {
                    known_url = Some(url);
                    if let Some(doc) = accessible_from_ref(conn, &doc_ref).await {
                        accessible = doc;
                    }
                }
            }
        }
    }

    let mut element_meta = element_meta_from(&accessible).await;
    element_meta.url = known_url.or(lookup_document_url(conn, &accessible).await);

    let app_ref = accessible.get_application().await.ok()?;
    let app_sender: OwnedUniqueName = app_ref.name()?.to_owned().into();
    let app_path = app_ref.path().to_owned();

    let app_proxy = AccessibleProxy::builder(conn)
        .destination(app_sender.clone())
        .ok()?
        .path(app_path.clone())
        .ok()?
        .build()
        .await
        .ok()?;

    let name = app_proxy.name().await.unwrap_or_default();
    let title = resolve_window_title(conn, &accessible, &name).await;

    let dbus = DBusProxy::new(conn).await.ok()?;
    let pid = dbus
        .get_connection_unix_process_id(BusName::from(&app_sender))
        .await
        .ok()?;

    let file_path = read_exe_path(pid as i32).unwrap_or_default();

    let product_name = if name.is_empty() {
        Path::new(&file_path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
    } else {
        Some(name.clone())
    };

    // Prefer a stable handle derived from the application object.
    let mut handle_hasher = DefaultHasher::new();
    app_sender.as_str().hash(&mut handle_hasher);
    app_path.as_str().hash(&mut handle_hasher);
    let handle = handle_hasher.finish();

    // Prefer toolkit application id when available as aumid.
    let aumid = app_proxy
        .get_attributes()
        .await
        .ok()
        .and_then(|attrs| {
            attrs
                .into_iter()
                .find(|(k, _)| {
                    let k = k.to_ascii_lowercase();
                    k == "app_id" || k == "application:id" || k == "id"
                })
                .map(|(_, v)| v)
        })
        .filter(|s| !s.is_empty());

    let details = enrich_app_details(AppDetails {
        title,
        file_path,
        aumid,
        company_name: None,
        product_name,
        version_info: None,
        signature_info: None,
    });

    // Keep element name as the focused control, not the window title when they match.
    if element_meta.name.as_deref() == Some(details.title.as_str()) {
        element_meta.name = None;
    }

    Some((details, handle, pid as u32, element_meta))
}

fn role_label(role: Role) -> String {
    format!("{role:?}")
}

/// Look up URL + focused UI via AT-SPI for a process (used when Sway drives window events).
pub fn enrich_metadata_from_atspi(pid: u32, meta: &mut EventMetadata) {
    if pid == 0 {
        return;
    }
    let title_hint = meta.window_title.clone();
    let Some(extra) = query_context_for_pid(pid, title_hint.as_deref()) else {
        tracing::debug!(pid, "AT-SPI enrichment found no context");
        return;
    };
    if meta.url.is_none() {
        meta.url = extra.url;
    }
    let is_document = extra
        .role_raw
        .as_deref()
        .is_some_and(|r| r.contains("Document"));
    if meta.focused_element.is_none() {
        meta.focused_element = extra.name.clone();
    }
    if meta.focused_element_class.is_none() {
        meta.focused_element_class = extra.class.or(extra.role);
    }
    if meta.focused_control_type.is_none() {
        meta.focused_control_type = extra.role_raw;
    }
    if meta.automation_id.is_none() {
        meta.automation_id = extra.automation_id;
    }
    if meta.document_name.is_none() && is_document {
        meta.document_name = extra.name;
    }
}

fn query_context_for_pid(pid: u32, title_hint: Option<&str>) -> Option<ElementMeta> {
    use std::sync::OnceLock;
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    let rt = RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .thread_name("intime-atspi-enrich")
            .build()
            .expect("atspi enrich runtime")
    });
    rt.block_on(query_context_for_pid_async(pid, title_hint))
}

async fn query_context_for_pid_async(pid: u32, title_hint: Option<&str>) -> Option<ElementMeta> {
    let connection = AccessibilityConnection::new().await.ok()?;
    let conn = connection.connection();
    let app = find_app_by_pid(conn, pid).await?;
    let frame = pick_frame(conn, &app, title_hint).await.unwrap_or(app);

    let mut known_url: Option<String> = None;
    let mut accessible = frame.clone();

    if let Some(focused_ref) = find_focused_ref(conn, &frame).await {
        if let Some(focused) = accessible_from_ref(conn, &focused_ref).await {
            accessible = focused;
        }
    } else if let Some((doc_ref, url)) = find_document_web(conn, &frame).await {
        known_url = Some(url);
        if let Some(doc) = accessible_from_ref(conn, &doc_ref).await {
            accessible = doc;
        }
    }

    let mut element_meta = element_meta_from(&accessible).await;
    element_meta.url = known_url.or(lookup_document_url(conn, &accessible).await);

    // If we still only have the frame, try document web for URL + better focus label.
    if element_meta.url.is_none() || element_meta.name.is_none() {
        if let Some((doc_ref, url)) = find_document_web(conn, &frame).await {
            element_meta.url = element_meta.url.or(Some(url));
            if element_meta.name.is_none() {
                if let Some(doc) = accessible_from_ref(conn, &doc_ref).await {
                    let doc_meta = element_meta_from(&doc).await;
                    element_meta.name = doc_meta.name;
                    element_meta.role = doc_meta.role;
                    element_meta.role_raw = doc_meta.role_raw;
                    element_meta.class = doc_meta.class;
                    element_meta.automation_id = doc_meta.automation_id;
                }
            }
        }
    }

    Some(element_meta)
}

async fn find_app_by_pid<'a>(
    conn: &'a zbus::Connection,
    pid: u32,
) -> Option<AccessibleProxy<'a>> {
    let registry = AccessibleProxy::builder(conn)
        .destination("org.a11y.atspi.Registry")
        .ok()?
        .path("/org/a11y/atspi/accessible/root")
        .ok()?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
        .ok()?;
    let dbus = DBusProxy::new(conn).await.ok()?;
    let children = registry.get_children().await.ok()?;
    for child in children {
        let Some(sender_ref) = child.name() else {
            continue;
        };
        let sender: OwnedUniqueName = sender_ref.to_owned().into();
        let Ok(app_pid) = dbus
            .get_connection_unix_process_id(BusName::from(&sender))
            .await
        else {
            continue;
        };
        if app_pid == pid {
            return accessible_from_ref(conn, &child).await;
        }
    }
    None
}

async fn pick_frame<'a>(
    conn: &'a zbus::Connection,
    app: &AccessibleProxy<'a>,
    title_hint: Option<&str>,
) -> Option<AccessibleProxy<'a>> {
    let children = app.get_children().await.ok()?;
    let mut frames: Vec<AccessibleProxy<'a>> = Vec::new();
    for child in children {
        let Some(acc) = accessible_from_ref(conn, &child).await else {
            continue;
        };
        let role = acc.get_role().await.ok();
        if role
            .map(|r| matches!(r, Role::Frame | Role::Window))
            .unwrap_or(false)
        {
            frames.push(acc);
        }
    }
    if frames.is_empty() {
        return None;
    }

    if let Some(hint) = title_hint.filter(|s| !s.is_empty()) {
        // Exact title match (Sway and AT-SPI frame names usually agree).
        for frame in &frames {
            if let Ok(name) = frame.name().await {
                if name == hint {
                    return Some(frame.clone());
                }
            }
        }
        let hint_l = hint.to_ascii_lowercase();
        for frame in &frames {
            if let Ok(name) = frame.name().await {
                if name.is_empty() {
                    continue;
                }
                let name_l = name.to_ascii_lowercase();
                if hint_l.contains(&name_l) || name_l.contains(&hint_l) {
                    return Some(frame.clone());
                }
            }
        }
    }

    for frame in &frames {
        if let Ok(name) = frame.name().await {
            if !name.is_empty() {
                return Some(frame.clone());
            }
        }
    }
    frames.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::pick_url_from_attrs;
    use std::collections::HashMap;

    #[test]
    fn picks_docurl_from_document_attributes() {
        let attrs = HashMap::from([
            ("DocURL".into(), "https://x.com/home".into()),
            ("MimeType".into(), "text/html".into()),
        ]);
        assert_eq!(
            pick_url_from_attrs(&attrs).as_deref(),
            Some("https://x.com/home")
        );
    }

    #[test]
    fn picks_uri_key_used_by_chromium() {
        let attrs = HashMap::from([
            ("URI".into(), "https://www.netflix.com/browse".into()),
            ("Title".into(), "Home - Netflix".into()),
            ("MimeType".into(), "text/html".into()),
        ]);
        assert_eq!(
            pick_url_from_attrs(&attrs).as_deref(),
            Some("https://www.netflix.com/browse")
        );
    }

    #[test]
    fn ignores_empty_uri_shells() {
        let attrs = HashMap::from([
            ("URI".into(), "".into()),
            ("Title".into(), "".into()),
        ]);
        assert!(pick_url_from_attrs(&attrs).is_none());
    }
}
