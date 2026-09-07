use std::path::Path;

use intime_core::{
    models::{AppDetails, Event, EventData, EventMetadata, UiActionKind},
    time::Timestamp,
};
use swayipc::{Connection, Event as SwayEvent, EventType, Node, WindowChange};

use crate::{
    error::PlatformError,
    linux::identity::enrich_app_details,
    shared::push_event,
};

pub fn run_event_loop() -> Result<(), PlatformError> {
    let conn = Connection::new()
        .map_err(|e| PlatformError::Other(anyhow::anyhow!("Failed to connect to Sway IPC: {e}")))?;

    let events = conn.subscribe(&[EventType::Window]).map_err(|e| {
        PlatformError::Other(anyhow::anyhow!("Failed to subscribe to Sway events: {e}"))
    })?;

    for event in events {
        let event = match event {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("Sway IPC event error: {e}");
                continue;
            }
        };

        if let SwayEvent::Window(window) = event {
            handle_window_event(window.change, &window.container);
        }
    }

    Ok(())
}

fn handle_window_event(change: WindowChange, node: &Node) {
    // Only surface real application windows (leaf containers with a process).
    if node.pid.is_none() {
        return;
    }

    let details = match app_details_from_node(node) {
        Some(d) => d,
        None => return,
    };
    let fingerprint = details.fingerprint();
    let window_handle = node.id as u64;
    let timestamp = Timestamp::now();
    let metadata = EventMetadata {
        window_title: Some(details.title.clone()),
        process_id: node.pid.map(|pid| pid as u32),
        executable_path: (!details.file_path.is_empty()).then(|| details.file_path.clone()),
        ..Default::default()
    };

    match change {
        WindowChange::Focus => {
            // AppSeen first so focus can resolve app_id in the same tick.
            push_event(Event {
                timestamp,
                data: EventData::AppSeen {
                    fingerprint,
                    details: details.clone(),
                    window_handle,
                },
                metadata: metadata.clone(),
            });
            push_event(Event {
                timestamp: Timestamp::now(),
                data: EventData::WindowFocus {
                    fingerprint,
                    window_handle,
                },
                metadata,
            });
        }
        WindowChange::Title => {
            push_event(Event {
                timestamp,
                data: EventData::AppSeen {
                    fingerprint,
                    details: details.clone(),
                    window_handle,
                },
                metadata: metadata.clone(),
            });
            push_event(Event {
                timestamp: Timestamp::now(),
                data: EventData::TitleChange {
                    fingerprint,
                    new_title: details.title.clone(),
                    window_handle,
                },
                metadata,
            });
        }
        WindowChange::New => {
            push_event(Event {
                timestamp,
                data: EventData::AppSeen {
                    fingerprint,
                    details,
                    window_handle,
                },
                metadata: metadata.clone(),
            });
            push_event(Event {
                timestamp: Timestamp::now(),
                data: EventData::UiAction {
                    kind: UiActionKind::Open,
                    fingerprint,
                    window_handle,
                    label: metadata.window_title.clone(),
                },
                metadata,
            });
        }
        WindowChange::Close => {
            push_event(Event {
                timestamp,
                data: EventData::UiAction {
                    kind: UiActionKind::Finish,
                    fingerprint,
                    window_handle,
                    label: metadata.window_title.clone(),
                },
                metadata,
            });
        }
        _ => {}
    }
}

pub fn app_details_from_node(node: &Node) -> Option<AppDetails> {
    let pid = node.pid?;
    let title = node.name.clone().unwrap_or_default();
    let app_id = node.app_id.clone();
    let file_path = read_exe_path(pid).unwrap_or_default();
    let product_name = app_id.clone().or_else(|| {
        Path::new(&file_path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
    });

    let details = AppDetails {
        title,
        file_path,
        aumid: app_id,
        company_name: None,
        product_name,
        version_info: None,
        signature_info: None,
    };
    Some(enrich_app_details(details))
}

pub fn find_node_by_id(root: &Node, id: i64) -> Option<&Node> {
    if root.id == id {
        return Some(root);
    }
    for child in root.nodes.iter().chain(root.floating_nodes.iter()) {
        if let Some(found) = find_node_by_id(child, id) {
            return Some(found);
        }
    }
    None
}

fn read_exe_path(pid: i32) -> Option<String> {
    std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}
