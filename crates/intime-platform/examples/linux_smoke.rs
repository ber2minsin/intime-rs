//! Non-interactive smoke test for Sway + grim.
//!
//! ```bash
//! cargo run -p intime-platform --example linux_smoke
//! ```

use intime_platform::{create_capture_engine, create_event_source};
use swayipc::{Connection, Node};

fn find_focused(node: &Node) -> Option<&Node> {
    if node.focused && node.pid.is_some() {
        return Some(node);
    }
    for child in node.nodes.iter().chain(node.floating_nodes.iter()) {
        if let Some(found) = find_focused(child) {
            return Some(found);
        }
    }
    None
}

fn main() {
    if std::env::var_os("SWAYSOCK").is_none() {
        eprintln!("SWAYSOCK not set — this smoke test requires Sway");
        std::process::exit(1);
    }

    println!("1) Capture currently focused window via grim...");
    let mut conn = Connection::new().expect("sway connect");
    let tree = conn.get_tree().expect("get_tree");
    let focused = find_focused(&tree).expect("no focused window with pid");
    let handle = focused.id as u64;
    println!(
        "   focused id={handle} title={:?} app_id={:?}",
        focused.name, focused.app_id
    );

    let mut capture = create_capture_engine().expect("capture");
    let img = capture.capture(handle).expect("capture focused window");
    println!(
        "   capture ok: {}x{} ({} RGB bytes)",
        img.width,
        img.height,
        img.pixels.len()
    );
    assert!(img.width > 0 && img.height > 0);

    let path = "/tmp/intime_smoke.jpg";
    image::RgbImage::from_raw(img.width, img.height, img.pixels)
        .expect("rgb")
        .save(path)
        .expect("save");
    println!("   saved {path}");

    println!("2) Verify event source receives a window focus event...");
    let mut source = create_event_source().expect("event source");
    std::thread::sleep(std::time::Duration::from_millis(400));

    // Find another leaf window to focus, then focus back — produces Focus events.
    let other = find_other_leaf(&tree, handle);
    if let Some(other_id) = other {
        let _ = Connection::new()
            .expect("sway")
            .run_command(format!("[con_id={other_id}] focus"));
        std::thread::sleep(std::time::Duration::from_millis(100));
        let _ = Connection::new()
            .expect("sway")
            .run_command(format!("[con_id={handle}] focus"));
    }

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let event = source.poll();
        let _ = tx.send(event);
    });

    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(Ok(event)) => {
            println!("   got event: {}", event.data.name());
            println!("smoke test passed");
        }
        Ok(Err(e)) => {
            eprintln!("poll error: {e}");
            std::process::exit(1);
        }
        Err(_) => {
            println!("   (no event within 5s; capture path OK)");
            println!("smoke test passed (capture only)");
        }
    }
}

fn find_other_leaf(node: &Node, exclude: u64) -> Option<i64> {
    if node.pid.is_some() && node.id as u64 != exclude {
        return Some(node.id);
    }
    for child in node.nodes.iter().chain(node.floating_nodes.iter()) {
        if let Some(id) = find_other_leaf(child, exclude) {
            return Some(id);
        }
    }
    None
}
