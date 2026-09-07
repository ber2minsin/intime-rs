use crate::{
    error::PlatformError,
    linux::identity::enrich_app_details,
    shared::push_event,
};
use std::path::Path;

use atspi::{
    AccessibilityConnection, Event as AtspiEvent, FocusEvents, ObjectEvents, Role, WindowEvents,
    events::object::StateChangedEvent,
    proxy::accessible::AccessibleProxy,
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
            if matches!(
                doc,
                atspi::DocumentEvents::LoadComplete(_) | atspi::DocumentEvents::Reload(_)
            ) {
                let item = match &doc {
                    atspi::DocumentEvents::LoadComplete(e) => &e.item,
                    atspi::DocumentEvents::Reload(e) => &e.item,
                    _ => return Ok(()),
                };
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

async fn emit_focus(
    conn: &zbus::Connection,
    item: &atspi::ObjectRefOwned,
) -> Result<(), PlatformError> {
    let Some((details, handle, process_id, element_meta)) = resolve_app(conn, item).await else {
        return Ok(());
    };
    let fingerprint = details.fingerprint();
    let metadata = EventMetadata {
        window_title: details.title.clone().into(),
        executable_path: (!details.file_path.is_empty()).then(|| details.file_path.clone()),
        process_id: Some(process_id),
        focused_element: element_meta.name,
        focused_element_class: element_meta.role,
        focused_control_type: element_meta.role_raw,
        automation_id: element_meta.automation_id,
        ..Default::default()
    };

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
    let Some((details, handle, process_id, _)) = resolve_app(conn, item).await else {
        return Ok(());
    };
    let fingerprint = details.fingerprint();
    let metadata = EventMetadata {
        window_title: details.title.clone().into(),
        executable_path: (!details.file_path.is_empty()).then(|| details.file_path.clone()),
        process_id: Some(process_id),
        ..Default::default()
    };

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

async fn emit_text_changed(
    conn: &zbus::Connection,
    item: &atspi::ObjectRefOwned,
) -> Result<(), PlatformError> {
    let Some((details, handle, process_id, element_meta)) = resolve_app(conn, item).await else {
        return Ok(());
    };
    let fingerprint = details.fingerprint();
    push_event(Event {
        timestamp: Timestamp::now(),
        data: EventData::TextChanged {
            fingerprint,
            window_handle: handle,
        },
        metadata: EventMetadata {
            window_title: details.title.clone().into(),
            executable_path: (!details.file_path.is_empty()).then(|| details.file_path.clone()),
            process_id: Some(process_id),
            focused_element: element_meta.name,
            focused_control_type: element_meta.role_raw,
            text_changed: true,
            ..Default::default()
        },
    });
    Ok(())
}

async fn emit_control_action(
    conn: &zbus::Connection,
    item: &atspi::ObjectRefOwned,
) -> Result<(), PlatformError> {
    let Some((details, handle, process_id, element_meta)) = resolve_app(conn, item).await else {
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
        metadata: EventMetadata {
            window_title: details.title.clone().into(),
            executable_path: (!details.file_path.is_empty()).then(|| details.file_path.clone()),
            process_id: Some(process_id),
            focused_element: element_meta.name,
            focused_control_type: element_meta.role_raw,
            automation_id: element_meta.automation_id,
            ..Default::default()
        },
    });
    Ok(())
}

async fn emit_ui_action(
    conn: &zbus::Connection,
    item: &atspi::ObjectRefOwned,
    kind: UiActionKind,
    label: Option<&str>,
) -> Result<(), PlatformError> {
    let Some((details, handle, process_id, _)) = resolve_app(conn, item).await else {
        return Ok(());
    };
    let fingerprint = details.fingerprint();
    push_event(Event {
        timestamp: Timestamp::now(),
        data: EventData::UiAction {
            kind,
            fingerprint,
            window_handle: handle,
            label: label.map(|s| s.to_string()).or_else(|| Some(details.title.clone())),
        },
        metadata: EventMetadata {
            window_title: details.title.clone().into(),
            executable_path: (!details.file_path.is_empty()).then(|| details.file_path.clone()),
            process_id: Some(process_id),
            ..Default::default()
        },
    });
    Ok(())
}

fn read_exe_path(pid: i32) -> Option<String> {
    std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

#[derive(Default)]
struct ElementMeta {
    name: Option<String>,
    role: Option<String>,
    role_raw: Option<String>,
    automation_id: Option<String>,
}

async fn resolve_app(
    conn: &zbus::Connection,
    item: &atspi::ObjectRefOwned,
) -> Option<(AppDetails, u64, u32, ElementMeta)> {
    let sender_ref = item.name()?;
    let sender: OwnedUniqueName = sender_ref.to_owned().into();
    let path = item.path().to_owned();

    let accessible = AccessibleProxy::builder(conn)
        .destination(sender.clone())
        .ok()?
        .path(path)
        .ok()?
        .build()
        .await
        .ok()?;

    let element_name = accessible.name().await.ok().filter(|s| !s.is_empty());
    let role = accessible.get_role().await.ok();
    let role_str = role.map(|r| format!("{r:?}"));
    let role_friendly = role.map(role_label);
    let mut element_meta = ElementMeta {
        name: element_name,
        role: role_friendly,
        role_raw: role_str,
        automation_id: None,
    };

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
    let title = accessible
        .name()
        .await
        .unwrap_or_else(|_| name.clone());

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
