// P18.09 — ZERO-TOUCH onboarding DESIGN (FDO-pattern late binding): the
//          ownership-voucher analog, the rendezvous service seam, and the
//          claim flow for factory-provisioned devices (phase-18 plan
//          P18.09; sources [S-165] FIDO Device Onboard, [S-166] T2TRG).
//
// THE HONEST DEFAULT (recorded twice — here and in the decision record):
// **the manual provisioning path (P9.33) stays the default AND the
// fallback.** Zero-touch is an ADDITIVE, separately-argued pattern that is
// structurally OFF unless a deployment composes a rendezvous
// implementation: the shipped rendezvous seam
// ([`NoRendezvousConfigured`]) refuses EVERY lookup, so a fleet composed
// with this module and nothing else behaves exactly as the P9.33 manual
// path does. Zero-touch is never a replacement for the ceremony — it is a
// different way to OPEN one.
//
// THE FDO PATTERN AS AN ANALOG (every FDO-specific detail is UNVERIFIED
// against the FDO 1.1 specification text; [S-165] is a secondary overview
// — this module is an analog of the PATTERN, never FDO conformance):
//
// - **Ownership voucher analog** ([`OwnershipVoucherEntry`]): the FDO
//   voucher binds a device to an owner so the device can take
//   instructions after factory release. The FlexCore analog is a
//   tenant-scoped register entry: (device identity, tenant, site, state)
//   recorded BEFORE the device ever connects, with an append-only event
//   history (the 0018 journal framing). A device WITHOUT a voucher entry
//   can never be zero-touch claimed — the claim flow refuses; the manual
//   path is the fallback for exactly that device.
// - **Rendezvous analog** ([`RendezvousSeam`]): the FDO rendezvous
//   service directs a factory-provisioned device to its current owner's
//   onboarding endpoint. FlexCore's analog is a PORT the deployment
//   implements (or does not): given a device identity, it returns the
//   onboarding endpoint reference the device should present its claim
//   to. The platform NEVER runs discovery: the device presents its
//   factory identity (IDevID reference, P18.08's mapping), the seam
//   resolves the owner, the voucher must agree.
// - **Late binding**: ownership is decided at claim time from the
//   voucher + rendezvous agreement, not at manufacture.
//
// THE CLAIM FLOW ([`ZeroTouchClaimFlow`], closed state machine):
//
// ```text
// AwaitingDevice --VerifyVoucher------> VoucherVerified
// VoucherVerified --ResolveRendezvous-> RendezvousDirected
// RendezvousDirected --ConfirmOwner--> OwnershipConfirmed
// OwnershipConfirmed --HandToCeremony-> CeremonyHandoff
// (any state)      --Refuse-----------> Refused (named reason, terminal)
// ```
//
// THE LOAD-BEARING RULE: `CeremonyHandoff` is the flow's LAST state and
// it ISSUES NOTHING. The handoff consumes a fresh platform-authority
// token (the same single-use, short-lived `EnrollmentTokenRecord` the
// manual path presents) and the claim flow's job ENDS there — the P9.33
// ceremony's challenge, approval (THE platform-side human/authority
// gate), and activation run UNCHANGED afterwards. Zero-touch removes the
// on-site OPERATOR TOKEN step, never the approval gate: the approval
// remains a platform-side authority decision recorded as
// `CeremonyApproval` (`SelfApprovalRefused` semantics untouched).
//
// REVOCATION INTERPLAY (P18.08): a device whose voucher entry is
// [`VoucherState::Invalidated`] (the revocation propagation's
// ZERO_TOUCH_VOUCHER target) can never be claimed; a claim attempt is
// the named [`ZeroTouchError::DeviceRevoked`] refusal.
//
// HARDWARE POSTURE (P18-E-2, P6.34): zero-touch presumes a
// factory-provisioned IDevID, whose verification strength depends on
// hardware anchors that are PENDING PILOT HARDWARE SPEC. With no
// verified anchor the claim degrades to reference-equality checks only,
// and this module records that honestly in [`OwnershipVoucherEntry`]
// evidence fields — it never implies a hardware root of trust exists.
//
// HONESTY BOUNDS: no device is claimed; no voucher exists; no rendezvous
// service is operated; every flow below is exercised against in-memory
// test doubles only (phase-18 constraint 6). No CA, no issuance: the
// handoff token comes from the SAME platform identity authority the
// manual path uses.

use std::collections::BTreeMap;

// --- The voucher register -------------------------------------------------------------

/// The closed ownership-voucher states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VoucherState {
    /// The voucher is recorded (factory/provisioning data) and the device
    /// is bound to its owner but has not connected.
    Held,
    /// The device has been directed to the owner's onboarding endpoint
    /// (the rendezvous resolved); the claim is in progress.
    ReleasedToRendezvous,
    /// The claim completed: the device handed off to the P9.33 ceremony.
    Claimed,
    /// The voucher was released at decommissioning (P18.10 consumes this
    /// state; the device may enter a second life with a new voucher).
    ReleasedForDecommission,
    /// The voucher was invalidated (revocation propagation, P18.08). A
    /// invalidated voucher can never claim; re-claiming requires a NEW
    /// voucher entry (append-only; never an in-place edit).
    Invalidated,
}

