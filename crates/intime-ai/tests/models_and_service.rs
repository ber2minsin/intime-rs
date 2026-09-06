use intime_ai::{
    embedding::{EmbeddingServer, EmbeddingService},
    models::{
        EmbeddingRequest, EmbeddingResponse, EmbeddingType, blob_to_vec, vec_to_blob,
    },
};

#[test]
fn vec_blob_round_trip() {
    let original = vec![0.0_f32, 1.5, -2.25, 42.0];
    let blob = vec_to_blob(&original);
    assert_eq!(blob.len(), original.len() * 4);
    assert_eq!(blob_to_vec(&blob), original);
}

#[test]
fn blob_to_vec_ignores_trailing_partial_bytes() {
    let mut blob = vec_to_blob(&[1.0, 2.0]);
    blob.push(0xff);
    assert_eq!(blob_to_vec(&blob), vec![1.0, 2.0]);
}

#[test]
fn empty_vector_round_trips() {
    assert!(vec_to_blob(&[]).is_empty());
    assert!(blob_to_vec(&[]).is_empty());
}

#[test]
fn embedding_service_rejects_invalid_url() {
    match EmbeddingService::new("not a url") {
        Ok(_) => panic!("expected invalid URL to fail"),
        Err(err) => assert!(format!("{err:#}").contains("invalid embedding server URL")),
    }
}

#[test]
fn embedding_service_accepts_http_base() {
    let service = EmbeddingService::new("http://127.0.0.1:8000").unwrap();
    assert_eq!(service.base_url.as_str(), "http://127.0.0.1:8000/");
}

#[test]
fn embedding_service_accepts_trailing_slash() {
    let service = EmbeddingService::new("http://localhost:9000/").unwrap();
    assert_eq!(service.base_url.as_str(), "http://localhost:9000/");
}

#[test]
fn embedding_request_serde_tags() {
    let text = serde_json::to_value(EmbeddingRequest::Text {
        text: "hello".into(),
    })
    .unwrap();
    assert_eq!(text["type"], "text");
    assert_eq!(text["text"], "hello");

    let image = serde_json::to_value(EmbeddingRequest::Image {
        image_path: "a.jpg".into(),
    })
    .unwrap();
    assert_eq!(image["type"], "image");
    assert_eq!(image["image_path"], "a.jpg");
}

#[test]
fn embedding_response_round_trips() {
    let resp = EmbeddingResponse {
        embedding_type: EmbeddingType::Text,
        backend: "unit".into(),
        embedding: vec![0.1, 0.2, 0.3],
    };
    let json = serde_json::to_string(&resp).unwrap();
    let decoded: EmbeddingResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.backend, "unit");
    assert_eq!(decoded.embedding, vec![0.1, 0.2, 0.3]);
    assert!(matches!(decoded.embedding_type, EmbeddingType::Text));
}

#[test]
fn embedding_type_serializes_lowercase() {
    assert_eq!(
        serde_json::to_string(&EmbeddingType::Image).unwrap(),
        "\"image\""
    );
    assert_eq!(
        serde_json::to_string(&EmbeddingType::Text).unwrap(),
        "\"text\""
    );
}

#[tokio::test]
async fn embedding_service_errors_when_server_unreachable() {
    let mut service = EmbeddingService::new("http://127.0.0.1:1").unwrap();
    let err = service
        .make_request(EmbeddingRequest::Text {
            text: "ping".into(),
        })
        .await
        .expect_err("unreachable server");
    let msg = format!("{err:#}").to_lowercase();
    assert!(
        msg.contains("connection")
            || msg.contains("connect")
            || msg.contains("refused")
            || msg.contains("error"),
        "unexpected error: {msg}"
    );
}
