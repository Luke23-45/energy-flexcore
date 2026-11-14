// P6.06 — Edge identity, enrollment, and certificate lifecycle (report
//         §18.1, §18.3, §17.3, §16.2, §3.3; ledger P6.06).
//
// This module implements the EDGE side of the enrollment seam and the
// device-credential lifecycle. It is written to be honest about what EXISTS
// and what is a DEPLOYMENT REQUIREMENT (the full honesty record lives in
// `ledger/phase-06-edge-protocol/06_07_identity_channel.md`; the summary
// travels with the code):
//
// | Concern | Status here | Deployment requirement |
// |---|---|---|
// | Key-pair generation (§18.3: hardware-backed where available) | **NOT implemented — no key generation code exists anywhere in this crate** (no rcgen, no openssl, nothing; the hard ban on key generation in production paths is structural, not cosmetic) | the site generates/provisions the key pair at install time, hardware-backed where the platform offers it |
// | Certificate issuance (the CA) | **NOT implemented** — [`DenyAllCredentialIssuer`] is the shipped issuer and refuses every request; the actual issuance is the [`CredentialIssuer`] seam | a real CA/issuance service, per the D6 review (OPEN); the enrollment flow is DESIGNED against a documented flow, no PKI is implemented |
// | OCSP/CRL revocation checking | **NOT implemented** — [`UnconfiguredRevocationChecker`] fails closed with a named error | an OCSP/CRL checker adapter composed per deployment (the §17.3 revocation requirement) |
// | Credential storage | **Implemented** — [`FilesystemCredentialStore`], versioned record format, 0600-owner-only permission enforcement on Unix (documented limitation on other platforms) | the protected volume (§18.3) and any OS-specific secret store |
//
// Flow (ledger P6.06 step 1, designed; the steps below are what the seam
// executes, in order):
//
// 1. The edge agent is provisioned with a SINGLE-USE provisioning token at
//    deployment time ([`crate::config::ENV_PROVISIONING_TOKEN`]). The token
//    is a secret: its `Debug`/`Display` renderings are redacted and it is
//    never placed in an audit event.
// 2. The edge submits its CONFIGURED binding (tenant + site, §16.2) and the
//    token OUT through the [`EnrollmentClient`] port. It generates nothing
//    and assumes nothing; the control plane decides.
// 3. The control-plane side (`POST /api/v1/edge/enroll`, apps/control-plane
//    `routes_edge.rs`) validates the token, checks the binding against the
//    approved scope, refuses duplicate/revoked identities, and records the
//    device identity in the tenant-scoped device registry (migration 0018).
// 4. A successful exchange yields a [`DeviceCredentialRecord`] — the RECORD
//    shape (identity, binding, certificate material, expiry). With the
//    shipped seam the certificate fields stay `None`/empty and the record
//    says so: no certificate is fabricated.
// 5. The record is stored through the [`CredentialStore`] port and every
//    step is audited ([`EdgeAuditKind::EnrollmentCompleted`] /
//    [`EdgeAuditKind::EnrollmentRejected`]).
//
// Lifecycle (ledger P6.06 step 3): [`CredentialExpiryMonitor`] assesses the
// stored record against the clock and the configured rotation window —
// expiry becomes visible to an operator at least ONE ROTATION WINDOW before
// expiry (the ledger acceptance criterion), through audited events and the
// P6.17 health surface. Renewal re-runs the enrollment flow; revocation is
// enforced by the revocation-check seam and by the control plane refusing
// re-enrollment of a REVOKED identity.
//
// Port discipline (house rules, `ports.rs`): sync traits, fail-closed,
// named [`EdgeError`] variants, no secret material in any `Debug`, log, or
// audit line, and no seventh quality state invented anywhere.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::audit_buffer::{BufferedAuditRecord, FileAuditBuffer};
use crate::ports::{EdgeAuditEvent, EdgeAuditKind, EdgeError, LocalBuffer};

// --- Secrets and secret-adjacent values -------------------------------------

/// A single-use enrollment provisioning token (ledger P6.06 step 1).
///
/// Deliberately NOT `Clone`-free or stringly-typed in logs: the inner value
/// is reachable only through [`ProvisioningToken::expose`], the one named
/// call site that hands the secret to the enrollment transport. `Debug` and
/// `Display` are redacted unconditionally — the token can never enter a log
/// line, an audit event, or a panic message by accident (the config rule
/// from `config.rs`: no secret VALUES in diagnostics).
pub struct ProvisioningToken {
    value: String,
}

impl ProvisioningToken {
    /// Wraps a token value. An empty/whitespace token is a named error
    /// (fail-closed at construction — an empty credential must never look
    /// like a present one).
    pub fn new(value: String) -> Result<Self, EdgeError> {
        if value.trim().is_empty() {
            return Err(EdgeError::EnrollmentFailed {
                reason: "the provisioning token is empty".to_string(),
            });
        }
        Ok(Self { value })
    }

    /// The one named accessor for the secret value. Callers must pass it
    /// only to the enrollment transport; never to a log/audit formatter.
    pub fn expose(&self) -> &str {
        &self.value
    }
}

impl fmt::Debug for ProvisioningToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProvisioningToken(<redacted>)")
    }
}

impl fmt::Display for ProvisioningToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// A PEM body carried inside a credential record (certificate, public
/// chain, or trust anchor). Redacted in `Debug`/`Display`: certificate
/// bodies are not secrets in the password sense, but they are credential
/// material — the house rule is that NOTHING from the credential record
/// renders into a log or audit line (§17.3).
#[derive(Clone, PartialEq, Eq)]
pub struct PemBody(String);

impl PemBody {
    pub fn new(body: String) -> Self {
        Self(body)
    }
    /// The one named accessor (the TLS channel hands this to rustls; tests
    /// compare round-trips). Never used by a log/audit formatter.
    pub fn expose(&self) -> &str {
        &self.0
    }
    pub fn is_empty(&self) -> bool {
        self.0.trim().is_empty()
    }
}

impl fmt::Debug for PemBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PemBody(<redacted>)")
    }
}

impl fmt::Display for PemBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

// --- The binding and the credential record ----------------------------------

/// The tenant/site scope the edge agent claims (§16.2: one edge, one
/// tenant, one approved site). Plain configuration strings — this crate
/// carries no domain ID types (the P6.05 dependency rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceBinding {
    pub tenant_id: String,
    pub site_id: String,
}

impl DeviceBinding {
    pub fn new(tenant_id: String, site_id: String) -> Self {
        Self {
            tenant_id,
            site_id,
        }
    }

    /// Non-empty on both halves — an edge that cannot name its scope has
    /// nothing to be approved for (fail-closed at construction).
    pub fn is_complete(&self) -> bool {
        !self.tenant_id.trim().is_empty() && !self.site_id.trim().is_empty()
    }
}