impl VoucherState {
    /// The stable audit/vocabulary name.
    pub fn name(self) -> &'static str {
        match self {
            VoucherState::Held => "HELD",
            VoucherState::ReleasedToRendezvous => "RELEASED_TO_RENDEZVOUS",
            VoucherState::Claimed => "CLAIMED",
            VoucherState::ReleasedForDecommission => "RELEASED_FOR_DECOMMISSION",
            VoucherState::Invalidated => "INVALIDATED",
        }
    }
}

/// The closed voucher-event vocabulary (the register is append-only; a
/// state change is a NEW event, the latest event wins).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoucherEventKind {
    VoucherRecorded,
    ReleasedToRendezvous,
    VoucherClaimed,
    VoucherReleased,
    VoucherInvalidated,
}

impl VoucherEventKind {
    /// The stable audit/vocabulary name.
    pub fn name(self) -> &'static str {
        match self {
            VoucherEventKind::VoucherRecorded => "VOUCHER_RECORDED",
            VoucherEventKind::ReleasedToRendezvous => "VOUCHER_RELEASED_TO_RENDEZVOUS",
            VoucherEventKind::VoucherClaimed => "VOUCHER_CLAIMED",
            VoucherEventKind::VoucherReleased => "VOUCHER_RELEASED",
            VoucherEventKind::VoucherInvalidated => "VOUCHER_INVALIDATED",
        }
    }
}

/// One ownership-voucher REGISTER ENTRY (the current-state projection of
/// the append-only event history for one device).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipVoucherEntry {
    /// The device identity exactly as the factory IDevID names it (a
    /// REFERENCE; no certificate material is held here).
    pub device_identity: String,
    /// The owning tenant (§16.2 scope the claim must match).
    pub tenant_id: String,
    /// The owning site.
    pub site_id: String,
    pub state: VoucherState,
    /// The factory IDevID reference the claim must present for equality
    /// (a reference/handle, never a certificate body).
    pub idevid_reference: String,
    /// The recorded hardware-anchor posture of this voucher (P18-E-2 —
    /// recorded data from provisioning, never a platform claim that an
    /// anchor exists).
    pub anchor_posture: VoucherAnchorPosture,
    /// When the latest event for this voucher was recorded (unix millis,
    /// injected-clock data).
    pub updated_at_unix_millis: i64,
}

/// The honest anchor-posture field: what provisioning recorded about the
/// device's hardware anchor, WITHOUT any platform claim that one exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoucherAnchorPosture {
    /// Provisioning recorded an anchor, unverified by this platform
    /// (PENDING PILOT HARDWARE SPEC — the P6.34 posture).
    RecordedUnverified,
    /// Provisioning recorded NO anchor; verification is software-only.
    RecordedAbsent,
    /// Provisioning recorded nothing; the claim treats this as
    /// software-only (never as anchor-present).
    NotRecorded,
}

impl VoucherAnchorPosture {
    /// The stable audit/vocabulary name.
    pub fn name(self) -> &'static str {
        match self {
            VoucherAnchorPosture::RecordedUnverified => "RECORDED_UNVERIFIED",
            VoucherAnchorPosture::RecordedAbsent => "RECORDED_ABSENT",
            VoucherAnchorPosture::NotRecorded => "NOT_RECORDED",
        }
    }
}

/// One append-only voucher event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoucherEvent {
    pub kind: VoucherEventKind,
    /// The tenant scope the event was recorded under (the register is
    /// tenant-scoped; a claim can only see its own tenant's vouchers).
    pub tenant_id: String,
    pub device_identity: String,
    /// The scope/site of the voucher at this event (a re-voucher after
    /// decommission may name a different site).
    pub site_id: String,
    pub actor: String,
    pub reference: String,
    pub at_unix_millis: i64,
}

/// The in-memory ownership-voucher register (append-only events + the
/// latest-state projection). The production form is a tenant-scoped
/// store behind the registry adapter; this in-memory shape is the test
/// double AND the projection contract.
#[derive(Debug, Default)]
pub struct OwnershipVoucherRegister {
    events: Vec<VoucherEvent>,
    latest: BTreeMap<(String, String), OwnershipVoucherEntry>,
}

