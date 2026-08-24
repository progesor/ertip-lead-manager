use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use crate::domain::product_interest::effective_product_interests;
use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeadListSort {
    LatestDesc,
    LatestAsc,
    NameAsc,
    NameDesc,
}

#[derive(Debug, Clone, Default)]
pub struct LeadListFilters {
    pub search: Option<String>,
    pub status: Option<String>,
    pub country_code: Option<String>,
    pub product_code: Option<String>,
    pub repeat_only: bool,
    pub warning_only: bool,
    pub follow_up_due_from: Option<String>,
    pub follow_up_due_to: Option<String>,
    pub follow_up_due_before: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LeadListQuery {
    pub filters: LeadListFilters,
    pub sort: LeadListSort,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeadListRecord {
    pub id: String,
    pub display_name: Option<String>,
    pub primary_email: Option<String>,
    pub primary_phone: Option<String>,
    pub country_code: Option<String>,
    pub status: String,
    pub latest_submission_at: Option<String>,
    pub submission_count: i64,
    pub product_codes: Vec<String>,
    pub platforms: Vec<String>,
    pub warning_count: i64,
    pub warning_types: Vec<String>,
}

#[derive(Clone)]
pub struct LeadWorkspaceRepository {
    pool: SqlitePool,
}

impl LeadWorkspaceRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, query: &LeadListQuery) -> Result<(Vec<LeadListRecord>, i64), AppError> {
        let total = self.count(query).await?;

        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                c.id,
                c.display_name,
                c.primary_email,
                c.primary_phone,
                c.country_code,
                c.status,
                c.latest_submission_at,
                c.submission_count,
                COALESCE((
                    SELECT GROUP_CONCAT(DISTINCT spi.product_code)
                    FROM lead_submissions s
                    JOIN submission_product_interests spi
                      ON spi.lead_submission_id = s.id
                    WHERE s.lead_contact_id = c.id
                ), '') AS automatic_product_codes,
                COALESCE((
                    SELECT GROUP_CONCAT(o.product_code || '=' || o.action, '|')
                    FROM contact_product_interest_overrides o
                    WHERE o.lead_contact_id = c.id
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
                ), '') AS product_overrides,
                COALESCE((
                    SELECT GROUP_CONCAT(DISTINCT LOWER(TRIM(s.platform)))
                    FROM lead_submissions s
                    WHERE s.lead_contact_id = c.id
                      AND TRIM(COALESCE(s.platform, '')) <> ''
                ), '') AS platforms,
                (
                    SELECT COUNT(*)
                    FROM lead_data_quality_issues q
                    WHERE q.lead_contact_id = c.id
                      AND q.status = 'OPEN'
                ) AS warning_count,
                COALESCE((
                    SELECT GROUP_CONCAT(q.issue_type)
                    FROM lead_data_quality_issues q
                    WHERE q.lead_contact_id = c.id
                      AND q.status = 'OPEN'
                ), '') AS warning_types
            FROM lead_contacts c
            "#,
        );

        append_filters(&mut builder, &query.filters);
        append_sort(&mut builder, query.sort);
        builder.push(" LIMIT ").push_bind(query.limit.max(1));
        builder.push(" OFFSET ").push_bind(query.offset.max(0));

        let rows = builder.build().fetch_all(&self.pool).await?;
        let records = rows
            .into_iter()
            .map(|row| {
                let raw_automatic_products: String = row.get("automatic_product_codes");
                let raw_overrides: String = row.get("product_overrides");
                let raw_platforms: String = row.get("platforms");
                let raw_warning_types: String = row.get("warning_types");

                LeadListRecord {
                    id: row.get("id"),
                    display_name: row.get("display_name"),
                    primary_email: row.get("primary_email"),
                    primary_phone: row.get("primary_phone"),
                    country_code: row.get("country_code"),
                    status: row.get("status"),
                    latest_submission_at: row.get("latest_submission_at"),
                    submission_count: row.get("submission_count"),
                    product_codes: effective_product_interests(
                        split_group_concat(&raw_automatic_products),
                        split_overrides(&raw_overrides),
                    ),
                    platforms: split_group_concat(&raw_platforms),
                    warning_count: row.get("warning_count"),
                    warning_types: split_group_concat(&raw_warning_types),
                }
            })
            .collect();

