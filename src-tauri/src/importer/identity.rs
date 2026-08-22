use std::collections::{BTreeSet, HashMap, HashSet};

use serde::Serialize;

use super::normalization::NormalizedSubmission;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactIdentity {
    pub contact_id: String,
    pub normalized_email: Option<String>,
    pub normalized_phone: Option<String>,
}

impl ContactIdentity {
    pub fn new(
        contact_id: impl Into<String>,
        normalized_email: Option<impl Into<String>>,
        normalized_phone: Option<impl Into<String>>,
    ) -> Self {
        Self {
            contact_id: contact_id.into(),
            normalized_email: normalized_email.map(Into::into),
            normalized_phone: normalized_phone.map(Into::into),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IdentityMatchKind {
    Email,
    Phone,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "outcome", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IdentityDecision {
    NewContact,
    RepeatContact {
        contact_id: String,
        matched_by: Vec<IdentityMatchKind>,
    },
    ExactDuplicateSubmission {
        external_lead_id: String,
    },
    IdentityConflictReview {
        candidate_contact_ids: Vec<String>,
    },
    RowError {
        code: &'static str,
    },
}

pub struct IdentityEngine {
    existing_external_ids: HashSet<String>,
    seen_external_ids: HashSet<String>,
    email_index: HashMap<String, BTreeSet<String>>,
    phone_index: HashMap<String, BTreeSet<String>>,
}

impl IdentityEngine {
    pub fn new(
        existing_external_ids: impl IntoIterator<Item = String>,
        contacts: impl IntoIterator<Item = ContactIdentity>,
    ) -> Self {
        let mut engine = Self {
            existing_external_ids: existing_external_ids.into_iter().collect(),
            seen_external_ids: HashSet::new(),
            email_index: HashMap::new(),
            phone_index: HashMap::new(),
        };

        for contact in contacts {
            engine.register_contact_identity(
                contact.contact_id,
                contact.normalized_email.as_deref(),
                contact.normalized_phone.as_deref(),
            );
        }

        engine
    }

    pub fn register_contact_identity(
        &mut self,
        contact_id: impl Into<String>,
        normalized_email: Option<&str>,
        normalized_phone: Option<&str>,
    ) {
        let contact_id = contact_id.into();

        if let Some(email) = normalized_email.filter(|value| !value.is_empty()) {
            self.email_index
                .entry(email.to_string())
                .or_default()
                .insert(contact_id.clone());
        }

        if let Some(phone) = normalized_phone.filter(|value| !value.is_empty()) {
            self.phone_index
                .entry(phone.to_string())
                .or_default()
                .insert(contact_id);
        }
    }

    pub fn decide(&mut self, submission: &NormalizedSubmission) -> IdentityDecision {
        let external_id = submission.external_lead_id.trim();
        if external_id.is_empty() {
            return IdentityDecision::RowError {
                code: "MISSING_EXTERNAL_LEAD_ID",
            };
        }

        if self.existing_external_ids.contains(external_id)
            || !self.seen_external_ids.insert(external_id.to_string())
        {
            return IdentityDecision::ExactDuplicateSubmission {
                external_lead_id: external_id.to_string(),
            };
        }

        let email_candidates = submission
            .normalized_email
            .as_ref()
            .and_then(|email| self.email_index.get(email))
            .cloned()
            .unwrap_or_default();
        let phone_candidates = submission
            .normalized_phone
            .as_ref()
            .and_then(|phone| self.phone_index.get(phone))
            .cloned()
            .unwrap_or_default();

        if email_candidates.len() > 1 || phone_candidates.len() > 1 {
            return conflict(email_candidates.union(&phone_candidates).cloned().collect());
        }

        let email_contact = email_candidates.iter().next().cloned();
        let phone_contact = phone_candidates.iter().next().cloned();

        match (email_contact, phone_contact) {
            (Some(email_id), Some(phone_id)) if email_id == phone_id => {
                IdentityDecision::RepeatContact {
                    contact_id: email_id,
                    matched_by: vec![IdentityMatchKind::Email, IdentityMatchKind::Phone],
                }
            }
            (Some(email_id), Some(phone_id)) => conflict(vec![email_id, phone_id]),
            (Some(contact_id), None) => IdentityDecision::RepeatContact {
                contact_id,
                matched_by: vec![IdentityMatchKind::Email],
            },
            (None, Some(contact_id)) => IdentityDecision::RepeatContact {
                contact_id,
                matched_by: vec![IdentityMatchKind::Phone],
            },
            (None, None) => IdentityDecision::NewContact,
        }
    }
}

fn conflict(mut candidate_contact_ids: Vec<String>) -> IdentityDecision {
    candidate_contact_ids.sort();
    candidate_contact_ids.dedup();
    IdentityDecision::IdentityConflictReview {
        candidate_contact_ids,
    }
}

#[cfg(test)]
mod tests {
    use super::{ContactIdentity, IdentityDecision, IdentityEngine, IdentityMatchKind};
    use crate::importer::normalization::NormalizedSubmission;

    fn submission(id: &str, email: Option<&str>, phone: Option<&str>) -> NormalizedSubmission {
        NormalizedSubmission {
            row_number: 2,
            external_lead_id: id.to_string(),
            created_at_utc: None,
            normalized_email: email.map(ToOwned::to_owned),
            normalized_phone: phone.map(ToOwned::to_owned),
            country_code: None,
            product_interests: Vec::new(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn existing_external_id_is_exact_duplicate() {
        let mut engine = IdentityEngine::new(vec!["l:1".to_string()], Vec::<ContactIdentity>::new());
        assert!(matches!(
            engine.decide(&submission("l:1", None, None)),
            IdentityDecision::ExactDuplicateSubmission { .. }
        ));
    }

    #[test]
    fn duplicate_id_inside_same_file_is_exact_duplicate_after_first_row() {
        let mut engine = IdentityEngine::new(Vec::<String>::new(), Vec::<ContactIdentity>::new());
        assert_eq!(
            engine.decide(&submission("l:1", None, None)),
            IdentityDecision::NewContact
        );
        assert!(matches!(
            engine.decide(&submission("l:1", None, None)),
            IdentityDecision::ExactDuplicateSubmission { .. }
        ));
    }

    #[test]
    fn matching_email_only_is_repeat_contact() {
        let contacts = vec![ContactIdentity::new(
            "contact-a",
            Some("person@example.com"),
            None::<String>,
        )];
        let mut engine = IdentityEngine::new(Vec::<String>::new(), contacts);

        assert_eq!(
            engine.decide(&submission("l:2", Some("person@example.com"), None)),
            IdentityDecision::RepeatContact {
                contact_id: "contact-a".to_string(),
                matched_by: vec![IdentityMatchKind::Email],
            }
        );
    }

    #[test]
    fn matching_phone_only_is_repeat_contact() {
        let contacts = vec![ContactIdentity::new(
            "contact-a",
            None::<String>,
            Some("+905551234567"),
        )];
        let mut engine = IdentityEngine::new(Vec::<String>::new(), contacts);

        assert_eq!(
            engine.decide(&submission("l:2", None, Some("+905551234567"))),
            IdentityDecision::RepeatContact {
                contact_id: "contact-a".to_string(),
                matched_by: vec![IdentityMatchKind::Phone],
            }
        );
    }

    #[test]
    fn email_and_phone_matching_same_contact_is_strong_repeat() {
        let contacts = vec![ContactIdentity::new(
            "contact-a",
            Some("person@example.com"),
            Some("+905551234567"),
        )];
        let mut engine = IdentityEngine::new(Vec::<String>::new(), contacts);

        assert_eq!(
            engine.decide(&submission(
                "l:2",
                Some("person@example.com"),
                Some("+905551234567")
            )),
            IdentityDecision::RepeatContact {
                contact_id: "contact-a".to_string(),
                matched_by: vec![IdentityMatchKind::Email, IdentityMatchKind::Phone],
            }
        );
    }

    #[test]
    fn email_and_phone_pointing_to_different_contacts_requires_review() {
        let contacts = vec![
            ContactIdentity::new("contact-a", Some("person@example.com"), None::<String>),
            ContactIdentity::new("contact-b", None::<String>, Some("+905551234567")),
        ];
        let mut engine = IdentityEngine::new(Vec::<String>::new(), contacts);

        assert_eq!(
            engine.decide(&submission(
                "l:2",
                Some("person@example.com"),
                Some("+905551234567")
            )),
            IdentityDecision::IdentityConflictReview {
                candidate_contact_ids: vec!["contact-a".to_string(), "contact-b".to_string()],
            }
        );
    }

    #[test]
    fn non_matching_identifiers_create_new_contact() {
        let contacts = vec![ContactIdentity::new(
            "contact-a",
            Some("old@example.com"),
            Some("+905550000000"),
        )];
        let mut engine = IdentityEngine::new(Vec::<String>::new(), contacts);

        assert_eq!(
            engine.decide(&submission(
                "l:new",
                Some("new@example.com"),
                Some("+905551234567")
            )),
            IdentityDecision::NewContact
        );
    }

    #[test]
    fn ambiguous_duplicate_identity_value_requires_review() {
        let contacts = vec![
            ContactIdentity::new("contact-a", Some("shared@example.com"), None::<String>),
            ContactIdentity::new("contact-b", Some("shared@example.com"), None::<String>),
        ];
        let mut engine = IdentityEngine::new(Vec::<String>::new(), contacts);

        assert_eq!(
            engine.decide(&submission("l:new", Some("shared@example.com"), None)),
            IdentityDecision::IdentityConflictReview {
                candidate_contact_ids: vec!["contact-a".to_string(), "contact-b".to_string()],
            }
        );
    }

    #[test]
    fn missing_external_id_is_row_error() {
        let mut engine = IdentityEngine::new(Vec::<String>::new(), Vec::<ContactIdentity>::new());
        assert_eq!(
            engine.decide(&submission("", None, None)),
            IdentityDecision::RowError {
                code: "MISSING_EXTERNAL_LEAD_ID"
            }
        );
    }

    #[test]
    fn provisional_contact_registered_from_earlier_row_is_repeat_later_in_same_file() {
        let mut engine = IdentityEngine::new(Vec::<String>::new(), Vec::<ContactIdentity>::new());
        let first = submission(
            "l:first",
            Some("same@example.com"),
            Some("+905551234567"),
        );
        assert_eq!(engine.decide(&first), IdentityDecision::NewContact);

        engine.register_contact_identity(
            "preview:l:first",
            first.normalized_email.as_deref(),
            first.normalized_phone.as_deref(),
        );

        assert_eq!(
            engine.decide(&submission(
                "l:second",
                Some("same@example.com"),
                Some("+905551234567")
            )),
            IdentityDecision::RepeatContact {
                contact_id: "preview:l:first".to_string(),
                matched_by: vec![IdentityMatchKind::Email, IdentityMatchKind::Phone],
            }
        );
    }
}