impl OwnershipVoucherRegister {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one voucher event (fail-closed validation; an invalid
    /// event never enters the register).
    pub fn append(&mut self, event: VoucherEvent) -> Result<(), ZeroTouchError> {
        if event.tenant_id.trim().is_empty()
            || event.device_identity.trim().is_empty()
            || event.actor.trim().is_empty()
            || event.reference.trim().is_empty()
            || event.at_unix_millis <= 0
        {
            return Err(ZeroTouchError::InvalidVoucherEvent {
                reason: "tenant, device identity, actor, reference, and a positive instant \
                         are all mandatory"
                    .to_string(),
            });
        }
        if event.site_id.trim().is_empty() {
            return Err(ZeroTouchError::InvalidVoucherEvent {
                reason: "the voucher site scope is mandatory (§16.2)".to_string(),
            });
        }
        self.events.push(event.clone());
        let key = (event.tenant_id.clone(), event.device_identity.clone());
        let previous = self.latest.get(&key);
        // The projected state moves only along the named edges; any other
        // event/state combination is refused (the register is a closed
        // state machine, not a status cell).
        let next_state = match event.kind {
            VoucherEventKind::VoucherRecorded => {
                if previous.is_some() {
                    return Err(ZeroTouchError::InvalidVoucherTransition {
                        from: previous.map(|e| e.state).map(VoucherState::name).unwrap_or("?"),
                        event: event.kind.name(),
                        detail: "a voucher is RECORDED once; re-binding a device creates a \
                                 NEW voucher identity, never an edit of this one"
                            .to_string(),
                    });
                }
                VoucherState::Held
            }
            VoucherEventKind::ReleasedToRendezvous => {
                require_state(previous, &[VoucherState::Held])?;
                VoucherState::ReleasedToRendezvous
            }
            VoucherEventKind::VoucherClaimed => {
                require_state(previous, &[VoucherState::ReleasedToRendezvous])?;
                VoucherState::Claimed
            }
            VoucherEventKind::VoucherReleased => {
                require_state(previous, &[VoucherState::Claimed])?;
                VoucherState::ReleasedForDecommission
            }
            VoucherEventKind::VoucherInvalidated => {
                require_state(
                    previous,
                    &[VoucherState::Held, VoucherState::ReleasedToRendezvous],
                )?;
                VoucherState::Invalidated
            }
        };
        // The projected entry's IDevID reference and anchor posture come
        // from the RECORDING event and are carried unchanged; the
        // reference rides the event's `reference` field at recording time.
        let entry = match previous {
            Some(existing) => OwnershipVoucherEntry {
                state: next_state,
                updated_at_unix_millis: event.at_unix_millis,
                ..existing.clone()
            },
            None => OwnershipVoucherEntry {
                device_identity: event.device_identity.clone(),
                tenant_id: event.tenant_id.clone(),
                site_id: event.site_id.clone(),
                state: next_state,
                idevid_reference: event.reference.clone(),
                anchor_posture: VoucherAnchorPosture::NotRecorded,
                updated_at_unix_millis: event.at_unix_millis,
            },
        };
        self.latest.insert(key, entry);
        Ok(())
    }

    /// The current voucher entry for a device within one tenant, when
    /// one exists.
    pub fn voucher(&self, tenant_id: &str, device_identity: &str) -> Option<&OwnershipVoucherEntry> {
        self.latest
            .get(&(tenant_id.to_string(), device_identity.to_string()))
    }

    /// How many events the register holds (diagnostics).
    pub fn event_len(&self) -> usize {
        self.events.len()
    }
}

fn require_state(
    previous: Option<&OwnershipVoucherEntry>,
    legal: &[VoucherState],
) -> Result<(), ZeroTouchError> {
    let entry = previous.ok_or(ZeroTouchError::InvalidVoucherTransition {
        from: "absent",
        event: "state change",
        detail: "no voucher is recorded for this (tenant, device) — a state change before \
                 VoucherRecorded is refused"
            .to_string(),
    })?;
    if legal.contains(&entry.state) {
        Ok(())
    } else {
        Err(ZeroTouchError::InvalidVoucherTransition {
            from: entry.state.name(),
            event: "state change",
            detail: format!(
                "legal predecessor states: {}",
                legal.iter().map(|s| s.name()).collect::<Vec<_>>().join(", ")
            ),
        })
    }
}

// --- The rendezvous seam (the port the deployment implements or not) --------------------

/// The rendezvous DIRECTIVE: where a factory-provisioned device presents
/// its claim. The device identity and the owner scope must agree with the
/// voucher before the flow advances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendezvousDirective {
    /// The onboarding-endpoint reference the device claims against (an
    /// endpoint LABEL/reference — this module never holds a URL-parsing
    /// or network surface; the transport is the deployment's).
    pub onboarding_endpoint_reference: String,
    /// The tenant the rendezvous resolved the device to (must equal the
    /// voucher's tenant).
    pub resolved_tenant_id: String,
    /// The resolved site (must equal the voucher's site).
    pub resolved_site_id: String,
}