        Ok((records, total))
    }

    pub async fn country_codes(&self) -> Result<Vec<String>, AppError> {
        let rows = sqlx::query_scalar::<_, String>(
            r#"
            SELECT DISTINCT UPPER(TRIM(country_code))
            FROM lead_contacts
            WHERE LENGTH(TRIM(COALESCE(country_code, ''))) = 2
            ORDER BY UPPER(TRIM(country_code)) ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn count(&self, query: &LeadListQuery) -> Result<i64, AppError> {
        let mut builder = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM lead_contacts c");
        append_filters(&mut builder, &query.filters);
        let count = builder.build_query_scalar::<i64>().fetch_one(&self.pool).await?;
        Ok(count)
    }
}

fn split_group_concat(value: &str) -> Vec<String> {
    value
        .split(',')
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn split_overrides(value: &str) -> Vec<(String, String)> {
    value
        .split('|')
        .filter_map(|entry| entry.split_once('='))
        .map(|(product_code, action)| (product_code.to_string(), action.to_string()))
        .collect()
}

fn append_filters(builder: &mut QueryBuilder<'_, Sqlite>, filters: &LeadListFilters) {
    builder.push(" WHERE 1 = 1");

    if let Some(search) = filters
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let pattern = format!("%{}%", search.to_lowercase());
        builder.push(" AND (");
        builder
            .push("LOWER(COALESCE(c.display_name, '')) LIKE ")
            .push_bind(pattern.clone());
        builder
            .push(" OR LOWER(COALESCE(c.primary_email, '')) LIKE ")
            .push_bind(pattern.clone());
        builder
            .push(" OR LOWER(COALESCE(c.primary_phone, '')) LIKE ")
            .push_bind(pattern.clone());
        builder.push(
            " OR EXISTS (SELECT 1 FROM lead_submissions search_submission WHERE search_submission.lead_contact_id = c.id AND LOWER(search_submission.external_lead_id) LIKE ",
        );
        builder.push_bind(pattern).push(")");
        builder.push(")");
    }

    if let Some(status) = filters
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        builder.push(" AND c.status = ").push_bind(status.to_string());
    }

    if let Some(country_code) = filters
        .country_code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        builder
            .push(" AND c.country_code = ")
            .push_bind(country_code.to_ascii_uppercase());
    }

    if let Some(product_code) = filters
        .product_code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        append_effective_product_filter(builder, product_code);
    }

    if filters.repeat_only {
        builder.push(" AND c.submission_count > 1");
    }

    if filters.warning_only {
        builder.push(
            " AND EXISTS (SELECT 1 FROM lead_data_quality_issues warning_issue WHERE warning_issue.lead_contact_id = c.id AND warning_issue.status = 'OPEN')",
        );
    }

    if let Some(before) = filters
        .follow_up_due_before
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        builder.push(
            " AND EXISTS (SELECT 1 FROM follow_ups follow_up_filter WHERE follow_up_filter.lead_contact_id = c.id AND follow_up_filter.status = 'OPEN' AND follow_up_filter.due_at < ",
        );
        builder.push_bind(before.to_string()).push(")");
    } else if filters.follow_up_due_from.is_some() || filters.follow_up_due_to.is_some() {
        builder.push(
            " AND EXISTS (SELECT 1 FROM follow_ups follow_up_filter WHERE follow_up_filter.lead_contact_id = c.id AND follow_up_filter.status = 'OPEN'",
        );
        if let Some(from) = filters
            .follow_up_due_from
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            builder.push(" AND follow_up_filter.due_at >= ").push_bind(from.to_string());
        }
        if let Some(to) = filters
            .follow_up_due_to
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            builder.push(" AND follow_up_filter.due_at < ").push_bind(to.to_string());
        }
        builder.push(")");
    }
}

fn append_effective_product_filter(builder: &mut QueryBuilder<'_, Sqlite>, product_code: &str) {
    builder.push(
        " AND COALESCE((SELECT o.action FROM contact_product_interest_overrides o WHERE o.lead_contact_id = c.id AND o.product_code = ",
    );
    builder.push_bind(product_code.to_string());
    builder.push(
        " ORDER BY o.created_at DESC, o.id DESC LIMIT 1), CASE WHEN EXISTS (SELECT 1 FROM lead_submissions product_submission JOIN submission_product_interests product_interest ON product_interest.lead_submission_id = product_submission.id WHERE product_submission.lead_contact_id = c.id AND product_interest.product_code = ",
    );
    builder.push_bind(product_code.to_string());
    builder.push(") THEN 'ADD' ELSE 'REMOVE' END) = 'ADD'");
}

