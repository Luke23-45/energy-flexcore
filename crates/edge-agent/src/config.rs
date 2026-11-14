// P6.05 — Edge deployment configuration (`EdgeConfig`).
//
// Conventions (same as the control plane's `FLEXCORE_*` scheme, apps/worker
// `config.rs` pattern — reused, not reinvented):
//
// - **Fail fast**: every variable is parsed before any connection is
//   opened; an invalid value aborts startup with a named, human-readable
//   error — never a half-configured edge agent.
// - **No default credentials, no secret logging**: the control-plane
//   endpoint (`FLEXCORE_EDGE_CONTROL_PLANE_ENDPOINT`) can embed credentials
//   in its URL; it is required, and the `Debug` rendering redacts it. The
//   site/tenant identifiers are required — an edge agent without its site
//   and tenant binding has nothing to be scoped to (§16.2; P6.06 binds the
//   issued identity to exactly one tenant and site).
// - **Command path defaults to FALSE** (§2.1 v0.4.0 read-only default;
//   §21.4; P6.14 implements the receiver and its enablement gate later).
//   This wave the flag exists in configuration only, and a `true` value
//   still activates NOTHING: no command-receive code exists in the crate
//   (fail-closed by absence). The runner logs the recorded flag state at
//   startup so the honest behaviour is visible.
// - Every optional variable has a safe, documented default; the defaults
//   tune polling, batching, and buffering bounds only. The buffer bound is
//   the §21.4-style resource ceiling: bounded, never unbounded (the
//   offline-first buffer must not grow without limit; P6.20 records the
//   resource profile).

use std::path::PathBuf;
use std::time::Duration;

/// Required: the site this edge agent is bound to (§16.2). PLACEHOLDER in
/// lab profiles: `pilot-site-A` (ENTRY_NOTE boundary 5 — site naming is a
/// D5 input).
pub const ENV_SITE_ID: &str = "FLEXCORE_EDGE_SITE_ID";
/// Required: the tenant this edge agent is bound to (§16.2; one edge, one
/// tenant/site scope — P6.21 isolates the rest).
pub const ENV_TENANT_ID: &str = "FLEXCORE_EDGE_TENANT_ID";
/// Required: the control-plane endpoint the edge dials OUT to (§3.3:
/// edge-initiated, no inbound OT access; P6.07 makes it mTLS). May embed
/// credentials: never logged.
pub const ENV_CONTROL_PLANE_ENDPOINT: &str = "FLEXCORE_EDGE_CONTROL_PLANE_ENDPOINT";
/// Required: the protocol adapter id this edge runs (P6.12 registry;
/// allowlist-enforced construction — only a registered id composes).
pub const ENV_ADAPTER_ID: &str = "FLEXCORE_EDGE_ADAPTER_ID";
/// Optional: durable buffer directory for store-and-forward (P6.09 owns the
/// disk implementation; this wave the path is recorded in configuration and
/// the runner uses the in-memory bounded buffer). Default `edge-buffer`.
pub const ENV_BUFFER_PATH: &str = "FLEXCORE_EDGE_BUFFER_PATH";
/// Optional: whether the (test-env-gated, P6.14) command path is enabled;
/// default **false**. `true` is accepted and recorded but activates nothing
/// in this wave — no command receiver exists (fail-closed by absence).
pub const ENV_COMMAND_PATH_ENABLED: &str = "FLEXCORE_EDGE_COMMAND_PATH_ENABLED";
/// Optional: EMS poll interval in milliseconds; default 5000 (supervisory
/// minutes-scale operation, §3.5 — not a sub-second control loop).
pub const ENV_POLL_INTERVAL_MS: &str = "FLEXCORE_EDGE_POLL_INTERVAL_MS";
/// Optional: bounded buffer capacity in samples; default 10 000, bounded to
/// 1..=1 000 000. At capacity the runner applies backpressure (stops
/// polling) — never a silent drop, never unbounded growth (§21.4 profile).
pub const ENV_BUFFER_CAPACITY: &str = "FLEXCORE_EDGE_BUFFER_CAPACITY";
/// Optional: uplink forward batch size in samples; default 100, bounded to
/// 1..=10 000.
pub const ENV_FORWARD_BATCH_SIZE: &str = "FLEXCORE_EDGE_FORWARD_BATCH_SIZE";
/// Optional (P6.08): edge deduplication window in samples; default 4 096,
/// bounded to 1..=1 000 000. The window is a BOUNDED optimization against
/// poll-cadence re-reads (the P4.24 identity discipline, cited) — NOT the
/// system's idempotency boundary; out-of-window duplicates are consumed by
/// the platform's P4.24-style idempotent ingest (§15.4).
pub const ENV_DEDUP_WINDOW: &str = "FLEXCORE_EDGE_DEDUP_WINDOW";
/// Optional (P6.09): maximum age a buffered sample may reach before the
/// audited age-retention policy drops it; default 3 600 000 ms (1 hour),
/// bounded to 1..=86 400 000 (24 h). Measured on `ingest_time` (the edge's
/// own receipt time). Drops are counted and logged — never silent.
pub const ENV_BUFFER_MAX_AGE_MS: &str = "FLEXCORE_EDGE_BUFFER_MAX_AGE_MS";
/// Optional (P6.09): the buffer overflow policy. EXACTLY ONE value is
/// accepted this wave: `drop-oldest-audited` (drop the OLDEST record with
/// a hard counter, a latched health flag, and a loud log — never a silent
/// overwrite). Any other value is a named startup error: the ledger
/// requires the policy to be a recorded configuration decision, not an
/// implicit default, and no second policy is implemented — refusing an
/// unknown one is the honest enforcement of that rule.
pub const ENV_OVERFLOW_POLICY: &str = "FLEXCORE_EDGE_OVERFLOW_POLICY";
/// The only implemented overflow-policy value (see [`ENV_OVERFLOW_POLICY`]).
pub const OVERFLOW_POLICY_DROP_OLDEST_AUDITED: &str = "drop-oldest-audited";
/// Optional: log level (`trace|debug|info|warn|error`); default `info`.
pub const ENV_LOG_LEVEL: &str = "FLEXCORE_EDGE_LOG_LEVEL";

