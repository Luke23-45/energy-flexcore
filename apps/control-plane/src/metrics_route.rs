// P1.17 — `/metrics` route handler.
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode};
use axum::response::IntoResponse;
use crate::state::AppState;

/// Serves the Prometheus text exposition of the request/error counters.
///
/// FRV1-R2-006 (final review): the P11.14 rate-limit rejection counters
/// render here too, appended after the P1.20 registry body under their
/// own P11.01-amended families (the same single `/metrics` surface — no
/// second metrics endpoint).
pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    let mut body = state.metrics.render();
    body.push_str(&state.rate_limit.render_rejection_metrics());
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
        )],
        body,
    )
}
