use sqlx::{FromRow, SqlitePool};

use crate::error::AppError;

#[derive(Debug, Clone, FromRow)]
pub struct LeadDetailContactRecord {
    pub id: String,
    pub display_name: Option<String>,
    pub primary_email: Option<String>,
    pub primary_phone: Option<String>,
    pub country_code: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub latest_submission_at: Option<String>,
    pub submission_count: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct LeadDetailSubmissionRecord {
    pub id: String,
    pub external_lead_id: String,
    pub source_created_at_utc: Option<String>,
    pub source_created_at_raw: String,
    pub ad_id: Option<String>,
    pub ad_name: Option<String>,
    pub adset_id: Option<String>,
    pub adset_name: Option<String>,
    pub campaign_id: Option<String>,
    pub campaign_name: Option<String>,
    pub form_id: Option<String>,
    pub form_name: Option<String>,
    pub is_organic: Option<i64>,
    pub platform: Option<String>,
    pub raw_procedure_answer: Option<String>,
    pub raw_product_answer: Option<String>,
    pub raw_full_name: Option<String>,
    pub raw_email: Option<String>,
    pub raw_phone: Option<String>,
    pub raw_country: Option<String>,
    pub raw_lead_status: Option<String>,
    pub raw_payload_json: String,
    pub product_codes: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct LeadQualityIssueRecord {
    pub id: String,
    pub lead_submission_id: Option<String>,
    pub issue_type: String,
    pub severity: String,
    pub details_json: String,
    pub status: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct LeadActivityRecord {
    pub id: String,
    pub activity_type: String,
    pub occurred_at: String,
    pub payload_json: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct LeadProductOverrideRecord {
    pub product_code: String,
    pub action: String,
    pub created_at: String,
}

#[derive(Clone)]
pub struct LeadDetailRepository {
    pool: SqlitePool,
}

impl LeadDetailRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn contact(
        &self,
        contact_id: &str,
    ) -> Result<Option<LeadDetailContactRecord>, AppError> {
        let contact = sqlx::query_as::<_, LeadDetailContactRecord>(
            r#"
            SELECT id, display_name, primary_email, primary_phone, country_code,
                   status, created_at, updated_at, latest_submission_at, submission_count
            FROM lead_contacts
            WHERE id = ?
            "#,
        )
        .bind(contact_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(contact)
    }

    pub async fn submissions(
        &self,
        contact_id: &str,
    ) -> Result<Vec<LeadDetailSubmissionRecord>, AppError> {
        let rows = sqlx::query_as::<_, LeadDetailSubmissionRecord>(
            r#"
            SELECT
                s.id,
                s.external_lead_id,
                s.source_created_at_utc,
                s.source_created_at_raw,
                s.ad_id,
                s.ad_name,
                s.adset_id,
                s.adset_name,
                s.campaign_id,
                s.campaign_name,
                s.form_id,
                s.form_name,
                s.is_organic,
                s.platform,
                s.raw_procedure_answer,
                s.raw_product_answer,
                s.raw_full_name,
                s.raw_email,
                s.raw_phone,
                s.raw_country,
                s.raw_lead_status,
                s.raw_payload_json,
                COALESCE((
                    SELECT GROUP_CONCAT(DISTINCT spi.product_code)
                    FROM submission_product_interests spi
                    WHERE spi.lead_submission_id = s.id
                ), '') AS product_codes
            FROM lead_submissions s
            WHERE s.lead_contact_id = ?
            ORDER BY s.source_created_at_utc IS NULL ASC,
                     s.source_created_at_utc DESC,
                     s.created_at DESC,
                     s.id DESC
            "#,
        )
        .bind(contact_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn quality_issues(
        &self,
        contact_id: &str,
    ) -> Result<Vec<LeadQualityIssueRecord>, AppError> {
        let rows = sqlx::query_as::<_, LeadQualityIssueRecord>(
            r#"
            SELECT id, lead_submission_id, issue_type, severity, details_json,
                   status, created_at, resolved_at
            FROM lead_data_quality_issues
            WHERE lead_contact_id = ?
            ORDER BY status = 'OPEN' DESC, created_at DESC, id DESC
            "#,
        )
        .bind(contact_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn activities(
        &self,
        contact_id: &str,
    ) -> Result<Vec<LeadActivityRecord>, AppError> {
        let rows = sqlx::query_as::<_, LeadActivityRecord>(
            r#"
            SELECT id, activity_type, occurred_at, payload_json
            FROM lead_activities
            WHERE lead_contact_id = ?
            ORDER BY occurred_at DESC, id DESC
            LIMIT 100
            "#,
        )
        .bind(contact_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn latest_product_overrides(
        &self,
        contact_id: &str,
    ) -> Result<Vec<LeadProductOverrideRecord>, AppError> {
        let rows = sqlx::query_as::<_, LeadProductOverrideRecord>(
            r#"
            SELECT o.product_code, o.action, o.created_at
            FROM contact_product_interest_overrides o
            WHERE o.lead_contact_id = ?
              AND NOT EXISTS (
                  SELECT 1
                  FROM contact_product_interest_overrides newer
                  WHERE newer.lead_contact_id = o.lead_contact_id
                    AND newer.product_code = o.product_code
                    AND (
                        newer.created_at > o.created_at
                        OR (newer.created_at = o.created_at AND newer.id > o.id)
                    )
              )
            ORDER BY o.product_code ASC
            "#,
        )
        .bind(contact_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}
