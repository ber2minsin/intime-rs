//! Live verification: resolve Brave frame → DocumentWeb URL.
//!
//! ```bash
//! cargo run -p intime-platform --example probe_brave_url
//! ```

use atspi::{
    AccessibilityConnection, MatchType, ObjectMatchRule, Role, SortOrder,
    proxy::{accessible::AccessibleProxy, proxy_ext::ProxyExt},
};
use std::collections::VecDeque;
use zbus::names::OwnedUniqueName;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connection = AccessibilityConnection::new().await?;
    let conn = connection.connection();
    println!("connected as {:?}", conn.unique_name());

    let dbus = zbus::fdo::DBusProxy::new(conn).await?;
    let names = dbus.list_names().await?;
    println!("bus names: {}", names.len());
    for n in &names {
        let s = n.as_str();
        if s.contains("Registry") || s.contains("a11y") || s.starts_with(':') {
            println!("  {s}");
        }
    }

    // Talk to Brave directly (unique name from bus list / pid).
    let mut brave_name: Option<String> = None;
    for n in &names {
        let s = n.as_str();
        if !s.starts_with(':') {
            continue;
        }
        // Probe root name
        let Ok(acc) = AccessibleProxy::builder(conn)
            .destination(s)?
            .path("/org/a11y/atspi/accessible/root")?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
        else {
            continue;
        };
        let name = acc.name().await.unwrap_or_default();
        if name.to_ascii_lowercase().contains("brave") {
            println!("found Brave at {s} ({name})");
            brave_name = Some(s.to_string());
            break;
        }
    }

    let Some(brave) = brave_name else {
        eprintln!("Brave not on a11y bus");
        std::process::exit(2);
    };

    let app = AccessibleProxy::builder(conn)
        .destination(brave.as_str())?
        .path("/org/a11y/atspi/accessible/root")?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;

    let mut ok = 0usize;
    for frame_ref in app.get_children().await? {
        let Some(frame) = proxy(conn, &frame_ref).await else {
            continue;
        };
        let title = frame.name().await.unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        println!("frame: {title}");
        match find_document_web(conn, &frame).await {
            Some((url, role, doc_name)) => {
                println!("  OK url={url}");
                println!("  OK role={role:?} name={doc_name:?}");
                assert!(url.starts_with("http"));
                ok += 1;
            }
            None => {
                eprintln!("  FAIL no DocumentWeb URL");
                std::process::exit(1);
            }
        }
    }

    if ok == 0 {
        eprintln!("no named frames");
        std::process::exit(2);
    }
    println!("probe_brave_url passed ({ok} frames)");
    Ok(())
}

async fn proxy<'a>(
    conn: &'a zbus::Connection,
    item: &atspi::ObjectRefOwned,
) -> Option<AccessibleProxy<'a>> {
    let sender: OwnedUniqueName = item.name()?.to_owned().into();
    let path = item.path().to_owned();
    AccessibleProxy::builder(conn)
        .destination(sender)
        .ok()?
        .path(path)
        .ok()?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
        .ok()
}

fn pick_url(attrs: &std::collections::HashMap<String, String>) -> Option<String> {
    for key in ["URI", "DocURL", "url", "URL"] {
        if let Some(v) = attrs.get(key) {
            let t = v.trim();
            if t.starts_with("http://") || t.starts_with("https://") {
                return Some(t.to_string());
            }
        }
    }
    None
}

async fn find_document_web(
    conn: &zbus::Connection,
    root: &AccessibleProxy<'_>,
) -> Option<(String, Role, String)> {
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
                    if let Some(acc) = proxy(conn, &obj).await {
                        if let Ok(p) = acc.proxies().await {
                            if let Ok(doc) = p.document().await {
                                if let Ok(attrs) = doc.get_attributes().await {
                                    if let Some(url) = pick_url(&attrs) {
                                        let role = acc.get_role().await.ok()?;
                                        let name = acc.name().await.unwrap_or_default();
                                        return Some((url, role, name));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut queue: VecDeque<(atspi::ObjectRefOwned, u8)> = VecDeque::new();
    if let Ok(children) = root.get_children().await {
        for c in children {
            queue.push_back((c, 0));
        }
    }
    let mut visited = 0usize;
    while let Some((obj, depth)) = queue.pop_front() {
        visited += 1;
        if visited > 400 || depth > 14 {
            continue;
        }
        let Some(acc) = proxy(conn, &obj).await else {
            continue;
        };
        let role = acc.get_role().await.ok();
        if matches!(role, Some(Role::DocumentWeb) | Some(Role::DocumentFrame)) {
            if let Ok(p) = acc.proxies().await {
                if let Ok(doc) = p.document().await {
                    if let Ok(attrs) = doc.get_attributes().await {
                        if let Some(url) = pick_url(&attrs) {
                            let name = acc.name().await.unwrap_or_default();
                            return Some((url, role.unwrap(), name));
                        }
                    }
                }
            }
        }
        if matches!(
            role,
            Some(Role::MenuBar) | Some(Role::ToolBar) | Some(Role::Menu) | Some(Role::PageTabList)
        ) {
            continue;
        }
        if let Ok(children) = acc.get_children().await {
            for child in children.into_iter().take(40) {
                queue.push_back((child, depth + 1));
            }
        }
    }
    None
}
