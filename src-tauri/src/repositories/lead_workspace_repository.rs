use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

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
    pub warning_count: i64,
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
                ), '') AS product_codes,
                (
                    SELECT COUNT(*)
                    FROM lead_data_quality_issues q
                    WHERE q.lead_contact_id = c.id
                      AND q.status = 'OPEN'
                ) AS warning_count
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
                let raw_products: String = row.get("product_codes");
                LeadListRecord {
                    id: row.get("id"),
                    display_name: row.get("display_name"),
                    primary_email: row.get("primary_email"),
                    primary_phone: row.get("primary_phone"),
                    country_code: row.get("country_code"),
                    status: row.get("status"),
                    latest_submission_at: row.get("latest_submission_at"),
                    submission_count: row.get("submission_count"),
                    product_codes: raw_products
                        .split(',')
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned)
                        .collect(),
                    warning_count: row.get("warning_count"),
                }
            })
            .collect();

        Ok((records, total))
    }

    async fn count(&self, query: &LeadListQuery) -> Result<i64, AppError> {
        let mut builder = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM lead_contacts c");
        append_filters(&mut builder, &query.filters);
        let count = builder.build_query_scalar::<i64>().fetch_one(&self.pool).await?;
        Ok(count)
    }
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
        builder.push(
            " AND EXISTS (SELECT 1 FROM lead_submissions product_submission JOIN submission_product_interests product_interest ON product_interest.lead_submission_id = product_submission.id WHERE product_submission.lead_contact_id = c.id AND product_interest.product_code = ",
        );
        builder.push_bind(product_code.to_string()).push(")");
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

    #[tokio::test]
    async fn list_supports_external_id_search_product_filter_and_repeat_warning_indicators() {
        let database = Database::connect_memory().await.expect("open database");
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

        for (id, contact_id, external_id) in [
            ("submission-a1", "contact-a", "l:external-a1"),
            ("submission-a2", "contact-a", "l:external-a2"),
            ("submission-b1", "contact-b", "l:external-b1"),
        ] {
            sqlx::query("INSERT INTO lead_submissions (id, lead_contact_id, import_batch_id, external_lead_id, source_created_at_raw, raw_payload_json, created_at) VALUES (?, ?, 'batch', ?, ?, '{}', ?)")
                .bind(id)
                .bind(contact_id)
                .bind(external_id)
                .bind(&now)
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

        sqlx::query("INSERT INTO lead_data_quality_issues (id, lead_contact_id, issue_type, severity, details_json, status, created_at) VALUES ('warning-a', 'contact-a', 'UNKNOWN_PRODUCT', 'WARNING', '{}', 'OPEN', ?)")
            .bind(&now)
            .execute(database.pool())
            .await
            .expect("insert warning");

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
        assert_eq!(rows[0].warning_count, 1);
        assert_eq!(rows[0].product_codes, vec!["FUE_PUNCHES"]);
    }
}
