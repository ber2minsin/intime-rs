use intime_core::{
    models::{
        AppDetails, Event, EventData, EventMetadata, SignatureInfo, VersionInfo,
    },
    time::Timestamp,
};

#[test]
fn fingerprint_prefers_aumid_then_company_then_name() {
    let aumid = AppDetails {
        title: "x".into(),
        file_path: "/a".into(),
        aumid: Some("App.Id".into()),
        company_name: Some("Ignored".into()),
        product_name: Some("Ignored".into()),
        version_info: None,
        signature_info: None,
    };
    let company = AppDetails {
        title: "x".into(),
        file_path: "/a/app".into(),
        aumid: None,
        company_name: Some("Acme".into()),
        product_name: Some("Editor".into()),
        version_info: None,
        signature_info: None,
    };
    let signed = AppDetails {
        title: "x".into(),
        file_path: "/a/app".into(),
        aumid: None,
        company_name: None,
        product_name: Some("Editor".into()),
        version_info: None,
        signature_info: Some(SignatureInfo {
            publisher: Some("Acme".into()),
            ..Default::default()
        }),
    };
    let named = AppDetails {
        title: "x".into(),
        file_path: "/opt/bin/editor".into(),
        aumid: None,
        company_name: None,
        product_name: None,
        version_info: None,
        signature_info: None,
    };

    assert_ne!(aumid.fingerprint(), company.fingerprint());
    assert_eq!(company.fingerprint(), signed.fingerprint());
    assert_eq!(named.display_name(), "editor");
}

#[test]
fn fingerprint_is_stable_across_calls() {
    let details = AppDetails {
        title: "t".into(),
        file_path: "/bin/x".into(),
        aumid: Some("Stable.Id".into()),
        company_name: None,
        product_name: None,
        version_info: None,
        signature_info: None,
    };
    assert_eq!(details.fingerprint(), details.fingerprint());
}

#[test]
fn whitespace_and_empty_identity_fields_are_normalized() {
    let blank_aumid = AppDetails {
        title: "t".into(),
        file_path: "/bin/tool".into(),
        aumid: Some("   ".into()),
        company_name: Some("  Acme  ".into()),
        product_name: Some(" Tool ".into()),
        version_info: None,
        signature_info: None,
    };
    let trimmed = AppDetails {
        title: "t".into(),
        file_path: "/bin/tool".into(),
        aumid: None,
        company_name: Some("Acme".into()),
        product_name: Some("Tool".into()),
        version_info: None,
        signature_info: None,
    };
    assert_eq!(blank_aumid.fingerprint(), trimmed.fingerprint());
    assert_eq!(blank_aumid.display_name(), "Tool");
    assert_eq!(blank_aumid.company().as_deref(), Some("Acme"));
}

#[test]
fn display_name_falls_back_through_version_info_then_path() {
    let from_version = AppDetails {
        title: "t".into(),
        file_path: "/ignored/path.bin".into(),
        aumid: None,
        company_name: None,
        product_name: None,
        version_info: Some(VersionInfo {
            original_filename: Some("RealName.exe".into()),
            ..Default::default()
        }),
        signature_info: None,
    };
    assert_eq!(from_version.display_name(), "RealName.exe");

    let from_path = AppDetails {
        title: "t".into(),
        file_path: "/usr/local/bin/myapp".into(),
        aumid: None,
        company_name: None,
        product_name: None,
        version_info: None,
        signature_info: None,
    };
    assert_eq!(from_path.display_name(), "myapp");
}