fn append_sort(builder: &mut QueryBuilder<'_, Sqlite>, sort: LeadListSort) {
    match sort {
        LeadListSort::LatestDesc => builder.push(
            " ORDER BY c.latest_submission_at IS NULL ASC, c.latest_submission_at DESC, c.display_name COLLATE NOCASE ASC, c.id ASC",
        ),
        LeadListSort::LatestAsc => builder.push(
            " ORDER BY c.latest_submission_at IS NULL ASC, c.latest_submission_at ASC, c.display_name COLLATE NOCASE ASC, c.id ASC",
        ),
        LeadListSort::NameAsc => builder.push(
            " ORDER BY c.display_name IS NULL ASC, c.display_name COLLATE NOCASE ASC, c.latest_submission_at DESC, c.id ASC",
        ),
        LeadListSort::NameDesc => builder.push(
            " ORDER BY c.display_name IS NULL ASC, c.display_name COLLATE NOCASE DESC, c.latest_submission_at DESC, c.id ASC",
        ),
    };
}

#[cfg(test)]
mod tests {
    use chrono::{SecondsFormat, Utc};

    use super::{LeadListFilters, LeadListQuery, LeadListSort, LeadWorkspaceRepository};
    use crate::db::Database;

    async fn seed_workspace(database: &Database) -> String {
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

        sqlx::query(
            "INSERT INTO lead_contacts (id, display_name, primary_email, normalized_email, country_code, status, created_at, updated_at, latest_submission_at, submission_count) VALUES ('contact-a', 'Alex Demo', 'alex@example.test', 'alex@example.test', 'TR', 'NEW', ?, ?, ?, 2), ('contact-b', 'Beta Demo', 'beta@example.test', 'beta@example.test', 'GB', 'CONTACTED', ?, ?, ?, 1)",
        )
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(database.pool())
        .await
        .expect("insert contacts");

        sqlx::query(
            "INSERT INTO import_batches (id, file_name, sheet_name, started_at, completed_at, status, total_rows, app_version) VALUES ('batch', 'fixture.csv', 'CSV', ?, ?, 'COMMITTED', 3, '0.1.0')",
        )
        .bind(&now)
        .bind(&now)
        .execute(database.pool())
        .await
        .expect("insert batch");

        for (id, contact_id, external_id, platform) in [
            ("submission-a1", "contact-a", "l:external-a1", "facebook"),
            ("submission-a2", "contact-a", "l:external-a2", "instagram"),
            ("submission-b1", "contact-b", "l:external-b1", "facebook"),
        ] {
            sqlx::query("INSERT INTO lead_submissions (id, lead_contact_id, import_batch_id, external_lead_id, source_created_at_raw, platform, raw_payload_json, created_at) VALUES (?, ?, 'batch', ?, ?, ?, '{}', ?)")
                .bind(id)
                .bind(contact_id)
                .bind(external_id)
                .bind(&now)
                .bind(platform)
                .bind(&now)
                .execute(database.pool())
                .await
                .expect("insert submission");
        }

        sqlx::query("INSERT INTO submission_product_interests (id, lead_submission_id, product_code, origin, confidence, created_at) VALUES ('product-a', 'submission-a1', 'FUE_PUNCHES', 'DIRECT_MULTI_SELECT', 'HIGH', ?), ('product-b', 'submission-b1', 'LONG_HAIR_FUE_SOLUTIONS', 'DIRECT_MULTI_SELECT', 'HIGH', ?)")
            .bind(&now)
            .bind(&now)
            .execute(database.pool())
            .await
            .expect("insert products");

        for warning_id in ["warning-a1", "warning-a2"] {
            sqlx::query("INSERT INTO lead_data_quality_issues (id, lead_contact_id, issue_type, severity, details_json, status, created_at) VALUES (?, 'contact-a', 'UNKNOWN_PRODUCT', 'WARNING', '{}', 'OPEN', ?)")
                .bind(warning_id)
                .bind(&now)
                .execute(database.pool())
                .await
                .expect("insert warning");
        }

        now
    }

    #[tokio::test]
    async fn list_supports_external_id_search_product_filter_and_repeat_warning_indicators() {
        let database = Database::connect_memory().await.expect("open database");
        seed_workspace(&database).await;

        let repository = LeadWorkspaceRepository::new(database.pool().clone());
        let query = LeadListQuery {
            filters: LeadListFilters {
                search: Some("external-a2".to_string()),
                product_code: Some("FUE_PUNCHES".to_string()),
                repeat_only: true,
                warning_only: true,
                ..LeadListFilters::default()
            },
            sort: LeadListSort::LatestDesc,
            limit: 50,
            offset: 0,
        };

        let (rows, total) = repository.list(&query).await.expect("list leads");
        assert_eq!(total, 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "contact-a");
        assert_eq!(rows[0].submission_count, 2);
        assert_eq!(rows[0].warning_count, 2);
        assert_eq!(rows[0].warning_types, vec!["UNKNOWN_PRODUCT", "UNKNOWN_PRODUCT"]);
        assert_eq!(rows[0].product_codes, vec!["FUE_PUNCHES"]);
        assert_eq!(rows[0].platforms, vec!["facebook", "instagram"]);

        let countries = repository.country_codes().await.expect("country options");
        assert_eq!(countries, vec!["GB", "TR"]);
    }

