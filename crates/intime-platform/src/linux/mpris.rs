//! MPRIS media player watcher (Linux).
//!
//! Browsers and media apps (Brave, Firefox, Spotify, …) expose play/pause via
//! `org.mpris.MediaPlayer2.*` on the session bus. Window titles alone never
//! surface those controls.

use std::collections::HashMap;
use std::time::Duration;

use intime_core::{
    models::{Event, EventData, EventMetadata, UiActionKind},
    time::Timestamp,
};
use zbus::{
    Connection,
    fdo::DBusProxy,
    names::BusName,
    proxy,
    zvariant::{ObjectPath, OwnedValue},
};

use crate::shared::push_event;

const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
const PLAYER_PATH: &str = "/org/mpris/MediaPlayer2";

#[proxy(
    interface = "org.mpris.MediaPlayer2.Player",
    default_path = "/org/mpris/MediaPlayer2",
    assume_defaults = true
)]
trait MediaPlayer2Player {
    #[zbus(property)]
    fn playback_status(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn metadata(&self) -> zbus::Result<HashMap<String, OwnedValue>>;
}

/// Block forever, polling MPRIS players and emitting play/pause UiActions.
pub fn run_event_loop() {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::warn!("MPRIS watcher: failed to create runtime: {e}");
            return;
        }
    };
    if let Err(e) = rt.block_on(async_loop()) {
        tracing::warn!("MPRIS watcher exited: {e:#}");
    }
}

async fn async_loop() -> anyhow::Result<()> {
    let conn = Connection::session().await?;
    let dbus = DBusProxy::new(&conn).await?;
    let mut last_status: HashMap<String, String> = HashMap::new();

    tracing::info!("MPRIS media watcher started");

    loop {
        match list_mpris_names(&dbus).await {
            Ok(names) => {
                let active: std::collections::HashSet<String> =
                    names.iter().cloned().collect();
                last_status.retain(|k, _| active.contains(k));

                for name in names {
                    if let Err(e) = poll_player(&conn, &name, &mut last_status).await {
                        tracing::debug!("MPRIS poll {name}: {e:#}");
                    }
                }
            }
            Err(e) => tracing::debug!("MPRIS list_names: {e:#}"),
        }
        tokio::time::sleep(Duration::from_millis(1500)).await;
    }
}

async fn list_mpris_names(dbus: &DBusProxy<'_>) -> anyhow::Result<Vec<String>> {
    let names = dbus.list_names().await?;
    Ok(names
        .into_iter()
        .map(|n| n.to_string())
        .filter(|n| n.starts_with(MPRIS_PREFIX))
        .collect())
}

async fn poll_player(
    conn: &Connection,
    name: &str,
    last_status: &mut HashMap<String, String>,
) -> anyhow::Result<()> {
    let dest = BusName::try_from(name.to_string())?;
    let path = ObjectPath::try_from(PLAYER_PATH)?;
    let player = MediaPlayer2PlayerProxy::builder(conn)
        .destination(dest)?
        .path(path)?
        .build()
        .await?;

    let status = player.playback_status().await?;
    let prev = last_status.insert(name.to_string(), status.clone());

    if prev.as_deref() == Some(status.as_str()) {
        return Ok(());
    }
    // Skip Stopped on first sight to avoid noise at watcher start.
    if prev.is_none() && status != "Playing" && status != "Paused" {
        return Ok(());
    }

    let meta = player.metadata().await.ok();
    let title = meta.as_ref().and_then(|m| extract_title(m));
    let url = meta.as_ref().and_then(|m| extract_url(m));
    let identity = short_player_id(name);

    let kind = match status.as_str() {
        "Playing" => UiActionKind::PlayMedia,
        "Paused" | "Stopped" => UiActionKind::PauseMedia,
        _ => return Ok(()),
    };

    let label = title
        .clone()
        .unwrap_or_else(|| format!("{identity}: {status}"));

    // Help category rules: browsers often expose only the track title via MPRIS,
    // so fold player id / site hints into the title when URL enrichment is thin.
    let window_title = match (&url, title.as_deref()) {
        (Some(u), Some(t)) if !u.is_empty() => {
            if u.contains("youtube") || u.contains("youtu.be") {
                format!("{t} - YouTube")
            } else {
                t.to_string()
            }
        }
        (None, Some(t)) if is_browser_player(&identity) => {
            format!("{t} - {identity}")
        }
        _ => label.clone(),
    };

    let fingerprint = blake3::hash(format!("mpris\x1f{identity}").as_bytes());
    let window_handle = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        identity.hash(&mut h);
        h.finish()
    };

    push_event(Event {
        timestamp: Timestamp::now(),
        data: EventData::UiAction {
            kind,
            fingerprint,
            window_handle,
            label: Some(label),
        },
        metadata: EventMetadata {
            window_title: Some(window_title),
            url,
            focused_element: Some(status),
            focused_control_type: Some("mpris".into()),
            automation_id: Some(identity),
            ..Default::default()
        },
    });

    Ok(())
}

fn is_browser_player(identity: &str) -> bool {
    let id = identity.to_ascii_lowercase();
    id.contains("brave")
        || id.contains("firefox")
        || id.contains("chrome")
        || id.contains("chromium")
        || id.contains("edge")
        || id.contains("vivaldi")
        || id.contains("opera")
}

fn short_player_id(bus_name: &str) -> String {
    bus_name
        .strip_prefix(MPRIS_PREFIX)
        .unwrap_or(bus_name)
        .split('.')
        .next()
        .unwrap_or(bus_name)
        .to_string()
}

fn extract_string_meta(meta: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    let value = meta.get(key)?;
    if let Ok(s) = <&str>::try_from(value) {
        let s = s.trim();
        if !s.is_empty() {
            return Some(s.to_string());
        }
    }
    if let Ok(s) = String::try_from(value.try_clone().ok()?) {
        let s = s.trim().to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }
    // Last resort: strip Debug wrappers like OwnedValue(Str("Title")).
    scrub_owned_value_debug(&format!("{value:?}"))
}

fn extract_title(meta: &HashMap<String, OwnedValue>) -> Option<String> {
    extract_string_meta(meta, "xesam:title")
}

fn extract_url(meta: &HashMap<String, OwnedValue>) -> Option<String> {
    extract_string_meta(meta, "xesam:url")
        .or_else(|| extract_string_meta(meta, "xesam:website"))
}

fn scrub_owned_value_debug(rendered: &str) -> Option<String> {
    let mut s = rendered.trim();
    for prefix in [
        "OwnedValue(Str(",
        "OwnedValue(Value::Str(",
        "Str(",
        "Value::Str(",
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest;
            break;
        }
    }
    s = s.trim_end_matches(')').trim().trim_matches('"');
    if s.is_empty() || s == "None" {
        None
    } else {
        Some(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_strips_owned_value_debug_wrapper() {
        assert_eq!(
            scrub_owned_value_debug(r#"OwnedValue(Str("GABBAGOOBLINS - TV INTRO"))"#).as_deref(),
            Some("GABBAGOOBLINS - TV INTRO")
        );
        assert_eq!(
            scrub_owned_value_debug(r#""Plain Title""#).as_deref(),
            Some("Plain Title")
        );
    }
}