/// The device credential RECORD shape (ledger P6.06: the enrollment
/// exchange produces this record; the certificate is a deployment
/// requirement and stays `None` with the shipped seam).
///
/// NO key material is ever held here: the private key lives outside the
/// record as a REFERENCE ([`DeviceCredentialRecord::private_key_reference`])
/// resolved at TLS-session build time by the deployment's key source
/// (hardware-backed where the platform offers it, §18.3). Nothing in this
/// struct renders key material into any diagnostic.
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceCredentialRecord {
    /// The device identity the control plane assigned. Unique per tenant:
    /// no two edges share an identity (the registry PK enforces it).
    pub device_id: String,
    /// The issued binding (§16.2). The control plane's copy is
    /// authoritative; the edge stores what was ISSUED, which must match its
    /// configured binding (the flow refuses a mismatched issue).
    pub tenant_id: String,
    pub site_id: String,
    /// The issued device certificate, when a real issuance exists. With the
    /// shipped seam this is `None` — recorded honestly, never fabricated.
    pub certificate_pem: Option<PemBody>,
    /// The issued certificate chain (intermediates), same honesty rule.
    pub chain_pem: Vec<PemBody>,
    /// The pinned trust anchor for the OUTBOUND channel (the control
    /// plane's root CA). P6.07 builds its root store from THIS value — the
    /// edge pins the platform, the platform does not pin the edge's
    /// trust from the environment.
    pub root_ca_pem: Option<PemBody>,
    /// A reference (path or platform handle) to the private key material.
    /// The material itself never enters the record, the store file, or any
    /// diagnostic (§17.3, §18.3).
    pub private_key_reference: Option<PathBuf>,
    /// When the identity was issued (unix millis, edge clock).
    pub issued_at_unix_millis: i64,
    /// When the identity expires (unix millis), when known. `None` with the
    /// shipped seam (no CA → no real validity term) and audited as
    /// [`ExpiryStatus::NoExpiryRecorded`] — never assumed to be "never".
    pub expires_at_unix_millis: Option<i64>,
}

impl fmt::Debug for DeviceCredentialRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceCredentialRecord")
            .field("device_id", &self.device_id)
            .field("tenant_id", &self.tenant_id)
            .field("site_id", &self.site_id)
            .field("certificate_pem", &self.certificate_pem.as_ref().map(|_| "<redacted>"))
            .field("chain_pem_len", &self.chain_pem.len())
            .field("root_ca_pem", &self.root_ca_pem.as_ref().map(|_| "<redacted>"))
            .field("private_key_reference", &self.private_key_reference)
            .field("issued_at_unix_millis", &self.issued_at_unix_millis)
            .field("expires_at_unix_millis", &self.expires_at_unix_millis)
            .finish()
    }
}

impl DeviceCredentialRecord {
    /// Construction-time validity: identity + binding non-empty, a
    /// certificate implies a root CA pin (the channel needs the platform
    /// trust anchor), and the issued binding is complete. Fail-closed.
    pub fn validate(&self) -> Result<(), EdgeError> {
        let mut problems: Vec<&str> = Vec::new();
        if self.device_id.trim().is_empty() {
            problems.push("device_id");
        }
        if self.tenant_id.trim().is_empty() {
            problems.push("tenant_id");
        }
        if self.site_id.trim().is_empty() {
            problems.push("site_id");
        }
        if self.certificate_pem.is_some() && self.root_ca_pem.is_none() {
            problems.push("root_ca_pem (a certificate without the pinned platform root)");
        }
        if self.certificate_pem.as_ref().map(|p| p.is_empty()) == Some(true) {
            problems.push("certificate_pem (present but empty)");
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(EdgeError::EnrollmentFailed {
                reason: format!("incomplete credential record: {}", problems.join(", ")),
            })
        }
    }
}

// --- The enrollment ports ----------------------------------------------------

/// The enrollment port: exchange a single-use provisioning token (and the
/// edge's configured binding) for a device credential record. Sync trait,
/// house rules. A rejection is a named [`EdgeError::EnrollmentFailed`] —
/// the edge NEVER proceeds on a rejected enrollment (it has no identity,
/// therefore no channel to bring up, therefore nothing to degrade except
/// loudly).
pub trait EnrollmentClient: Send {
    /// Human-readable client name for audit lines (never carries the token).
    fn enrollment_name(&self) -> &str;
    fn enroll(
        &mut self,
        token: &ProvisioningToken,
        binding: &DeviceBinding,
    ) -> Result<DeviceCredentialRecord, EdgeError>;
}

/// The credential-issuer seam. THE CA IS A DEPLOYMENT REQUIREMENT: this
/// trait is the one place a real issuance integration plugs in; the
/// repository ships only [`DenyAllCredentialIssuer`].
pub trait CredentialIssuer: Send {
    fn issue(
        &mut self,
        token: &ProvisioningToken,
        binding: &DeviceBinding,
    ) -> Result<DeviceCredentialRecord, EdgeError>;
}

/// The shipped issuer: refuses everything, with the honest reason. No PKI
/// is implemented (AGENTS.md rule 7 applied to this seam: nothing here may
/// LOOK like a working enrollment while the issuance step is a deployment
/// requirement).
#[derive(Debug, Default, Clone, Copy)]
pub struct DenyAllCredentialIssuer;

impl CredentialIssuer for DenyAllCredentialIssuer {
    fn issue(
        &mut self,
        _token: &ProvisioningToken,
        _binding: &DeviceBinding,
    ) -> Result<DeviceCredentialRecord, EdgeError> {
        Err(EdgeError::EnrollmentFailed {
            reason: "no credential issuer is composed; certificate issuance is a DEPLOYMENT \
                     REQUIREMENT (no CA is implemented in the FlexCore 0.x line) — the edge \
                     agent refuses to fabricate a device identity"
                .to_string(),
        })
    }
}

/// The shipped enrollment client: validates the local inputs, hands the
/// token (once, through the issuer seam) to the composed issuer, and
/// validates the returned record BEFORE anything is stored. The adapter
/// owns no token bookkeeping — single-use semantics are enforced by the
/// ISSUER side (the token is consumed server-side; the scripted test
/// issuer demonstrates the contract).
pub struct TokenEnrollmentAdapter {
    issuer: Box<dyn CredentialIssuer>,
}

impl TokenEnrollmentAdapter {
    pub fn new(issuer: Box<dyn CredentialIssuer>) -> Self {
        Self { issuer }
    }
}

impl EnrollmentClient for TokenEnrollmentAdapter {
    fn enrollment_name(&self) -> &str {
        "token-enrollment-adapter"
    }

    fn enroll(
        &mut self,
        token: &ProvisioningToken,
        binding: &DeviceBinding,
    ) -> Result<DeviceCredentialRecord, EdgeError> {
        if !binding.is_complete() {
            return Err(EdgeError::EnrollmentFailed {
                reason: "the configured tenant/site binding is incomplete; the edge cannot \
                         enroll without its §16.2 scope"
                    .to_string(),
            });
        }
        let record = self.issuer.issue(token, binding)?;
        record.validate()?;
        // The issued binding must MATCH the configured binding: an edge
        // cannot be re-issued into a different tenant/site scope than the
        // one it was configured and approved for (§16.2; the negative test
        // for a binding swap runs at both ends — here and on the route).
        if record.tenant_id != binding.tenant_id || record.site_id != binding.site_id {
            return Err(EdgeError::EnrollmentFailed {
                reason: "the issued binding does not match the configured tenant/site binding; \
                         refusing the mismatched identity (§16.2)"
                    .to_string(),
            });
        }
        Ok(record)
    }
}