// --- P6.06 / P6.07 additive variables (identity, enrollment, uplink TLS) -----
// Wave 2a (P6.06/P6.07) — additive block; the P6.05 variables above are
// untouched and every default here is a safe documented one.

/// Optional: the filesystem credential-store path for the P6.06 device
/// identity (owner-only permissions enforced on Unix; the store refuses to
/// parse a file with group/other bits set). Default
/// `edge-credentials/device-identity.conf`.
pub const ENV_CREDENTIAL_STORE_PATH: &str = "FLEXCORE_EDGE_CREDENTIAL_STORE_PATH";
/// Optional: the SINGLE-USE enrollment provisioning token (P6.06). SECRET —
/// never rendered: `EdgeConfig`'s `Debug` redacts it and audit details never
/// carry it. Absent = this run does not attempt enrollment.
pub const ENV_PROVISIONING_TOKEN: &str = "FLEXCORE_EDGE_PROVISIONING_TOKEN";
/// Optional: the credential rotation window in milliseconds (P6.06: expiry
/// must be visible at least ONE rotation window before expiry). Default
/// 604 800 000 ms = 7 days; bounded to 1..=30 days.
pub const ENV_ROTATION_WINDOW_MS: &str = "FLEXCORE_EDGE_ROTATION_WINDOW_MS";
/// Optional (P6.07): the explicitly pinned TLS server name for the outbound
/// mTLS channel. Absent = the channel does NOT compose (fail closed — trust
/// is never derived from an unpinned name).
pub const ENV_UPLINK_SERVER_NAME: &str = "FLEXCORE_EDGE_UPLINK_SERVER_NAME";
/// Optional (P6.07): the pinned minimum TLS version; `tls13` (default) or
/// `tls12-floor` (opt-in per a site D6 record). The cipher-suite policy is
/// the rustls provider defaults — recorded, never invented here.
pub const ENV_UPLINK_TLS_MINIMUM: &str = "FLEXCORE_EDGE_UPLINK_TLS_MINIMUM";
/// Optional (P6.07): outbound connect timeout in milliseconds; default
/// 10 000, bounded to 1..=300 000.
pub const ENV_UPLINK_CONNECT_TIMEOUT_MS: &str = "FLEXCORE_EDGE_UPLINK_CONNECT_TIMEOUT_MS";
/// Optional (P6.07): reconnect backoff initial wait in milliseconds;
/// default 1 000, bounded to 1..=3 600 000.
pub const ENV_UPLINK_BACKOFF_INITIAL_MS: &str = "FLEXCORE_EDGE_UPLINK_BACKOFF_INITIAL_MS";
/// Optional (P6.07): reconnect backoff cap in milliseconds; default
/// 60 000, bounded to 1..=3 600 000, must be >= the initial wait.
pub const ENV_UPLINK_BACKOFF_MAX_MS: &str = "FLEXCORE_EDGE_UPLINK_BACKOFF_MAX_MS";
/// Optional (P6.07): idle keepalive window in milliseconds; a session idle
/// longer than this is re-established (fresh handshake — no silent half-open
/// reuse). Default 30 000.
pub const ENV_UPLINK_KEEPALIVE_MS: &str = "FLEXCORE_EDGE_UPLINK_KEEPALIVE_MS";