/// The rendezvous SEAM (P18.09's port): resolves a factory-provisioned
/// device to its current owner's onboarding endpoint. Sync trait, house
/// rules. The SHIPPED implementation is [`NoRendezvousConfigured`].
pub trait RendezvousSeam {
    /// The seam name for audit lines.
    fn name(&self) -> &'static str;
    /// Resolves the directive for one device identity. A refusal is the
    /// named [`ZeroTouchError`] — a device the rendezvous cannot resolve
    /// is refused, never guessed to an owner.
    fn resolve(&self, device_identity: &str) -> Result<RendezvousDirective, ZeroTouchError>;
}

/// The SHIPPED fail-closed rendezvous: refuses EVERY resolution. A
/// deployment composed with this default performs NO zero-touch claims —
/// the manual P9.33 path is the default and the fallback, structurally.
#[derive(Debug, Default)]
pub struct NoRendezvousConfigured;

impl NoRendezvousConfigured {
    pub fn new() -> Self {
        Self
    }
}

impl RendezvousSeam for NoRendezvousConfigured {
    fn name(&self) -> &'static str {
        "NO_RENDEZVOUS_CONFIGURED"
    }

    fn resolve(&self, _device_identity: &str) -> Result<RendezvousDirective, ZeroTouchError> {
        Err(ZeroTouchError::RendezvousNotConfigured)
    }
}

// --- The claim flow ------------------------------------------------------------------

/// The closed claim-flow states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimFlowState {
    /// The flow is open; the device's voucher has not been verified.
    AwaitingDevice,
    /// The voucher exists, is claimable, and the presented IDevID
    /// reference matches it.
    VoucherVerified,
    /// The rendezvous resolved a directive (endpoint + owner scope).
    RendezvousDirected,
    /// The directive's owner scope matches the voucher's scope.
    OwnershipConfirmed,
    /// The flow handed the platform-authority token to the P9.33
    /// ceremony. TERMINAL for this flow: the ceremony owns everything
    /// after this point.
    CeremonyHandoff,
    /// The flow refused (terminal; the named reason is carried by the
    /// error, the audit line, and the register's refusal event).
    Refused,
}

impl ClaimFlowState {
    /// The stable audit/vocabulary name.
    pub fn name(self) -> &'static str {
        match self {
            ClaimFlowState::AwaitingDevice => "AWAITING_DEVICE",
            ClaimFlowState::VoucherVerified => "VOUCHER_VERIFIED",
            ClaimFlowState::RendezvousDirected => "RENDEZVOUS_DIRECTED",
            ClaimFlowState::OwnershipConfirmed => "OWNERSHIP_CONFIRMED",
            ClaimFlowState::CeremonyHandoff => "CEREMONY_HANDOFF",
            ClaimFlowState::Refused => "REFUSED",
        }
    }
}

/// The zero-touch claim flow over ONE device identity: verify the
/// voucher, resolve the rendezvous, confirm ownership, hand off to the
/// P9.33 ceremony. The flow ISSUES NOTHING and NEVER approves: the
/// ceremony's platform-side approval gate runs unchanged after handoff.
pub struct ZeroTouchClaimFlow<'a> {
    device_identity: String,
    /// The IDevID reference the DEVICE presented at claim time (the flow
    /// compares it for equality against the voucher's recorded reference;
    /// WITHOUT a verified hardware anchor this is reference-equality
    /// only — recorded honestly, P18-E-2).
    presented_idevid_reference: String,
    tenant_id: String,
    site_id: String,
    state: ClaimFlowState,
    vouchers: &'a OwnershipVoucherRegister,
    rendezvous: &'a dyn RendezvousSeam,
}

impl<'a> ZeroTouchClaimFlow<'a> {
    /// Opens a claim flow for one device within one tenant/site scope.
    pub fn new(
        device_identity: &str,
        presented_idevid_reference: &str,
        tenant_id: &str,
        site_id: &str,
        vouchers: &'a OwnershipVoucherRegister,
        rendezvous: &'a dyn RendezvousSeam,
    ) -> Result<Self, ZeroTouchError> {
        if device_identity.trim().is_empty()
            || presented_idevid_reference.trim().is_empty()
            || tenant_id.trim().is_empty()
            || site_id.trim().is_empty()
        {
            return Err(ZeroTouchError::InvalidClaimInput {
                reason: "device identity, presented IDevID reference, tenant, and site are \
                         all mandatory"
                    .to_string(),
            });
        }
        Ok(Self {
            device_identity: device_identity.to_string(),
            presented_idevid_reference: presented_idevid_reference.to_string(),
            tenant_id: tenant_id.to_string(),
            site_id: site_id.to_string(),
            state: ClaimFlowState::AwaitingDevice,
            vouchers,
            rendezvous,
        })
    }

    /// The current flow state.
    pub fn state(&self) -> ClaimFlowState {
        self.state
    }

