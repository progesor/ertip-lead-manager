use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use thiserror::Error;
use uuid::Uuid;

use crate::authz::{Action, Actor, AuthorizationError, LeadScope, Role};

const ROLES: [&str; 3] = ["ADMIN", "MANAGER", "SALES"];
const LEAD_STATUSES: [&str; 8] = [
    "NEW",
    "CONTACTED",
    "REPLIED",
    "QUALIFIED",
    "QUOTE_SENT",
    "WON",
    "LOST",
    "INVALID",
];
const DEFAULT_PAGE_SIZE: u32 = 50;
const MAX_PAGE_SIZE: u32 = 100;

#[derive(Debug, Error)]
pub enum CrmError {
    #[error(transparent)]
    Authorization(#[from] AuthorizationError),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("resource not found: {0}")]
    NotFound(String),
    #[error("stale revision for {resource}; current revision is {current_revision}")]
    Conflict {
        resource: String,
        current_revision: i64,
    },
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StaffMember {
    pub id: String,
    pub display_name: String,
    pub email: Option<String>,
    pub role: String,
    pub is_active: bool,
    pub revision: i64,
    pub auth_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStaffRequest {
    pub display_name: String,
    pub email: Option<String>,
    pub role: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStaffRequest {
    pub display_name: String,
    pub email: Option<String>,
    pub role: String,
    pub expected_revision: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetStaffActiveRequest {
    pub is_active: bool,
    pub expected_revision: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LeadAssignee {
    pub id: String,
    pub display_name: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentRequest {
    pub assigned_user_id: Option<String>,
    pub expected_revision: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentResult {
    pub changed: bool,
    pub revision: i64,
    pub assignee: Option<LeadAssignee>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LeadListRequest {
    pub search: Option<String>,
    pub status: Option<String>,
    pub country_code: Option<String>,
    pub product_code: Option<String>,
    pub assigned_user_id: Option<String>,
    pub unassigned_only: Option<bool>,
    pub repeat_only: Option<bool>,
    pub warning_only: Option<bool>,
    pub sort: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LeadWarningSummary {
    pub issue_type: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LeadListItem {
    pub id: String,
    pub display_name: String,
    pub primary_email: Option<String>,
    pub primary_phone: Option<String>,
    pub country_code: Option<String>,
    pub status: String,
    pub revision: i64,
    pub assigned_user_id: Option<String>,
    pub assigned_user_name: Option<String>,
    pub assigned_user_active: Option<bool>,
    pub latest_submission_at: Option<DateTime<Utc>>,
    pub submission_count: i64,
    pub is_repeat: bool,
    pub product_interests: Vec<String>,
    pub platforms: Vec<String>,
    pub warning_count: i64,
    pub warning_summaries: Vec<LeadWarningSummary>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LeadListResponse {
    pub items: Vec<LeadListItem>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeadDetailProductOverride {
    pub product_code: String,
    pub action: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeadDetailContact {
    pub id: String,
    pub display_name: String,
    pub primary_email: Option<String>,
    pub primary_phone: Option<String>,
    pub country_code: Option<String>,
    pub status: String,
    pub revision: i64,
    pub assignee: Option<LeadAssignee>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub latest_submission_at: Option<DateTime<Utc>>,
    pub submission_count: i64,
    pub automatic_product_interests: Vec<String>,
    pub product_interests: Vec<String>,
    pub product_overrides: Vec<LeadDetailProductOverride>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeadDetailSubmission {
    pub id: String,
    pub external_lead_id: String,
    pub source_created_at_utc: Option<DateTime<Utc>>,
    pub source_created_at_raw: String,
    pub ad_id: Option<String>,
    pub ad_name: Option<String>,
    pub adset_id: Option<String>,
    pub adset_name: Option<String>,
    pub campaign_id: Option<String>,
    pub campaign_name: Option<String>,
    pub form_id: Option<String>,
    pub form_name: Option<String>,
    pub is_organic: Option<bool>,
    pub platform: Option<String>,
    pub raw_procedure_answer: Option<String>,
    pub raw_product_answer: Option<String>,
    pub raw_full_name: Option<String>,
    pub raw_email: Option<String>,
    pub raw_phone: Option<String>,
    pub raw_country: Option<String>,
    pub raw_lead_status: Option<String>,
    pub raw_payload_json: String,
    pub product_interests: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeadDetailQualityIssue {
    pub id: String,
    pub lead_submission_id: Option<String>,
    pub issue_type: String,
    pub severity: String,
    pub details_json: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeadDetailNote {
    pub id: String,
    pub body: String,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeadDetailActivity {
    pub id: String,
    pub activity_type: String,
    pub occurred_at: DateTime<Utc>,
    pub payload_json: String,
    pub actor_user_id: Option<String>,
    pub actor_display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeadDetailResponse {
    pub contact: LeadDetailContact,
    pub submissions: Vec<LeadDetailSubmission>,
    pub quality_issues: Vec<LeadDetailQualityIssue>,
    pub notes: Vec<LeadDetailNote>,
    pub activities: Vec<LeadDetailActivity>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeLeadStatusRequest {
    pub status: String,
    pub expected_revision: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LeadStatusResult {
    pub changed: bool,
    pub status: String,
    pub revision: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct CrmService {
    pool: PgPool,
}

impl CrmService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_staff(
        &self,
        actor: &Actor,
        include_inactive: bool,
    ) -> Result<Vec<StaffMember>, CrmError> {
        actor.require(Action::PersonnelRead)?;
        let rows = if include_inactive {
            sqlx::query(
                r#"
                SELECT
                    u.id, u.display_name, u.email, u.role, u.is_active, u.revision,
                    EXISTS(SELECT 1 FROM app_credentials c WHERE c.user_id = u.id) AS auth_enabled,
                    u.created_at, u.updated_at
                FROM app_users u
                ORDER BY u.is_active DESC, lower(u.display_name) ASC, u.id ASC
                "#,
            )
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT
                    u.id, u.display_name, u.email, u.role, u.is_active, u.revision,
                    EXISTS(SELECT 1 FROM app_credentials c WHERE c.user_id = u.id) AS auth_enabled,
                    u.created_at, u.updated_at
                FROM app_users u
                WHERE u.is_active = TRUE
                ORDER BY lower(u.display_name) ASC, u.id ASC
                "#,
            )
            .fetch_all(&self.pool)
            .await?
        };
        rows.into_iter().map(map_staff).collect()
    }

    pub async fn create_staff(
        &self,
        actor: &Actor,
        request: CreateStaffRequest,
    ) -> Result<StaffMember, CrmError> {
        actor.require(Action::PersonnelManage)?;
        let (display_name, email, role) = validate_staff_input(
            request.display_name,
            request.email,
            request.role,
        )?;
        let user_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let result = sqlx::query(
            r#"
            INSERT INTO app_users (
                id, display_name, email, role, is_active, revision, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, TRUE, 0, $5, $5)
            "#,
        )
        .bind(&user_id)
        .bind(&display_name)
        .bind(email.as_deref())
        .bind(&role)
        .bind(now)
        .execute(&self.pool)
        .await;
        map_staff_write_error(result)?;
        self.get_staff(&user_id)
            .await?
            .ok_or_else(|| CrmError::NotFound("staff member".to_string()))
    }

    pub async fn update_staff(
        &self,
        actor: &Actor,
        user_id: &str,
        request: UpdateStaffRequest,
    ) -> Result<StaffMember, CrmError> {
        actor.require(Action::PersonnelManage)?;
        let user_id = required_id(user_id, "userId")?;
        validate_expected_revision(request.expected_revision)?;
        let (display_name, email, role) = validate_staff_input(
            request.display_name,
            request.email,
            request.role,
        )?;
        if actor.user_id == user_id && role != "ADMIN" {
            return Err(CrmError::Validation(
                "current ADMIN cannot demote own account".to_string(),
            ));
        }

        let result = sqlx::query(
            r#"
            UPDATE app_users
            SET display_name = $1, email = $2, role = $3,
                revision = revision + 1, updated_at = now()
            WHERE id = $4 AND revision = $5
            "#,
        )
        .bind(&display_name)
        .bind(email.as_deref())
        .bind(&role)
        .bind(user_id)
        .bind(request.expected_revision)
        .execute(&self.pool)
        .await;
        let result = map_staff_write_error(result)?;
        if result.rows_affected() == 0 {
            return Err(self.revision_or_not_found("staff member", user_id).await?);
        }
        self.get_staff(user_id)
            .await?
            .ok_or_else(|| CrmError::NotFound("staff member".to_string()))
    }

    pub async fn set_staff_active(
        &self,
        actor: &Actor,
        user_id: &str,
        request: SetStaffActiveRequest,
    ) -> Result<StaffMember, CrmError> {
        actor.require(Action::PersonnelManage)?;
        let user_id = required_id(user_id, "userId")?;
        validate_expected_revision(request.expected_revision)?;
        if actor.user_id == user_id && !request.is_active {
            return Err(CrmError::Validation(
                "current ADMIN cannot deactivate own account".to_string(),
            ));
        }

        let result = sqlx::query(
            r#"
            UPDATE app_users
            SET is_active = $1, revision = revision + 1, updated_at = now()
            WHERE id = $2 AND revision = $3
            "#,
        )
        .bind(request.is_active)
        .bind(user_id)
        .bind(request.expected_revision)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(self.revision_or_not_found("staff member", user_id).await?);
        }
        self.get_staff(user_id)
            .await?
            .ok_or_else(|| CrmError::NotFound("staff member".to_string()))
    }

    pub async fn assign_lead(
        &self,
        actor: &Actor,
        contact_id: &str,
        request: AssignmentRequest,
    ) -> Result<AssignmentResult, CrmError> {
        actor.require(Action::LeadAssign)?;
        let contact_id = required_id(contact_id, "contactId")?;
        validate_expected_revision(request.expected_revision)?;
        let requested_user_id = clean_optional(request.assigned_user_id);
        let mut tx = self.pool.begin().await?;

        let row = sqlx::query(
            "SELECT assigned_user_id, revision FROM lead_contacts WHERE id = $1 FOR UPDATE",
        )
        .bind(contact_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| CrmError::NotFound("lead contact".to_string()))?;
        let old_user_id: Option<String> = row.try_get("assigned_user_id")?;
        let current_revision: i64 = row.try_get("revision")?;
        if current_revision != request.expected_revision {
            return Err(CrmError::Conflict {
                resource: "lead contact".to_string(),
                current_revision,
            });
        }

        let old_assignee = match old_user_id.as_deref() {
            Some(user_id) => self.assignee_in_tx(&mut tx, user_id).await?,
            None => None,
        };
        let new_assignee = match requested_user_id.as_deref() {
            Some(user_id) => {
                let assignee = self
                    .assignee_in_tx(&mut tx, user_id)
                    .await?
                    .ok_or_else(|| CrmError::NotFound("staff member".to_string()))?;
                if !assignee.is_active {
                    return Err(CrmError::Validation(
                        "inactive staff member cannot receive new assignments".to_string(),
                    ));
                }
                Some(assignee)
            }
            None => None,
        };

        if old_user_id.as_deref() == new_assignee.as_ref().map(|item| item.id.as_str()) {
            tx.commit().await?;
            return Ok(AssignmentResult {
                changed: false,
                revision: current_revision,
                assignee: new_assignee,
            });
        }

        let next_revision = current_revision + 1;
        sqlx::query(
            r#"
            UPDATE lead_contacts
            SET assigned_user_id = $1, revision = $2, updated_at = now()
            WHERE id = $3
            "#,
        )
        .bind(new_assignee.as_ref().map(|item| item.id.as_str()))
        .bind(next_revision)
        .bind(contact_id)
        .execute(&mut *tx)
        .await?;

        let payload = json!({
            "fromUserId": old_assignee.as_ref().map(|item| item.id.as_str()),
            "fromDisplayName": old_assignee.as_ref().map(|item| item.display_name.as_str()),
            "toUserId": new_assignee.as_ref().map(|item| item.id.as_str()),
            "toDisplayName": new_assignee.as_ref().map(|item| item.display_name.as_str()),
        })
        .to_string();
        insert_activity(
            &mut tx,
            contact_id,
            &actor.user_id,
            "ASSIGNEE_CHANGED",
            payload,
        )
        .await?;
        tx.commit().await?;

        Ok(AssignmentResult {
            changed: true,
            revision: next_revision,
            assignee: new_assignee,
        })
    }

    pub async fn list_leads(
        &self,
        actor: &Actor,
        request: LeadListRequest,
    ) -> Result<LeadListResponse, CrmError> {
        actor.require(Action::LeadRead)?;
        validate_sales_list_request(actor, &request)?;

        let page = request.page.unwrap_or(0);
        let page_size = request
            .page_size
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE);
        let filters = LeadFilters::from_request(request)?;

        let mut count_builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*)::BIGINT FROM lead_contacts c");
        append_lead_filters(&mut count_builder, actor, &filters);
        let total = count_builder
            .build_query_scalar::<i64>()
            .fetch_one(&self.pool)
            .await?;

        let mut builder = QueryBuilder::<Postgres>::new(
            r#"
            SELECT
                c.id,
                c.display_name,
                c.primary_email,
                c.primary_phone,
                c.country_code,
                c.status,
                c.revision,
                c.assigned_user_id,
                assigned.display_name AS assigned_user_name,
                assigned.is_active AS assigned_user_active,
                c.latest_submission_at,
                c.submission_count::BIGINT AS submission_count,
                ARRAY(
                    SELECT DISTINCT spi.product_code
                    FROM lead_submissions s
                    JOIN submission_product_interests spi ON spi.lead_submission_id = s.id
                    WHERE s.lead_contact_id = c.id
                    ORDER BY spi.product_code
                ) AS automatic_product_codes,
                ARRAY(
                    SELECT latest.product_code || '=' || latest.action
                    FROM (
                        SELECT DISTINCT ON (o.product_code) o.product_code, o.action
                        FROM contact_product_interest_overrides o
                        WHERE o.lead_contact_id = c.id
                        ORDER BY o.product_code, o.created_at DESC, o.id DESC
                    ) latest
                    ORDER BY latest.product_code
                ) AS product_overrides,
                ARRAY(
                    SELECT DISTINCT lower(trim(s.platform))
                    FROM lead_submissions s
                    WHERE s.lead_contact_id = c.id
                      AND trim(COALESCE(s.platform, '')) <> ''
                    ORDER BY lower(trim(s.platform))
                ) AS platforms,
                (
                    SELECT COUNT(*)::BIGINT
                    FROM lead_data_quality_issues q
                    WHERE q.lead_contact_id = c.id AND q.status = 'OPEN'
                ) AS warning_count,
                ARRAY(
                    SELECT q.issue_type
                    FROM lead_data_quality_issues q
                    WHERE q.lead_contact_id = c.id AND q.status = 'OPEN'
                    ORDER BY q.issue_type, q.id
                ) AS warning_types
            FROM lead_contacts c
            LEFT JOIN app_users assigned ON assigned.id = c.assigned_user_id
            "#,
        );
        append_lead_filters(&mut builder, actor, &filters);
        append_lead_sort(&mut builder, filters.sort);
        builder.push(" LIMIT ").push_bind(page_size as i64);
        builder
            .push(" OFFSET ")
            .push_bind(page as i64 * page_size as i64);

        let rows = builder.build().fetch_all(&self.pool).await?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let automatic: Vec<String> = row.try_get("automatic_product_codes")?;
            let overrides: Vec<String> = row.try_get("product_overrides")?;
            let warning_types: Vec<String> = row.try_get("warning_types")?;
            let submission_count: i64 = row.try_get("submission_count")?;
            items.push(LeadListItem {
                id: row.try_get("id")?,
                display_name: display_name(row.try_get("display_name")?),
                primary_email: row.try_get("primary_email")?,
                primary_phone: row.try_get("primary_phone")?,
                country_code: row.try_get("country_code")?,
                status: row.try_get("status")?,
                revision: row.try_get("revision")?,
                assigned_user_id: row.try_get("assigned_user_id")?,
                assigned_user_name: row.try_get("assigned_user_name")?,
                assigned_user_active: row.try_get("assigned_user_active")?,
                latest_submission_at: row.try_get("latest_submission_at")?,
                submission_count,
                is_repeat: submission_count > 1,
                product_interests: effective_products_from_encoded(automatic, overrides),
                platforms: row.try_get("platforms")?,
                warning_count: row.try_get("warning_count")?,
                warning_summaries: summarize_warnings(warning_types),
            });
        }

        let total_pages = if total <= 0 {
            0
        } else {
            ((total as u64 + page_size as u64 - 1) / page_size as u64) as u32
        };
        Ok(LeadListResponse {
            items,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    pub async fn get_lead(
        &self,
        actor: &Actor,
        contact_id: &str,
    ) -> Result<Option<LeadDetailResponse>, CrmError> {
        actor.require(Action::LeadRead)?;
        let contact_id = required_id(contact_id, "contactId")?;

        let mut contact_builder = QueryBuilder::<Postgres>::new(
            r#"
            SELECT
                c.id, c.display_name, c.primary_email, c.primary_phone, c.country_code,
                c.status, c.revision, c.created_at, c.updated_at, c.latest_submission_at,
                c.submission_count::BIGINT AS submission_count,
                assigned.id AS assigned_id,
                assigned.display_name AS assigned_display_name,
                assigned.is_active AS assigned_is_active
            FROM lead_contacts c
            LEFT JOIN app_users assigned ON assigned.id = c.assigned_user_id
            WHERE c.id =
            "#,
        );
        contact_builder.push_bind(contact_id);
        if let LeadScope::AssignedTo(user_id) = actor.lead_scope() {
            contact_builder
                .push(" AND c.assigned_user_id = ")
                .push_bind(user_id.to_string());
        }
        let Some(contact_row) = contact_builder.build().fetch_optional(&self.pool).await? else {
            return Ok(None);
        };

        let assignee_id: Option<String> = contact_row.try_get("assigned_id")?;
        let assignee = match assignee_id {
            Some(id) => Some(LeadAssignee {
                id,
                display_name: contact_row.try_get("assigned_display_name")?,
                is_active: contact_row.try_get("assigned_is_active")?,
            }),
            None => None,
        };

        let submission_rows = sqlx::query(
            r#"
            SELECT
                s.id, s.external_lead_id, s.source_created_at_utc, s.source_created_at_raw,
                s.ad_id, s.ad_name, s.adset_id, s.adset_name,
                s.campaign_id, s.campaign_name, s.form_id, s.form_name,
                s.is_organic, s.platform, s.raw_procedure_answer, s.raw_product_answer,
                s.raw_full_name, s.raw_email, s.raw_phone, s.raw_country,
                s.raw_lead_status, s.raw_payload_json,
                ARRAY(
                    SELECT spi.product_code
                    FROM submission_product_interests spi
                    WHERE spi.lead_submission_id = s.id
                    ORDER BY spi.product_code
                ) AS product_codes
            FROM lead_submissions s
            WHERE s.lead_contact_id = $1
            ORDER BY s.source_created_at_utc DESC NULLS LAST, s.created_at DESC, s.id DESC
            "#,
        )
        .bind(contact_id)
        .fetch_all(&self.pool)
        .await?;
        let mut automatic_products = BTreeSet::new();
        let mut submissions = Vec::with_capacity(submission_rows.len());
        for row in submission_rows {
            let product_interests: Vec<String> = row.try_get("product_codes")?;
            automatic_products.extend(product_interests.iter().cloned());
            submissions.push(LeadDetailSubmission {
                id: row.try_get("id")?,
                external_lead_id: row.try_get("external_lead_id")?,
                source_created_at_utc: row.try_get("source_created_at_utc")?,
                source_created_at_raw: row.try_get("source_created_at_raw")?,
                ad_id: row.try_get("ad_id")?,
                ad_name: row.try_get("ad_name")?,
                adset_id: row.try_get("adset_id")?,
                adset_name: row.try_get("adset_name")?,
                campaign_id: row.try_get("campaign_id")?,
                campaign_name: row.try_get("campaign_name")?,
                form_id: row.try_get("form_id")?,
                form_name: row.try_get("form_name")?,
                is_organic: row.try_get("is_organic")?,
                platform: row.try_get("platform")?,
                raw_procedure_answer: row.try_get("raw_procedure_answer")?,
                raw_product_answer: row.try_get("raw_product_answer")?,
                raw_full_name: row.try_get("raw_full_name")?,
                raw_email: row.try_get("raw_email")?,
                raw_phone: row.try_get("raw_phone")?,
                raw_country: row.try_get("raw_country")?,
                raw_lead_status: row.try_get("raw_lead_status")?,
                raw_payload_json: row.try_get("raw_payload_json")?,
                product_interests,
            });
        }

        let override_rows = sqlx::query(
            r#"
            SELECT DISTINCT ON (product_code) product_code, action, created_at
            FROM contact_product_interest_overrides
            WHERE lead_contact_id = $1
            ORDER BY product_code, created_at DESC, id DESC
            "#,
        )
        .bind(contact_id)
        .fetch_all(&self.pool)
        .await?;
        let mut product_overrides = Vec::with_capacity(override_rows.len());
        let mut effective_products = automatic_products.clone();
        for row in override_rows {
            let product_code: String = row.try_get("product_code")?;
            let action: String = row.try_get("action")?;
            match action.as_str() {
                "ADD" => {
                    effective_products.insert(product_code.clone());
                }
                "REMOVE" => {
                    effective_products.remove(&product_code);
                }
                _ => {}
            }
            product_overrides.push(LeadDetailProductOverride {
                product_code,
                action,
                created_at: row.try_get("created_at")?,
            });
        }

        let quality_issues = sqlx::query(
            r#"
            SELECT id, lead_submission_id, issue_type, severity, details_json, status, created_at, resolved_at
            FROM lead_data_quality_issues
            WHERE lead_contact_id = $1
            ORDER BY created_at DESC, id DESC
            "#,
        )
        .bind(contact_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(LeadDetailQualityIssue {
                id: row.try_get("id")?,
                lead_submission_id: row.try_get("lead_submission_id")?,
                issue_type: row.try_get("issue_type")?,
                severity: row.try_get("severity")?,
                details_json: row.try_get("details_json")?,
                status: row.try_get("status")?,
                created_at: row.try_get("created_at")?,
                resolved_at: row.try_get("resolved_at")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;

        let notes = sqlx::query(
            r#"
            SELECT id, body, revision, created_at, updated_at
            FROM lead_notes
            WHERE lead_contact_id = $1
            ORDER BY created_at DESC, id DESC
            "#,
        )
        .bind(contact_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(LeadDetailNote {
                id: row.try_get("id")?,
                body: row.try_get("body")?,
                revision: row.try_get("revision")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;

        let activities = sqlx::query(
            r#"
            SELECT
                a.id, a.activity_type, a.occurred_at, a.payload_json,
                a.actor_user_id, actor.display_name AS actor_display_name
            FROM lead_activities a
            LEFT JOIN app_users actor ON actor.id = a.actor_user_id
            WHERE a.lead_contact_id = $1
            ORDER BY a.occurred_at DESC, a.id DESC
            "#,
        )
        .bind(contact_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(LeadDetailActivity {
                id: row.try_get("id")?,
                activity_type: row.try_get("activity_type")?,
                occurred_at: row.try_get("occurred_at")?,
                payload_json: row.try_get("payload_json")?,
                actor_user_id: row.try_get("actor_user_id")?,
                actor_display_name: row.try_get("actor_display_name")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;

        Ok(Some(LeadDetailResponse {
            contact: LeadDetailContact {
                id: contact_row.try_get("id")?,
                display_name: display_name(contact_row.try_get("display_name")?),
                primary_email: contact_row.try_get("primary_email")?,
                primary_phone: contact_row.try_get("primary_phone")?,
                country_code: contact_row.try_get("country_code")?,
                status: contact_row.try_get("status")?,
                revision: contact_row.try_get("revision")?,
                assignee,
                created_at: contact_row.try_get("created_at")?,
                updated_at: contact_row.try_get("updated_at")?,
                latest_submission_at: contact_row.try_get("latest_submission_at")?,
                submission_count: contact_row.try_get("submission_count")?,
                automatic_product_interests: automatic_products.into_iter().collect(),
                product_interests: effective_products.into_iter().collect(),
                product_overrides,
            },
            submissions,
            quality_issues,
            notes,
            activities,
        }))
    }

    pub async fn change_lead_status(
        &self,
        actor: &Actor,
        contact_id: &str,
        request: ChangeLeadStatusRequest,
    ) -> Result<LeadStatusResult, CrmError> {
        actor.require(Action::LeadStatusUpdate)?;
        let contact_id = required_id(contact_id, "contactId")?;
        validate_expected_revision(request.expected_revision)?;
        let status = request.status.trim().to_ascii_uppercase();
        if !LEAD_STATUSES.contains(&status.as_str()) {
            return Err(CrmError::Validation(format!(
                "unsupported lead status {status}"
            )));
        }

        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT status, revision, assigned_user_id, updated_at FROM lead_contacts WHERE id = $1 FOR UPDATE",
        )
        .bind(contact_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| CrmError::NotFound("lead contact".to_string()))?;
        let assigned_user_id: Option<String> = row.try_get("assigned_user_id")?;
        if let LeadScope::AssignedTo(user_id) = actor.lead_scope() {
            if assigned_user_id.as_deref() != Some(user_id) {
                return Err(CrmError::NotFound("lead contact".to_string()));
            }
        }
        let current_revision: i64 = row.try_get("revision")?;
        if current_revision != request.expected_revision {
            return Err(CrmError::Conflict {
                resource: "lead contact".to_string(),
                current_revision,
            });
        }
        let old_status: String = row.try_get("status")?;
        if old_status == status {
            tx.commit().await?;
            return Ok(LeadStatusResult {
                changed: false,
                status,
                revision: current_revision,
                updated_at: row.try_get("updated_at")?,
            });
        }

        let next_revision = current_revision + 1;
        let updated_at = Utc::now();
        sqlx::query(
            r#"
            UPDATE lead_contacts
            SET status = $1, revision = $2, updated_at = $3
            WHERE id = $4
            "#,
        )
        .bind(&status)
        .bind(next_revision)
        .bind(updated_at)
        .bind(contact_id)
        .execute(&mut *tx)
        .await?;
        insert_activity(
            &mut tx,
            contact_id,
            &actor.user_id,
            "STATUS_CHANGED",
            json!({
                "fromStatus": old_status,
                "toStatus": status,
            })
            .to_string(),
        )
        .await?;
        tx.commit().await?;

        Ok(LeadStatusResult {
            changed: true,
            status,
            revision: next_revision,
            updated_at,
        })
    }

    async fn get_staff(&self, user_id: &str) -> Result<Option<StaffMember>, CrmError> {
        let row = sqlx::query(
            r#"
            SELECT
                u.id, u.display_name, u.email, u.role, u.is_active, u.revision,
                EXISTS(SELECT 1 FROM app_credentials c WHERE c.user_id = u.id) AS auth_enabled,
                u.created_at, u.updated_at
            FROM app_users u
            WHERE u.id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(map_staff).transpose()
    }

    async fn revision_or_not_found(
        &self,
        resource: &str,
        user_id: &str,
    ) -> Result<CrmError, CrmError> {
        let revision = sqlx::query_scalar::<_, i64>("SELECT revision FROM app_users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(match revision {
            Some(current_revision) => CrmError::Conflict {
                resource: resource.to_string(),
                current_revision,
            },
            None => CrmError::NotFound(resource.to_string()),
        })
    }

    async fn assignee_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        user_id: &str,
    ) -> Result<Option<LeadAssignee>, CrmError> {
        let row = sqlx::query("SELECT id, display_name, is_active FROM app_users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await?;
        row.map(|row| {
            Ok(LeadAssignee {
                id: row.try_get("id")?,
                display_name: row.try_get("display_name")?,
                is_active: row.try_get("is_active")?,
            })
        })
        .transpose()
    }
}

#[derive(Debug, Clone, Copy)]
enum LeadListSort {
    LatestDesc,
    LatestAsc,
    NameAsc,
    NameDesc,
}

#[derive(Debug, Clone)]
struct LeadFilters {
    search: Option<String>,
    status: Option<String>,
    country_code: Option<String>,
    product_code: Option<String>,
    assigned_user_id: Option<String>,
    unassigned_only: bool,
    repeat_only: bool,
    warning_only: bool,
    sort: LeadListSort,
}

impl LeadFilters {
    fn from_request(request: LeadListRequest) -> Result<Self, CrmError> {
        let status = clean_optional(request.status).map(|value| value.to_ascii_uppercase());
        if let Some(status) = &status {
            if !LEAD_STATUSES.contains(&status.as_str()) {
                return Err(CrmError::Validation(format!(
                    "unsupported lead status {status}"
                )));
            }
        }
        Ok(Self {
            search: clean_optional(request.search),
            status,
            country_code: clean_optional(request.country_code)
                .map(|value| value.to_ascii_uppercase()),
            product_code: clean_optional(request.product_code)
                .map(|value| value.to_ascii_uppercase()),
            assigned_user_id: clean_optional(request.assigned_user_id),
            unassigned_only: request.unassigned_only.unwrap_or(false),
            repeat_only: request.repeat_only.unwrap_or(false),
            warning_only: request.warning_only.unwrap_or(false),
            sort: parse_sort(request.sort.as_deref()),
        })
    }
}

fn validate_sales_list_request(actor: &Actor, request: &LeadListRequest) -> Result<(), CrmError> {
    if actor.role != Role::Sales {
        return Ok(());
    }
    if request.unassigned_only.unwrap_or(false) {
        return Err(CrmError::Authorization(AuthorizationError::Forbidden));
    }
    if let Some(requested_user_id) = request
        .assigned_user_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if requested_user_id != actor.user_id.as_str() {
            return Err(CrmError::Authorization(AuthorizationError::Forbidden));
        }
    }
    Ok(())
}

fn append_lead_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    actor: &Actor,
    filters: &LeadFilters,
) {
    builder.push(" WHERE 1 = 1");

    if let LeadScope::AssignedTo(user_id) = actor.lead_scope() {
        builder
            .push(" AND c.assigned_user_id = ")
            .push_bind(user_id.to_string());
    }

    if let Some(search) = &filters.search {
        let pattern = format!("%{search}%");
        builder.push(" AND (");
        builder
            .push("COALESCE(c.display_name, '') ILIKE ")
            .push_bind(pattern.clone());
        builder
            .push(" OR COALESCE(c.primary_email, '') ILIKE ")
            .push_bind(pattern.clone());
        builder
            .push(" OR COALESCE(c.primary_phone, '') ILIKE ")
            .push_bind(pattern.clone());
        builder.push(
            " OR EXISTS (SELECT 1 FROM lead_submissions search_submission WHERE search_submission.lead_contact_id = c.id AND search_submission.external_lead_id ILIKE ",
        );
        builder.push_bind(pattern).push(")");
        builder.push(")");
    }

    if let Some(status) = &filters.status {
        builder.push(" AND c.status = ").push_bind(status.clone());
    }
    if let Some(country_code) = &filters.country_code {
        builder
            .push(" AND upper(c.country_code) = ")
            .push_bind(country_code.clone());
    }

    if actor.role != Role::Sales {
        if filters.unassigned_only {
            builder.push(" AND c.assigned_user_id IS NULL");
        } else if let Some(user_id) = &filters.assigned_user_id {
            builder
                .push(" AND c.assigned_user_id = ")
                .push_bind(user_id.clone());
        }
    }

    if let Some(product_code) = &filters.product_code {
        builder.push(
            " AND COALESCE((SELECT o.action FROM contact_product_interest_overrides o WHERE o.lead_contact_id = c.id AND o.product_code = ",
        );
        builder.push_bind(product_code.clone());
        builder.push(
            " ORDER BY o.created_at DESC, o.id DESC LIMIT 1), CASE WHEN EXISTS (SELECT 1 FROM lead_submissions product_submission JOIN submission_product_interests product_interest ON product_interest.lead_submission_id = product_submission.id WHERE product_submission.lead_contact_id = c.id AND product_interest.product_code = ",
        );
        builder.push_bind(product_code.clone());
        builder.push(") THEN 'ADD' ELSE 'REMOVE' END) = 'ADD'");
    }

    if filters.repeat_only {
        builder.push(" AND c.submission_count > 1");
    }
    if filters.warning_only {
        builder.push(
            " AND EXISTS (SELECT 1 FROM lead_data_quality_issues warning_issue WHERE warning_issue.lead_contact_id = c.id AND warning_issue.status = 'OPEN')",
        );
    }
}

fn append_lead_sort(builder: &mut QueryBuilder<'_, Postgres>, sort: LeadListSort) {
    match sort {
        LeadListSort::LatestDesc => builder.push(
            " ORDER BY c.latest_submission_at DESC NULLS LAST, lower(c.display_name) ASC NULLS LAST, c.id ASC",
        ),
        LeadListSort::LatestAsc => builder.push(
            " ORDER BY c.latest_submission_at ASC NULLS LAST, lower(c.display_name) ASC NULLS LAST, c.id ASC",
        ),
        LeadListSort::NameAsc => builder.push(
            " ORDER BY lower(c.display_name) ASC NULLS LAST, c.latest_submission_at DESC NULLS LAST, c.id ASC",
        ),
        LeadListSort::NameDesc => builder.push(
            " ORDER BY lower(c.display_name) DESC NULLS LAST, c.latest_submission_at DESC NULLS LAST, c.id ASC",
        ),
    };
}

fn parse_sort(value: Option<&str>) -> LeadListSort {
    match value.unwrap_or_default() {
        "LATEST_ASC" => LeadListSort::LatestAsc,
        "NAME_ASC" => LeadListSort::NameAsc,
        "NAME_DESC" => LeadListSort::NameDesc,
        _ => LeadListSort::LatestDesc,
    }
}

fn map_staff(row: sqlx::postgres::PgRow) -> Result<StaffMember, CrmError> {
    Ok(StaffMember {
        id: row.try_get("id")?,
        display_name: row.try_get("display_name")?,
        email: row.try_get("email")?,
        role: row.try_get("role")?,
        is_active: row.try_get("is_active")?,
        revision: row.try_get("revision")?,
        auth_enabled: row.try_get("auth_enabled")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn map_staff_write_error(
    result: Result<sqlx::postgres::PgQueryResult, sqlx::Error>,
) -> Result<sqlx::postgres::PgQueryResult, CrmError> {
    match result {
        Ok(result) => Ok(result),
        Err(sqlx::Error::Database(database_error)) if database_error.is_unique_violation() => {
            Err(CrmError::Validation(
                "staff email is already in use".to_string(),
            ))
        }
        Err(error) => Err(CrmError::Database(error)),
    }
}

fn validate_staff_input(
    display_name: String,
    email: Option<String>,
    role: String,
) -> Result<(String, Option<String>, String), CrmError> {
    let display_name = display_name.trim().to_string();
    if !(2..=100).contains(&display_name.chars().count()) {
        return Err(CrmError::Validation(
            "staff display name must be between 2 and 100 characters".to_string(),
        ));
    }
    let email = clean_optional(email).map(|value| value.to_ascii_lowercase());
    if let Some(email) = &email {
        let valid = email.len() <= 254
            && email.contains('@')
            && !email.starts_with('@')
            && !email.ends_with('@')
            && !email.chars().any(char::is_whitespace);
        if !valid {
            return Err(CrmError::Validation("invalid staff email".to_string()));
        }
    }
    let role = role.trim().to_ascii_uppercase();
    if !ROLES.contains(&role.as_str()) {
        return Err(CrmError::Validation("unsupported staff role".to_string()));
    }
    Ok((display_name, email, role))
}

fn validate_expected_revision(revision: i64) -> Result<(), CrmError> {
    if revision < 0 {
        return Err(CrmError::Validation(
            "expectedRevision must be zero or greater".to_string(),
        ));
    }
    Ok(())
}

fn required_id<'a>(value: &'a str, field: &str) -> Result<&'a str, CrmError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CrmError::Validation(format!("{field} is required")));
    }
    Ok(value)
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn display_name(value: Option<String>) -> String {
    value
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "İsimsiz lead".to_string())
}

fn effective_products_from_encoded(
    automatic: Vec<String>,
    overrides: Vec<String>,
) -> Vec<String> {
    let mut products = automatic.into_iter().collect::<BTreeSet<_>>();
    for override_value in overrides {
        let Some((product_code, action)) = override_value.split_once('=') else {
            continue;
        };
        match action {
            "ADD" => {
                products.insert(product_code.to_string());
            }
            "REMOVE" => {
                products.remove(product_code);
            }
            _ => {}
        }
    }
    products.into_iter().collect()
}

fn summarize_warnings(warning_types: Vec<String>) -> Vec<LeadWarningSummary> {
    let mut counts = BTreeMap::<String, i64>::new();
    for issue_type in warning_types {
        *counts.entry(issue_type).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .map(|(issue_type, count)| LeadWarningSummary { issue_type, count })
        .collect()
}

async fn insert_activity(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    contact_id: &str,
    actor_user_id: &str,
    activity_type: &str,
    payload_json: String,
) -> Result<(), CrmError> {
    sqlx::query(
        r#"
        INSERT INTO lead_activities (
            id, lead_contact_id, actor_user_id, activity_type, occurred_at, payload_json
        ) VALUES ($1, $2, $3, $4, now(), $5)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(contact_id)
    .bind(actor_user_id)
    .bind(activity_type)
    .bind(payload_json)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::postgres::PgPoolOptions;

    use super::{
        AssignmentRequest, ChangeLeadStatusRequest, CrmError, CrmService, LeadListRequest,
    };
    use crate::{
        authz::{Actor, AuthorizationError, Role},
        db::run_migrations,
    };

    #[test]
    fn product_override_resolution_matches_local_semantics() {
        assert_eq!(
            super::effective_products_from_encoded(
                vec!["FUE_PUNCHES".to_string(), "LONG_HAIR_FUE_SOLUTIONS".to_string()],
                vec![
                    "FUE_PUNCHES=REMOVE".to_string(),
                    "MEDICAL_CHAIRS_CLINIC_FURNITURE=ADD".to_string(),
                ],
            ),
            vec![
                "LONG_HAIR_FUE_SOLUTIONS".to_string(),
                "MEDICAL_CHAIRS_CLINIC_FURNITURE".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn postgres_crm_scope_assignment_status_and_revision_work_when_configured() {
        let Ok(database_url) = std::env::var("ELM_TEST_DATABASE_URL") else {
            eprintln!("ELM_TEST_DATABASE_URL is not set; skipping CRM integration test");
            return;
        };
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url)
            .await
            .expect("connect PostgreSQL test database");
        run_migrations(&pool).await.expect("migrations");

        sqlx::query("DELETE FROM lead_activities WHERE lead_contact_id LIKE 'crm-api-test-%'")
            .execute(&pool)
            .await
            .expect("cleanup activities");
        sqlx::query("DELETE FROM lead_contacts WHERE id LIKE 'crm-api-test-%'")
            .execute(&pool)
            .await
            .expect("cleanup contacts");
        sqlx::query("DELETE FROM app_users WHERE id LIKE 'crm-api-test-%'")
            .execute(&pool)
            .await
            .expect("cleanup users");

        for (id, name, role) in [
            ("crm-api-test-admin", "CRM Admin", "ADMIN"),
            ("crm-api-test-manager", "CRM Manager", "MANAGER"),
            ("crm-api-test-sales-a", "CRM Sales A", "SALES"),
            ("crm-api-test-sales-b", "CRM Sales B", "SALES"),
        ] {
            sqlx::query(
                "INSERT INTO app_users (id, display_name, role, is_active, created_at, updated_at) VALUES ($1, $2, $3, TRUE, now(), now())",
            )
            .bind(id)
            .bind(name)
            .bind(role)
            .execute(&pool)
            .await
            .expect("seed user");
        }
        for (id, name, assignee) in [
            ("crm-api-test-lead-a", "Lead A", "crm-api-test-sales-a"),
            ("crm-api-test-lead-b", "Lead B", "crm-api-test-sales-b"),
        ] {
            sqlx::query(
                "INSERT INTO lead_contacts (id, display_name, status, assigned_user_id, created_at, updated_at, submission_count) VALUES ($1, $2, 'NEW', $3, now(), now(), 0)",
            )
            .bind(id)
            .bind(name)
            .bind(assignee)
            .execute(&pool)
            .await
            .expect("seed lead");
        }

        let service = CrmService::new(pool.clone());
        let sales_a = Actor {
            user_id: "crm-api-test-sales-a".to_string(),
            role: Role::Sales,
        };
        let sales_b = Actor {
            user_id: "crm-api-test-sales-b".to_string(),
            role: Role::Sales,
        };
        let manager = Actor {
            user_id: "crm-api-test-manager".to_string(),
            role: Role::Manager,
        };

        let sales_list = service
            .list_leads(&sales_a, LeadListRequest::default())
            .await
            .expect("sales list");
        assert_eq!(sales_list.total, 1);
        assert_eq!(sales_list.items[0].id, "crm-api-test-lead-a");

        let assignment = service
            .assign_lead(
                &manager,
                "crm-api-test-lead-b",
                AssignmentRequest {
                    assigned_user_id: Some("crm-api-test-sales-a".to_string()),
                    expected_revision: 0,
                },
            )
            .await
            .expect("manager assignment");
        assert!(assignment.changed);
        assert_eq!(assignment.revision, 1);

        let status = service
            .change_lead_status(
                &sales_a,
                "crm-api-test-lead-b",
                ChangeLeadStatusRequest {
                    status: "CONTACTED".to_string(),
                    expected_revision: 1,
                },
            )
            .await
            .expect("assigned sales status update");
        assert!(status.changed);
        assert_eq!(status.revision, 2);

        assert!(matches!(
            service
                .change_lead_status(
                    &sales_a,
                    "crm-api-test-lead-b",
                    ChangeLeadStatusRequest {
                        status: "QUALIFIED".to_string(),
                        expected_revision: 1,
                    },
                )
                .await,
            Err(CrmError::Conflict { current_revision: 2, .. })
        ));
        assert!(service
            .get_lead(&sales_b, "crm-api-test-lead-b")
            .await
            .expect("scoped detail")
            .is_none());
        assert!(matches!(
            service
                .list_staff(&sales_a, false)
                .await,
            Err(CrmError::Authorization(AuthorizationError::Forbidden))
        ));

        let activity_actor = sqlx::query_scalar::<_, String>(
            "SELECT actor_user_id FROM lead_activities WHERE lead_contact_id = 'crm-api-test-lead-b' AND activity_type = 'STATUS_CHANGED' ORDER BY occurred_at DESC LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("status activity actor");
        assert_eq!(activity_actor, sales_a.user_id);

        pool.close().await;
    }
}
