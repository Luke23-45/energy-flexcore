# Record 01 — P18.01: OTA architecture decision record (ADR-1801) + P18-E-1 registered OPEN

Date: 2026-11-17. Subtask: P18.01 (`ledger_phase_18.md` §P18.01). Status:
**WRITTEN — decision recorded, nothing built.** No metadata repository
exists, none is running, no manifest was produced, no fleet exists.

| Field | Value |
|---|---|
| ADR id | ADR-1801 |
| Subject | Edge OTA update architecture: metadata repository + signed manifests for the FlexCore edge fleet |
| Status | ACCEPTED as the phase-18 design baseline (the code-level P18-E-1 DEFAULT, option (b), is implemented additively; the E-decision itself stays OPEN — agents never close it) |
| Deciders | Phase 18 wave 1, Agent A (per plan §5); closer of P18-E-1 is the human owner (conventions §4) |
| Sources | [S-161] (TUF specification v1.0.37), [S-162] (IETF RFC 9019, SUIT manifest architecture); seam `crates/edge-agent/src/update_verification.rs` (P9.34); `ledger/phase-10-v1-release-hardening/21_signed_artifacts.md` (P10.21) |

## 1. Context (the recorded gap)

The Phase 10 exit-gate record names the gap this phase owns: **"No
update-bundle producer exists (P9.34/P10.12 largest recorded gap)"**
(`ledger/phase-10-v1-release-hardening/31_phase_exit_gate_record.md` §5).
The device-side verification surface EXISTS and is complete and
fail-closed (`crates/edge-agent/src/update_verification.rs`, P9.34):
`UpdateGate::verify_before_staging` verifies BEFORE any write, the
default `NoUpdateVerifier` refuses EVERYTHING, and the append-only
`FileUpdateLedger` register is the source of truth for the active
version. What does not exist is everything upstream of that seam: a
bundle format, a metadata distribution mechanism, a key/role plan, and
a fleet-rollout control path. This ADR fixes that architecture at
DESIGN level. It closes the gap in design only; acceptance criterion 2
of the phase plan applies verbatim: **no update bundle is produced,
signed, staged, or applied in this phase.**

## 2. Options considered (P18-E-1)

| Option | Description | Assessment |
|---|---|---|
| (a) Full TUF repository from day one | All four metadata roles (root / targets / snapshot / timestamp [S-161]), N+1 root rotation, expiry discipline operational from the first release | Strongest compromise-recovery posture, but it is an ONGOING OPERATIONAL COMMITMENT (expiry publishing cadence, key-ceremony logistics, rotation rehearsals) with no fleet to protect yet — the operational burden would be carried with zero devices and zero updates |
| (b) **Minimal signed manifest first, TUF-shaped role separation DESIGNED-IN, full TUF repository at a recorded fleet threshold** (the DEFAULT) | The device verifies one signed manifest extending the P9.34 `UpdateManifest`; the manifest format and key plan reserve the TUF role structure (root / targets / snapshot / timestamp, per-device-class target delegations, N+1 rotation) so that migrating to a full repository is an additive operational step, not a format break | Matches the actual state (no fleet, no producer, everything WRITTEN-UNRUN) while refusing the trap of option (c) — the role boundaries and rotation story exist in the design from day one |
| (c) Plain signed bundles, no role separation | One key signs everything; compromise of that key compromises the whole fleet with no key-compromise recovery path | **REJECTED** — this is precisely the failure mode TUF's role separation exists to prevent [S-161]; the rejection is recorded, not silently dropped |

## 3. Decision (ADR-1801)

1. **The device-side trust anchor for OTA remains the P9.34 seam.** All
   update verification happens through `UpdateGate::verify_before_staging`
   (`crates/edge-agent/src/update_verification.rs`): manifest present →
   manifest valid → algorithm matches the configured verifier → signer
   reference trusted → version respects the floor → digest matches,
   refusal BEFORE any write, audited on every verdict. Nothing in this
   phase modifies the gate's order, the fail-closed default
   (`NoUpdateVerifier` semantics stay exactly as shipped), the
   append-only register, or the never-silent-downgrade rule.
