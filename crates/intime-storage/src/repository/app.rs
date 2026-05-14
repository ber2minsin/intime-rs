use async_trait::async_trait;
use blake3::Hash;
use intime_core::models::AppDetails;

use crate::{db::models::CompanyRecord, error::StorageError};

#[async_trait]
pub trait AppRepository: Send + Sync {
    async fn add_app(
        &self,
        fingerprint: Hash,
        details: &AppDetails,
        company_id: Option<i64>,
    ) -> Result<(), StorageError>;

    async fn seen_app(&self, fingerprint: Hash) -> Result<bool, StorageError>;
    async fn add_company(&self, company_name: &String) -> Result<i64, StorageError>;
    async fn seen_company(&self, company_name: &String) -> Result<i64, StorageError>;
    async fn get_app_company(&self, app_id: i64) -> Result<CompanyRecord, StorageError>;
    async fn get_app_id(&self, fingerprint: Hash) -> Result<i64, StorageError>;
}
