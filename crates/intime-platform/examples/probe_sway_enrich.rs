//! Verify AT-SPI enrichment used by the Sway event path.
//!
//! ```bash
//! DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$UID/bus \
//!   cargo run -p intime-platform --example probe_sway_enrich
//! ```

use intime_core::models::EventMetadata;
use intime_platform::enrich_metadata_from_atspi;

fn main() {
    let pid = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            // First brave browser PID
            std::fs::read_dir("/proc").ok().and_then(|entries| {
                for e in entries.flatten() {
                    let pid: u32 = e.file_name().to_string_lossy().parse().ok()?;
                    let exe = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
                    if exe.to_string_lossy().contains("brave") {
                        return Some(pid);
                    }
                }
                None
            })
        })
        .expect("pass a PID or run Brave");

    let title = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default();
    println!("enriching pid={pid} ({})", title.trim());

    let mut meta = EventMetadata {
        window_title: std::env::args().nth(2),
        process_id: Some(pid),
        ..Default::default()
    };
    // If no title arg, leave None — enrichment still finds a named frame.
    enrich_metadata_from_atspi(pid, &mut meta);
    println!("{meta:#?}");
    assert!(
        meta.url.as_deref().is_some_and(|u| u.starts_with("http")),
        "expected http(s) url, got {:?}",
        meta.url
    );
    assert!(
        meta.focused_element.is_some() || meta.focused_control_type.is_some(),
        "expected focused fields, got {:?}",
        meta
    );
    println!("probe_sway_enrich passed");
}
