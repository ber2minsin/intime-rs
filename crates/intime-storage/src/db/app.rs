use async_trait::async_trait;
use intime_core::{models::AppDetails, time::Timestamp};
use sqlx::types::chrono::{DateTime, Utc};

use crate::{
    db::{SqliteRepository, models::CompanyRecord},
    error::StorageError,
    repository::app::AppRepository,
};
use blake3::Hash;

#[async_trait]
impl AppRepository for SqliteRepository {
    async fn add_app(
        &self,
        fingerprint: Hash,
        details: &AppDetails,
        company_id: Option<i64>,
    ) -> Result<(), StorageError> {
        let version = details
            .version_info
            .as_ref()
            .and_then(|v| v.file_version.clone());
        let display_name = details.display_name();
        let first_seen = Timestamp::now().as_datetime();

        let fingerprint_bytes = fingerprint.as_bytes().as_slice();
        let _ = sqlx::query!(
            "INSERT INTO app(fingerprint, product_version, display_name, file_path, first_seen_at, product_name, company_id) VALUES($1, $2, $3, $4, $5, $6, $7)",
            fingerprint_bytes,
            version,
            display_name,
            details.file_path,
            first_seen,
            details.product_name,
            company_id
        )
        .execute(&self.pool.clone())
        .await?;
        Ok(())
    }

    async fn seen_app(&self, fingerprint: Hash) -> Result<bool, StorageError> {
        let fingerprint_bytes = fingerprint.as_bytes().as_slice();
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM app WHERE fingerprint = ? LIMIT 1) as "exists!: bool""#,
            fingerprint_bytes
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(exists)
    }

    async fn add_company(&self, company_name: &String) -> Result<i64, StorageError> {
        let company_id = sqlx::query_scalar!(
            "INSERT INTO company(company_name) VALUES ($1) RETURNING id",
            company_name
        )
        .fetch_one(&self.pool.clone())
        .await?;

        Ok(company_id)
    }

    async fn seen_company(&self, company_name: &String) -> Result<i64, StorageError> {
        let company_id = sqlx::query_scalar!(
            "SELECT id FROM company WHERE company_name = ?",
            company_name
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(company_id)
    }

    async fn get_app_company(&self, app_id: i64) -> Result<CompanyRecord, StorageError> {
        let record = sqlx::query_as!(
            CompanyRecord,
            r#"SELECT company.id, company.company_name, company.created_at AS "created_at: DateTime<Utc>"
            FROM app 
            JOIN company ON app.company_id = company.id 
            WHERE app.id = ?"#,
            app_id

        )
        .fetch_one(&self.pool.clone())
        .await?;

        Ok(record)
    }

    async fn get_app_id(&self, fingerprint: Hash) -> Result<i64, StorageError> {
        let fingerprint_bytes = fingerprint.as_bytes().as_slice();
        let record = sqlx::query!(
            "SELECT id FROM app WHERE fingerprint = ?",
            fingerprint_bytes
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(record.id)
    }
}
