<!-- P19.03 — IEC 62443-4-2 component requirements mapping. HONEST RECORD:
     the edge agent and the control plane are modeled as COMPONENTS in the
     62443-4-2 sense for MAPPING PURPOSES ONLY — no component assessment is
     performed, no CR is claimed satisfied, and no SL is achieved. The CR
     enumeration is REQUIRES-STANDARD-TEXT (the research covers [S-163] the
     4-2:2019 publication entry, not the standard text); every CR id below
     is UNVERIFIED against the standard. Per-CR justification ties to
     existing FlexCore seams BY CITATION. The not-a-certification
     discipline of P10.25 §7 (§8) carries verbatim. -->
# P19.03 — IEC 62443-4-2 Component Requirements Mapping (edge agent + control plane as mapped components; FR/CR matrix with seam citations; SL targets are RECOMMENDATION only)

Date: 2026-11-17. Subtask: P19.03 (`ledger_phase_19.md` §P19.03).
Plan trace: `../plan.md` §2 row P19.03, §3 blocking order row 3 (consumed by
P19.04 SL placement and P19.17 CR-derived test list — both consumed by
path). Sources: [S-163] (IEC 62443-4-2:2019 publication entry — the
webstore listing, NOT the standard text); the P18.02 update-manifest record
(`../../phase-18-edge-fleet-lifecycle-ota/records/02_p18_02_update_bundle_format.md`);
the P13/P14 identity and permission seams (record paths in §4); the P0.17
threat model (`ledger/phase-00-product-freeze/17_threat_model.md`).

## 1. Honest status — the plain statement

**No component of FlexCore has been assessed against IEC 62443-4-2, and no
CR is claimed satisfied.** This record models two components (§2) and
assembles the FR/CR matrix skeleton (§3–§4) with per-CR justification
pointing at the FlexCore seam that would carry the evidence — every
justification is a CITATION, never a capability claim, and every evidence
cell is PENDING-<producer> or `<EMPTY>` until the producing record's human
gate fills it. All CR identifiers and requirement titles are
**UNVERIFIED / REQUIRES-STANDARD-TEXT** (the research's [S-163] entry is
the publication record, not the text); the matrix must be re-validated
against the standard before any assessment use.

## 2. The mapped components (mapping-only modeling)

| Component (4-2 sense) | What it covers in FlexCore | Component-record anchor |
|---|---|---|
| C-1 — Edge agent | The site edge agent: command receipt/validation, offline behavior, update verification (`UpdateGate`, update manifest v2), audit proof path | `crates/edge-agent/` (WRITTEN-UNRUN); `../../phase-18-edge-fleet-lifecycle-ota/records/02_p18_02_update_bundle_format.md`; P10.21 §5 (`ledger/phase-10-v1-release-hardening/21_signed_artifacts.md`) |
| C-2 — Control plane (host system application) | The central application zone stack: API/command lifecycle, authorization, tenancy, audit chain, scheduling, plugin host | The Phase 0–10 workspace crates (WRITTEN-UNRUN); the §17 threat model zones 1–2 (`ledger/phase-00-product-freeze/17_threat_model.md` §3) |

Scope note: site devices, site EMS/SCADA, and gateways (P0.17 zones 4–5)
are NOT FlexCore components — they are deployment-owned and outside any
component claim this record could make; the zone/conduit record
(`04_p19_04_zone_conduit_deployment_architecture.md`) treats them as
external zones.

## 3. FR-level posture (the seven foundational requirements — UNVERIFIED skeleton)

| FR (UNVERIFIED id/title) | FlexCore posture (citation-backed, unclaimed) | Evidence status |
|---|---|---|
| FR 1 — Identification and authentication control | mTLS for edge/service identities; OIDC federation; step-up approval (§17.3 controls 1, 3, 4); the P14.03 fail-closed `MtlsRequirements` | PENDING-P13 (identity records), PENDING-P14 (transport seam execution) |
| FR 2 — Use control | RBAC + resource/site/asset-scoped authorization (§17.3 control 2); command lifecycle gates (validation/feasibility/approval distinct); never-granted permission set | PENDING-P13 (P13.05 role model), PENDING-P10-suite-run (P10.04 suite WRITTEN-UNRUN) |
| FR 3 — System integrity | Signed update manifests (P18.02), plugin signature/provenance + SBOM checks (§17.3 control 7), production audit chain (P11.15 HMAC-SHA-256, keying UNSIGNED) | PENDING-P18.02-human-build, PENDING-P11.15-key-signoff |
| FR 4 — Data confidentiality | Encrypted transport and storage (§17.3 control 10); no secrets in plugin config (control 11); secret references, never material | PENDING-P13.16 (secrets program), PENDING-deployment |
| FR 5 — Restricted data flow | Zone/conduit discipline: documented flows only, outbound-only edge connection (P0.17 §3) | Design-frozen (P0.17); deployment evidence `<EMPTY>` |
| FR 6 — Timely response to events | Audit logging of control-plane and dispatch actions (§17.3 control 12); IR/notification pipeline (P13.11; P19.16 plan v2 — Agent B) | PENDING-P11 (log/audit chain operation), PENDING-P19.16 |
| FR 7 — Resource availability | Backup/DR (P12 records), rate limiting (P11.14), stuck-state recovery (P11.20), site-local fallback (§17.4 threat 10) | PENDING-P12 (drills UNRUN) |

