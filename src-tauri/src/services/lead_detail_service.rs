use std::collections::BTreeSet;

use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::repositories::lead_crm_repository::{LeadCrmRepository, LeadNoteRecord};
use crate::repositories::lead_detail_repository::{
    LeadActivityRecord, LeadDetailRepository, LeadDetailSubmissionRecord, LeadQualityIssueRecord,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeadDetailContact {
    pub id: String,
    pub display_name: String,
    pub primary_email: Option<String>,
    pub primary_phone: Option<String>,
    pub country_code: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub latest_submission_at: Option<String>,
    pub submission_count: i64,
    pub product_interests: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeadDetailSubmission {
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
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeadDetailActivity {
    pub id: String,
    pub activity_type: String,
    pub occurred_at: String,
    pub payload_json: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeadDetailNote {
    pub id: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
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

#[derive(Clone)]
pub struct LeadDetailService {
    repository: LeadDetailRepository,
    crm_repository: LeadCrmRepository,
}

impl LeadDetailService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            repository: LeadDetailRepository::new(pool.clone()),
            crm_repository: LeadCrmRepository::new(pool),
        }
    }

    pub async fn get(&self, contact_id: &str) -> Result<Option<LeadDetailResponse>, AppError> {
        let Some(contact) = self.repository.contact(contact_id).await? else {
            return Ok(None);
        };

        let submission_records = self.repository.submissions(contact_id).await?;
        let mut product_interests = BTreeSet::new();
        let submissions = submission_records
            .into_iter()
            .map(|record| {
                let products = split_group_concat(&record.product_codes);
                product_interests.extend(products.iter().cloned());
                map_submission(record, products)
            })
            .collect();

        let quality_issues = self
            .repository
            .quality_issues(contact_id)
            .await?
            .into_iter()
            .map(map_quality_issue)
            .collect();

        let notes = self
            .crm_repository
            .notes(contact_id)
            .await?
            .into_iter()
            .map(map_note)
            .collect();

        let activities = self
            .repository
            .activities(contact_id)
            .await?
            .into_iter()
            .map(map_activity)
            .collect();

        Ok(Some(LeadDetailResponse {
            contact: LeadDetailContact {
                id: contact.id,
                display_name: contact
                    .display_name
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "İsimsiz lead".to_string()),
                primary_email: contact.primary_email,
                primary_phone: contact.primary_phone,
                country_code: contact.country_code,
                status: contact.status,
                created_at: contact.created_at,
                updated_at: contact.updated_at,
                latest_submission_at: contact.latest_submission_at,
                submission_count: contact.submission_count,
                product_interests: product_interests.into_iter().collect(),
            },
            submissions,
            quality_issues,
            notes,
            activities,
        }))
    }
}

fn split_group_concat(value: &str) -> Vec<String> {
    value
        .split(',')
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn map_submission(
    record: LeadDetailSubmissionRecord,
    product_interests: Vec<String>,
) -> LeadDetailSubmission {
    LeadDetailSubmission {
        id: record.id,
        external_lead_id: record.external_lead_id,
        source_created_at_utc: record.source_created_at_utc,
        source_created_at_raw: record.source_created_at_raw,
        ad_id: record.ad_id,
        ad_name: record.ad_name,
        adset_id: record.adset_id,
        adset_name: record.adset_name,
        campaign_id: record.campaign_id,
        campaign_name: record.campaign_name,
        form_id: record.form_id,
        form_name: record.form_name,
        is_organic: record.is_organic.map(|value| value != 0),
        platform: record.platform,
        raw_procedure_answer: record.raw_procedure_answer,
        raw_product_answer: record.raw_product_answer,
        raw_full_name: record.raw_full_name,
        raw_email: record.raw_email,
        raw_phone: record.raw_phone,
        raw_country: record.raw_country,
        raw_lead_status: record.raw_lead_status,
        raw_payload_json: record.raw_payload_json,
        product_interests,
    }
}

fn map_quality_issue(record: LeadQualityIssueRecord) -> LeadDetailQualityIssue {
    LeadDetailQualityIssue {
        id: record.id,
        lead_submission_id: record.lead_submission_id,
        issue_type: record.issue_type,
        severity: record.severity,
        details_json: record.details_json,
        status: record.status,
        created_at: record.created_at,
        resolved_at: record.resolved_at,
    }
}

fn map_note(record: LeadNoteRecord) -> LeadDetailNote {
    LeadDetailNote {
        id: record.id,
        body: record.body,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn map_activity(record: LeadActivityRecord) -> LeadDetailActivity {
    LeadDetailActivity {
        id: record.id,
        activity_type: record.activity_type,
        occurred_at: record.occurred_at,
        payload_json: record.payload_json,
    }
}