2. **The update-bundle format is a versioned signed manifest extending
   the P9.34 `UpdateManifest`** with SUIT-shaped applicability metadata
   and sequence-number anti-rollback [S-162]: applicability (vendor /
   device class / device-ID allowlist), a monotonically increasing
   anti-rollback sequence number checked against a device-side
   monotonic counter (beyond the v1 minimum-version floor), per-payload
   digests, and size caps. Format id `flexcore-edge-update-manifest-v2`
   (record 02; code `crates/edge-agent/src/update_bundle.rs`).
3. **TUF-shaped role separation is DESIGNED-IN, not yet operated**
   [S-161]. The repository plan (record 03) fixes: the root role's key
   is OFFLINE; the targets role signs per-device-class target metadata
   with delegations; snapshot binds targets consistency; timestamp
   carries freshness; root rotation follows the N+1 overlap pattern
   [S-161]. Under the DEFAULT (b) the device initially consumes the
   targets-style signed manifest directly; the role keys, delegation
   boundaries, and rotation procedure are recorded so the full
   repository is a staged operational migration — the format's signer
   reference is a ROLE-QUALIFIED key reference from the start, never a
   single undifferentiated key.
4. **A full TUF-style repository is stood up when the fleet exceeds a
   recorded fleet-size threshold.** The threshold is a recording
   obligation of the human owner (P18-E-1's closing condition); until
   recorded, the trigger is pending and no repository may be stood up
   on an agent's assumption.
5. **Every signing key is a human ceremony** (P10.21 interplay). No
   production key material exists; test-grade signing (the FNV-1a-64
   family) stays loudly labeled test-grade and is never a release
   signature; production crypto is a DEPLOYMENT REQUIREMENT
   (conventions §6.6).
6. **Hardware trust anchors remain P18-E-2 / the P6.34 posture**: every
   hardware determination is PENDING PILOT HARDWARE SPEC; nothing here
   claims or implies a root of trust in hardware.

## 4. Consequences

| Consequence | Detail |
|---|---|
| Positive | The P9.34 gap gets a produced-to-consumed DESIGN at minimal operational cost; role separation and rotation are not retrofit debt |
| Positive | The device-side code change is additive behind an existing, fail-closed seam |
| Cost / risk | Until the full repository exists, freshness (timestamp-role equivalent) and multi-target consistency (snapshot-role equivalent) are carried by the manifest's own mandatory expiry field and the platform-side evidence chain — the honest residual is recorded in record 03 §7 |
| Cost / risk | The fleet-size threshold is a human recording obligation; until recorded, the minimal path remains the enforced default and no threshold may be assumed |
| Respected | The kernel plugin-rollout policy (`crates/flexcore-kernel/src/revocation.rs`) stays cohort/percentage-free BY CONSTRUCTION and byte-identical; the device-fleet cohort orchestrator (P18.06) is a NEW, separately-argued mechanism (record 06 quotes the kernel policy verbatim) |

## 5. UNVERIFIED register (implementation-time checks, per the honesty rules)

| # | Claim used in this ADR | Source | Status |
|---|---|---|---|
| U-1 | TUF's four metadata roles (root / targets / snapshot / timestamp) and their division of trust, N+1 root-revocation rotation pattern, and expiry/freshness discipline | [S-161] (TUF specification v1.0.37) as summarized in `00_GAP_ANALYSIS.md` §11 and phase-18 `context.md` §1 | CITED — the role SEMANTICS as implemented in record 03 must be checked against the spec text before any repository is stood up; UNVERIFIED until that check |
| U-2 | SUIT manifest concepts used: applicability metadata, sequence-number anti-rollback, manifest-enclosed payload digests | [S-162] (IETF RFC 9019) as summarized in the gap analysis | CITED — the field shapes in record 02 are FlexCore formats SHAPED BY these concepts, not SUIT CBOR encodings; any claim of SUIT conformance is forbidden |
| U-3 | "Canary 5–10% of a block per practice" (used by P18.06) | Plan §2 P18.06 row: a DEFAULT recommendation, NOT a sourced figure | Pre-registered DEFAULT; never presented as external evidence |

## 6. Honest status

No repository, no manifest, no bundle, no key, no fleet, no update.
The ADR is the argued decision record the phase plan requires
(record-only depth per plan §1). The P18-E-1 entry in
`ledger_phase_18.md` §Decision points remains OPEN with the DEFAULT
implemented as design; the closer is the human owner.
