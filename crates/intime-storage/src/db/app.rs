use async_trait::async_trait;
use blake3::Hash;
use intime_core::{models::AppDetails, time::Timestamp};

use crate::{
    db::{
        SqliteRepository,
        models::{AumidRecord, CompanyRecord},
    },
    error::StorageError,
    repository::app::AppRepository,
};

#[async_trait]
impl AppRepository for SqliteRepository {
    async fn ensure_app(
        &self,
        fingerprint: Hash,
        details: &AppDetails,
    ) -> Result<i64, StorageError> {
        let company_id = if let Some(name) = details.company() {
            Some(match self.seen_company(&name).await {
                Ok(id) => id,
                Err(_) => self.add_company(&name).await?,
            })
        } else {
            None
        };

        let aumid_id = if let Some(aumid) = details
            .aumid
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(self.ensure_aumid(aumid).await?)
        } else {
            None
        };

        if !self.seen_app(fingerprint).await? {
            self.insert_app(fingerprint, details, company_id, aumid_id)
                .await?;
        } else {
            // Refresh linkage for existing apps when new identity appears.
            let app_id = self.get_app_id(fingerprint).await?;
            if company_id.is_some() || aumid_id.is_some() {
                sqlx::query(
                    "UPDATE app SET company_id = COALESCE(?, company_id), aumid_id = COALESCE(?, aumid_id) WHERE id = ?",
                )
                .bind(company_id)
                .bind(aumid_id)
                .bind(app_id)
                .execute(&self.pool)
                .await?;
            }
        }