## 4. CR-level matrix — the CRs the FlexCore seams anchor (each UNVERIFIED against the standard text)

| CR (UNVERIFIED id) | CR subject (skeleton label) | Component | FlexCore seam the evidence would come from (citation) | Justification status | Evidence cell |
|---|---|---|---|---|---|
| CR 1.1 | Human user identification and authentication (label UNVERIFIED) | C-2 | P13 identity records — `../../phase-13-enterprise-identity-integration/records/01_p13_01_identity_architecture.md`, `02_p13_02_oidc_token_validation.md` | Design seam exists; claims-derived principal, verified tokens | `<PENDING-P13 run evidence>` |
| CR 1.2 | Software process and device identification (label UNVERIFIED) | C-1, C-2 | Edge enrollment journal + machine identity (P13.07; P18 device registry, migration 0018) | Design seam exists | `<PENDING-P18>` |
| CR 1.14 | Strength of authentication (label UNVERIFIED) | C-2 | Step-up MFA (P13.04) | Design seam exists | `<PENDING-P13>` |
| **CR 2.1** | **Authorization enforcement** (label UNVERIFIED) | C-1, C-2 | P13.05 role model (`../../phase-13-enterprise-identity-integration/records/05_p13_05_role_model.md`); P14.16 adapter permission model (`../../phase-14-protocol-device-connectivity/records/64_p14_16_adapter_permission_model.md`); the never-granted permission set (`../../01_PROGRAM_CONVENTIONS.md` §6.4); the P10.04 authz/tenant suite (`ledger/phase-10-v1-release-hardening/04_authz_tenant_tests.md`) | The strongest anchor in the matrix: authorization is structurally separated from dispatch, scoped per resource/site/asset, and the suite asserts the boundary — but the suite is WRITTEN-UNRUN, so NO enforcement is evidenced | `<PENDING-P10.04 human run>` |
| CR 2.4 | Mobile code / plugin discipline (label UNVERIFIED) | C-2 | Plugin boundary suite (P10.08); WASM sandbox (§17.3 control 8) | Design seam exists | `<PENDING-P10.08 human run>` |
| CR 2.11 | Usage restrictions / never-granted capabilities (label UNVERIFIED) | C-1, C-2 | `dispatch.authorize` / `dispatch.issue` / `secret.read` / `network.any` stay never-granted (conventions §6.4) | Recorded invariant; no bypass path introduced by any phase | `<PENDING-first human build>` |
| **CR 3.4** | **Software and information integrity** (label UNVERIFIED) | C-1, C-2 | P18.02 update-bundle format v2 (`../../phase-18-edge-fleet-lifecycle-ota/records/02_p18_02_update_bundle_format.md`): manifest digest, closed `UPDATE_DIGEST_ALGORITHMS` (`sha-256` production / `fnv1a64-test` refused for real), anti-rollback sequence counter, fail-closed `NoUpdateVerifier` default, mandatory `approval_reference` (empty = refused); plus the P10.21 production-signing DEPLOYMENT REQUIREMENT | The CR 3.4 anchor story is recorded and strong AS DESIGN — but no bundle has ever been produced, signed, staged, or applied; the test-grade FNV-1a digests are NOT release signatures and never count | `<PENDING-P18.02 build + P10.21 signing>` |
| CR 3.1 | Communication integrity (label UNVERIFIED) | C-1, C-2 | mTLS transport (§17.3 control 4); the P14.03 `MtlsRequirements` fail-closed posture | Design seam exists | `<PENDING-P14 execution>` |
| CR 4.1 | Information confidentiality (label UNVERIFIED) | C-2 | Encrypted transport/storage (§17.3 control 10); journal-encryption-at-rest recorded OPEN (P10.07) | Partial design; one recorded OPEN gap | `<EMPTY — gap carried>` |
| CR 5.1 | Network segmentation (label UNVERIFIED) | C-1 | Outbound-only edge channel, allowlisted paths (P0.17 §3; §17.3 control 9) | Design-frozen | `<EMPTY — deployment evidence>` |
| CR 6.1 | Audit log accessibility (label UNVERIFIED) | C-2 | P11.15 production audit chain (`../../phase-11-operational-robustness/records/31_p11_15_audit_chain_upgrade.md`) — keyed HMAC-SHA-256, per-tenant namespace; keying UNSIGNED | Design + code written-unrun; tamper-EVIDENCE, never tamper-PROOF (that record's own §3.6) | `<PENDING-P11.15 key signoff>` |
| CR 7.1 | Denial-of-service protection (label UNVERIFIED) | C-2 | Rate limiting (P11.14, `../../phase-11-operational-robustness/records/26_p11_14_rate_limiting.md`); backpressure (P14.20) | Design seam exists | `<PENDING-P11 execution>` |
| CR 7.3 | Control-system backup (label UNVERIFIED) | C-1, C-2 | P12 backup architecture (`../../phase-12-backup-dr-resilience/records/01_p12_01_backup_architecture.md`) | Design exists; drills UNRUN | `<PENDING-P12.07/08 drills>` |

**CRs not listed above** (the remaining 4-2 CR and requirement-entry set):
not mapped in this record because no FlexCore seam citation exists for
them yet — they are carried as the standard's full requirement set,
REQUIRES-STANDARD-TEXT, and the matrix is extended only when a producer
record gives a seam to cite. Absence from this table is a mapping gap, NOT
a finding of non-compliance and NOT a claim of compliance.

## 5. SL targets — RECOMMENDATION, never achieved posture

| Component | SL target (RECOMMENDATION, per [S-22]'s SL1–SL4 concept) | Basis | Achieved |
|---|---|---|---|
| C-1 Edge agent | SL 2 (RECOMMENDED) | OT-adjacent supervisory role; moderate capability requirements assumed — the actual SL per CR requires the standard's capability definitions (REQUIRES-STANDARD-TEXT) | **NONE — no SL achieved or claimed** |
| C-2 Control plane | SL 2 (RECOMMENDED) | Same basis; enterprise deployment posture | **NONE — no SL achieved or claimed** |

The zone-level SL placement these feed is
`04_p19_04_zone_conduit_deployment_architecture.md` §4 (by path; not
duplicated here).

## 6. Thresholds (X-8)

None set here. The CR-derived test list P19.17 consumes (plan §3 row 3)
is produced by P19.17 under its own record; no pass/fail threshold is
invented in this mapping.

## 7. Not-a-certification statement (verbatim discipline carried)

Quoted verbatim from `AGENTS.md` §3 (rules 2–3), as the P10.25 §7
discipline requires of every phase-19 record:

> 2. **No safety or certification claims.** FlexCore is not certified,
> not a safety case, and not permission for live grid control. Never
> write text that implies otherwise.
> 3. **No protection-grade claims.** FlexCore is a supervisory
> orchestration layer. It does not perform primary frequency response,
> synthetic inertia, relay protection, inverter inner-loop control, or
> sub-cycle control.

No component assessment, CR satisfaction, SL achievement, or conformance
of any kind is claimed anywhere in this record.

## 8. Reviewer-verification procedure (human)

1. Confirm §1/§4: every CR id and label is UNVERIFIED /
   REQUIRES-STANDARD-TEXT; every evidence cell is `<PENDING-…>` or
   `<EMPTY>`; no "satisfied" wording appears.
2. Spot-check the CR 2.1 and CR 3.4 citation paths; confirm the source
   records still carry the honesty statuses quoted here (P10.04 suite
   WRITTEN-UNRUN; no bundle produced/signed/staged/applied).
3. Confirm §5 SL rows read RECOMMENDED, with the Achieved column NONE.
4. Confirm §7's verbatim block matches `AGENTS.md` §3 rules 2–3 exactly.

## 9. Status

**RECORD-ONLY / WRITTEN.** Components are modeled, CRs are mapped with
citations, and nothing is assessed or claimed. Ticking P19.03 means
"implemented (mapping written), awaiting human standard-text validation
and producer evidence".