    /// Step 1 — verify the voucher: the device must have a voucher entry
    /// in this tenant, in the HELD state, whose IDevID reference equals
    /// the presented reference. Refusals: no voucher (the manual path is
    /// the fallback for this device), an invalidated voucher (revoked —
    /// never claimable), a wrong reference, or a voucher already in
    /// flight/claimed.
    pub fn verify_voucher(&mut self) -> Result<(), ZeroTouchError> {
        if self.state != ClaimFlowState::AwaitingDevice {
            return Err(ZeroTouchError::InvalidTransition {
                from: self.state.name(),
                step: "VERIFY_VOUCHER",
            });
        }
        let Some(voucher) = self.vouchers.voucher(&self.tenant_id, &self.device_identity) else {
            // A refusal like every other in this step: the flow is refused
            // (the manual path is the fallback for this device).
            return Err(self.refuse(ZeroTouchError::NoVoucherForDevice));
        };
        let outcome = if voucher.state == VoucherState::Invalidated {
            Err(ZeroTouchError::DeviceRevoked)
        } else if voucher.idevid_reference != self.presented_idevid_reference {
            Err(ZeroTouchError::IDevIDReferenceMismatch)
        } else if voucher.state != VoucherState::Held {
            Err(ZeroTouchError::VoucherNotClaimable {
                state: voucher.state.name(),
            })
        } else if voucher.site_id != self.site_id {
            Err(ZeroTouchError::ScopeMismatch {
                detail: format!(
                    "the voucher binds site '{}' but the claim scope is '{}'",
                    voucher.site_id, self.site_id
                ),
            })
        } else {
            Ok(())
        };
        // The refusal marks the flow refused and travels with the state
        // change; the borrow of the register ends before the state moves.
        outcome.map_err(|error| self.refuse(error))?;
        self.state = ClaimFlowState::VoucherVerified;
        Ok(())
    }

    /// Step 2 — resolve the rendezvous. With the shipped
    /// [`NoRendezvousConfigured`] this is ALWAYS the named
    /// [`ZeroTouchError::RendezvousNotConfigured`] refusal: the manual
    /// P9.33 path remains the default and the fallback.
    pub fn resolve_rendezvous(&mut self) -> Result<RendezvousDirective, ZeroTouchError> {
        if self.state != ClaimFlowState::VoucherVerified {
            return Err(ZeroTouchError::InvalidTransition {
                from: self.state.name(),
                step: "RESOLVE_RENDEZVOUS",
            });
        }
        let directive = self
            .rendezvous
            .resolve(&self.device_identity)
            .map_err(|error| self.refuse(error))?;
        self.state = ClaimFlowState::RendezvousDirected;
        Ok(directive)
    }

    /// Step 3 — confirm ownership: the resolved directive's scope must
    /// equal the voucher's scope (the late-binding agreement).
    pub fn confirm_ownership(
        &mut self,
        directive: &RendezvousDirective,
    ) -> Result<(), ZeroTouchError> {
        if self.state != ClaimFlowState::RendezvousDirected {
            return Err(ZeroTouchError::InvalidTransition {
                from: self.state.name(),
                step: "CONFIRM_OWNERSHIP",
            });
        }
        if directive.resolved_tenant_id != self.tenant_id || directive.resolved_site_id != self.site_id
        {
            return Err(self.refuse(ZeroTouchError::ScopeMismatch {
                detail: format!(
                    "the rendezvous resolved tenant/site '{}/{}' but the voucher scope is \
                     '{}/{}' — late binding refuses a disagreement",
                    directive.resolved_tenant_id,
                    directive.resolved_site_id,
                    self.tenant_id,
                    self.site_id
                ),
            }));
        }
        if directive.onboarding_endpoint_reference.trim().is_empty() {
            return Err(self.refuse(ZeroTouchError::InvalidClaimInput {
                reason: "the rendezvous directive names no onboarding endpoint reference"
                    .to_string(),
            }));
        }
        self.state = ClaimFlowState::OwnershipConfirmed;
        Ok(())
    }