        let app_id = self.get_app_id(fingerprint).await?;
        self.upsert_version_and_signature(app_id, details).await?;
        Ok(app_id)
    }

    async fn add_app(
        &self,
        fingerprint: Hash,
        details: &AppDetails,
        company_id: Option<i64>,
    ) -> Result<(), StorageError> {
        let aumid_id = if let Some(aumid) = details
            .aumid
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(self.ensure_aumid(aumid).await?)
        } else {
            None
        };
        self.insert_app(fingerprint, details, company_id, aumid_id)
            .await?;
        let app_id = self.get_app_id(fingerprint).await?;
        self.upsert_version_and_signature(app_id, details).await?;
        Ok(())
    }

    async fn seen_app(&self, fingerprint: Hash) -> Result<bool, StorageError> {
        let fingerprint_bytes = fingerprint.as_bytes().as_slice();
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM app WHERE fingerprint = ? LIMIT 1)",
        )
        .bind(fingerprint_bytes)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists)
    }

    async fn add_company(&self, company_name: &String) -> Result<i64, StorageError> {
        let company_id = sqlx::query_scalar("INSERT INTO company(company_name) VALUES (?) RETURNING id")
            .bind(company_name)
            .fetch_one(&self.pool)
            .await?;
        Ok(company_id)
    }

    async fn seen_company(&self, company_name: &String) -> Result<i64, StorageError> {
        let company_id = sqlx::query_scalar("SELECT id FROM company WHERE company_name = ?")
            .bind(company_name)
            .fetch_one(&self.pool)
            .await?;
        Ok(company_id)
    }

    async fn ensure_aumid(&self, aumid: &str) -> Result<i64, StorageError> {
        if let Some(id) = sqlx::query_scalar::<_, i64>("SELECT id FROM aumid WHERE aumid = ?")
            .bind(aumid)
            .fetch_optional(&self.pool)
            .await?
        {
            return Ok(id);
        }
        let id = sqlx::query_scalar("INSERT INTO aumid(aumid) VALUES (?) RETURNING id")
            .bind(aumid)
            .fetch_one(&self.pool)
            .await?;
        Ok(id)
    }

    async fn get_aumid(&self, aumid_id: i64) -> Result<AumidRecord, StorageError> {
        let record = sqlx::query_as::<_, AumidRecord>(
            "SELECT id, aumid, created_at FROM aumid WHERE id = ?",
        )
        .bind(aumid_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(record)
    }

    async fn get_app_company(&self, app_id: i64) -> Result<CompanyRecord, StorageError> {
        let record = sqlx::query_as::<_, CompanyRecord>(
            "SELECT company.id, company.company_name, company.created_at
            FROM app
            JOIN company ON app.company_id = company.id
            WHERE app.id = ?",
        )
        .bind(app_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(record)
    }

    async fn get_app_id(&self, fingerprint: Hash) -> Result<i64, StorageError> {
        let fingerprint_bytes = fingerprint.as_bytes().as_slice();
        let id = sqlx::query_scalar("SELECT id FROM app WHERE fingerprint = ?")
            .bind(fingerprint_bytes)
            .fetch_one(&self.pool)
            .await?;
        Ok(id)
    }

    async fn find_app_id_by_hint(&self, hint: &str) -> Result<Option<i64>, StorageError> {
        let hint = hint.trim();
        if hint.is_empty() {
            return Ok(None);
        }
        let pattern = format!("%{hint}%");
        let id: Option<i64> = sqlx::query_scalar(
            "SELECT a.id FROM app a
             LEFT JOIN aumid u ON u.id = a.aumid_id
             WHERE lower(coalesce(u.aumid, '')) LIKE lower(?)
                OR lower(coalesce(a.product_name, '')) LIKE lower(?)
                OR lower(a.display_name) LIKE lower(?)
             ORDER BY a.id ASC
             LIMIT 1",
        )
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .fetch_optional(&self.pool)
        .await?;
        Ok(id)
    }
}

impl SqliteRepository {
    async fn insert_app(
        &self,
        fingerprint: Hash,
        details: &AppDetails,
        company_id: Option<i64>,
        aumid_id: Option<i64>,
    ) -> Result<(), StorageError> {
        let version = details
            .version_info
            .as_ref()
            .and_then(|v| v.file_version.clone());
        let display_name = details.display_name();
        let first_seen = Timestamp::now().as_datetime();
        let fingerprint_bytes = fingerprint.as_bytes().as_slice();

        sqlx::query(
            "INSERT INTO app(fingerprint, product_version, display_name, file_path, first_seen_at, product_name, company_id, aumid_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(fingerprint_bytes)
        .bind(version)
        .bind(display_name)
        .bind(&details.file_path)
        .bind(first_seen)
        .bind(&details.product_name)
        .bind(company_id)
        .bind(aumid_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn upsert_version_and_signature(
        &self,
        app_id: i64,
        details: &AppDetails,
    ) -> Result<(), StorageError> {
        if let Some(v) = &details.version_info {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM app_version_info WHERE app_id = ? LIMIT 1)",
            )
            .bind(app_id)
            .fetch_one(&self.pool)
            .await?;
            if !exists {
                sqlx::query(
                    "INSERT INTO app_version_info(
                        app_id, file_description, product_version, file_version,
                        original_filename, internal_name, legal_copyright
                     ) VALUES (?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(app_id)
                .bind(&v.file_description)
                .bind(&v.product_version)
                .bind(&v.file_version)
                .bind(&v.original_filename)
                .bind(&v.internal_name)
                .bind(&v.legal_copyright)
                .execute(&self.pool)
                .await?;
            }
        }

        if let Some(s) = &details.signature_info {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM app_signature WHERE app_id = ? LIMIT 1)",
            )
            .bind(app_id)
            .fetch_one(&self.pool)
            .await?;
            if !exists {
                sqlx::query(
                    "INSERT INTO app_signature(app_id, publisher, subject_full, issuer, serial_number)
                     VALUES (?, ?, ?, ?, ?)",
                )
                .bind(app_id)
                .bind(&s.publisher)
                .bind(&s.subject_full)
                .bind(&s.issuer)
                .bind(&s.serial_number)
                .execute(&self.pool)
                .await?;
            }
        }
        Ok(())
    }
}
