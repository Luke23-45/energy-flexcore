// P1.17 — `/metrics` route handler.
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode};
use axum::response::IntoResponse;
use crate::state::AppState;

/// Serves the Prometheus text exposition of the request/error counters.
pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
        )],
        state.metrics.render(),
    )
}
