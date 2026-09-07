use async_trait::async_trait;
use blake3::Hash;
use intime_core::models::AppDetails;

use crate::{
    db::models::{AumidRecord, CompanyRecord},
    error::StorageError,
};

#[async_trait]
pub trait AppRepository: Send + Sync {
    /// Ensure company / aumid / app / version / signature rows exist; return app id.
    async fn ensure_app(
        &self,
        fingerprint: Hash,
        details: &AppDetails,
    ) -> Result<i64, StorageError>;

    async fn add_app(
        &self,
        fingerprint: Hash,
        details: &AppDetails,
        company_id: Option<i64>,
    ) -> Result<(), StorageError>;

    async fn seen_app(&self, fingerprint: Hash) -> Result<bool, StorageError>;
    async fn add_company(&self, company_name: &String) -> Result<i64, StorageError>;
    async fn seen_company(&self, company_name: &String) -> Result<i64, StorageError>;
    async fn ensure_aumid(&self, aumid: &str) -> Result<i64, StorageError>;
    async fn get_aumid(&self, aumid_id: i64) -> Result<AumidRecord, StorageError>;
    async fn get_app_company(&self, app_id: i64) -> Result<CompanyRecord, StorageError>;
    async fn get_app_id(&self, fingerprint: Hash) -> Result<i64, StorageError>;

    /// Resolve an app from a loose identity hint (MPRIS player id, aumid fragment, …).
    async fn find_app_id_by_hint(&self, hint: &str) -> Result<Option<i64>, StorageError>;
}
