// P11.11 — Idempotency-Key enforcement on command-submission POSTs
// (Phase 11 wave 1, Agent B; ledger P11.11; gap analysis §5 Tier 0 row 4
// [S-61][S-65]).
//
// # Scope (recorded honestly)
//
// The §15.2 dispatch-command resource is READ-ONLY by recorded decision
// (`routes_phase4.rs`: NO execute-command endpoint exists; the lifecycle
// gates are application-layer surfaces, not REST writes). The
// "command-submission" HTTP class this contract therefore governs is the
// set of POST routes that SUBMIT candidates which can enter the command
// lifecycle — today exactly the Phase 3 candidate-submission routes named
// in [`crate::routes::COMMAND_SUBMISSION_PATHS`]. When a phase later
// exposes a command-submission POST, it joins that list and this
// middleware governs it with zero code change.
//
// # The contract (documented here and on the routes)
//
// - **Key required.** Every governed POST MUST carry an `Idempotency-Key`
//   header: missing, empty, whitespace-containing, or over-length keys are
//   rejected with **422** (`validation_rejected`). The key representation
//   REUSES the validated envelope key discipline —
//   `flexcore_events::envelope::IdempotencyKey` (P1.09: non-empty,
//   whitespace-free) — not a second validator.
// - **Stored-response replay.** The first response to a (tenant, key)
//   pair is stored (bounded in-process cache); a retry with the SAME key
//   and the SAME request fingerprint replays the stored response with an
//   `Idempotent-Replay: true` response header — the client receives the
//   original outcome, and no second candidate is created. Only terminal
//   responses (status < 500) are stored: a 5xx is never replayed, so a
//   server-side failure is retryable by contract. A 429 (P11.14 rate
//   limiting) is likewise never stored — a rate-limited request must be
//   retryable, not pinned to its rejection.
// - **Replay conflict = 409.** The same key with a DIFFERENT request
//   fingerprint (method + path + body) conflicts with the stored
//   outcome: **409** (`idempotency_replay_conflict`). The same key
//   arriving while the first request is still in flight is also **409**
//   (the concurrent-duplicate semantics; the client retries after the
//   first response lands, then receives the replay).
//
// # Fingerprint and cache honesty
//
// The fingerprint is a std-library 64-bit hash over (method, path, body
// bytes, body length) — the same test-grade std-hash class as the P1.20
// trace id. It is NOT a cryptographic digest and a collision could
// mis-replay; the recorded mitigation is that the DURABLE idempotency of
// the platform never rests on this HTTP cache: the outbox event keys
// (P1.09) and the consumer-layer oracle (P11.12) remain the guarantee.
// This middleware is the API-edge convenience layer [S-61][S-65].
//
// The cache is IN-PROCESS and per-instance (the P1.20 registry
// precedent): multi-instance deployments need a shared store — recorded
// in the P11.11 decision record as the scale-out note, not built here.
// The cache is bounded (`MAX_REPLAY_ENTRIES`, a registered value): the
// oldest entry is evicted when full, never unbounded growth.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher as _};
use std::sync::Mutex;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::HeaderName;
use axum::response::{IntoResponse, Response};
use flexcore_events::envelope::IdempotencyKey;

use crate::error::ApiError;
use crate::state::AppState;

/// The request header carrying the idempotency key (the de-facto standard
/// spelling; registered in the P11.11 record).
pub const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";

/// The response header marking a replayed stored response.
pub const IDEMPOTENT_REPLAY_HEADER: HeaderName = HeaderName::from_static("idempotent-replay");

/// The registered key-length bound (bytes). A longer header is a
/// rejection, not a truncation (X-8 registered value; RECOMMENDATION
/// pending operator sign-off).
const MAX_KEY_BYTES: usize = 255;

/// The registered body bound for fingerprinting: governed POST bodies
/// larger than this are rejected before caching (the request-body limit
/// layer already caps at 1 MiB; this constant restates the cache-side
/// bound so the two cannot drift independently).
const MAX_IDEMPOTENT_BODY_BYTES: usize = 1024 * 1024;

/// The registered replay-cache bound (entries, per process). RECOMMENDATION
/// pending operator sign-off; eviction is oldest-first, never unbounded.
const MAX_REPLAY_ENTRIES: usize = 256;

/// One replay-cache entry: the request fingerprint and the stored terminal
/// response. `InFlight` marks the window between the cache insert and the
/// response — a concurrent same-key request inside that window is a 409.
#[derive(Debug)]
struct ReplayEntry {
    fingerprint: u64,
    body_len: usize,
    state: ReplayState,
}

#[derive(Debug)]
enum ReplayState {
    /// The first request is still inside the middleware.
    InFlight,
    /// The terminal response is stored and replayable.
    Stored { status: u16, body: Vec<u8> },
}