// --- P6.10 additive variables (OpenEMS REST adapter) --------------------------
// Wave 3 (P6.10) — additive block; every default here is fail-closed: the
// adapter selection itself is the REQUIRED `FLEXCORE_EDGE_ADAPTER_ID`
// (P6.12), and selecting `openems-rest` without a base URL aborts
// composition (the composition root refuses, never guesses a URL).

/// Optional (P6.10): the OpenEMS Edge REST base URL for the `openems-rest`
/// adapter (`http://host[:port]`). SECRET-ISH: may embed credentials —
/// redacted in `Debug`, never logged. ONLY the `http://` scheme composes
/// this wave (the TLS client for the REST hop is a recorded open item; a
/// TLS URL is a named startup error, never a downgrade). Absent while
/// `FLEXCORE_EDGE_ADAPTER_ID=openems-rest` → composition refuses
/// (fail-closed: no default URL is invented).
pub const ENV_OPENEMS_BASE_URL: &str = "FLEXCORE_EDGE_OPENEMS_BASE_URL";
/// Optional (P6.10): the versioned OpenEMS channel-mapping profile to
/// load. Default: the shipped `openems-mapping-profile.v1.json` relative to
/// the working directory (see
/// [`OPENEMS_MAPPING_PROFILE_DEFAULT`]). A missing or refused profile
/// aborts startup (fail-closed — the adapter never runs unmapped).
pub const ENV_OPENEMS_MAPPING_PROFILE: &str = "FLEXCORE_EDGE_OPENEMS_MAPPING_PROFILE";
/// The shipped versioned mapping profile (P6.10 step 4). Relative to the
/// working directory of the edge-agent process; the lab run book (P6.11)
/// records how it is located in the deployment.
pub const OPENEMS_MAPPING_PROFILE_DEFAULT: &str =
    "crates/edge-agent/config/openems-mapping-profile.v1.json";


/// Fail-fast configuration errors. `Display` never includes the
/// environment VALUE — the control-plane endpoint can carry credentials
/// (worker `config.rs` rule, applied here from day one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// A required variable was absent.
    MissingEnv { name: &'static str },
    /// A variable was present but unusable.
    InvalidEnv { name: &'static str, reason: String },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::MissingEnv { name } => {
                write!(f, "missing required environment variable {name}")
            }
            ConfigError::InvalidEnv { name, reason } => {
                write!(f, "invalid environment variable {name}: {reason}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// Log level, restricted to the five tracing levels (same shape as the
/// worker's `LogLevel`; the env filter is built from this, not from
/// free-form filter syntax).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }

    fn parse(raw: &str) -> Result<Self, ConfigError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" | "warning" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            other => Err(ConfigError::InvalidEnv {
                name: ENV_LOG_LEVEL,
                reason: format!(
                    "expected one of trace|debug|info|warn|error, got a value that parses as '{other}'"
                ),
            }),
        }
    }
}

