use serde::Serialize;

use super::identity::{ContactIdentity, IdentityDecision, IdentityEngine};
use super::normalization::{normalize_source_row, NormalizedSubmission};
use super::source::{SourceRow, SourceTable};

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlanSummary {
    pub total_rows: usize,
    pub importable_submissions: usize,
    pub new_contacts: usize,
    pub repeat_submissions: usize,
    pub exact_duplicates: usize,
    pub identity_conflicts: usize,
    pub row_errors: usize,
    pub warning_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedImportRow {
    pub source: SourceRow,
    pub normalized: NormalizedSubmission,
    pub decision: IdentityDecision,
    pub target_contact_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPlan {
    pub summary: ImportPlanSummary,
    pub rows: Vec<PlannedImportRow>,
}

impl ImportPlan {
    pub fn has_blocking_rows(&self) -> bool {
        self.summary.identity_conflicts > 0 || self.summary.row_errors > 0
    }
}

pub fn build_import_plan<F>(
    table: &SourceTable,
    existing_external_ids: impl IntoIterator<Item = String>,
    contacts: impl IntoIterator<Item = ContactIdentity>,
    mut create_contact_id: F,
) -> ImportPlan
where
    F: FnMut(&NormalizedSubmission) -> String,
{
    let mut identity = IdentityEngine::new(existing_external_ids, contacts);
    let mut summary = ImportPlanSummary {
        total_rows: table.rows.len(),
        ..ImportPlanSummary::default()
    };
    let mut rows = Vec::with_capacity(table.rows.len());

    for source in &table.rows {
        let normalized = normalize_source_row(source);
        summary.warning_count += normalized.warnings.len();

        let mut decision = identity.decide(&normalized);
        if matches!(decision, IdentityDecision::NewContact)
            && !has_useful_identity(source, &normalized)
        {
            decision = IdentityDecision::RowError {
                code: "MISSING_IDENTITY_FIELDS",
            };
        }

        let target_contact_id = match &decision {
            IdentityDecision::NewContact => {
                summary.new_contacts += 1;
                summary.importable_submissions += 1;

                let contact_id = create_contact_id(&normalized);
                identity.register_contact_identity(
                    contact_id.clone(),
                    normalized.normalized_email.as_deref(),
                    normalized.normalized_phone.as_deref(),
                );
                Some(contact_id)
            }
            IdentityDecision::RepeatContact { contact_id, .. } => {
                summary.repeat_submissions += 1;
                summary.importable_submissions += 1;

                identity.register_contact_identity(
                    contact_id.clone(),
                    normalized.normalized_email.as_deref(),
                    normalized.normalized_phone.as_deref(),
                );
                Some(contact_id.clone())
            }
            IdentityDecision::ExactDuplicateSubmission { .. } => {
                summary.exact_duplicates += 1;
                None
            }
            IdentityDecision::IdentityConflictReview { .. } => {
                summary.identity_conflicts += 1;
                None
            }
            IdentityDecision::RowError { .. } => {
                summary.row_errors += 1;
                None
            }
        };

        rows.push(PlannedImportRow {
            source: source.clone(),
            normalized,
            decision,
            target_contact_id,
        });
    }

    ImportPlan { summary, rows }
}

fn has_useful_identity(source: &SourceRow, normalized: &NormalizedSubmission) -> bool {
    !source.get("full_name").unwrap_or_default().trim().is_empty()
        || normalized.normalized_email.is_some()
        || normalized.normalized_phone.is_some()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::build_import_plan;
    use crate::importer::identity::IdentityDecision;
    use crate::importer::source::{SourceFormat, SourceRow, SourceTable};

    fn row(number: usize, id: &str, name: &str, email: &str, phone: &str) -> SourceRow {
        let mut fields = BTreeMap::new();
        fields.insert("id".to_string(), id.to_string());
        fields.insert(
            "created_time".to_string(),
            "2026-08-21T12:00:00+03:00".to_string(),
        );
        fields.insert("full_name".to_string(), name.to_string());
        fields.insert("email".to_string(), email.to_string());
        fields.insert("phone_number".to_string(), phone.to_string());
        SourceRow::new(number, fields)
    }

    #[test]
    fn same_batch_repeat_uses_the_contact_id_created_for_first_row() {
        let table = SourceTable::new(
            SourceFormat::Csv,
            "fixture.csv".to_string(),
            None,
            vec![],
            vec![
                row(2, "l:1", "First", "same@example.test", "p:+905551234567"),
                row(3, "l:2", "Second", "same@example.test", "p:+905551234567"),
            ],
        );

        let plan = build_import_plan(&table, Vec::<String>::new(), vec![], |normalized| {
            format!("contact:{}", normalized.external_lead_id)
        });

        assert_eq!(plan.summary.new_contacts, 1);
        assert_eq!(plan.summary.repeat_submissions, 1);
        assert_eq!(
            plan.rows[1].target_contact_id.as_deref(),
            Some("contact:l:1")
        );
    }

    #[test]
    fn row_without_name_or_valid_contact_method_is_blocking_error() {
        let table = SourceTable::new(
            SourceFormat::Csv,
            "fixture.csv".to_string(),
            None,
            vec![],
            vec![row(2, "l:empty", "", "bad-email", "123")],
        );

        let plan = build_import_plan(&table, Vec::<String>::new(), vec![], |_| {
            "unused".to_string()
        });

        assert!(plan.has_blocking_rows());
        assert_eq!(plan.summary.row_errors, 1);
        assert!(matches!(
            plan.rows[0].decision,
            IdentityDecision::RowError {
                code: "MISSING_IDENTITY_FIELDS"
            }
        ));
    }
}
