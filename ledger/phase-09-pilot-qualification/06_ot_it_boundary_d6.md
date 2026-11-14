<!-- P9.06 — OT/IT boundary review, D6 (review record). REVIEW INPUT ONLY: D6 closure is a human decision. No review convened, no attendees, no sign-off, no site. Baseline reviewed: docs/security/edge-ot-boundary.md (P6.26). -->
# P9.06 — OT/IT Boundary Review — D6 (Review Record — REVIEW INPUT, NOT CLOSURE)

Date: 2026-11-14. Subtask: P9.06 (`ledger_phase_9.md` §P9.06).
Owner role: Security (owner of D6 per `.agent/DECISIONS.md`).
Report trace: §23 Stage 9; §17; §25.2 (D6); §22.3 ("before any read-only
pilot").
Baseline under review: **`docs/security/edge-ot-boundary.md` (P6.26)** —
the Phase 6 OT boundary note — extended by the P6.03 question set
(`ledger/phase-06-edge-protocol/03_d6_boundary_review.md`), the P6.34
secure-boot position, and the P6.18/P6.21 audit/offline/isolation record.
Inputs that do not exist yet: P9.05 filled network design (template only),
P9.07 draft security assessment (not written in this wave), customer OT
architecture/asset inventory/OT security policies — **none available**.

> **D6 closure is a human decision; this record is the review input.**
> D6 remains **OPEN**. This record does not update
> `.agent/DECISIONS.md`, does not sign anything, and does not permit any
> read-only pilot activity (§22.3 keeps the gate; the P9.13/P9.14 entry
> conditions keep requiring a SIGNED D6 record).

## 1. Honest status — the plain statement

**The OT/IT boundary review has NOT been convened and D6 is NOT closed.**
No site exists to review; the ledger's required attendees (Neryva security,
customer OT owner, customer IT owner, site operator — ledger step 1) cannot
be convened; no verdict of the vocabulary the ledger requires ("approved /
approved with condition / rejected") can be recorded for a real site. What
this record delivers is what an agent CAN deliver (the P6.03 precedent):
the full question list, each question answered **from the design as
written** — report §17 + the built edge code + the P6.26 baseline note —
with a design-level review verdict per item, site-confirmation cells EMPTY,
the D6 recommendation with DEFAULT/RECOMMENDATION labels, and the closure
PROPOSAL text for the human decision-maker (§6). Per ENTRY_NOTE boundary 2,
the closure itself stays a human decision recorded OPEN until signed; D6 is
per deployment (§25.2), and the deployment it would cover is
`<DEPLOYMENT-NAME>` — EMPTY.

## 2. Scope, baseline, and method

| Field | Value |
|---|---|
| Deployment covered | `<EMPTY — D6 closure is per deployment, never global (§25.2)>` |
| Sites covered | `<EMPTY — one record per site set named by the signed P9.01 freeze>` |
| Baseline reviewed | `docs/security/edge-ot-boundary.md` (P6.26): five-zone model, outbound-only connectivity, mTLS profile, command-path gate, clock rule, blast radius, secrets rules, open items 1–7 |
| Governing reference | NIST SP 800-82 **Rev. 3** (September 2023) final baseline; Rev. 4 (initial public draft, 21 September 2026; comments due 30 November 2026) is DRAFT input only, never a requirement (§17.1; P6.26 header) |
| Method | Each review question below is answered in two columns: (i) the DESIGN-AS-WRITTEN answer with cited seams; (ii) the site-specific confirmation cell (EMPTY). The review verdict per item is the design-level verdict; the site-level verdict (approved / approved-with-condition / rejected) is NOT RECORDED |
| What would invalidate answers | Any filled P9.04/P9.05 design that deviates (a new flow, a different connectivity pattern, an adapter with write posture) re-opens the affected question rows |

## 3. The full question list — answered from the design as written

### 3.A Data flows, both directions (ledger step 2 walk; P9.05 §4)

| # | Question | Design-as-written answer (cited) | Site confirmation | Review verdict (design level) |
|---|---|---|---|---|
| F1 | What flows cross each zone boundary, and who initiates each? | Telemetry in: field → gateways → site EMS → edge adapter (allowlisted) → outbound mTLS session → central. Commands out: central → the SAME outbound session → edge verification → allowlisted adapter → EMS → device, **only** when the P6.14 gate + per-command verification pass. Initiator of the edge↔central session: the edge, always (§3.3). Health/audit flow: edge → central on the same session | `<EMPTY>` | APPROVED AS DESIGNED (RECOMMENDATION) — outbound-initiated, allowlisted, both directions documented; site-level verdict NOT RECORDED |
| F2 | Is any flow inbound from the internet or from central into OT? | None exists and none is creatable by the shipped code: no listen/bind/accept surface; the only socket op is outbound `connect_timeout` (`crates/edge-agent/src/uplink_mtls.rs` `RustlsUplinkTransport`; lexical-scan test `crates/edge-agent/tests/no_inbound_listener.rs`). P9.05 §6 pre-prints the standing denies | `<EMPTY>` | APPROVED AS DESIGNED (RECOMMENDATION); must be re-walked per site against the customer's own rules |
| F3 | What does the site NOT permit? (P6.03 F3 — prohibitions explicit) | Prohibitions pre-printed in P9.05 §6 (deny-by-default; no DB/NATS/plugin access from field networks; no temporary rule without expiry). Site-specific prohibitions: `<EMPTY>` | `<EMPTY>` | PENDING SITE INPUT — cannot be answered from the design |
| F4 | Jump host / DMZ / unidirectional gateway / broker pattern required? (P6.03 F4) | P9.05 §5 offers the five patterns; selection EMPTY; the built edge is compatible with all five (broker = dial-target configuration; unidirectional gateway = command path structurally impossible, compatible with read-only default) | `<EMPTY>` | DEFAULT: direct outbound (5.1). RECOMMENDATION to confirm at review |
| F5 | Which flows does the site EMS accept? (P6.03 F5) | OpenEMS Edge↔Backend auth is API-key over WebSocket — adapter-internal, never FlexCore's channel; FlexCore's EMS hop is a separate path, not downgraded (R6-C1). The OpenEMS **REST hop is `http://` today — a recorded open item** (P6.26 §9 item 5) | `<EMPTY>` | APPROVED WITH CONDITION (design level): the cleartext EMS leg must be either closed (TLS) or explicitly accepted by the signed D6 record with the residual risk stated |

### 3.B Trust zones (P6.03 Z1–Z3; §17.2)

| # | Question | Design-as-written answer | Site confirmation | Review verdict (design level) |
|---|---|---|---|---|
| Z1 | Does the site operate the full five-zone model? | Design assumes §17.2's five zones (P9.05 §3 skeleton); site membership EMPTY | `<EMPTY>` | PENDING SITE INPUT |
| Z2 | Where does the edge sit? | RECOMMENDATION: separate daemon in the site's OT DMZ (Zone 3), per §3.3 and P6.26 §1 | `<EMPTY>` | RECOMMENDATION (DEFAULT position), unconfirmed |
| Z3 | EMS inside OT with the edge as the only crossing point? | Report topology says yes; the edge host's outbound set is exactly two endpoints (control plane, site EMS — P6.20 §2 shape), making the edge the designed sole crossing point from the FlexCore side. Whether the CUSTOMER's own network matches is a site fact | `<EMPTY>` | RECOMMENDATION; customer-side confirmation required |

### 3.C Edge as an untrusted component (§18.2; ledger step 3)

| # | Question | Design-as-written answer | Site confirmation | Review verdict (design level) |
|---|---|---|---|---|
| U1 | Does the edge authenticate each central session? | Yes: mTLS both directions; edge verifies the central certificate against the root CA pinned FROM the credential record (empty/unparsable store is a named fail-closed error); scheme `wss://`/`https://` required (`crates/edge-agent/src/uplink_mtls.rs` `MtlsUplinkConfig::validate`, `assemble_material`) | `<EMPTY>` | APPROVED AS DESIGNED (RECOMMENDATION) |
| U2 | Does the edge validate every command? | Yes: signature, expiry against a clock whose quality must be `Synchronized`, monotonic sequence with restart-persisted checkpoint, scope/allowlist — before any forwarding (`command_verify.rs` `CommandAuthorization::validate`; `command_path.rs` `CommandPathRuntime::evaluate`) | `<EMPTY>` | APPROVED AS DESIGNED (RECOMMENDATION) |
| U3 | Does central assume a command was applied without acknowledgement and telemetry evidence? | No — §18.2 is implemented as design: a receipt is not an issuance (§14.1); tracking requires ack + telemetry comparison (§3.4 steps 10–11); the pilot must not treat ISSUED as completed | `<EMPTY>` | APPROVED AS DESIGNED (RECOMMENDATION) |
| U4 | Can a compromised edge widen its reach? | Blast radius fixed server-side: one identity = one (tenant, site) binding (`enrollment.rs` `DeviceBinding`; migration 0018 partial unique index; `resolve_edge_session_scope` server-side scoping; RLS ENABLE+FORCE; P6.21 suite). A request-carried field cannot widen scope (no such parameter exists) | `<EMPTY>` | APPROVED AS DESIGNED (RECOMMENDATION); the modified-edge-client lab run (P6.18 H2) remains UNRUN — recorded, not passed |

### 3.D Protocol boundaries (P9.04 linkage)

| # | Question | Design-as-written answer | Site confirmation | Review verdict (design level) |
|---|---|---|---|---|
| PB1 | Which protocol surfaces cross the OT boundary? | Two by design: (1) edge↔central mTLS uplink; (2) edge↔site-EMS adapter hop (P9.04 row 1). Every other protocol (SunSpec/OpenADR/OCPP/2030.5) lives behind the site EMS or is out of scope until P9.04 fills it | `<EMPTY>` | APPROVED AS DESIGNED (RECOMMENDATION) |
| PB2 | Can an adapter or plugin enable the command path or bypass a gate? | No setter exists: enablement is computed once by a private constructor and re-derived read-only (`CommandPathRuntime::enablement_at` read-time re-derivation; §5.7 authority model; §17.5 invariant 7 — no direct physical command from any optimizer/plugin). Per-adapter admission via `SitePolicyEngine::admit_command` | `<EMPTY>` | APPROVED AS DESIGNED (RECOMMENDATION) |
| PB3 | Are vendor default credentials in play? | OpenEMS Getting Started defaults (`admin`/`admin`, `Metadata.Dummy`, `ws://`) must never be the enrollment or pilot model (R6-C2; P6.26 §8). FlexCore generates, stores, ships, and distributes NO key material; config carries non-secret references only | `<EMPTY>` | APPROVED AS DESIGNED (RECOMMENDATION); site must confirm the EMS's own credential posture |

### 3.E Update path (§18.3; P6.34 baseline)

| # | Question | Design-as-written answer | Site confirmation | Review verdict (design level) |
|---|---|---|---|---|
| UP1 | Is the signed-update path real? | **DESIGN ONLY**: P6.20 defines build/package/update/rollback shape; "no PKI is implemented — no release key pair, no signature format chosen, no verification code written, nothing signed" (P6.34 carried status, verbatim). Signed-update verification is P9.34's code+test block (WRITTEN-UNRUN by the concurrent edge agent) | `<EMPTY>` | CONDITION (design level): the pilot must NOT run on a path that cannot verify updates; the D6 record must either accept the compensating control or gate on P9.34 completion |
| UP2 | Secure boot? | Per-profile STRUCTURE only; every hardware determination **PENDING PILOT HARDWARE SPEC** (P6.34 §2 — no board named, nothing verified). Fallback = P6.20 signed updates, itself design-only | `<EMPTY>` | CONDITION: no "secure boot: yes" claim exists or may be made; the record states the fallback and its residual risk |
| UP3 | Staged/rollback capability? | P6.20 §6 shape: previous verified bundle retained; automatic rollback on failed health; downgrade is an explicit operator-signed action. UNRUN (no install/upgrade test has ever executed) | `<EMPTY>` | CONDITION: re-verified at commissioning (P9.13) |

### 3.F Time synchronization (§17.4 clock drift; P6.26 §6)

| # | Question | Design-as-written answer | Site confirmation | Review verdict (design level) |
|---|---|---|---|---|
| TS1 | What is the edge clock policy? | ANY clock quality other than `Synchronized` fails the command-expiry check CLOSED — commands rejected, never accepted on a drifting/unknown clock, rejection audited (`clock_quality.rs`: `ClockQualityState::is_expiry_trusted`, `ClockQualityMonitor::expiry_check_allowed`, `audited_expiry_check`). systemd starts the agent only after `time-sync.target` | `<EMPTY>` | APPROVED AS DESIGNED (RECOMMENDATION) |
| TS2 | What sync source, and what tolerance does command expiry assume? | `TimeSyncProfile` / `SyncSourceConfig` are configuration seams (`clock_quality.rs`); profile/source/tolerance for the site are reviewer decisions — cells EMPTY. Measured offset/drift is P9.18's deliverable, UNRUN | `<EMPTY>` | PENDING SITE INPUT (C5) |
| TS3 | Does a central outage affect time or safety? | No: the edge never trusts expiry on an untrusted clock regardless of connectivity; site-local safety remains independent of FlexCore (§17.5 invariant 10; §21.2) | `<EMPTY>` | APPROVED AS DESIGNED (RECOMMENDATION) |

### 3.G The §17 inventory (controls, threats, invariants)

| §17 item | Design-as-written position (pointer) | Reviewer question remaining |
|---|---|---|
| §17.1 model (NIST 800-82 R3 baseline; R4 draft-only) | Adopted verbatim (P6.26 header; this record §2) | Site's own NIST alignment — site input |
| §17.2 zones + documented-flows-only rule | P9.05 §3/§4/§6 (deny-by-default, standing denies) | Zone membership, per-flow verdicts |
| §17.3 controls (16 bullets) | Edge-side: mTLS, signed commands + expiry + replay protection, network allowlist, audit logging, no-secrets-in-config/logs (redacted `Debug` on secret-bearing types), short-lived credentials/expiry visibility (P6.06). Deployment-side gaps: OIDC/RBAC/MFA (P9.08), secrets manager/PKI (P9.09), encryption at rest (open item), secure boot (P6.34), plugin signature/SBOM + WASM sandbox (Stage 7 scope), vulnerability management (P9.07 step 6) | Each control's DEPLOYED evidence — P9.07 owns the assessment; none is "implemented" until its evidence exists |
| §17.4 threats (10 rows) | Edge-relevant rows have cited controls (forged/replayed: `command_verify.rs`; clock drift: `clock_quality.rs`; OT lateral movement: outbound-only + allowlist; central outage: local fallback, no queue; broker duplication: idempotent in-order commit `OutOfOrderCommit`) | Testing subtask per row — P9.07 step 1 names them; nothing is "tested" yet |
| §17.5 invariants (1–10) | Enforced by protected code per P6.26 §5–§7 and the P6.14/6.15/6.16 chain; the PILOT-SPECIFIC invariant→test map is P9.35's deliverable, not written here | P9.35 map |
| Audit trail across the boundary | Durable bounded edge audit buffer, in-order at-least-once upload (`audit_buffer.rs` `FileAuditBuffer`); P9.32 owns the integrity/outage proof (WRITTEN-UNRUN) | Execution evidence |
| Encryption at rest (§18.3) | NOT implemented — deployment requirement (P6.18 D-8); OS-level site protection per D6 record until a license-audited crypto dependency closes it | Reviewer accepts or conditions |

## 4. D6 recommendation (DEFAULT/RECOMMENDATION labels; nothing decided)

| # | Position | Label |
|---|---|---|
| R1 | Close D6 per deployment/site only after a convened review (ledger step 1 attendee list) with every P9.05 flow walked and every §3 question's site column filled; record per-boundary verdicts approved / approved-with-condition / rejected | RECOMMENDATION (procedure) |
| R2 | Keep the outbound-initiated, mTLS, no-inbound-listener edge position as the pilot boundary pattern | DEFAULT (matches built code; §3.3) |
| R3 | Keep TLS 1.3 minimum on the uplink; a per-site `tls12-floor` only via configuration per the signed record | DEFAULT unless the reviewer pins otherwise (C2) |
| R4 | Keep the command path DISABLED for the pilot entry; any later enablement follows the P6.14 four-condition gate including condition (d) — the site's signed D6 record — and the P9.26 control-scope ceiling (operator-approved at most) | DEFAULT (release ladder; ENTRY_NOTE boundary 3) |
| R5 | Accept no flow with an undeclared EMS-hop protection gap: the `http://` OpenEMS REST leg is either closed or explicitly accepted with residual risk | RECOMMENDATION (condition) |
| R6 | Gate read-only pilot entry on: signed D6 record + signed-update path verified (P9.34) + buffer bound set (P9.05 §7) + encryption-at-rest disposition stated | RECOMMENDATION (condition set) |
| R7 | Carry the P6.26 open items 1–7 into this deployment's D6 record verbatim as the reviewer's worklist | RECOMMENDATION |

## 5. Review convening and sign-off — NOT CONVENED, NOT SIGNED

Ledger step 1 attendee requirements, with honest cells:

| Required attendee | Present? | Name | Signature | Date |
|---|---|---|---|---|
| Neryva security (D6 owner) | **NOT CONVENED** | `<EMPTY>` | **UNSIGNED** | `<EMPTY>` |
| Customer OT owner | **NOT CONVENED** | `<EMPTY>` | **UNSIGNED** | `<EMPTY>` |
| Customer IT owner | **NOT CONVENED** | `<EMPTY>` | **UNSIGNED** | `<EMPTY>` |
| Site operator | **NOT CONVENED** | `<EMPTY>` | **UNSIGNED** | `<EMPTY>` |

Per-boundary decision ledger (ledger step 2 vocabulary; to be filled by the
convened review):

| Zone boundary | Flows walked | Decision (approved / approved-with-condition / rejected) | Condition / required change |
|---|---|---|---|
| Zone 1 ↔ Zone 2 | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Zone 2 ↔ Zone 3 | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Zone 3 ↔ Zone 4 | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Zone 4 ↔ Zone 5 | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

## 6. Closure PROPOSAL text (for the human decision-maker)

The following is the PROPOSED text a human decision-maker may adopt,
amend, or reject when closing D6 for a named deployment — it is not a
closure:

> "D6 for deployment `<DEPLOYMENT-NAME>` (sites `<SITE-ID-…>`) is closed on
> `<DATE>` by the review convened on that date with the attendees named in
> §5. The review walked every zone boundary and every flow of the P9.05
> network design `<REVISION>` and recorded a decision per boundary in §5.
> The review accepts the baseline of `docs/security/edge-ot-boundary.md`
> as amended by its conditions: outbound-initiated mTLS edge connectivity
> with no inbound path (TLS 1.3 minimum unless amended); command path
> disabled for pilot entry, enableable only under the P6.14 four-condition
> gate; edge as an untrusted component with server-side scope enforcement;
> closed-clock expiry rule; audit buffer with bounded in-order upload. The
> following conditions attach: `<FROM §4 R5/R6/R7 AND §3 CONDITIONS>` and
> the following residual risks are carried to the P9.27 register: `<LIST>`.
> D6 covers ONLY this deployment and these sites; any new site, flow, or
> connectivity-pattern change requires re-review. This closure does not
> certify anything, does not approve bounded automation, and does not by
> itself authorize read-only pilot operation — it removes the D6 gate
> condition (§22.3) only."

Until a decision-maker fills and signs that block against a real
deployment, D6 stays OPEN and no read-only pilot activity proceeds.

## 7. What this record does NOT do

- It does not close D6, sign anything, or update `.agent/DECISIONS.md`.
- It does not convene the review, name attendees, or record site verdicts.
- It does not approve any flow, zone membership, or connectivity pattern.
- It does not constitute a security assessment (P9.07 owns that) or the
  §17.5 pilot map (P9.35).
- It does not grant read-only/shadow entry or any field write authority.

## 8. Reviewer verification procedure

1. Confirm §1 and §6 carry the explicit statements that D6 closure is a
   human decision and this record is the review input.
2. Confirm every "Site confirmation" cell and every §5 name/signature/date
   cell is `<EMPTY>` / NOT CONVENED / UNSIGNED, and every design-level
   verdict is labelled RECOMMENDATION/DEFAULT/CONDITION — never "approved"
   for a site.
3. Trace each §3 design-as-written answer to its cited seam or document
   (`uplink_mtls.rs`, `command_path.rs`, `command_verify.rs`,
   `clock_quality.rs`, `policy.rs`, `buffer_file.rs`, `audit_buffer.rs`,
   `no_inbound_listener.rs`, `edge-ot-boundary.md`, P6.03, P6.34,
   P6.18/P6.21).
4. Confirm the P6.34 carried status is quoted correctly (no PKI
   implemented; hardware cells PENDING) and no secure-boot or
   signed-update claim upgrades it.
5. Confirm Rev. 4 appears only as draft input, never as a requirement.
6. Confirm §4 labels and the proposal text carry no closure language.
7. Confirm the ledger tick wording is "implemented, awaiting human
   build/test".

## 9. Filling commands (for the human decision-maker)

- Convene the review per ledger step 1 (four attendee roles) after P9.05's
  filled design and P9.07's draft assessment exist; fill §3 site columns,
  §5 per-boundary decisions, and adopt/amend the §6 proposal text; sign.
- On signature: file the signed record as the P9.06 evidence named in
  `CHECKLIST.md` ("D6 closed"); record the closure in `.agent/DECISIONS.md`
  via an ADR per its rule; only then may P9.13/P9.14/P9.17 entry conditions
  treat D6 as closed for the named sites.
- Any "rejected" boundary blocks read-only pilot work at that site until
  re-reviewed and approved (ledger step 5).

## 10. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| D6 OPEN — no signed per-deployment closure exists; every read-only-pilot entry condition stays blocked | §1, §6; §22.3 |
| No real site/architecture to review against; the design-level verdicts could diverge from the customer's actual network | §3 site-confirmation cells |
| OpenEMS REST hop cleartext (`http://`) inside the OT boundary until closed | §3.A F5 condition; P6.26 §9 item 5 |
| Signed-update path is DESIGN ONLY (no PKI, no verification code) — the pilot must not run without the P9.34 control verified | §3.E UP1; P6.34 carried status |
| Secure boot undetermined per profile (no hardware spec); compensating control also unexecuted | §3.E UP2 |
| Encryption at rest for edge journals unimplemented — deployment requirement | §3.G row; P6.18 D-8 |
| Modified-edge-client lab acceptance (P6.18 H2) and app-role/RLS ownership review (H3) remain UNRUN human gates | §3.C U4 |
| Rev. 4 draft pressure: a final Rev. 4 during the pilot may change baseline expectations — tracked, not a requirement | §2; §17.1 |

Status: **WRITTEN-UNRUN** — a review record cannot be "run"; the review
itself (convening, walking the real site, verdicts, signatures) is the
human gate and has not happened and is not scheduled. D6 remains OPEN; no
pilot exists and none is claimed.