    /// Step 4 — hand off to the P9.33 ceremony: the flow consumes the
    /// fresh platform-authority token RECORD and transitions to
    /// [`ClaimFlowState::CeremonyHandoff`]. The handoff VALIDATES the
    /// token record's shape (issuer named, positive lifetime window,
    /// unexpired against the passed instant) but the CEREMONY performs
    /// its own full validation at `begin_request` — this check is the
    /// flow's early refusal, never a substitute.
    pub fn hand_to_ceremony(
        &mut self,
        token_reference: &str,
        issued_by: &str,
        expires_at_unix_millis: i64,
        now_unix_millis: i64,
    ) -> Result<(), ZeroTouchError> {
        if self.state != ClaimFlowState::OwnershipConfirmed {
            return Err(ZeroTouchError::InvalidTransition {
                from: self.state.name(),
                step: "HAND_TO_CEREMONY",
            });
        }
        // Input validation refuses the ATTEMPT, not the flow: a malformed
        // token record is caller-correctable (retry with a live record),
        // so the state stays OwnershipConfirmed. Only substantive
        // refusals (no voucher, revoked, mismatch, scope, rendezvous)
        // mark the flow Refused.
        if token_reference.trim().is_empty() {
            return Err(ZeroTouchError::InvalidClaimInput {
                reason: "the handoff token reference is mandatory".to_string(),
            });
        }
        if issued_by.trim().is_empty() {
            return Err(ZeroTouchError::InvalidClaimInput {
                reason: "the handoff token must name its issuing authority (an \
                         un-attributable token is refused, mirroring the ceremony)"
                    .to_string(),
            });
        }
        if expires_at_unix_millis <= now_unix_millis {
            return Err(ZeroTouchError::InvalidClaimInput {
                reason: "the handoff token is already expired at handoff".to_string(),
            });
        }
        self.state = ClaimFlowState::CeremonyHandoff;
        Ok(())
    }

    /// Marks the flow refused and returns the error (the state change and
    /// the refusal travel together).
    fn refuse(&mut self, error: ZeroTouchError) -> ZeroTouchError {
        self.state = ClaimFlowState::Refused;
        error
    }
}

// --- Errors -----------------------------------------------------------------------

/// The named zero-touch errors. Nothing implies a silent pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZeroTouchError {
    /// A claim input was unusable.
    InvalidClaimInput { reason: String },
    /// A voucher event was malformed.
    InvalidVoucherEvent { reason: String },
    /// A voucher state change was refused (the closed state machine).
    InvalidVoucherTransition { from: &'static str, event: &'static str, detail: String },
    /// The rendezvous seam is the shipped fail-closed default: zero-touch
    /// is NOT configured — the manual P9.33 path is the default and the
    /// fallback.
    RendezvousNotConfigured,
    /// The device has no voucher in this tenant: it cannot be zero-touch
    /// claimed; the manual path is the fallback for exactly this device.
    NoVoucherForDevice,
    /// The device's voucher was invalidated (revocation propagation):
    /// never claimable.
    DeviceRevoked,
    /// The presented IDevID reference does not match the voucher.
    IDevIDReferenceMismatch,
    /// The voucher exists but is not in the claimable state.
    VoucherNotClaimable { state: &'static str },
    /// The rendezvous and voucher disagree about the owner scope.
    ScopeMismatch { detail: String },
    /// The attempted flow step does not exist from the current state.
    InvalidTransition { from: &'static str, step: &'static str },
}

impl ZeroTouchError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            ZeroTouchError::InvalidClaimInput { .. } => "ZERO_TOUCH_CLAIM_INPUT_INVALID",
            ZeroTouchError::InvalidVoucherEvent { .. } => "ZERO_TOUCH_VOUCHER_EVENT_INVALID",
            ZeroTouchError::InvalidVoucherTransition { .. } => {
                "ZERO_TOUCH_VOUCHER_TRANSITION_INVALID"
            }
            ZeroTouchError::RendezvousNotConfigured => "ZERO_TOUCH_RENDEZVOUS_NOT_CONFIGURED",
            ZeroTouchError::NoVoucherForDevice => "ZERO_TOUCH_NO_VOUCHER_FOR_DEVICE",
            ZeroTouchError::DeviceRevoked => "ZERO_TOUCH_DEVICE_REVOKED",
            ZeroTouchError::IDevIDReferenceMismatch => "ZERO_TOUCH_IDEVID_REFERENCE_MISMATCH",
            ZeroTouchError::VoucherNotClaimable { .. } => "ZERO_TOUCH_VOUCHER_NOT_CLAIMABLE",
            ZeroTouchError::ScopeMismatch { .. } => "ZERO_TOUCH_SCOPE_MISMATCH",
            ZeroTouchError::InvalidTransition { .. } => "ZERO_TOUCH_TRANSITION_INVALID",
        }
    }
}

impl std::fmt::Display for ZeroTouchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZeroTouchError::InvalidClaimInput { reason } => {
                write!(f, "the zero-touch claim input is invalid: {reason}")
            }
            ZeroTouchError::InvalidVoucherEvent { reason } => {
                write!(f, "the voucher event is invalid: {reason}")
            }
            ZeroTouchError::InvalidVoucherTransition { from, event, detail } => write!(
                f,
                "the voucher event {event} is refused from state {from}: {detail}"
            ),
            ZeroTouchError::RendezvousNotConfigured => write!(
                f, "no rendezvous is configured — zero-touch is structurally OFF and the \
                     manual P9.33 path is the default and the fallback"
            ),
            ZeroTouchError::NoVoucherForDevice => write!(
                f, "the device has no ownership voucher in this tenant — it cannot be \
                     zero-touch claimed; the manual P9.33 path is the fallback"
            ),
            ZeroTouchError::DeviceRevoked => write!(
                f, "the device's voucher was invalidated by revocation propagation — a \
                     revoked device can never be zero-touch claimed"
            ),
            ZeroTouchError::IDevIDReferenceMismatch => write!(
                f, "the presented IDevID reference does not match the voucher's recorded \
                     reference (reference-equality only — no hardware anchor is verified, \
                     P18-E-2)"
            ),
            ZeroTouchError::VoucherNotClaimable { state } => write!(
                f, "the voucher exists but is in state {state}, not claimable"
            ),
            ZeroTouchError::ScopeMismatch { detail } => {
                write!(f, "the ownership scope disagrees: {detail}")
            }
            ZeroTouchError::InvalidTransition { from, step } => write!(
                f, "the flow step {step} does not exist from state {from}"
            ),
        }
    }
}