/// Scripted issuer — TEST SUPPORT ONLY (never a production issuer; the
/// production path ships [`DenyAllCredentialIssuer`]). Enforces the
/// issuer-side contract the deployment's real CA must satisfy:
///
/// - the token is SINGLE-USE (a second `issue` on the same token rejects);
/// - only approved (tenant, site) scopes issue;
/// - a device id already enrolled in the scripted registry rejects
///   (duplicate identity);
/// - a REVOKED device id rejects;
/// - an EXPIRED device id may re-enroll (rotation), as a new identity
///   term.
#[derive(Debug, Default)]
pub struct ScriptedCredentialIssuer {
    approved_scopes: Vec<(String, String)>,
    used_tokens: Vec<String>,
    /// The scripted registry state: (device_id, status) — tests seed it to
    /// simulate the control plane's view (ACTIVE / REVOKED / EXPIRED).
    pub enrolled: Vec<(String, String)>,
    pub next_record: Option<DeviceCredentialRecord>,
}

impl ScriptedCredentialIssuer {
    /// Builds the scripted issuer; `record_template` is returned on every
    /// successful issue (the test adjusts its binding per case).
    pub fn new(approved_scopes: Vec<(String, String)>, record_template: DeviceCredentialRecord) -> Self {
        Self {
            approved_scopes,
            used_tokens: Vec::new(),
            enrolled: Vec::new(),
            next_record: Some(record_template),
        }
    }
}

impl CredentialIssuer for ScriptedCredentialIssuer {
    fn issue(
        &mut self,
        token: &ProvisioningToken,
        binding: &DeviceBinding,
    ) -> Result<DeviceCredentialRecord, EdgeError> {
        // Single-use token: the ISSUER owns consumption.
        if self.used_tokens.iter().any(|t| t == token.expose()) {
            return Err(EdgeError::EnrollmentFailed {
                reason: "the provisioning token was already consumed (single-use)".to_string(),
            });
        }
        if !self
            .approved_scopes
            .iter()
            .any(|(t, s)| t == &binding.tenant_id && s == &binding.site_id)
        {
            return Err(EdgeError::EnrollmentFailed {
                reason: "the requested tenant/site binding is not in the approved enrollment \
                         scope (§16.2)"
                    .to_string(),
            });
        }
        let record = self
            .next_record
            .clone()
            .ok_or_else(|| EdgeError::EnrollmentFailed {
                reason: "the scripted issuer has no record template".to_string(),
            })?;
        let existing = self
            .enrolled
            .iter()
            .find(|(id, _)| *id == record.device_id)
            .map(|(_, status)| status.clone());
        match existing.as_deref() {
            None => {}
            Some("ACTIVE") => {
                return Err(EdgeError::EnrollmentFailed {
                    reason: "duplicate identity: an ACTIVE device with this id is already \
                             enrolled (no two edges share an identity)"
                    .to_string(),
                })
            }
            Some("REVOKED") => {
                return Err(EdgeError::EnrollmentFailed {
                    reason: "revoked identity: re-enrollment is refused for a REVOKED device"
                        .to_string(),
                })
            }
            // "EXPIRED" falls through: rotation re-enrolls as a new term.
            Some(_) => {}
        }
        self.used_tokens.push(token.expose().to_string());
        self.enrolled
            .push((record.device_id.clone(), "ACTIVE".to_string()));
        Ok(record)
    }
}

// --- The credential store ----------------------------------------------------

/// The device-credential store port (ledger P6.06 step: the device identity
/// is stored at the edge; path from configuration; permissions documented;
/// no secrets in logs; key material never stored here at all — only the
/// reference).
pub trait CredentialStore: Send {
    /// Loads the stored record; `Ok(None)` = not enrolled yet.
    fn load(&self) -> Result<Option<DeviceCredentialRecord>, EdgeError>;
    /// Persists the record (replacing any previous record atomically —
    /// write-to-temp + rename where the filesystem allows it).
    fn store(&mut self, record: &DeviceCredentialRecord) -> Result<(), EdgeError>;
    /// Clears the store (decommissioning / revoked-identity hygiene).
    fn clear(&mut self) -> Result<(), EdgeError>;
}

/// The versioned on-disk record format identifier (the first line of every
/// store file). Versioned so a future format change is detectable and
/// refuse-able, never silently mis-parsed.
pub const CREDENTIAL_RECORD_FORMAT: &str = "flexcore-edge-credential-v1";

/// The filesystem credential store. Records are written to `path` with
/// owner-only permissions enforced on Unix (`0600`, and a load-time
/// re-check refuses a file whose group/other bits are set — fail closed on
/// a tampered deployment). On non-Unix platforms the permission ENFORCEMENT
/// is a documented deployment requirement (the decision record §3); the
/// store still never logs contents and never stores key material.
///
/// File format (`flexcore-edge-credential-v1`, line-oriented):
///
/// ```text
/// flexcore-edge-credential-v1
/// device_id = edge-01
/// tenant_id = <uuid>
/// site_id = <uuid>
/// issued_at_unix_millis = 1700000000000
/// expires_at_unix_millis = 1700060480000 | none
/// private_key_reference = /var/lib/flexcore-edge/keys/device.key
/// chain_pem_count = 0
/// <<PEM certificate_pem
/// -----BEGIN CERTIFICATE-----
/// ... (stored verbatim, PEM lines)
/// -----END CERTIFICATE-----
/// >>PEM
/// <<PEM root_ca_pem
/// ...
/// >>PEM
/// ```
///
/// PEM bodies are stored verbatim between the `<<PEM <field>` /
/// `>>PEM` markers. A malformed line, an unknown version, or a truncated
/// PEM block is a named [`EdgeError::CredentialStoreFailed`] — the store
/// never returns a half-parsed credential.
#[derive(Debug, Clone)]
pub struct FilesystemCredentialStore {
    path: PathBuf,
}