/// The buffer overflow policy (P6.09). Exactly ONE policy is implemented;
/// extending the enum is a reviewed change (ledger P6.09 step 2: the
/// policy is a recorded configuration decision, not an implicit default —
/// an unknown `FLEXCORE_EDGE_OVERFLOW_POLICY` value is a named startup
/// error, never a silent fallback).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowPolicy {
    /// Drop the OLDEST buffered record on overflow — audited: a hard
    /// counter, a latched health flag, and a loud log line. Never a silent
    /// overwrite; the drop counter is queryable for P6.17 health.
    DropOldestAudited,
}

impl OverflowPolicy {
    fn parse(raw: &str) -> Result<Self, ConfigError> {
        match raw.trim() {
            OVERFLOW_POLICY_DROP_OLDEST_AUDITED => Ok(OverflowPolicy::DropOldestAudited),
            other => Err(ConfigError::InvalidEnv {
                name: ENV_OVERFLOW_POLICY,
                reason: format!(
                    "unknown overflow policy '{other}'; the only implemented policy is \
                     '{OVERFLOW_POLICY_DROP_OLDEST_AUDITED}' (P6.09: the policy is a recorded \
                     configuration decision — extending it is a reviewed change, not a config edit)"
                ),
            }),
        }
    }
}

/// The complete edge deployment configuration. The control-plane endpoint
/// is redacted in `Debug` (it can embed credentials).
#[derive(Clone)]
pub struct EdgeConfig {
    /// The bound site (§16.2). Identifier, not a secret.
    pub site_id: String,
    /// The bound tenant (§16.2). Identifier, not a secret.
    pub tenant_id: String,
    /// The control-plane endpoint the edge dials out to (§3.3).
    /// Secret-ish: never logged.
    pub control_plane_endpoint: String,
    /// The protocol adapter id (P6.12 registry; the composition validates
    /// it against the registry allowlist before the runner starts).
    pub adapter_id: String,
    /// Buffer directory for the P6.09 disk buffer. Path, not a secret.
    pub buffer_path: PathBuf,
    /// The command-path flag. **Default `false`.** In this wave a `true`
    /// value activates nothing (no receiver exists); the P6.14 gate owns
    /// the real enablement semantics.
    pub command_path_enabled: bool,
    /// EMS poll interval (supervisory cadence; §3.5).
    pub poll_interval: Duration,
    /// Bounded buffer capacity in samples (backpressure at capacity).
    pub buffer_capacity: usize,
    /// Uplink forward batch size in samples.
    pub forward_batch_size: usize,
    /// The edge deduplication window (P6.08; bounded optimization, see the
    /// env-var docs).
    pub dedup_window: usize,
    /// The buffered-sample age bound (P6.09 age retention; audited drops).
    pub buffer_max_age: Duration,
    /// The buffer overflow policy (P6.09; exactly one implemented value).
    pub overflow_policy: OverflowPolicy,
    pub log_level: LogLevel,

    // --- P6.06 / P6.07 additive fields ---
    /// The filesystem credential-store path (P6.06; owner-only permissions
    /// enforced on Unix). Path, not a secret.
    pub credential_store_path: PathBuf,
    /// The single-use provisioning token (P6.06). SECRET: redacted in
    /// `Debug`, never logged, never audited. `None` = not enrolling this run.
    pub provisioning_token: Option<crate::enrollment::ProvisioningToken>,
    /// The rotation window (P6.06): the expiry warning fires when the
    /// remaining validity drops inside this window.
    pub credential_rotation_window: Duration,
    /// The explicitly pinned TLS server name (P6.07). `None` = the mTLS
    /// channel does not compose (fail closed).
    pub uplink_server_name: Option<String>,
    /// The pinned minimum TLS version (P6.07; DEFAULT TLS 1.3).
    pub uplink_tls_minimum: crate::uplink_mtls::TlsMinimum,
    /// Outbound connect timeout (P6.07).
    pub uplink_connect_timeout: Duration,
    /// Reconnect backoff initial wait (P6.07; exponential ×2 to the cap).
    pub uplink_backoff_initial: Duration,
    /// Reconnect backoff cap (P6.07; >= the initial wait).
    pub uplink_backoff_max: Duration,
    /// Idle keepalive window (P6.07; re-establish instead of silent reuse).
    pub uplink_keepalive_interval: Duration,

