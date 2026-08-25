use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::get,
};
use serde::Serialize;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

#[derive(Debug, Serialize)]
struct ApiErrorResponse {
    error: ApiErrorBody,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorBody {
    code: &'static str,
    message: &'static str,
}

pub fn build_pool(database_url: &str, max_connections: u32) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .connect_lazy(database_url)
}

pub fn router(state: AppState) -> Router {
    let request_id_header = axum::http::HeaderName::from_static("x-request-id");

    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/api/v1", get(api_root))
        .with_state(state)
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
        .layer(TraceLayer::new_for_http())
}

async fn live() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "ertip-lead-manager-server",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn api_root() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "ertip-lead-manager-api-v1",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn ready(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, (StatusCode, Json<ApiErrorResponse>)> {
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
    {
        Ok(1) => Ok(Json(HealthResponse {
            status: "ready",
            service: "ertip-lead-manager-server",
            version: env!("CARGO_PKG_VERSION"),
        })),
        Ok(_) | Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiErrorResponse {
                error: ApiErrorBody {
                    code: "DATABASE_UNAVAILABLE",
                    message: "Database dependency is not ready.",
                },
            }),
        )),
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    use super::{AppState, build_pool, router};

    #[tokio::test]
    async fn liveness_does_not_require_database_round_trip() {
        let pool = build_pool("postgres://user:pass@127.0.0.1:1/app", 1)
            .expect("lazy postgres URL should be valid");
        let response = router(AppState { pool })
            .oneshot(
                Request::builder()
                    .uri("/health/live")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert!(response.headers().contains_key("x-request-id"));
    }

    #[tokio::test]
    async fn api_v1_root_is_explicit() {
        let pool = build_pool("postgres://user:pass@127.0.0.1:1/app", 1)
            .expect("lazy postgres URL should be valid");
        let response = router(AppState { pool })
            .oneshot(
                Request::builder()
                    .uri("/api/v1")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }
}