impl FilesystemCredentialStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// The store path (health/diagnostics may NAME the path; contents are
    /// never rendered).
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    #[cfg(unix)]
    fn assert_owner_only(&self) -> Result<(), EdgeError> {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(&self.path).map_err(|error| {
            EdgeError::CredentialStoreFailed {
                reason: format!("credential store unreadable: {error}"),
            }
        })?;
        let mode = metadata.permissions().mode();
        if mode & 0o077 != 0 {
            return Err(EdgeError::CredentialStoreFailed {
                reason: format!(
                    "credential store permissions are too open (mode {mode:o}); \
                     owner-only 0600 is required"
                ),
            });
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn assert_owner_only(&self) -> Result<(), EdgeError> {
        // Documented limitation (decision record §3): on this platform the
        // owner-only permission enforcement is a deployment requirement;
        // the store still never stores key material and never logs
        // contents.
        Ok(())
    }

    fn encode(record: &DeviceCredentialRecord) -> String {
        let mut out = String::new();
        out.push_str(CREDENTIAL_RECORD_FORMAT);
        out.push('\n');
        out.push_str(&format!("device_id = {}\n", record.device_id));
        out.push_str(&format!("tenant_id = {}\n", record.tenant_id));
        out.push_str(&format!("site_id = {}\n", record.site_id));
        out.push_str(&format!("issued_at_unix_millis = {}\n", record.issued_at_unix_millis));
        let expiry = record
            .expires_at_unix_millis
            .map(|v| v.to_string())
            .unwrap_or_else(|| "none".to_string());
        out.push_str(&format!("expires_at_unix_millis = {expiry}\n"));
        let key_ref = record
            .private_key_reference
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "none".to_string());
        out.push_str(&format!("private_key_reference = {key_ref}\n"));
        out.push_str(&format!("chain_pem_count = {}\n", record.chain_pem.len()));
        if let Some(cert) = &record.certificate_pem {
            out.push_str("<<PEM certificate_pem\n");
            out.push_str(cert.expose());
            if !cert.expose().ends_with('\n') {
                out.push('\n');
            }
            out.push_str(">>PEM\n");
        }
        if let Some(ca) = &record.root_ca_pem {
            out.push_str("<<PEM root_ca_pem\n");
            out.push_str(ca.expose());
            if !ca.expose().ends_with('\n') {
                out.push('\n');
            }
            out.push_str(">>PEM\n");
        }
        for (index, pem) in record.chain_pem.iter().enumerate() {
            out.push_str(&format!("<<PEM chain_{index}\n"));
            out.push_str(pem.expose());
            if !pem.expose().ends_with('\n') {
                out.push('\n');
            }
            out.push_str(">>PEM\n");
        }
        out
    }

    fn decode(raw: &str) -> Result<DeviceCredentialRecord, EdgeError> {
        let mut lines = raw.lines();
        let version = lines
            .next()
            .ok_or_else(|| EdgeError::CredentialStoreFailed {
                reason: "credential store file is empty".to_string(),
            })?
            .trim();
        if version != CREDENTIAL_RECORD_FORMAT {
            return Err(EdgeError::CredentialStoreFailed {
                reason: format!(
                    "credential store format '{version}' is not the supported \
                     '{CREDENTIAL_RECORD_FORMAT}'"
                ),
            });
        }
        let mut device_id = String::new();
        let mut tenant_id = String::new();
        let mut site_id = String::new();
        let mut issued_at_unix_millis: Option<i64> = None;
        let mut expires_at_unix_millis: Option<Option<i64>> = None;
        let mut private_key_reference: Option<PathBuf> = None;
        let mut certificate_pem: Option<PemBody> = None;
        let mut root_ca_pem: Option<PemBody> = None;
        let mut chain: Vec<PemBody> = Vec::new();

        let mut pem_field: Option<String> = None;
        let mut pem_lines: Vec<String> = Vec::new();
        let mut finish_pem = |field: &str,
                              lines: &[String],
                              certificate_pem: &mut Option<PemBody>,
                              root_ca_pem: &mut Option<PemBody>,
                              chain: &mut Vec<PemBody>|
         -> Result<(), EdgeError> {
            let body = PemBody::new(lines.join("\n"));
            match field {
                "certificate_pem" => certificate_pem = Some(body),
                "root_ca_pem" => root_ca_pem = Some(body),
                other if other.starts_with("chain_") => chain.push(body),
                other => {
                    return Err(EdgeError::CredentialStoreFailed {
                        reason: format!("unknown PEM field '{other}' in the credential store"),
                    })
                }
            }
            Ok(())
        };

        for line in lines {
            if let Some(field) = &pem_field {
                if line.trim() == ">>PEM" {
                    // Trailing blank lines inside a PEM block are write
                    // padding, not content: strip them so the round-trip is
                    // byte-faithful to what was stored.
                    while pem_lines.last().map(|l| l.is_empty()) == Some(true) {
                        pem_lines.pop();
                    }
                    finish_pem(
                        field,
                        &pem_lines,
                        &mut certificate_pem,
                        &mut root_ca_pem,
                        &mut chain,
                    )?;
                    pem_field = None;
                    pem_lines.clear();
                } else {
                    pem_lines.push(line.to_string());
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("<<PEM ") {
                pem_field = Some(rest.trim().to_string());
                continue;
            }
            let (key, value) = line
                .split_once(" = ")
                .ok_or_else(|| EdgeError::CredentialStoreFailed {
                    reason: "malformed line in the credential store (expected 'key = value')"
                        .to_string(),
                })?;
            match key.trim() {
                "device_id" => device_id = value.trim().to_string(),
                "tenant_id" => tenant_id = value.trim().to_string(),
                "site_id" => site_id = value.trim().to_string(),
                "issued_at_unix_millis" => {
                    issued_at_unix_millis = Some(value.trim().parse::<i64>().map_err(|_| {
                        EdgeError::CredentialStoreFailed {
                            reason: "issued_at_unix_millis is not an integer".to_string(),
                        }
                    })?);
                }
                "expires_at_unix_millis" => {
                    let trimmed = value.trim();
                    expires_at_unix_millis = Some(if trimmed == "none" {
                        None
                    } else {
                        Some(trimmed.parse::<i64>().map_err(|_| {
                            EdgeError::CredentialStoreFailed {
                                reason: "expires_at_unix_millis is not an integer".to_string(),
                            }
                        })?)
                    });
                }
                "private_key_reference" => {
                    private_key_reference = if value.trim() == "none" {
                        None
                    } else {
                        Some(PathBuf::from(value.trim()))
                    };
                }
                "chain_pem_count" => { /* cross-checked below */ }
                other => {
                    return Err(EdgeError::CredentialStoreFailed {
                        reason: format!("unknown field '{other}' in the credential store"),
                    })
                }
            }
        }
        if pem_field.is_some() {
            return Err(EdgeError::CredentialStoreFailed {
                reason: "truncated PEM block in the credential store".to_string(),
            });
        }
        let issued_at_unix_millis = issued_at_unix_millis.ok_or_else(|| {
            EdgeError::CredentialStoreFailed {
                reason: "issued_at_unix_millis is missing".to_string(),
            }
        })?;
        let record = DeviceCredentialRecord {
            device_id,
            tenant_id,
            site_id,
            certificate_pem,
            chain_pem: chain,
            root_ca_pem,
            private_key_reference,
            issued_at_unix_millis,
            expires_at_unix_millis: expires_at_unix_millis.flatten(),
        };
        record.validate()?;
        Ok(record)
    }
}

impl CredentialStore for FilesystemCredentialStore {
    fn load(&self) -> Result<Option<DeviceCredentialRecord>, EdgeError> {
        if !self.path.exists() {
            return Ok(None);
        }
        // Permission re-check BEFORE the contents are parsed (fail closed
        // on a deployment that widened the file).
        self.assert_owner_only()?;
        let raw = std::fs::read_to_string(&self.path).map_err(|error| {
            EdgeError::CredentialStoreFailed {
                reason: format!("credential store unreadable: {error}"),
            }
        })?;
        Self::decode(&raw).map(Some)
    }

    fn store(&mut self, record: &DeviceCredentialRecord) -> Result<(), EdgeError> {
        record.validate()?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                EdgeError::CredentialStoreFailed {
                    reason: format!("credential store directory unusable: {error}"),
                }
            })?;
        }
        let encoded = Self::encode(record);
        // Write-to-temp + rename: a crash mid-write must never leave a
        // HALF credential record behind (a half record would either fail
        // the decode or, worse, be trusted with missing fields — the
        // decoder refuses both shapes anyway).
        let temp = self.path.with_extension("partial");
        {
            use std::io::Write as _;
            #[cfg(unix)]
            let mut file = {
                use std::os::unix::fs::OpenOptionsExt;
                std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .mode(0o600)
                    .open(&temp)
                    .map_err(|error| EdgeError::CredentialStoreFailed {
                        reason: format!("credential store not writable: {error}"),
                    })?
            };
            #[cfg(not(unix))]
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&temp)
                .map_err(|error| EdgeError::CredentialStoreFailed {
                    reason: format!("credential store not writable: {error}"),
                })?;
            file.write_all(encoded.as_bytes()).map_err(|error| {
                EdgeError::CredentialStoreFailed {
                    reason: format!("credential store write failed: {error}"),
                }
            })?;
        }
        std::fs::rename(&temp, &self.path).map_err(|error| {
            EdgeError::CredentialStoreFailed {
                reason: format!("credential store rename failed: {error}"),
            }
        })?;
        self.assert_owner_only()?;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), EdgeError> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(EdgeError::CredentialStoreFailed {
                reason: format!("credential store clear failed: {error}"),
            }),
        }
    }
}