    // --- P6.10 additive fields ---
    /// The OpenEMS Edge REST base URL for the `openems-rest` adapter
    /// (P6.10). SECRET-ISH: redacted in `Debug`. `None` = not configured;
    /// the composition REFUSES to wire `openems-rest` without it
    /// (fail-closed — no default URL).
    pub openems_base_url: Option<String>,
    /// The versioned OpenEMS channel-mapping profile path (P6.10). Defaults
    /// to the shipped v1 profile (see [`OPENEMS_MAPPING_PROFILE_DEFAULT`]);
    /// a missing or refused profile aborts startup.
    pub openems_mapping_profile_path: PathBuf,
}

impl std::fmt::Debug for EdgeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EdgeConfig")
            .field("site_id", &self.site_id)
            .field("tenant_id", &self.tenant_id)
            .field("control_plane_endpoint", &"<redacted>")
            .field("adapter_id", &self.adapter_id)
            .field("buffer_path", &self.buffer_path)
            .field("command_path_enabled", &self.command_path_enabled)
            .field("poll_interval", &self.poll_interval)
            .field("buffer_capacity", &self.buffer_capacity)
            .field("forward_batch_size", &self.forward_batch_size)
            .field("dedup_window", &self.dedup_window)
            .field("buffer_max_age", &self.buffer_max_age)
            .field("overflow_policy", &self.overflow_policy)
            .field("log_level", &self.log_level)
            .field("credential_store_path", &self.credential_store_path)
            .field(
                "provisioning_token",
                &self.provisioning_token.as_ref().map(|_| "<redacted>"),
            )
            .field("credential_rotation_window", &self.credential_rotation_window)
            .field("uplink_server_name", &self.uplink_server_name)
            .field("uplink_tls_minimum", &self.uplink_tls_minimum)
            .field("uplink_connect_timeout", &self.uplink_connect_timeout)
            .field("uplink_backoff_initial", &self.uplink_backoff_initial)
            .field("uplink_backoff_max", &self.uplink_backoff_max)
            .field("uplink_keepalive_interval", &self.uplink_keepalive_interval)
            // P6.10 — the OpenEMS base URL may embed credentials: redacted.
            .field(
                "openems_base_url",
                &self.openems_base_url.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "openems_mapping_profile_path",
                &self.openems_mapping_profile_path,
            )
            .finish()
    }
}

