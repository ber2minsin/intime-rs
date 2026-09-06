use serde::Serialize;

#[derive(Clone, Serialize, Debug)]
pub struct StorageConfig {
    pub database_file: String, // TODO convert to option
                               // folder_path: Path, // TODO in the future
}