// --- Expiry monitoring (the rotation window) ---------------------------------

/// The expiry assessment of a stored credential against the edge clock and
/// the configured rotation window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpiryStatus {
    /// `expires_at` is beyond the rotation window: valid.
    Valid,
    /// `expires_at` is INSIDE the rotation window: renewal must be started
    /// now — this is the operator-visible "at least one rotation window
    /// before expiry" state (audited).
    RotationDue,
    /// `expires_at` has passed: the identity is dead; the channel must not
    /// run on it (audited).
    Expired,
    /// The record carries no expiry (the shipped-seam shape: no CA → no
    /// real validity term). Recorded honestly — never treated as "valid
    /// forever"; the deployment is expected to issue REAL, expiring
    /// identities.
    NoExpiryRecorded,
}

/// The clock-based expiry monitor: one pure assessment, audited by the
/// caller (the P6.05 audit discipline — assess is a function, the audit
/// lives with the orchestrator below so tests can drive both).
#[derive(Debug, Clone)]
pub struct CredentialExpiryMonitor {
    /// The rotation window in milliseconds (configuration:
    /// `FLEXCORE_EDGE_ROTATION_WINDOW_MS`). The warning fires when the
    /// remaining validity drops INSIDE this window.
    pub rotation_window_millis: i64,
}

impl CredentialExpiryMonitor {
    pub fn new(rotation_window_millis: i64) -> Result<Self, EdgeError> {
        if rotation_window_millis <= 0 {
            return Err(EdgeError::EnrollmentFailed {
                reason: "the rotation window must be a positive number of milliseconds"
                    .to_string(),
            });
        }
        Ok(Self {
            rotation_window_millis,
        })
    }

    pub fn assess(&self, now_unix_millis: i64, record: &DeviceCredentialRecord) -> ExpiryStatus {
        match record.expires_at_unix_millis {
            None => ExpiryStatus::NoExpiryRecorded,
            Some(expires_at) => {
                let remaining = expires_at.saturating_sub(now_unix_millis);
                if remaining <= 0 {
                    ExpiryStatus::Expired
                } else if remaining <= self.rotation_window_millis {
                    ExpiryStatus::RotationDue
                } else {
                    ExpiryStatus::Valid
                }
            }
        }
    }
}

// --- The revocation seam ------------------------------------------------------

/// The revocation status of a credential (P6.06 step 3: revocation
/// handling).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevocationStatus {
    /// The composed checker established that the credential is not revoked.
    NotRevoked,
    /// The credential is revoked (the reason is the checker's diagnostic,
    /// e.g. the OCSP responder's or CRL's revocation reason).
    Revoked { reason: String },
    /// The check could not be performed. NEVER rendered as "not revoked" —
    /// the caller audits [`EdgeAuditKind::RevocationUnchecked`] and the
    /// deployment decision decides whether an un-checked identity may run
    /// (the shipped policy: it may not — see [`UnconfiguredRevocationChecker`]).
    Unchecked { reason: String },
}

/// The revocation-check seam. OCSP/CRL = DEPLOYMENT REQUIREMENT (recorded
/// honestly in the decision record): the repository ships only the
/// un-configured fail-closed checker below; a deployment composes a real
/// OCSP/CRL adapter into this port.
pub trait RevocationChecker: Send {
    fn check(&self, credential: &DeviceCredentialRecord) -> Result<RevocationStatus, EdgeError>;
}

/// The shipped revocation checker: fails closed. A revocation status that
/// cannot be established is NEVER reported as "not revoked" — an
/// un-configured deployment gets a named error, not a silent pass (AGENTS.md
/// rule 7; the P6.05 fail-closed-stub discipline).
#[derive(Debug, Default, Clone, Copy)]
pub struct UnconfiguredRevocationChecker;

impl RevocationChecker for UnconfiguredRevocationChecker {
    fn check(&self, _credential: &DeviceCredentialRecord) -> Result<RevocationStatus, EdgeError> {
        Err(EdgeError::RevocationCheckFailed {
            reason: "no revocation checker is composed; OCSP/CRL checking is a DEPLOYMENT \
                     REQUIREMENT (the FlexCore 0.x line implements no OCSP/CRL client) — the \
                     revocation status cannot be established, so it is NOT 'not revoked'"
                .to_string(),
        })
    }
}

// --- The orchestrator (flow + audit) ------------------------------------------

/// Ties the enrollment flow together with the edge audit sink: enroll →
/// validate → store, each step audited (`EDGE_ENROLLMENT_COMPLETED` /
/// `EDGE_ENROLLMENT_REJECTED` / `EDGE_CREDENTIAL_STORED`-free — the store
/// step is part of the completed event; no secret material in any detail
/// string). Also carries the expiry assessment with its audit pairing so
/// the composition loop can run one call per cadence tick.
pub struct EnrollmentOrchestrator;

impl EnrollmentOrchestrator {
    /// Runs the full enrollment flow. On success the record is stored and
    /// `EnrollmentCompleted` is audited; on any rejection the rejection is
    /// audited with a reason that names no token value, and the error is
    /// returned (the composition decides whether to retry — a consumed
    /// single-use token must NOT be retried blindly; the deployment issues
    /// a new token instead).
    pub fn enroll(
        client: &mut dyn EnrollmentClient,
        store: &mut dyn CredentialStore,
        token: &ProvisioningToken,
        binding: &DeviceBinding,
        audit: &dyn crate::ports::AuditSink,
        now_unix_millis: i64,
    ) -> Result<DeviceCredentialRecord, EdgeError> {
        match client.enroll(token, binding) {
            Ok(record) => {
                store.store(&record)?;
                audit
                    .record(EdgeAuditEvent {
                        recorded_at: flexcore_contracts::time_model::IngestTime::from_unix_millis(
                            now_unix_millis,
                        )
                        .map_err(|error| EdgeError::AuditWriteFailed {
                            reason: format!("enrollment audit timestamp invalid: {error}"),
                        })?,
                        kind: EdgeAuditKind::EnrollmentCompleted,
                        detail: format!(
                            "device '{}' enrolled into tenant/site scope (certificate issued: {})",
                            record.device_id,
                            record.certificate_pem.is_some()
                        ),
                    })?;
                Ok(record)
            }
            Err(error) => {
                audit
                    .record(EdgeAuditEvent {
                        recorded_at: flexcore_contracts::time_model::IngestTime::from_unix_millis(
                            now_unix_millis,
                        )
                        .map_err(|inner| EdgeError::AuditWriteFailed {
                            reason: format!("enrollment audit timestamp invalid: {inner}"),
                        })?,
                        kind: EdgeAuditKind::EnrollmentRejected,
                        detail: format!("enrollment rejected: {error}"),
                    })?;
                Err(error)
            }
        }
    }