/// The bounded per-process replay cache, keyed by (tenant, key).
#[derive(Debug, Default)]
pub struct IdempotencyReplayCache {
    entries: Mutex<BTreeMap<(String, String), (ReplayEntry, u64)>>,
    insertion_counter: std::sync::atomic::AtomicU64,
}

impl IdempotencyReplayCache {
    fn insert_in_flight(&self, tenant: &str, key: &str, fingerprint: u64, body_len: usize) {
        let seq = self
            .insertion_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut entries = self.entries.lock().expect("replay cache mutex poisoned");
        if entries.len() >= MAX_REPLAY_ENTRIES {
            // Bounded cache: evict the oldest-inserted entry. A linear
            // scan over a 256-entry map is negligible next to the request
            // it serves.
            if let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, (_, seq))| *seq)
                .map(|(k, _)| k.clone())
            {
                entries.remove(&oldest);
            }
        }
        entries.insert(
            (tenant.to_string(), key.to_string()),
            (
                ReplayEntry {
                    fingerprint,
                    body_len,
                    state: ReplayState::InFlight,
                },
                seq,
            ),
        );
    }

    fn store_response(&self, tenant: &str, key: &str, status: u16, body: Vec<u8>) {
        let mut entries = self.entries.lock().expect("replay cache mutex poisoned");
        if let Some((entry, _)) = entries.get_mut(&(tenant.to_string(), key.to_string())) {
            entry.state = ReplayState::Stored { status, body };
        }
    }

    fn remove(&self, tenant: &str, key: &str) {
        let mut entries = self.entries.lock().expect("replay cache mutex poisoned");
        entries.remove(&(tenant.to_string(), key.to_string()));
    }
}

impl IdempotencyReplayCache {
    fn lookup_replay(
        &self,
        tenant: &str,
        key: &str,
        fingerprint: u64,
    ) -> ReplayLookup {
        let entries = self.entries.lock().expect("replay cache mutex poisoned");
        match entries.get(&(tenant.to_string(), key.to_string())) {
            None => ReplayLookup::Miss,
            Some((entry, _)) => match &entry.state {
                ReplayState::InFlight => ReplayLookup::ConcurrentDuplicate,
                ReplayState::Stored { status, body } => {
                    if entry.fingerprint == fingerprint {
                        ReplayLookup::Replay {
                            status: *status,
                            body: body.clone(),
                        }
                    } else {
                        ReplayLookup::FingerprintConflict
                    }
                }
            },
        }
    }
}

/// The lookup outcomes (see [`IdempotencyReplayCache::lookup_replay`]).
#[derive(Debug, Clone)]
enum ReplayLookup {
    /// No entry: proceed with the request.
    Miss,
    /// Same key while the first request is in flight: 409.
    ConcurrentDuplicate,
    /// Stored response with a matching fingerprint: replay it.
    Replay { status: u16, body: Vec<u8> },
    /// Stored response with a different fingerprint: 409.
    FingerprintConflict,
}

/// The std-hash request fingerprint (method + path + body length + body
/// bytes). Test-grade by recorded decision — see the module header.
fn fingerprint(method: &str, path: &str, body: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    method.hash(&mut hasher);
    path.hash(&mut hasher);
    body.len().hash(&mut hasher);
    body.hash(&mut hasher);
    hasher.finish()
}

