use std::{str::FromStr as _, sync::Once};

use libsqlite3_sys::sqlite3_auto_extension;
use sqlite_vec::sqlite3_vec_init;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

use crate::error::StorageError;

static SQLITE_EXTENSIONS_INIT: Once = Once::new();

fn register_sqlite_extensions() {
    SQLITE_EXTENSIONS_INIT.call_once(|| unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
    });
}

pub async fn build_pool(database_url: &str) -> Result<SqlitePool, StorageError> {
    register_sqlite_extensions();

    let connect_options = SqliteConnectOptions::from_str(database_url)?
        .foreign_keys(true)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(connect_options)
        .await?;

    Ok(pool)
}
