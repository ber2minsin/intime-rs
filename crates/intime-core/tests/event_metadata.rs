use intime_core::{
    models::{AppDetails, Event, EventData, EventMetadata},
    time::Timestamp,
};

#[test]
fn persisted_event_contains_context_and_application_data() {
    let event = Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint: blake3::hash(b"mail"),
            details: AppDetails {
                title: "Compose".into(),
                file_path: "/usr/bin/mail".into(),
                aumid: None,
                company_name: Some("Example".into()),
                product_name: Some("Mail".into()),
                version_info: None,
                signature_info: None,
            },
            window_handle: 9,
        },
        metadata: EventMetadata {
            window_title: Some("Writing to Alice".into()),
            focused_element: Some("Message body".into()),
            focused_control_type: Some("Edit".into()),
            text_changed: true,
            ..Default::default()
        },
    };

    let json = serde_json::to_value(event).expect("event should serialize");
    assert_eq!(json["metadata"]["focused_element"], "Message body");
    assert_eq!(json["metadata"]["text_changed"], true);
    assert_eq!(json["data"]["data"]["details"]["product_name"], "Mail");
}
