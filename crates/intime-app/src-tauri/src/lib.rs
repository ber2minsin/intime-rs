use std::env;
use tokio::sync::Mutex;

use intime_ai::{
    embedding::{EmbeddingServer, EmbeddingService},
    models::EmbeddingRequest,
};
use intime_storage::{config::StorageConfig, storage::Storage};
use tauri::Manager as _;

use crate::error::AppError;
mod error;

struct AppState {
    embedding_server: Mutex<EmbeddingService>,
    storage: Storage,
}

#[tauri::command]
async fn search_embedding(
    query: &str,
    state: tauri::State<'_, AppState>,
) -> Result<String, AppError> {
    let embedding_request = EmbeddingRequest::Text {
        text: query.to_string(),
    };
    let mut embedding_server = state.embedding_server.lock().await;

    let embedding_response = embedding_server.make_request(embedding_request).await?;
    let found = state
        .storage
        .embedding_repository
        .search_embedding(embedding_response.embedding)
        .await
        .map_err(anyhow::Error::from)?;
    // let related_event_ids = found.iter().map(|embedding| storage.event_repository.get_event(embedding.event_id))

    Ok(format!("{:#?}", found))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            dotenv::dotenv().expect("Failed to load dotenv");
            let embedding_server = Mutex::new(
                EmbeddingService::new("http://localhost:8000")
                    .expect("invalid embedding server URL"),
            );
            let storage_config = StorageConfig {
                database_file: env::var("DATABASE_FILE").expect("DATABASE_FILE must be set"),
            };

            println!("{:?}", storage_config);
            let storage = tauri::async_runtime::block_on(Storage::connect(&storage_config))
                .expect("Can not connect to storage");

            app.manage(AppState {
                embedding_server,
                storage,
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![search_embedding])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