impl std::error::Error for ZeroTouchError {}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: i64 = 1_700_000_000_000;

    fn recorded(register: &mut OwnershipVoucherRegister) {
        register
            .append(VoucherEvent {
                kind: VoucherEventKind::VoucherRecorded,
                tenant_id: "tenant-a".to_string(),
                device_identity: "edge-factory-01".to_string(),
                site_id: "pilot-site-A".to_string(),
                actor: "provisioning".to_string(),
                reference: "idevid-ref-01".to_string(),
                at_unix_millis: BASE,
            })
            .expect("valid recording event");
    }

    #[test]
    fn the_shipped_rendezvous_refuses_every_resolution() {
        let rendezvous = NoRendezvousConfigured::new();
        assert_eq!(rendezvous.name(), "NO_RENDEZVOUS_CONFIGURED");
        assert_eq!(
            rendezvous.resolve("edge-factory-01").unwrap_err().name(),
            "ZERO_TOUCH_RENDEZVOUS_NOT_CONFIGURED",
            "the shipped default keeps zero-touch structurally OFF"
        );
    }

    #[test]
    fn a_device_without_a_voucher_is_refused_to_the_manual_path() {
        let mut vouchers = OwnershipVoucherRegister::new();
        let rendezvous = NoRendezvousConfigured::new();
        let mut flow = ZeroTouchClaimFlow::new(
            "edge-unvouchered",
            "idevid-ref-x",
            "tenant-a",
            "pilot-site-A",
            &vouchers,
            &rendezvous,
        )
        .expect("valid inputs");
        assert_eq!(flow.verify_voucher().unwrap_err().name(), "ZERO_TOUCH_NO_VOUCHER_FOR_DEVICE");
        assert_eq!(flow.state(), ClaimFlowState::Refused);
        recorded(&mut vouchers);
        // The register itself is untouched by the refused flow.
        assert_eq!(vouchers.event_len(), 1);
    }

    #[test]
    fn the_claim_flow_completes_only_through_the_named_steps() {
        let mut vouchers = OwnershipVoucherRegister::new();
        recorded(&mut vouchers);
        // The voucher stays HELD through the flow: the register moves to
        // RELEASED_TO_RENDEZVOUS / CLAIMED by the deployment composing the
        // flow's outcomes, not as a precondition of it.
        let mut directives = BTreeMap::new();
        directives.insert(
            "edge-factory-01".to_string(),
            RendezvousDirective {
                onboarding_endpoint_reference: "onboard-endpoint-1".to_string(),
                resolved_tenant_id: "tenant-a".to_string(),
                resolved_site_id: "pilot-site-A".to_string(),
            },
        );
        let rendezvous = MapRendezvous { directives };
        let mut flow = ZeroTouchClaimFlow::new(
            "edge-factory-01",
            "idevid-ref-01",
            "tenant-a",
            "pilot-site-A",
            &vouchers,
            &rendezvous,
        )
        .expect("valid inputs");
        assert_eq!(flow.state(), ClaimFlowState::AwaitingDevice);
        // Steps cannot run out of order.
        assert_eq!(
            flow.resolve_rendezvous().unwrap_err().name(),
            "ZERO_TOUCH_TRANSITION_INVALID"
        );
        flow.verify_voucher().expect("voucher verifies");
        assert_eq!(flow.state(), ClaimFlowState::VoucherVerified);
        // Ownership cannot be confirmed before the rendezvous resolved.
        assert_eq!(
            flow.confirm_ownership(&RendezvousDirective {
                onboarding_endpoint_reference: "x".to_string(),
                resolved_tenant_id: "tenant-a".to_string(),
                resolved_site_id: "pilot-site-A".to_string(),
            })
            .unwrap_err()
            .name(),
            "ZERO_TOUCH_TRANSITION_INVALID"
        );
        let directive = flow.resolve_rendezvous().expect("the test rendezvous resolves");
        assert_eq!(flow.state(), ClaimFlowState::RendezvousDirected);
        flow.confirm_ownership(&directive).expect("scopes agree");
        assert_eq!(flow.state(), ClaimFlowState::OwnershipConfirmed);
        // Handoff refuses an expired token record early.
        assert_eq!(
            flow.hand_to_ceremony("tok-ref-1", "platform-identity-authority", BASE, BASE + 1)
                .unwrap_err()
                .name(),
            "ZERO_TOUCH_CLAIM_INPUT_INVALID"
        );
        flow.hand_to_ceremony(
            "tok-ref-1",
            "platform-identity-authority",
            BASE + 60_000,
            BASE + 1,
        )
        .expect("the handoff accepts a live token record");
        assert_eq!(flow.state(), ClaimFlowState::CeremonyHandoff);
        // The flow is terminal after handoff — the P9.33 ceremony owns
        // the challenge, the approval gate, and the activation.
        assert_eq!(
            flow.hand_to_ceremony("tok-ref-2", "authority", BASE + 120_000, BASE + 2)
                .unwrap_err()
                .name(),
            "ZERO_TOUCH_TRANSITION_INVALID"
        );
    }

    #[test]
    fn a_claimed_voucher_cannot_be_recorded_twice_and_an_invalidated_one_never_claims() {
        let mut vouchers = OwnershipVoucherRegister::new();
        recorded(&mut vouchers);
        // A second VoucherRecorded is refused (the register is a closed
        // state machine; re-binding creates a NEW voucher identity).
        assert_eq!(
            vouchers
                .append(VoucherEvent {
                    kind: VoucherEventKind::VoucherRecorded,
                    tenant_id: "tenant-a".to_string(),
                    device_identity: "edge-factory-01".to_string(),
                    site_id: "pilot-site-A".to_string(),
                    actor: "provisioning".to_string(),
                    reference: "idevid-ref-01".to_string(),
                    at_unix_millis: BASE + 1,
                })
                .unwrap_err()
                .name(),
            "ZERO_TOUCH_VOUCHER_TRANSITION_INVALID"
        );
        // Revocation propagation invalidates the voucher...
        vouchers
            .append(VoucherEvent {
                kind: VoucherEventKind::VoucherInvalidated,
                tenant_id: "tenant-a".to_string(),
                device_identity: "edge-factory-01".to_string(),
                site_id: "pilot-site-A".to_string(),
                actor: "revocation-propagation".to_string(),
                reference: "decommission-ticket-1".to_string(),
                at_unix_millis: BASE + 2_000,
            })
            .expect("valid invalidation");
        assert_eq!(
            vouchers
                .voucher("tenant-a", "edge-factory-01")
                .map(|e| e.state)
                .expect("entry exists"),
            VoucherState::Invalidated
        );
        // ...and a claim attempt on it is the named revoked refusal.
        let rendezvous = NoRendezvousConfigured::new();
        let mut flow = ZeroTouchClaimFlow::new(
            "edge-factory-01",
            "idevid-ref-01",
            "tenant-a",
            "pilot-site-A",
            &vouchers,
            &rendezvous,
        )
        .expect("valid inputs");
        assert_eq!(flow.verify_voucher().unwrap_err().name(), "ZERO_TOUCH_DEVICE_REVOKED");
        assert_eq!(flow.state(), ClaimFlowState::Refused);
    }

    #[test]
    fn a_reference_mismatch_and_a_scope_disagreement_are_named_refusals() {
        let mut vouchers = OwnershipVoucherRegister::new();
        recorded(&mut vouchers);
        let rendezvous = NoRendezvousConfigured::new();
        let mut flow = ZeroTouchClaimFlow::new(
            "edge-factory-01",
            "idevid-ref-WRONG",
            "tenant-a",
            "pilot-site-A",
            &vouchers,
            &rendezvous,
        )
        .expect("valid inputs");
        assert_eq!(
            flow.verify_voucher().unwrap_err().name(),
            "ZERO_TOUCH_IDEVID_REFERENCE_MISMATCH"
        );
        let mut flow_scope = ZeroTouchClaimFlow::new(
            "edge-factory-01",
            "idevid-ref-01",
            "tenant-a",
            "other-site",
            &vouchers,
            &rendezvous,
        )
        .expect("valid inputs");
        assert_eq!(flow_scope.verify_voucher().unwrap_err().name(), "ZERO_TOUCH_SCOPE_MISMATCH");
    }

    /// A test-double rendezvous backed by a BTreeMap (deterministic).
    struct MapRendezvous {
        directives: BTreeMap<String, RendezvousDirective>,
    }

    impl RendezvousSeam for MapRendezvous {
        fn name(&self) -> &'static str {
            "MAP_RENDEZVOUS_TEST_DOUBLE"
        }

        fn resolve(&self, device_identity: &str) -> Result<RendezvousDirective, ZeroTouchError> {
            self.directives
                .get(device_identity)
                .cloned()
                .ok_or(ZeroTouchError::NoVoucherForDevice)
        }
    }
}
