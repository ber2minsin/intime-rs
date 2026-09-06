#![cfg(target_os = "linux")]

use intime_platform::{create_capture_engine, create_event_source};

#[test]
#[ignore = "requires a running Sway session and grim"]
fn sway_capture_and_focus_event_round_trip() {
    let mut sway = swayipc::Connection::new().expect("Sway IPC connection");
    let tree = sway.get_tree().expect("Sway tree");
    let focused = find_focused(&tree).expect("focused application window");
    let handle = focused.id as u64;

    let mut capture = create_capture_engine().expect("capture engine");
    let image = capture.capture(handle).expect("focused window capture");
    assert!(image.width > 0);
    assert!(image.height > 0);
    assert_eq!(
        image.pixels.len(),
        (image.width * image.height * 3) as usize
    );

    let mut source = create_event_source().expect("event source");
    let event = std::thread::spawn(move || source.poll());
    let _ = sway
        .run_command(format!("[con_id={handle}] focus"))
        .expect("refocus window");

    let received = event.join().expect("event thread").expect("focus event");
    assert!(received.data.window_handle().is_some());
}

fn find_focused(node: &swayipc::Node) -> Option<&swayipc::Node> {
    if node.focused && node.pid.is_some() {
        return Some(node);
    }
    node.nodes
        .iter()
        .chain(node.floating_nodes.iter())
        .find_map(find_focused)
}