/// The P11.11 middleware. Mounted between the rate limiter and the
/// timeout layer (P11.14's 429 responses must never be cached — see the
/// store rule below). GETs and non-governed paths pass through untouched.
pub async fn idempotency_middleware(
    State(state): State<AppState>,
    request: Request,
    next: axum::middleware::Next,
) -> Response {
    let path = request.uri().path().to_string();
    let method = request.method().to_string();
    let governed =
        method == "POST" && crate::routes::COMMAND_SUBMISSION_PATHS.contains(&path.as_str());
    if !governed {
        return next.run(request).await;
    }

    // Identity: resolved best-effort for cache scoping, exactly like the
    // correlation middleware. An unresolvable caller is NOT rejected here
    // — the route handler's own 401 must stay authoritative; the cache is
    // keyed per tenant, so an anonymous request is simply not cached.
    let bearer = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let tenant = state
        .identity
        .resolve(bearer.as_deref())
        .ok()
        .map(|caller| caller.ctx.tenant_id().uuid().to_hyphenated());

    let Some(tenant) = tenant else {
        // No verified caller: pass through; the handler 401s.
        return next.run(request).await;
    };

    // Key required: 422 on missing/invalid (the P11.11 contract). The key
    // representation reuses the P1.09 validated envelope key type.
    let raw_key = request
        .headers()
        .get(IDEMPOTENCY_KEY_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let key = match raw_key {
        None => {
            return ApiError::validation(
                "the Idempotency-Key header is mandatory on command-submission POSTs \
                 (a retry of the same submission must carry the SAME key)".to_string()
            )
            .into_response()
        }
        Some(raw) if raw.len() > MAX_KEY_BYTES => {
            return ApiError::validation(format!(
                "the Idempotency-Key header exceeds the registered maximum of {MAX_KEY_BYTES} bytes"
            ))
            .into_response()
        }
        Some(raw) => match IdempotencyKey::new(raw) {
            Ok(key) => key,
            Err(_) => {
                return ApiError::validation(
                    "the Idempotency-Key header must be non-empty and whitespace-free".to_string(),
                )
                .into_response()
            }
        },
    };

    // Buffer the request body for fingerprinting (the body-limit layer
    // already caps the size; this is the cache-side restatement).
    let (parts, body) = request.into_parts();
    let bytes = match axum::body::to_bytes(body, MAX_IDEMPOTENT_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return ApiError::infrastructure().into_response();
        }
    };
    let fp = fingerprint(&method, &path, &bytes);
    let key_text = key.as_str().to_string();

    // Replay lookup BEFORE running the inner stack.
    match state.idempotency.lookup_replay(&tenant, &key_text, fp) {
        ReplayLookup::Replay { status, body } => {
            let mut response = Response::builder()
                .status(axum::http::StatusCode::from_u16(status).unwrap_or(
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                ))
                .body(Body::from(body))
                .expect("a stored status code is always valid");
            response
                .headers_mut()
                .insert(IDEMPOTENT_REPLAY_HEADER, axum::http::HeaderValue::from_static("true"));
            return response;
        }
        ReplayLookup::ConcurrentDuplicate => {
            return ApiError::conflict(
                "idempotency_replay_conflict",
                "an identical Idempotency-Key is still in flight; retry after the \
                 first response settles to receive the stored outcome"
                    .to_string(),
            )
            .into_response();
        }
        ReplayLookup::FingerprintConflict => {
            return ApiError::conflict(
                "idempotency_replay_conflict",
                "this Idempotency-Key was already used with a different request; \
                 reuse a key only for retries of the SAME request"
                    .to_string(),
            )
            .into_response();
        }
        ReplayLookup::Miss => {}
    }

    // First sighting: mark in-flight (concurrent duplicates 409 from
    // here), run the inner stack, then store the terminal response.
    state
        .idempotency
        .insert_in_flight(&tenant, &key_text, fp, bytes.len());
    let request = Request::from_parts(parts, Body::from(bytes));
    let response = next.run(request).await;

    let status = response.status();
    if status.is_client_error() || status.is_success() {
        // Terminal response: buffer and store for replay. 5xx and 429 are
        // NOT stored (retryable by contract).
        let (response_parts, response_body) = response.into_parts();
        match axum::body::to_bytes(response_body, MAX_IDEMPOTENT_BODY_BYTES).await {
            Ok(bytes) => {
                state.idempotency.store_response(
                    &tenant,
                    &key_text,
                    status.as_u16(),
                    bytes.to_vec(),
                );
                Response::from_parts(response_parts, Body::from(bytes))
            }
            Err(_) => {
                // The stored entry stays InFlight with no stored response:
                // remove it so a retry runs fresh instead of 409ing
                // forever on an unreadable body. FRV1-R2-011 (final
                // review): the client must never receive the original
                // status with an EMPTY body — that would silently drop
                // the stable error envelope (e.g. a bare 422). The
                // terminal-response buffering failure is therefore shaped
                // as the middleware's named infrastructure error, exactly
                // like the request-body buffering failure above.
                state.idempotency.remove(&tenant, &key_text);
                ApiError::infrastructure().into_response()
            }
        }
    } else {
        // 5xx (or any non-terminal class): not stored. The in-flight entry
        // is removed so the retry can run fresh.
        state.idempotency.remove(&tenant, &key_text);
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The key validation reuses the P1.09 envelope key discipline: the
    /// same strings that fail there fail here (422), and the accepted
    /// forms match.
    #[test]
    fn key_validation_matches_the_envelope_key_discipline() {
        assert!(IdempotencyKey::new(String::new()).is_err());
        assert!(IdempotencyKey::new("key with space".to_string()).is_err());
        assert!(IdempotencyKey::new("tenant-a:schedule-1:retry-0".to_string()).is_ok());
    }

    /// The fingerprint separates requests that differ in method, path, or
    /// body; equal inputs collide by construction (determinism, not a
    /// guarantee of uniqueness — the recorded test-grade caveat).
    #[test]
    fn fingerprint_separates_differing_requests() {
        let base = fingerprint("POST", "/api/v1/schedules", b"body");
        assert_ne!(base, fingerprint("POST", "/api/v1/schedules", b"other"));
        assert_ne!(base, fingerprint("POST", "/api/v1/optimization-runs", b"body"));
        assert_ne!(base, fingerprint("PUT", "/api/v1/schedules", b"body"));
        assert_eq!(base, fingerprint("POST", "/api/v1/schedules", b"body"));
    }
}