impl EdgeConfig {
    /// Parses the configuration from the process environment. Fail-fast:
    /// every variable is validated here, before any connection is opened.
    pub fn from_env() -> Result<Self, ConfigError> {
        let site_id = require(ENV_SITE_ID)?;
        let tenant_id = require(ENV_TENANT_ID)?;
        let control_plane_endpoint = require(ENV_CONTROL_PLANE_ENDPOINT)?;
        let adapter_id = require(ENV_ADAPTER_ID)?;
        let buffer_path = match std::env::var(ENV_BUFFER_PATH) {
            Ok(raw) if !raw.trim().is_empty() => PathBuf::from(raw.trim()),
            Ok(_) => PathBuf::from("edge-buffer"),
            Err(_) => PathBuf::from("edge-buffer"),
        };
        let command_path_enabled = match std::env::var(ENV_COMMAND_PATH_ENABLED) {
            Ok(raw) => match raw.trim().to_ascii_lowercase().as_str() {
                "true" => true,
                "false" => false,
                _ => {
                    return Err(ConfigError::InvalidEnv {
                        name: ENV_COMMAND_PATH_ENABLED,
                        reason: "expected 'true' or 'false' (default 'false')".to_string(),
                    })
                }
            },
            Err(_) => false,
        };
        let poll_interval =
            optional_ms(ENV_POLL_INTERVAL_MS, Duration::from_millis(5_000))?;
        let buffer_capacity = optional_parse(ENV_BUFFER_CAPACITY, 10_000usize).and_then(|value| {
            if (1..=1_000_000).contains(&value) {
                Ok(value)
            } else {
                Err(ConfigError::InvalidEnv {
                    name: ENV_BUFFER_CAPACITY,
                    reason: "expected an integer in 1..=1_000_000 samples".to_string(),
                })
            }
        })?;
        let forward_batch_size = optional_parse(ENV_FORWARD_BATCH_SIZE, 100usize).and_then(
            |value| {
                if (1..=10_000).contains(&value) {
                    Ok(value)
                } else {
                    Err(ConfigError::InvalidEnv {
                        name: ENV_FORWARD_BATCH_SIZE,
                        reason: "expected an integer in 1..=10_000 samples".to_string(),
                    })
                }
            },
        )?;
        // --- P6.08 / P6.09 additive parsing (fail-fast with the same rules) ---
        // P6.08: the bounded edge dedup window (bounded optimization, NOT
        // the system's idempotency boundary).
        let dedup_window = optional_parse(ENV_DEDUP_WINDOW, 4_096usize).and_then(|value| {
            if (1..=1_000_000).contains(&value) {
                Ok(value)
            } else {
                Err(ConfigError::InvalidEnv {
                    name: ENV_DEDUP_WINDOW,
                    reason: "expected an integer in 1..=1_000_000 samples".to_string(),
                })
            }
        })?;
        // P6.09: the audited age-retention bound (1 h default; 24 h cap).
        let buffer_max_age = bounded_ms(ENV_BUFFER_MAX_AGE_MS, 3_600_000, 1, 86_400_000)?;
        // P6.09: the recorded overflow policy (exactly one implemented;
        // unknown values refuse at startup, never silently fall back).
        let overflow_policy = match std::env::var(ENV_OVERFLOW_POLICY) {
            Ok(raw) => OverflowPolicy::parse(&raw)?,
            Err(_) => OverflowPolicy::DropOldestAudited,
        };
        let log_level = match std::env::var(ENV_LOG_LEVEL) {
            Ok(raw) => LogLevel::parse(&raw)?,
            Err(_) => LogLevel::Info,
        };
        // --- P6.06 / P6.07 additive parsing (fail-fast with the same rules) ---
        let credential_store_path = match std::env::var(ENV_CREDENTIAL_STORE_PATH) {
            Ok(raw) if !raw.trim().is_empty() => PathBuf::from(raw.trim()),
            Ok(_) => PathBuf::from("edge-credentials/device-identity.conf"),
            Err(_) => PathBuf::from("edge-credentials/device-identity.conf"),
        };
        let provisioning_token = match std::env::var(ENV_PROVISIONING_TOKEN) {
            Ok(raw) if !raw.trim().is_empty() => {
                Some(crate::enrollment::ProvisioningToken::new(raw).map_err(|error| {
                    ConfigError::InvalidEnv {
                        name: ENV_PROVISIONING_TOKEN,
                        reason: error.to_string(),
                    }
                })?)
            }
            Ok(_) => Err(ConfigError::InvalidEnv {
                name: ENV_PROVISIONING_TOKEN,
                reason: "set but empty".to_string(),
            })?,
            Err(_) => None,
        };
        let credential_rotation_window =
            optional_ms(ENV_ROTATION_WINDOW_MS, Duration::from_millis(604_800_000)).and_then(
                |duration| {
                    if duration.as_millis() > 30 * 86_400_000 {
                        Err(ConfigError::InvalidEnv {
                            name: ENV_ROTATION_WINDOW_MS,
                            reason: "the rotation window must be at most 30 days".to_string(),
                        })
                    } else {
                        Ok(duration)
                    }
                },
            )?;
        let uplink_server_name = match std::env::var(ENV_UPLINK_SERVER_NAME) {
            Ok(raw) if !raw.trim().is_empty() => Some(raw.trim().to_string()),
            Ok(_) => Err(ConfigError::InvalidEnv {
                name: ENV_UPLINK_SERVER_NAME,
                reason: "set but empty".to_string(),
            })?,
            Err(_) => None,
        };
        let uplink_tls_minimum = match std::env::var(ENV_UPLINK_TLS_MINIMUM) {
            Ok(raw) => crate::uplink_mtls::TlsMinimum::parse(&raw).map_err(|error| {
                ConfigError::InvalidEnv {
                    name: ENV_UPLINK_TLS_MINIMUM,
                    reason: error.to_string(),
                }
            })?,
            Err(_) => crate::uplink_mtls::TlsMinimum::DEFAULT,
        };
        let uplink_connect_timeout = bounded_ms(
            ENV_UPLINK_CONNECT_TIMEOUT_MS,
            10_000,
            1,
            300_000,
        )?;
        let uplink_backoff_initial = bounded_ms(
            ENV_UPLINK_BACKOFF_INITIAL_MS,
            1_000,
            1,
            3_600_000,
        )?;
        let uplink_backoff_max = bounded_ms(ENV_UPLINK_BACKOFF_MAX_MS, 60_000, 1, 3_600_000)
            .and_then(|millis| {
                if millis < uplink_backoff_initial {
                    Err(ConfigError::InvalidEnv {
                        name: ENV_UPLINK_BACKOFF_MAX_MS,
                        reason: "the backoff cap must be >= the initial wait".to_string(),
                    })
                } else {
                    Ok(millis)
                }
            })?;
        let uplink_keepalive_interval = bounded_ms(ENV_UPLINK_KEEPALIVE_MS, 30_000, 1, 3_600_000)?;
        // --- P6.10 additive parsing (fail-fast with the same rules) ---
        // The OpenEMS REST base URL: ONLY 'http://' composes this wave (the
        // REST-hop TLS client is a recorded open item — a TLS URL is a
        // named startup error, never a downgrade). Absent = None; the
        // composition root refuses to wire `openems-rest` without it.
        let openems_base_url = match std::env::var(ENV_OPENEMS_BASE_URL) {
            Ok(raw) if !raw.trim().is_empty() => {
                let trimmed = raw.trim().to_string();
                validate_openems_base_url(&trimmed)?;
                Some(trimmed)
            }
            Ok(_) => Err(ConfigError::InvalidEnv {
                name: ENV_OPENEMS_BASE_URL,
                reason: "set but empty".to_string(),
            })?,
            Err(_) => None,
        };
        // The versioned mapping profile: defaults to the shipped v1 file
        // (relative to the working directory); a missing/refused file
        // aborts startup in the composition (fail-closed).
        let openems_mapping_profile_path = match std::env::var(ENV_OPENEMS_MAPPING_PROFILE) {
            Ok(raw) if !raw.trim().is_empty() => PathBuf::from(raw.trim()),
            Ok(_) | Err(_) => PathBuf::from(OPENEMS_MAPPING_PROFILE_DEFAULT),
        };
        Ok(Self {
            site_id,
            tenant_id,
            control_plane_endpoint,
            adapter_id,
            buffer_path,
            command_path_enabled,
            poll_interval,
            buffer_capacity,
            forward_batch_size,
            dedup_window,
            buffer_max_age,
            overflow_policy,
            log_level,
            credential_store_path,
            provisioning_token,
            credential_rotation_window,
            uplink_server_name,
            uplink_tls_minimum,
            uplink_connect_timeout,
            uplink_backoff_initial,
            uplink_backoff_max,
            uplink_keepalive_interval,
            openems_base_url,
            openems_mapping_profile_path,
        })
    }
}

