use crate::{error::PlatformError, shared::push_event};
use std::path::Path;

use atspi::{
    AccessibilityConnection, Event as AtspiEvent, FocusEvents, ObjectEvents,
    events::object::StateChangedEvent,
    proxy::accessible::AccessibleProxy,
    zbus::{self, fdo::DBusProxy, names::BusName},
};
use futures_lite::StreamExt;
use intime_core::{
    models::{AppDetails, Event, EventData, EventMetadata},
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
        }
        AtspiEvent::Object(ObjectEvents::PropertyChange(ev)) => {
            if ev.property != "accessible-name" && ev.property != "name" {
                return Ok(());
            }
            emit_title_change(conn, &ev.item).await?;
        }
        _ => {}
    }
    Ok(())
}

async fn emit_focus(
    conn: &zbus::Connection,
    item: &atspi::ObjectRefOwned,
) -> Result<(), PlatformError> {
    let Some((details, handle, process_id)) = resolve_app(conn, item).await else {
        return Ok(());
    };
    let fingerprint = details.fingerprint();

    push_event(Event {
        timestamp: Timestamp::now(),
        data: EventData::WindowFocus {
            fingerprint,
            window_handle: handle,
        },
        metadata: EventMetadata {
            window_title: details.title.clone().into(),
            executable_path: (!details.file_path.is_empty()).then(|| details.file_path.clone()),
            process_id: Some(process_id),
            ..Default::default()
        },
    });
    push_event(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint,
            details,
            window_handle: handle,
        },
        metadata: EventMetadata::default(),
    });
    Ok(())
}

async fn emit_title_change(
    conn: &zbus::Connection,
    item: &atspi::ObjectRefOwned,
) -> Result<(), PlatformError> {
    let Some((details, handle, process_id)) = resolve_app(conn, item).await else {
        return Ok(());
    };
    let fingerprint = details.fingerprint();

    push_event(Event {
        timestamp: Timestamp::now(),
        data: EventData::TitleChange {
            fingerprint,
            new_title: details.title.clone(),
            window_handle: handle,
        },
        metadata: EventMetadata {
            window_title: details.title.clone().into(),
            executable_path: (!details.file_path.is_empty()).then(|| details.file_path.clone()),
            process_id: Some(process_id),
            ..Default::default()
        },
    });
    push_event(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint,
            details,
            window_handle: handle,
        },
        metadata: EventMetadata::default(),
    });
    Ok(())
}

fn read_exe_path(pid: i32) -> Option<String> {
    std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

async fn resolve_app(
    conn: &zbus::Connection,
    item: &atspi::ObjectRefOwned,
) -> Option<(AppDetails, u64, u32)> {
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
    let title = accessible.name().await.unwrap_or_else(|_| name.clone());

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

    Some((
        AppDetails {
            title,
            file_path,
            aumid: None,
            company_name: None,
            product_name,
            version_info: None,
            signature_info: None,
        },
        handle,
        pid as u32,
    ))
}