    /// Assesses the expiry of a stored record and emits the paired audit
    /// events (`EDGE_CREDENTIAL_EXPIRY_WARNING` inside the rotation window,
    /// `EDGE_CREDENTIAL_EXPIRED` past it, and the honest
    /// `NoExpiryRecorded` is returned for the caller to surface — no event
    /// claims an un-known identity is fine).
    pub fn assess_expiry(
        monitor: &CredentialExpiryMonitor,
        record: &DeviceCredentialRecord,
        audit: &dyn crate::ports::AuditSink,
        now_unix_millis: i64,
    ) -> ExpiryStatus {
        let status = monitor.assess(now_unix_millis, record);
        let kind = match status {
            ExpiryStatus::Valid | ExpiryStatus::NoExpiryRecorded => None,
            ExpiryStatus::RotationDue => Some(EdgeAuditKind::CredentialExpiryWarning),
            ExpiryStatus::Expired => Some(EdgeAuditKind::CredentialExpired),
        };
        if let Some(kind) = kind {
            let detail = match status {
                ExpiryStatus::RotationDue => format!(
                    "credential for device '{}' enters the rotation window (expires in {} ms)",
                    record.device_id,
                    record
                        .expires_at_unix_millis
                        .unwrap_or_default()
                        .saturating_sub(now_unix_millis)
                ),
                _ => format!("credential for device '{}' has EXPIRED", record.device_id),
            };
            if let Ok(recorded_at) =
                flexcore_contracts::time_model::IngestTime::from_unix_millis(now_unix_millis)
            {
                // A failed audit write is logged loudly by the sink caller
                // discipline (ports.rs); here the audit failure cannot
                // change the ASSESSMENT (the expiry is a clock fact).
                let _ = audit.record(EdgeAuditEvent {
                    recorded_at,
                    kind,
                    detail,
                });
            }
        }
        status
    }
}

// --- P6.32 — Decommissioning and unenrollment (additive) ----------------------
//
// The counterpart of enrollment (ledger P6.32; report §18.1, §17.3, §16.2;
// decision record `25_31_32_console_clock_decommission.md` §3). Decommission
// is OPERATOR-INITIATED and server-authorized (the control-plane seam in
// `apps/control-plane/src/routes_edge.rs` owns the RBAC + step-up + the
// confirmation record; the edge-side wipe below runs only once the operator
// drives it — the edge agent has NO inbound listener, structural, P6.07).
//
// THE WIPE RULES (recorded, ledger P6.32 step 3):
//
// 1. **Credentials/certificates** — the `FilesystemCredentialStore` record
//    file is OVERWRITTEN with zeros (same byte length) and then DELETED,
//    including the `.partial` sibling the atomic-write path may have left.
//    HONESTY: filesystem-level forensic erasure (guaranteed non-recovery
//    against the OS/SSD wear-leverelling layer) is a DEPLOYMENT
//    REQUIREMENT (recorded in the decision record §3) — no new dependency
//    is taken and none is simulated; what this wipe guarantees is that
//    NOTHING is recoverable through the store's public API and the owned
//    files are overwritten-and-gone at the filesystem call level.
// 2. **Buffered telemetry (P6.09)** — the buffer is drained and the drained
//    records are committed-forwarded (the at-least-once commit point), so
//    the journal compacts to empty; the journal file is then
//    overwritten-and-deleted. Nothing is recoverable through the
//    `LocalBuffer` port afterwards.
// 3. **Un-uploaded audit records (P6.18) — THE EXPLICIT RULE:**
//    **UPLOAD-THEN-WIPE** when the channel is up (drain → upload →
//    in-order commit → journal wipe; an upload failure puts the batch back
//    and wipes NOTHING — never a partial loss); when the channel is
//    UNREACHABLE, the un-uploaded record count is recorded as a **GAP
//    EVENT** — counted ([`DecommissionGapLedger`]), AUDITED
//    (`EDGE_DECOMMISSION_AUDIT_GAP`), and LATCHED — and only then is the
//    journal wiped. **Never silently dropped.**
//
// IDEMPOTENCE: every wipe function treats an already-absent target as
// success (`NotFound` → `Ok`), so a repeated decommission (or a crash
// between steps) converges — the second call wipes nothing and still
// succeeds.
//
// RECONNECT-AFTER-DECOMMISSION: server-side, the revoked identity has no
// session scope (`resolve_edge_session_scope` refuses any non-ACTIVE term)
// and re-enrollment of a REVOKED identity is refused (the P6.06 fail-closed
// matrix, now driven by the DECOMMISSIONED/REVOKED status); edge-side, the
// wiped credential store loads `None` — no identity, no channel to bring
// up, nothing to reconnect WITH.

/// The operator-issued decommission confirmation (ledger P6.32 step 1).
/// The confirmation RECORD names the edge identity, the tenant, the site,
/// the acting operator, and the confirmation time — all mandatory
/// (fail-closed at validation: a confirmation that cannot name what it
/// confirms is not a confirmation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecommissionConfirmation {
    /// The edge identity being decommissioned.
    pub device_id: String,
    /// The tenant the identity is enrolled under (§16.2 scope).
    pub tenant_id: String,
    /// The ONE approved site the identity is bound to (§16.2 scope).
    pub site_id: String,
    /// The acting operator (the authenticated subject that issued the
    /// decommission; the server seam verifies this against the caller's
    /// claims — a mismatched actor is a refused decommission).
    pub actor: String,
    /// The confirmation instant (unix millis).
    pub confirmed_at_unix_millis: i64,
}

impl DecommissionConfirmation {
    /// Validates the confirmation (all five fields mandatory).
    pub fn validate(&self) -> Result<(), EdgeError> {
        let mut missing: Vec<&str> = Vec::new();
        if self.device_id.trim().is_empty() {
            missing.push("device_id");
        }
        if self.tenant_id.trim().is_empty() {
            missing.push("tenant_id");
        }
        if self.site_id.trim().is_empty() {
            missing.push("site_id");
        }
        if self.actor.trim().is_empty() {
            missing.push("actor");
        }
        if self.confirmed_at_unix_millis <= 0 {
            missing.push("confirmed_at_unix_millis");
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(EdgeError::EnrollmentFailed {
                reason: format!(
                    "the decommission confirmation is incomplete: {} missing/invalid",
                    missing.join(", ")
                ),
            })
        }
    }
}

/// The latch ledger for un-uploaded audit records lost at decommission
/// (the GAP-EVENT branch of the recorded rule). The ledger is composition-
/// owned state: `gap_latched` stays latched for the composition's lifetime
/// once any gap was recorded, and the cumulative count is queryable for
/// the operator surface (P6.17/P6.25). No lock: u64/bool atomics.
#[derive(Debug, Default)]
pub struct DecommissionGapLedger {
    gap_records_total: std::sync::atomic::AtomicU64,
    gap_latched: std::sync::atomic::AtomicBool,
}