fn require(name: &'static str) -> Result<String, ConfigError> {
    match std::env::var(name) {
        Ok(raw) if !raw.trim().is_empty() => Ok(raw.trim().to_string()),
        Ok(_) => Err(ConfigError::InvalidEnv {
            name,
            reason: "set but empty".to_string(),
        }),
        Err(_) => Err(ConfigError::MissingEnv { name }),
    }
}

fn optional_parse<T: std::str::FromStr>(
    name: &'static str,
    default: T,
) -> Result<T, ConfigError> {
    match std::env::var(name) {
        Ok(raw) => raw.trim().parse::<T>().map_err(|_| ConfigError::InvalidEnv {
            name,
            reason: format!("expected a value parsable as {}", std::any::type_name::<T>()),
        }),
        Err(_) => Ok(default),
    }
}

fn optional_ms(name: &'static str, default: Duration) -> Result<Duration, ConfigError> {
    let millis = optional_parse(name, default.as_millis() as u64).and_then(|value| {
        if value == 0 {
            Err(ConfigError::InvalidEnv {
                name,
                reason: "expected a positive number of milliseconds".to_string(),
            })
        } else {
            Ok(value)
        }
    })?;
    Ok(Duration::from_millis(millis))
}

/// P6.06/P6.07 helper: an optional millisecond variable with an explicit
/// inclusive bound (`min..=max`), so every P6.07 tuning value fails fast
/// on nonsense instead of degrading silently.
fn bounded_ms(
    name: &'static str,
    default_millis: u64,
    min: u64,
    max: u64,
) -> Result<Duration, ConfigError> {
    let millis = optional_parse(name, default_millis).and_then(|value| {
        if !(min..=max).contains(&value) {
            Err(ConfigError::InvalidEnv {
                name,
                reason: format!("expected a number of milliseconds in {min}..={max}"),
            })
        } else {
            Ok(value)
        }
    })?;
    Ok(Duration::from_millis(millis))
}