#[test]
fn event_helpers_cover_all_variants() {
    let fp = blake3::hash(b"x");
    let cases = [
        (
            EventData::WindowFocus {
                fingerprint: fp,
                window_handle: 1,
            },
            "window_focus",
            Some(1),
            Some(fp),
        ),
        (
            EventData::TitleChange {
                fingerprint: fp,
                new_title: "t".into(),
                window_handle: 2,
            },
            "title_change",
            Some(2),
            Some(fp),
        ),
        (
            EventData::TextChanged {
                fingerprint: fp,
                window_handle: 3,
            },
            "text_changed",
            Some(3),
            Some(fp),
        ),
        (
            EventData::UiAction {
                kind: intime_core::models::UiActionKind::Save,
                fingerprint: fp,
                window_handle: 8,
                label: Some("Save".into()),
            },
            "ui_action",
            Some(8),
            Some(fp),
        ),
        (EventData::IdleStart, "idle_start", None, None),
        (EventData::IdleEnd, "idle_end", None, None),
        (EventData::Gap, "gap", None, None),
        (
            EventData::AppSeen {
                fingerprint: fp,
                details: AppDetails {
                    title: "a".into(),
                    file_path: "/a".into(),
                    aumid: None,
                    company_name: None,
                    product_name: Some("a".into()),
                    version_info: None,
                    signature_info: None,
                },
                window_handle: 4,
            },
            "app_seen",
            Some(4),
            Some(fp),
        ),
        (
            EventData::Background { fingerprint: fp },
            "background",
            None,
            Some(fp),
        ),
    ];

    for (data, name, handle, fingerprint) in cases {
        assert_eq!(data.name(), name);
        assert_eq!(data.window_handle(), handle);
        assert_eq!(data.fingerprint(), fingerprint);
    }
}

#[test]
fn every_event_variant_round_trips_through_json() {
    let fp = blake3::hash(b"roundtrip");
    let details = AppDetails {
        title: "Window".into(),
        file_path: "/bin/app".into(),
        aumid: Some("A.B".into()),
        company_name: Some("Co".into()),
        product_name: Some("App".into()),
        version_info: Some(VersionInfo {
            file_version: Some("1".into()),
            ..Default::default()
        }),
        signature_info: Some(SignatureInfo {
            publisher: Some("Co".into()),
            ..Default::default()
        }),
    };
    let variants = [
        EventData::WindowFocus {
            fingerprint: fp,
            window_handle: 1,
        },
        EventData::TitleChange {
            fingerprint: fp,
            new_title: "New".into(),
            window_handle: 2,
        },
        EventData::TextChanged {
            fingerprint: fp,
            window_handle: 3,
        },
        EventData::UiAction {
            kind: intime_core::models::UiActionKind::FormSubmit,
            fingerprint: fp,
            window_handle: 5,
            label: Some("Submit".into()),
        },
        EventData::IdleStart,
        EventData::IdleEnd,
        EventData::Gap,
        EventData::AppSeen {
            fingerprint: fp,
            details: details.clone(),
            window_handle: 4,
        },
        EventData::Background { fingerprint: fp },
    ];

    for data in variants {
        let event = Event {
            timestamp: Timestamp::now(),
            data,
            metadata: EventMetadata {
                window_title: Some("t".into()),
                process_id: Some(7),
                executable_path: Some("/bin/app".into()),
                focused_element: Some("e".into()),
                focused_element_class: Some("c".into()),
                focused_control_type: Some("Edit".into()),
                automation_id: Some("id".into()),
                text_changed: true,
                ..Default::default()
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.data.name(), event.data.name());
        assert_eq!(decoded.metadata, event.metadata);
        assert_eq!(decoded.data.window_handle(), event.data.window_handle());
    }
}

#[test]
fn timestamp_filename_includes_millis() {
    let name = Timestamp::now().to_filename_readable();
    assert!(name.contains('_'));
    assert!(name.ends_with('Z'));
    assert!(name.len() >= 24);
}

#[test]
fn timestamp_ordering_and_datetime_access() {
    let a = Timestamp::now();
    std::thread::sleep(std::time::Duration::from_millis(2));
    let b = Timestamp::now();
    assert!(a <= b);
    assert!(a.as_datetime() <= b.as_datetime());
}

#[test]
fn metadata_defaults_are_empty() {
    let event = Event {
        timestamp: Timestamp::now(),
        data: EventData::Gap,
        metadata: EventMetadata::default(),
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["metadata"]["text_changed"], false);
    assert!(json["metadata"]["window_title"].is_null());
}

#[test]
fn event_json_uses_tagged_event_data() {
    let event = Event {
        timestamp: Timestamp::now(),
        data: EventData::IdleStart,
        metadata: Default::default(),
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["data"]["type"], "IdleStart");
}
