use intime_platform::{create_capture_engine, create_event_source};

#[test]
fn factories_construct_on_host_platform() {
    let source = create_event_source().expect("event source");
    let capture = create_capture_engine().expect("capture engine");
    // Keep values alive briefly so Drop paths are exercised.
    drop(source);
    drop(capture);
}

#[cfg(target_os = "linux")]
#[test]
fn linux_capture_engine_reports_backend_without_panicking() {
    // Construction itself validates grim/portal selection logic.
    let mut capture = create_capture_engine().expect("linux capture");
    // Unknown handles may fail (grim) or fall back to a full-desktop portal
    // capture. Either path must not panic and must return a coherent result.
    match capture.capture(u64::MAX) {
        Ok(image) => {
            assert!(image.width > 0);
            assert!(image.height > 0);
            assert_eq!(
                image.pixels.len(),
                (image.width * image.height * 3) as usize
            );
        }
        Err(_) => {}
    }
}