/// P6.10 helper: the OpenEMS base URL scheme gate. ONLY `http://host[:port]`
/// composes this wave — the TLS client for the REST hop is a recorded open
/// item (decision record §6), so any other scheme is a named startup error,
/// never a silent downgrade. The VALUE is never echoed into the error (it
/// may embed credentials — the config redaction rule).
fn validate_openems_base_url(value: &str) -> Result<(), ConfigError> {
    const SCHEME: &str = "http://";
    if !value.starts_with(SCHEME) {
        return Err(ConfigError::InvalidEnv {
            name: ENV_OPENEMS_BASE_URL,
            reason: format!(
                "only the '{SCHEME}' scheme composes this wave (the TLS client for the \
                 OpenEMS REST hop is a recorded open item); any other scheme is refused \
                 fail-closed, never downgraded"
            ),
        });
    }
    let rest = &value[SCHEME.len()..];
    if rest.is_empty() || rest.contains('/') {
        return Err(ConfigError::InvalidEnv {
            name: ENV_OPENEMS_BASE_URL,
            reason: "expected 'http://host[:port]' with no path or credentials".to_string(),
        });
    }
    if let Some((_, port)) = rest.rsplit_once(':') {
        if port.parse::<u16>().is_err() {
            return Err(ConfigError::InvalidEnv {
                name: ENV_OPENEMS_BASE_URL,
                reason: "the port is not a valid u16".to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE (run-status honesty): these tests are written under the recorded
    // loop bans (ENTRY_NOTE.md — nothing built, nothing run). Env-var tests
    // assert the parse logic shape; the required-variable cases are
    // exercised in tests/config_and_runner.rs through a serialized
    // environment lock.

    #[test]
    fn missing_required_variable_is_a_named_error() {
        let error = ConfigError::MissingEnv { name: ENV_SITE_ID };
        let rendered = error.to_string();
        assert!(rendered.contains("FLEXCORE_EDGE_SITE_ID"));
        assert!(!rendered.contains(':') || !rendered.contains("value"));
    }

    #[test]
    fn log_level_parses_and_rejects() {
        assert_eq!(LogLevel::parse("INFO").unwrap(), LogLevel::Info);
        assert_eq!(LogLevel::parse(" warn ").unwrap(), LogLevel::Warn);
        assert!(LogLevel::parse("verbose").is_err());
    }

    // --- P6.10: the OpenEMS base URL scheme gate ----------------------------

    #[test]
    fn openems_base_url_accepts_http_and_refuses_other_schemes() {
        assert!(validate_openems_base_url("http://lab-edge:8084").is_ok());
        assert!(validate_openems_base_url("http://lab-edge").is_ok());
        // A TLS URL is refused fail-closed (never a downgrade).
        assert!(validate_openems_base_url("https://lab-edge").is_err());
        assert!(validate_openems_base_url("ftp://lab-edge").is_err());
        // No path, no credentials, no bad port.
        assert!(validate_openems_base_url("http://lab-edge/rest").is_err());
        assert!(validate_openems_base_url("http://user:secret@host").is_err());
        assert!(validate_openems_base_url("http://host:notaport").is_err());
        // The error never echoes the value.
        let error = validate_openems_base_url("https://lab-edge").unwrap_err();
        assert!(!error.to_string().contains("lab-edge"));
    }
}
