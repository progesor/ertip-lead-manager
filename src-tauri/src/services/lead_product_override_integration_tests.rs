use std::path::PathBuf;

use super::import_commit_service::ImportCommitService;
use super::lead_crm_service::LeadCrmService;
use super::lead_detail_service::LeadDetailService;
use crate::db::Database;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures/leads_sample_multiselect_sanitized.csv")
}

#[tokio::test]
async fn reimport_does_not_overwrite_manual_product_interest_override() {
    let database = Database::connect_memory().await.expect("open database");
    let import_service = ImportCommitService::new(database.pool().clone());

    import_service
        .commit(&fixture(), "0.1.0")
        .await
        .expect("initial import");

    let contact_id: String = sqlx::query_scalar(
        "SELECT lead_contact_id FROM lead_submissions WHERE external_lead_id = 'l:demo2002'",
    )
    .fetch_one(database.pool())
    .await
    .expect("resolve imported contact");

    let crm = LeadCrmService::new(database.pool().clone());
    assert!(crm
        .set_product_interest(&contact_id, "MEDICAL_CHAIRS_CLINIC_FURNITURE", true)
        .await
        .expect("add manual product"));

    import_service
        .commit(&fixture(), "0.1.0")
        .await
        .expect("idempotent reimport");

    let detail = LeadDetailService::new(database.pool().clone())
        .get(&contact_id)
        .await
        .expect("load detail")
        .expect("contact exists");

    assert!(detail
        .contact
        .automatic_product_interests
        .contains(&"FUE_PUNCHES".to_string()));
    assert!(!detail
        .contact
        .automatic_product_interests
        .contains(&"MEDICAL_CHAIRS_CLINIC_FURNITURE".to_string()));
    assert!(detail
        .contact
        .product_interests
        .contains(&"MEDICAL_CHAIRS_CLINIC_FURNITURE".to_string()));
    assert!(detail.contact.product_overrides.iter().any(|override_record| {
        override_record.product_code == "MEDICAL_CHAIRS_CLINIC_FURNITURE"
            && override_record.action == "ADD"
    }));

    let override_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM contact_product_interest_overrides WHERE lead_contact_id = ? AND product_code = 'MEDICAL_CHAIRS_CLINIC_FURNITURE'",
    )
    .bind(&contact_id)
    .fetch_one(database.pool())
    .await
    .expect("count overrides");
    assert_eq!(override_count, 1);
}