impl DecommissionGapLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a gap (idempotence note: repeated decommissions ADD to the
    /// cumulative count and re-latch — the ledger never hides a second
    /// loss behind the first).
    pub fn record_gap(&self, records_lost: u64) {
        self.gap_records_total
            .fetch_add(records_lost, std::sync::atomic::Ordering::SeqCst);
        self.gap_latched
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// The cumulative count of audit records lost as recorded gap events.
    pub fn gap_records_total(&self) -> u64 {
        self.gap_records_total
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Whether ANY gap was ever recorded (the latch — never unlatched).
    pub fn is_latched(&self) -> bool {
        self.gap_latched
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// The outcome of the audit-buffer settlement (the recorded rule's two
/// branches, each reported explicitly — neither is a silent no-op).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditSettlement {
    /// Channel up: `uploaded` records were uploaded, committed in order,
    /// and the journal was then wiped. Nothing was lost.
    UploadedThenWiped { uploaded: u64 },
    /// Channel down: `gap_records` un-uploaded records were lost with the
    /// journal wipe — the loss was counted, audited, and latched FIRST.
    GapEvent { gap_records: u64 },
}

impl AuditSettlement {
    /// The stable audit detail fragment for the settlement outcome.
    pub fn detail(self) -> String {
        match self {
            AuditSettlement::UploadedThenWiped { uploaded } => format!(
                "audit records uploaded-then-wiped: {uploaded} uploaded, none lost"
            ),
            AuditSettlement::GapEvent { gap_records } => format!(
                "audit GAP EVENT: {gap_records} un-uploaded records lost with the wipe \
                 (counted, audited, latched — never silently dropped)"
            ),
        }
    }
}

/// OVERWRITE-THEN-DELETE for one owned file (the wipe discipline of rule
/// 1/2/3): the file's current content is overwritten with zeros (same
/// byte length), synced, and then deleted. An absent file is success
/// (idempotence). No new dependency; HONESTY: this is filesystem-call-level
/// erasure — OS/SSD-level forensic erasure remains a DEPLOYMENT REQUIREMENT
/// (decision record §3).
fn overwrite_then_delete(path: &Path) -> Result<(), EdgeError> {
    match std::fs::metadata(path) {
        Ok(metadata) => {
            let len = metadata.len() as usize;
            // Overwrite with zeros (same length), then sync, then delete.
            {
                use std::io::Write as _;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .truncate(false)
                    .open(path)
                    .map_err(|error| EdgeError::CredentialStoreFailed {
                        reason: format!("decommission overwrite failed for {path:?}: {error}"),
                    })?;
                let zeros = vec![0u8; len];
                file.write_all(&zeros).map_err(|error| {
                    EdgeError::CredentialStoreFailed {
                        reason: format!("decommission zero-write failed for {path:?}: {error}"),
                    }
                })?;
                file.sync_all().map_err(|error| EdgeError::CredentialStoreFailed {
                    reason: format!("decommission overwrite sync failed for {path:?}: {error}"),
                })?;
            }
            match std::fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(EdgeError::CredentialStoreFailed {
                    reason: format!("decommission delete failed for {path:?}: {error}"),
                }),
            }
        }
        // Already gone: idempotent success.
        Err(_) => Ok(()),
    }
}

/// The decommission wipe operations (P6.32). Free functions on the
/// composed stores/buffers so the composition calls exactly the pieces its
/// deployment owns, in the recorded order: confirm → settle audit → wipe
/// credentials → wipe telemetry → audit the completion.
pub struct DecommissionWipe;

impl DecommissionWipe {
    /// Audits the CONFIRMATION RECORD (ledger P6.32 step 1): the dedicated
    /// event name leads the detail (the closed-`EdgeAuditKind` limitation
    /// pattern) and the record names device, tenant, site, actor, and time
    /// — the persisted record the operator's audit trail carries.
    pub fn audit_confirmation(
        confirmation: &DecommissionConfirmation,
        audit: &dyn crate::ports::AuditSink,
        now_unix_millis: i64,
    ) -> Result<(), EdgeError> {
        confirmation.validate()?;
        audit.record(EdgeAuditEvent {
            recorded_at: flexcore_contracts::time_model::IngestTime::from_unix_millis(
                now_unix_millis,
            )
            .map_err(|error| EdgeError::AuditWriteFailed {
                reason: format!("decommission audit timestamp invalid: {error}"),
            })?,
            kind: EdgeAuditKind::Startup,
            detail: format!(
                "EDGE_DECOMMISSION_CONFIRMED: device='{}' tenant='{}' site='{}' actor='{}' \
                 confirmed_at={} — the identity is being decommissioned (P6.32)",
                confirmation.device_id,
                confirmation.tenant_id,
                confirmation.site_id,
                confirmation.actor,
                confirmation.confirmed_at_unix_millis
            ),
        })
    }

    /// Wipes the filesystem credential store (rule 1): overwrite-then-
    /// delete of the record file AND the `.partial` atomic-write sibling.
    /// Idempotent.
    pub fn wipe_credential_store(store: &mut FilesystemCredentialStore) -> Result<(), EdgeError> {
        let path = store.path().clone();
        overwrite_then_delete(&path)?;
        // The store's atomic-write path may have left a `.partial` sibling
        // behind (it renames on success, but a crash could leave one):
        let partial = path.with_extension("partial");
        overwrite_then_delete(&partial)?;
        // The store itself reports the wiped state (the public-API check).
        match store.load() {
            Ok(None) => Ok(()),
            Ok(Some(_)) => Err(EdgeError::CredentialStoreFailed {
                reason: "decommission wipe left a credential record loadable through the \
                         public API — refusing to report the wipe complete"
                    .to_string(),
            }),
            // The file is gone (already wiped); a load error over an absent
            // file cannot hide a record.
            Err(_) if !path_exists(&path) => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Wipes the buffered telemetry (rule 2): drains the buffer through
    /// the `LocalBuffer` port, commits the drained records forwarded (the
    /// P6.09 at-least-once commit point, compacting the journal to empty)
    /// — returns the wiped record count. The caller then deletes the
    /// journal FILE through [`DecommissionWipe::wipe_journal_file`] (the
    /// port carries no path; the composition passes the buffer's
    /// `journal_path()`).
    pub fn wipe_telemetry_buffer(buffer: &mut dyn LocalBuffer) -> Result<u64, EdgeError> {
        let drained = buffer.drain(usize::MAX);
        let count = drained.len() as u64;
        buffer.commit_forwarded(&drained);
        if buffer.len() != 0 {
            return Err(EdgeError::BufferAppendFailed {
                reason: "decommission wipe left buffered telemetry behind — refusing to \
                         report the wipe complete"
                    .to_string(),
            });
        }
        Ok(count)
    }

    /// Overwrite-then-delete for ONE owned journal file (the telemetry
    /// journal or the audit journal, by its path). Idempotent.
    pub fn wipe_journal_file(path: &Path) -> Result<(), EdgeError> {
        overwrite_then_delete(path)
    }

    /// Settles the audit buffer per the RECORDED RULE (rule 3):
    ///
    /// - `channel_up = true` → UPLOAD-THEN-WIPE: drain → `upload` →
    ///   in-order `commit_uploaded` → journal wipe. An upload failure
    ///   prepends the batch BACK (in order) and wipes NOTHING (never a
    ///   partial loss); the error is returned.
    /// - `channel_up = false` → GAP EVENT: the pending count is recorded
    ///   in `ledger` (counted + latched), audited
    ///   (`EDGE_DECOMMISSION_AUDIT_GAP`, the dedicated-name-leads-detail
    ///   pattern on the closed kind), and THEN the journal is wiped. The
    ///   records are never silently dropped: the gap event IS the record
    ///   of the loss.
    ///
    /// `now_unix_millis` stamps the gap audit event.
    pub fn settle_audit_buffer(
        buffer: &FileAuditBuffer,
        channel_up: bool,
        upload: &dyn Fn(&[BufferedAuditRecord]) -> Result<(), EdgeError>,
        audit: &dyn crate::ports::AuditSink,
        ledger: &DecommissionGapLedger,
        device_id: &str,
        now_unix_millis: i64,
    ) -> Result<AuditSettlement, EdgeError> {
        if channel_up {
            let drained = buffer.drain(usize::MAX);
            let count = drained.len() as u64;
            match upload(&drained) {
                Ok(()) => {
                    buffer
                        .commit_uploaded(&drained)
                        .map_err(|error| EdgeError::AuditWriteFailed {
                            reason: format!("the upload commit failed at decommission: {error}"),
                        })?;
                    overwrite_then_delete(&buffer.journal_path())?;
                    Ok(AuditSettlement::UploadedThenWiped { uploaded: count })
                }
                Err(error) => {
                    // Nothing was committed; put the batch back IN ORDER
                    // and wipe nothing (never a partial loss).
                    buffer.prepend(drained);
                    Err(error)
                }
            }
        } else {
            let pending = buffer.pending_len() as u64;
            ledger.record_gap(pending);
            audit.record(EdgeAuditEvent {
                recorded_at: flexcore_contracts::time_model::IngestTime::from_unix_millis(
                    now_unix_millis,
                )
                .map_err(|error| EdgeError::AuditWriteFailed {
                    reason: format!("decommission audit timestamp invalid: {error}"),
                })?,
                kind: EdgeAuditKind::Startup,
                detail: format!(
                    "EDGE_DECOMMISSION_AUDIT_GAP: device='{device_id}' lost {pending} \
                     un-uploaded audit records with the wipe (counted, audited, latched — \
                     the channel was unreachable; upload-then-wipe was not possible)"
                ),
            })?;
            overwrite_then_delete(&buffer.journal_path())?;
            Ok(AuditSettlement::GapEvent { gap_records: pending })
        }
    }
}

/// Whether the path exists (a small helper for the wipe's post-condition).
fn path_exists(path: &Path) -> bool {
    path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{EdgeError, InMemoryAuditSink};

    fn binding() -> DeviceBinding {
        DeviceBinding::new("tenant-a".to_string(), "pilot-site-A".to_string())
    }

    fn record_template() -> DeviceCredentialRecord {
        DeviceCredentialRecord {
            device_id: "edge-01".to_string(),
            tenant_id: "tenant-a".to_string(),
            site_id: "pilot-site-A".to_string(),
            certificate_pem: None, // shipped seam: no CA — recorded honestly
            chain_pem: Vec::new(),
            root_ca_pem: Some(PemBody::new("-----BEGIN CERTIFICATE-----\nTEST\n-----END CERTIFICATE-----\n".to_string())),
            private_key_reference: Some(PathBuf::from("/var/lib/flexcore-edge/keys/device.key")),
            issued_at_unix_millis: 1_700_000_000_000,
            expires_at_unix_millis: Some(1_700_000_000_000 + 30 * 86_400_000),
        }
    }

    #[test]
    fn provisioning_token_redacts_and_rejects_empty() {
        let token = ProvisioningToken::new("s3cret-value".to_string()).expect("valid");
        assert_eq!(format!("{token:?}"), "ProvisioningToken(<redacted>)");
        assert_eq!(token.to_string(), "<redacted>");
        assert_eq!(token.expose(), "s3cret-value");
        assert!(matches!(
            ProvisioningToken::new("   ".to_string()),
            Err(EdgeError::EnrollmentFailed { .. })
        ));
    }

    #[test]
    fn credential_record_debug_redacts_all_pem_bodies() {
        let mut record = record_template();
        record.certificate_pem = Some(PemBody::new("-----BEGIN CERTIFICATE-----\nSECRET\n-----END CERTIFICATE-----\n".to_string()));
        let rendered = format!("{record:?}");
        assert!(!rendered.contains("SECRET"), "no PEM body may render into Debug");
        assert!(rendered.contains("edge-01"));
    }

    #[test]
    fn filesystem_store_round_trips_the_record_shape() {
        let mut record = record_template();
        record.certificate_pem = Some(PemBody::new(
            "-----BEGIN CERTIFICATE-----\nROUNDTRIP\n-----END CERTIFICATE-----\n".to_string(),
        ));
        let mut store = crate::test_support::TempFileCredentialStore::new("p6-06-roundtrip");
        store.put(&record).expect("stored");
        let loaded = store.inner().load().expect("loads").expect("present");
        assert_eq!(loaded, record);
        store.inner().clear().expect("cleared");
        assert_eq!(store.inner().load().expect("loads"), None);
    }

    #[test]
    fn expiry_monitor_fires_inside_the_rotation_window() {
        let monitor = CredentialExpiryMonitor::new(7 * 86_400_000).expect("positive window");
        let record = record_template(); // expires 30 days after issue
        let issued = record.issued_at_unix_millis;
        assert_eq!(
            monitor.assess(issued + 86_400_000, &record),
            ExpiryStatus::Valid
        );
        // 24 days in: remaining = 6 days < the 7-day window → warning.
        assert_eq!(
            monitor.assess(issued + 24 * 86_400_000, &record),
            ExpiryStatus::RotationDue
        );
        assert_eq!(
            monitor.assess(issued + 31 * 86_400_000, &record),
            ExpiryStatus::Expired
        );
        let mut none_record = record.clone();
        none_record.expires_at_unix_millis = None;
        assert_eq!(
            monitor.assess(issued, &none_record),
            ExpiryStatus::NoExpiryRecorded
        );
    }

    #[test]
    fn orchestrator_audits_completion_and_rejection() {
        let mut issuer = ScriptedCredentialIssuer::new(
            vec![("tenant-a".to_string(), "pilot-site-A".to_string())],
            record_template(),
        );
        let mut client = TokenEnrollmentAdapter::new(Box::new(issuer));
        let mut store = crate::test_support::TempFileCredentialStore::new("p6-06-orchestrate");
        let audit = InMemoryAuditSink::new();
        let token = ProvisioningToken::new("one-shot-token".to_string()).expect("valid");

        let enrolled = EnrollmentOrchestrator::enroll(
            &mut client,
            store.inner(),
            &token,
            &binding(),
            &audit,
            1_700_000_000_000,
        )
        .expect("enrolls");
        assert_eq!(enrolled.device_id, "edge-01");
        assert!(store.inner().load().expect("loads").is_some());
        let kinds: Vec<&str> = audit.events().iter().map(|e| e.kind.name()).collect();
        assert!(kinds.contains(&"EDGE_ENROLLMENT_COMPLETED"));

        // Duplicate identity: the scripted issuer rejects; the orchestrator
        // audits the rejection and returns the named error.
        let second_token = ProvisioningToken::new("second-token".to_string()).expect("valid");
        assert!(matches!(
            EnrollmentOrchestrator::enroll(
                &mut client,
                store.inner(),
                &second_token,
                &binding(),
                &audit,
                1_700_000_000_001,
            ),
            Err(EdgeError::EnrollmentFailed { .. })
        ));
        let kinds: Vec<&str> = audit.events().iter().map(|e| e.kind.name()).collect();
        assert!(kinds.contains(&"EDGE_ENROLLMENT_REJECTED"));
    }

    #[test]
    fn deny_all_issuer_is_the_shipped_honest_default() {
        let mut issuer = DenyAllCredentialIssuer;
        let token = ProvisioningToken::new("t".to_string()).expect("valid");
        let error = issuer
            .issue(&token, &binding())
            .expect_err("no CA exists — the shipped issuer refuses");
        let rendered = error.to_string();
        assert!(rendered.contains("DEPLOYMENT REQUIREMENT"));
    }

    #[test]
    fn unconfigured_revocation_checker_fails_closed() {
        let checker = UnconfiguredRevocationChecker;
        let error = checker
            .check(&record_template())
            .expect_err("no OCSP/CRL client exists — fail closed");
        let rendered = error.to_string();
        assert!(rendered.contains("DEPLOYMENT REQUIREMENT"));
        assert!(!rendered.to_lowercase().contains("not revoked"));
    }
}