    #[tokio::test]
    async fn latest_manual_product_override_controls_list_display_and_filtering() {
        let database = Database::connect_memory().await.expect("open database");
        let now = seed_workspace(&database).await;

        sqlx::query("INSERT INTO contact_product_interest_overrides (id, lead_contact_id, product_code, action, created_at) VALUES ('override-remove', 'contact-a', 'FUE_PUNCHES', 'REMOVE', ?), ('override-add', 'contact-a', 'LONG_HAIR_FUE_SOLUTIONS', 'ADD', ?)")
            .bind(&now)
            .bind(&now)
            .execute(database.pool())
            .await
            .expect("insert overrides");

        let repository = LeadWorkspaceRepository::new(database.pool().clone());
        let all_query = LeadListQuery {
            filters: LeadListFilters {
                search: Some("Alex".to_string()),
                ..LeadListFilters::default()
            },
            sort: LeadListSort::LatestDesc,
            limit: 50,
            offset: 0,
        };
        let (rows, _) = repository.list(&all_query).await.expect("list Alex");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].product_codes, vec!["LONG_HAIR_FUE_SOLUTIONS"]);

        let removed_query = LeadListQuery {
            filters: LeadListFilters {
                product_code: Some("FUE_PUNCHES".to_string()),
                ..LeadListFilters::default()
            },
            sort: LeadListSort::LatestDesc,
            limit: 50,
            offset: 0,
        };
        let (_, removed_total) = repository.list(&removed_query).await.expect("filter removed product");
        assert_eq!(removed_total, 0);

        let added_query = LeadListQuery {
            filters: LeadListFilters {
                product_code: Some("LONG_HAIR_FUE_SOLUTIONS".to_string()),
                ..LeadListFilters::default()
            },
            sort: LeadListSort::LatestDesc,
            limit: 50,
            offset: 0,
        };
        let (_, added_total) = repository.list(&added_query).await.expect("filter added product");
        assert_eq!(added_total, 2);
    }

    #[tokio::test]
    async fn follow_up_window_filters_open_follow_ups_without_counting_closed_ones() {
        let database = Database::connect_memory().await.expect("open database");
        seed_workspace(&database).await;

        sqlx::query("INSERT INTO follow_ups (id, lead_contact_id, due_at, status, created_at) VALUES ('fu-overdue', 'contact-a', '2026-08-24T06:00:00.000Z', 'OPEN', '2026-08-23T00:00:00.000Z'), ('fu-today', 'contact-b', '2026-08-24T10:00:00.000Z', 'OPEN', '2026-08-23T00:00:00.000Z'), ('fu-closed', 'contact-b', '2026-08-24T05:00:00.000Z', 'COMPLETED', '2026-08-23T00:00:00.000Z')")
            .execute(database.pool())
            .await
            .expect("seed follow-ups");

        let repository = LeadWorkspaceRepository::new(database.pool().clone());
        let overdue_query = LeadListQuery {
            filters: LeadListFilters {
                follow_up_due_before: Some("2026-08-24T07:00:00.000Z".to_string()),
                ..LeadListFilters::default()
            },
            sort: LeadListSort::LatestDesc,
            limit: 50,
            offset: 0,
        };
        let (overdue_rows, overdue_total) = repository.list(&overdue_query).await.expect("filter overdue");
        assert_eq!(overdue_total, 1);
        assert_eq!(overdue_rows[0].id, "contact-a");

        let today_query = LeadListQuery {
            filters: LeadListFilters {
                follow_up_due_from: Some("2026-08-24T07:00:00.000Z".to_string()),
                follow_up_due_to: Some("2026-08-24T21:00:00.000Z".to_string()),
                ..LeadListFilters::default()
            },
            sort: LeadListSort::LatestDesc,
            limit: 50,
            offset: 0,
        };
        let (today_rows, today_total) = repository.list(&today_query).await.expect("filter today");
        assert_eq!(today_total, 1);
        assert_eq!(today_rows[0].id, "contact-b");
    }
}
