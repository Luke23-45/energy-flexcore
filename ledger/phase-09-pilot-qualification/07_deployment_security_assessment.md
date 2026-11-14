<!-- P9.07 — Deployment security assessment. ASSESSMENT TEMPLATE only: threat-model structure, control/threat walk skeleton, attack trees by citation of the P9.31 rejection matrix. NO assessment was executed: every risk rating, likelihood/impact, evidence, and residual-risk cell is EMPTY. NIST SP 800-82 Rev. 3 is the baseline; Rev. 4 is DRAFT input only. -->
# P9.07 — Deployment Security Assessment (Assessment TEMPLATE — no assessment executed)

Date: 2026-11-14. Subtask: P9.07 (`ledger_phase_9.md` §P9.07).
Owner role: Security. Report trace: §23 Stage 9; §17.1–§17.4.
Inputs: P9.01 scope freeze (record exists; the deployment it names is
`<EMPTY>`); P9.04 integration design (TEMPLATE —
`04_customer_integration_design.md`); P9.05 network design (TEMPLATE —
`05_site_network_design.md`); P9.06 D6 boundary review (REVIEW INPUT,
NOT CLOSURE — `06_ot_it_boundary_d6.md`); `RESEARCH.md`.

## 1. Honest status — the plain statement

**No deployment security assessment has been performed, and none can be
performed by an agent.** There is no deployed configuration to assess: no
site exists (P9.01 deployment cell EMPTY), no network is designed against
a real customer architecture (P9.05 is a template), no integration exists
(P9.04 is a template), and the D6 boundary review has not been convened
(P9.06). Every verdict this subtask's ledger requires — control status
(implemented / partial / missing-with-compensation), threat evidence,
severity, likelihood, impact, residual risk, and the must-fix vs
accepted-residual disposition — is a HUMAN assessor judgement about a REAL
deployment. This record delivers what an agent CAN deliver: the complete
assessment STRUCTURE — the asset inventory skeleton, the trust-boundary
walk (from the P9.05 zones), the actor model skeleton keyed to the §17.4
threat rows, the attack trees for the edge command path (reusing the
P9.31 rejection matrix BY CITATION), the §17.4 threat-by-threat table and
§17.3 control-by-control table with every assessment cell EMPTY, the
NIST SP 800-82 baseline rule, the vulnerability-management cadence
template, and the findings-register template. **Nothing here is a risk
rating, a finding, or an approval.**

## 2. What an assessment can and cannot establish (recorded up front)

The assessor signs this section as part of the assessment; the agent
records it as the discipline the assessment must carry:

| An assessment of THIS deployment CAN establish | It CANNOT establish |
|---|---|
| That each §17.3 control has DEPLOYED evidence (configuration inspected, test output inspected — not a claim read) | That any control is perfect; "implemented" means evidence exists for the inspected configuration, not a guarantee |
| That each §17.4 threat has a named control AND a named testing subtask whose evidence exists — a control with no testing subtask is recorded UNTESTED, never implemented | That an untested or finite-tested control cannot be bypassed: a test suite is a finite test set; zero observed misses is a TEST OUTCOME (the §13.3/P9.31 discipline) |
| That findings are dispositioned (must-fix before pilot vs accepted residual risk carried to P9.27) with owners and due states | That a residual risk is "acceptable" for the customer — acceptance is the customer's signed acknowledgement (P9.27 step 4), never the assessor's |
| That the assessment covers the DEPLOYED configuration (versions, firewall rules, IdP settings as actually deployed) | Anything about a configuration that was not inspected; the ledger's named failure mode is an assessment based on the generic product design instead of the deployed configuration |
| Compliance POSITION against the NIST SP 800-82 Rev. 3 baseline | Certification, approval, or safety: FlexCore is supervisory orchestration only; no assessment output is a certification or safety claim (AGENTS.md §3) |

## 3. Assessment scope and inputs

| Field | Value |
|---|---|
| Deployment assessed | `<EMPTY>` |
| Sites in scope | `<EMPTY>` (per the signed P9.01 freeze — none exists) |
| Deployed versions assessed (control plane, edge agent, adapters, OS) | `<EMPTY>` — the assessment cites the deployed build, never "the repo" |
| Config sources inspected | `<EMPTY>` (firewall export, IdP config, deployment manifests — the ledger's verification method requires inspection, not claims) |
| Baseline reference | NIST SP 800-82 **Rev. 3** (final, September 2023). Rev. 4 initial public draft (21 September 2026; comments due 30 November 2026) is DRAFT INPUT ONLY — recorded separately (§7), never a requirement (§17.1; P6.26 header) |
| DER-cybersecurity guide | IEEE 1547.3-2023: **UNREAD** — ledger step 4 requires it to be READ and its applicable recommendations recorded WITHOUT assuming its contents; the applicability table in §8 is the empty structure for that recording. No claim about its contents is made or may be made |
| Assessment team | `<EMPTY>` (ledger verification method: independent review by a SECOND security reviewer — both named at signature) |

## 4. Asset inventory skeleton (to be classified by the assessor)

Assets are the assessment's valuation input; the classification and
impact ratings are EMPTY. The asset rows are the design's own list
(each cited to its seam, so the assessor inspects the real thing):

| # | Asset | Where it lives (cited) | Classification | Impact if compromised |
|---|---|---|---|---|
| A1 | Dispatch command path (PROPOSED→…→COMPLETED lifecycle + audit) | §14; P4.11/P4.13/P4.16 gates; `crates/edge-agent/src/command_verify.rs` + `command_path.rs` | `<EMPTY>` | `<EMPTY>` |
| A2 | Edge identity + mTLS uplink credential | `crates/edge-agent/src/enrollment.rs`, `enrollment_ceremony.rs` (P9.33), `uplink_mtls.rs` | `<EMPTY>` | `<EMPTY>` |
| A3 | Telemetry pipeline (ingest → quality states → storage) | §10; P2.07 quarantine; quality states `GOOD…STALE` | `<EMPTY>` | `<EMPTY>` |
| A4 | Audit evidence (platform append-only trail + edge audit buffer) | P1.10 `AuditSink`; P4.36 read resource; `crates/edge-agent/src/audit_buffer.rs` (P9.32) | `<EMPTY>` | `<EMPTY>` |
| A5 | Authorization policy set + role grants | `crates/flexcore-authz` (P1.11/P4.13); `apps/control-plane/src/auth.rs` | `<EMPTY>` | `<EMPTY>` |
| A6 | Secrets and signing keys | P9.09 inventory (`09_secrets_certs_keys.md`); kernel references-only rule | `<EMPTY>` | `<EMPTY>` |
| A7 | Update path (manifest, trusted-signer set, register) | `crates/edge-agent/src/update_verification.rs` (P9.34) | `<EMPTY>` | `<EMPTY>` |
| A8 | Grid model + portfolio/schedule data | P9.03 model build; §12.6 records | `<EMPTY>` | `<EMPTY>` |
| A9 | Site EMS / OT equipment (customer-owned) | P9.05 Zones 4–5; OpenEMS hop (P9.04 row 1) | `<EMPTY>` | `<EMPTY>` |
| A10 | Plugin host + plugin configs/manifests | `crates/flexcore-kernel` (manifest, plugin_config, plugin_path_protection) | `<EMPTY>` | `<EMPTY>` |

## 5. Trust boundaries — walked from the P9.05 zones

The boundaries are the P9.05 five-zone model (Zone 1 Enterprise IT,
Zone 2 Central application, Zone 3 Customer integration/DMZ — edge agent,
Zone 4 OT/site control, Zone 5 Field/device; P9.05 §3). Zone MEMBERSHIP
for the real site is EMPTY (P9.05 §3) and the per-boundary walk is the
P9.06 review's output (P9.06 §5 — the decision table there is the same
walk). This section is the ASSESSMENT's copy of that walk, to be filled
from the deployed firewall/network state, not from the design:

| Boundary | Designed crossing (cited) | Deployed crossing (inspect) | Assessment verdict |
|---|---|---|---|
| Zone 1 ↔ Zone 2 | Operator browser/IdP → control plane (OIDC/OAuth2, §17.3) | `<EMPTY>` | `<EMPTY>` |
| Zone 3 ↔ Zone 2 | F1: outbound-initiated mTLS edge uplink; no inbound path (P9.05 §2; `no_inbound_listener.rs`) | `<EMPTY>` | `<EMPTY>` |
| Zone 3 ↔ Zone 4 | F2: edge → site EMS adapter hop (P9.04 row 1; the OpenEMS REST leg is `http://` today — recorded open item, P6.26 §9 item 5) | `<EMPTY>` | `<EMPTY>` |
| Zone 4 ↔ Zone 5 | F3: EMS → device gateways (customer-owned) | `<EMPTY>` | `<EMPTY>` |
| Zone 2 ↔ external | F4: market/program, ONLY if D1 closes | `<EMPTY>` | `<EMPTY>` |

Standing denies the deployed configuration must show (P9.05 §6): no
inbound internet into any OT network; no DB/NATS/plugin-network access
from field networks; every allow rule carries an owner and a review date.

## 6. Threat actors and the attack trees

### 6.1 Actor model skeleton

The actor classes below are the assessment's working structure, derived
from the §17.4 threat rows — §17.1's own text governs, and the assessor
reconciles this skeleton against it before signing:

| Actor class | Maps to §17.4 rows | Assessed capability/motive |
|---|---|---|
| External network attacker (internet-facing surfaces) | forged/replayed command; broker duplication/replay | `<EMPTY>` |
| Compromised or malicious insider (operator/tenant credentials) | unauthorized operator action; cross-tenant leakage | `<EMPTY>` |
| Compromised edge device (physical/remote takeover of the DMZ host) | forged command from a stolen identity; OT lateral movement | `<EMPTY>` |
| Compromised or buggy plugin / supply chain | compromised plugin; plugin supply-chain compromise | `<EMPTY>` |
| Misbehaving data source or infrastructure (drift, outage, duplicating broker) | malicious or incorrect telemetry; clock drift; central outage | `<EMPTY>` |

### 6.2 Attack tree — the edge command path (reuses the P9.31 rejection matrix BY CITATION)

Goal node: **an unauthorized command produces a physical effect at the
pilot**. Every leaf is a named refusal in the built Phase 6 + P9.31 code;
the P9.31 attack matrix (§1 and §3 of
`31_edge_rejection_proof.md`) is the exercise record for these leaves —
cited here, not re-described. The tree below is the assessment
STRUCTURE; the assessor rates each leaf's likelihood/impact and checks
the testing-subtask evidence. The ratings are EMPTY.

```text
GOAL: unauthorized command executes at the edge/asset
├─ 1. Inject a command that never passed the central gates
│   ├─ 1.1 forge/alter payload or signer
│   │     → refused: CommandVerificationRejection::Signature
│   │       (command_verify.rs check 1; P9.31 test 1)
│   ├─ 1.2 present it outside its validity window
│   │     → refused: CommandVerificationRejection::Expiry
│   │       (check 2, clock-quality fail-closed; P9.31 test 2)
│   └─ 1.3 bypass the edge gate entirely (no gate receipt)
│         → refused: no route exists without a P6.14 gate receipt
│           (command_path.rs; P9.31 §1 "reachability" row)
├─ 2. Reuse a legitimately signed command
│   ├─ 2.1 byte-identical replay (equal sequence)
│   │     → refused: CommandVerificationRejection::Duplicate
│   │       (check 3; P9.31 test 3, incl. across edge restart)
│   ├─ 2.2 out-of-sequence (behind last accepted; unsequenced 0)
│   │     → refused: CommandVerificationRejection::Sequence
│   │       (check 3, evaluated LAST; P9.31 test 4)
│   └─ 2.3 re-signed command under a USED id, different payload
│         → refused: ConflictingIdempotencyPayload
│           (FileCommandIdBinding; P9.31 test 5; platform §14.3
│            registry = Phase 7 follow-up — the edge journal is a
│            LOCAL stand-in, not the authoritative control)
├─ 3. Make the edge forward something unverified
│   └─ 3.1 any route to an adapter requires a VerifiedCommand through
│         command_forward.rs → forward_verified (its own allowlist +
│         policy re-checks); a rejected verification produces NO
│         VerifiedCommand → zero adapter reach (P9.31 test 7)
└─ 4. Attackers BELOW the FlexCore layer (bootloader/OS replaces the
      agent wholesale; cleartext EMS leg; host-level journal truncation)
      → NOT refused by the above controls. Residual-risk leaves: the
      assessor rates them and the compensating controls (secure-boot
      position P6.34 — PENDING PILOT HARDWARE SPEC; encryption at rest
      P6.18 D-8; truncate-and-AUDIT recovery) — ratings EMPTY
```

Honesty the tree carries (mirroring P9.31 §4): the signature underneath
is the P6.15 keyed-checksum scheme (`edge-sig:...:fnv1a64:<digest>`) —
tamper-EVIDENCE, not a cryptographic guarantee; the production
replacement is the recorded DEPLOYMENT REQUIREMENT (P9.09 §6 carries it).
Head-gaps ahead of the last accepted sequence are ADMITTED by design
(monotonic window; P9.31 §5 finding 2) — the assessor rates that
explicitly rather than assuming "out-of-sequence = always refused".

### 6.3 Attack trees for the other §17.4 rows

Structure only (goal → sub-goals → controls → testing subtask); content
to be authored by the assessor against the deployed configuration:

| Goal | Sub-goals the assessor elaborates | Primary controls (cited) |
|---|---|---|
| Act beyond a role's authority | approve as Operator; cross-tenant read; shared/generic approval account | deny-by-default `PolicySet` (P1.11); `command.approve` TenantAdmin-only at site/asset (P4.13); tenant match `ScopeDenied` (no role bypasses) |
| Exfiltrate or poison another tenant's data | cross-tenant scope; RLS bypass; log leakage | claims-only `TenantContext` (P0.16 rule 5); PostgreSQL RLS ENABLE+FORCE; log rules §21.3 |
| Move from OT into IT surfaces | reach DB/NATS/plugin network from field; undocumented flows | P9.05 §6 standing denies; two-endpoint edge allowlist; `no_inbound_listener.rs` |
| Poison decisions with bad telemetry | stale/estimated values read as GOOD; freshness lies | telemetry quality states (§10.3); quarantine (P2.07); P9.18 verification |
| Persist a compromise across updates | below-floor downgrade; unverified bundle; silent rollback | `UpdateGate` verify-before-any-write; `NoUpdateVerifier` default; never-silent downgrade (P9.34 §2–§3) |
| Survive detection | truncate edge audit/identity registers | append-only registers, CRC-32 framing, truncate-and-AUDIT with counted loss (P9.32/P9.33) |

## 7. Baseline discipline — Rev. 3 final, Rev. 4 draft only

| Item | Rule recorded |
|---|---|
| Baseline | NIST SP 800-82 Rev. 3 (final, September 2023) — the assessment's reference framework |
| Rev. 4 initial public draft (21 September 2026; comments due 30 November 2026) | DRAFT INPUT ONLY. The assessor records any Rev. 4 change that WOULD affect this deployment in the table below, marked draft; it never becomes a requirement from this record (`RESEARCH.md` is the source note) |
| Rev. 4-draft changes relevant to this deployment | `<EMPTY — to be recorded by the assessor after reading the draft; none may be asserted now>` |

## 8. IEEE 1547.3-2023 applicability table (EMPTY — the read is a human step)

Ledger step 4 forbids assuming the guide's contents. The reading and the
applicability decisions below are the assessor's:

| Guide recommendation (numbered per the standard, after reading) | Applies to this deployment? | Evidence / configuration inspected | Owner |
|---|---|---|---|
| `<EMPTY — UNREAD>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

## 9. The §17.4 threat-by-threat table (controls cited; assessment cells EMPTY)

Ledger step 1's ten threats. Every row: the DESIGN control is cited (so
the assessor inspects the real seam); the evidence, status, and ratings
are EMPTY; the testing subtask is NAMED — a row whose testing subtask
shows no execution evidence is UNTESTED, never implemented.

| # | §17.4 threat | Design control (cited) | Testing subtask (named) | Evidence the control is deployed | Status (implemented/partial/missing) | Likelihood / impact / residual |
|---|---|---|---|---|---|---|
| T1 | Forged / replayed command | P6.15 four-check verifier (`command_verify.rs`: signature → expiry vs `Synchronized` clock → monotonic sequence; `command_path.rs` gate); P9.31 idempotency guard (`pilot_rejection_proof.rs` `FileCommandIdBinding`) | P9.31 (code-level proof), P9.36 (live drill), P9.19 (shadow dry run) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| T2 | Compromised plugin | §5.7 capability model; never-granted permissions incl. `dispatch.authorize`/`dispatch.issue`/`secret.read`/`network.any` refused at the manifest gate (`crates/flexcore-kernel/src/manifest.rs`); no direct physical command from any optimizer/plugin (§17.5 invariant 7) | Stage 7 scope per P6.26 §3.G; no pilot subtask currently injects a compromised plugin — recorded UNTESTED unless the assessor names one | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| T3 | Malicious or incorrect telemetry | Telemetry quality states (exactly one of `GOOD/SUSPECT/BAD/MISSING/ESTIMATED/STALE` per value); ingest quarantine (P2.07, admin-only `quarantine.read`) | P9.18 (stale/missing injection holds/rejects) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| T4 | Cross-tenant leakage | Claims-only `TenantContext` (P0.16 rule 5); `PolicySet` tenant match — `ScopeDenied`, no role bypasses; `tenant_id` on every tenant-owned record + PostgreSQL RLS ENABLE+FORCE (§16.2) | P9.10 step 3 (tenancy test) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| T5 | OT lateral movement | Outbound-only edge with NO listen/bind/accept surface (`uplink_mtls.rs`; `no_inbound_listener.rs`); two-endpoint allowlist; P9.05 §6 standing denies | P9.14 (lab, incl. fault injection); P9.06 (boundary walk) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| T6 | Broker duplication / replay | Idempotent in-order commit (`OutOfOrderCommit` posture, cited in P9.06 §3.G); at-least-once semantics | P9.32 (audit path: zero loss, zero duplication after outage); telemetry-broker duplication mapping is a reviewer decision cell — UNTESTED until named and run | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| T7 | Unauthorized operator action | Deny-by-default RBAC (P1.11); separation of duties: `command.approve` TenantAdmin-only at site/asset scope (P4.13); distinct validation/feasibility/authorization gates (§14.1); append-only audit with read-only surface action `audit.event.read` (P4.36) | P9.08 (negative role-matrix tests); P9.19 (dry run) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| T8 | Plugin supply-chain compromise | P7.14 signing/provenance seam (`crates/flexcore-kernel/src/signing.rs`; `NoVerifier` default = absence of signing infrastructure never looks like success); edge-update analogue P9.34 | P9.34 (edge update verification); SBOM/provenance check for plugins is a PROCESS step — no pilot subtask executes it; recorded UNTESTED | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| T9 | Clock drift | ANY clock quality other than `Synchronized` fails command expiry CLOSED (`clock_quality.rs` `is_expiry_trusted`/`expiry_check_allowed`); systemd `time-sync.target` ordering | P9.18 (clock-sync verification, measured offset/drift) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| T10 | Central outage | Site-local safety independent of FlexCore (§21.2; §17.5 invariant 10); bounded store-and-forward journals, audited drop-of-oldest, no new commands while disconnected (`buffer_file.rs`, `audit_buffer.rs`) | P9.20 (fallback drills); P9.32 (outage upload) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

## 10. The §17.3 control-by-control table (design position cited; status EMPTY)

Ledger step 2's sixteen controls. The design position is what the code
and records today actually provide; "implemented" status is the
ASSESSOR's verdict about the DEPLOYED configuration and is EMPTY.

| # | §17.3 control | Design position (cited; what exists vs what is a requirement) | Deployed status | Compensating control + owner + expiry |
|---|---|---|---|---|
| C1 | OIDC/OAuth2 | REQUIREMENT (§17.3), NOT implemented: control-plane auth today is `IdentityResolver` with a `DenyAllResolver` fail-closed default and a dev resolver only (`apps/control-plane/src/auth.rs`) — real IdP wiring is deployment composition | `<EMPTY>` | `<EMPTY>` |
| C2 | RBAC + scoped authorization | IMPLEMENTED in code: closed `Action`/`Role` vocabularies, deny-by-default `PolicySet`, typed scopes (P1.11 `crates/flexcore-authz`) | `<EMPTY>` | `<EMPTY>` |
| C3 | MFA / step-up where required | REQUIREMENT, NOT implemented anywhere in code; enforced (if at all) by the customer IdP + console (P9.08 design) | `<EMPTY>` | `<EMPTY>` |
| C4 | mTLS | IMPLEMENTED edge-side: mTLS both directions, scheme `wss://`/`https://` required, TLS 1.3 minimum default (`uplink_mtls.rs`) | `<EMPTY>` | `<EMPTY>` |
| C5 | Short-lived credentials + rotation | PARTIALLY in code: enrollment token single-use, ≤15 min (P9.33 §1); service-credential rotation is deployment design (P9.09) | `<EMPTY>` | `<EMPTY>` |
| C6 | Signed commands (sequence/expiry/replay) | IMPLEMENTED as keyed-checksum + sequence/expiry (P6.15; P9.31 stand-in for idempotency); production cryptographic signature = DEPLOYMENT REQUIREMENT (P6.15; P9.09 §6) | `<EMPTY>` | `<EMPTY>` |
| C7 | Plugin signature/provenance + SBOM | P7.14 provenance seam exists (digest-and-record, fail-closed default); SBOM check is a process requirement with no tooling recorded | `<EMPTY>` | `<EMPTY>` |
| C8 | WASM sandbox limits | Kernel path protection exists (`plugin_path_protection.rs`); WASM host resource limits are Stage 7 scope (P6.26 §3.G) — reviewer verifies what the deployed plugin host actually enforces | `<EMPTY>` | `<EMPTY>` |
| C9 | Segmentation + allowlisting | P9.05 §6 deny-by-default rule set (template) + the edge's structural two-endpoint allowlist; deployed firewall state is what gets assessed | `<EMPTY>` | `<EMPTY>` |
| C10 | Encryption in transport and storage | Transport: implemented edge↔central (C4). Storage: **NOT implemented** — DEPLOYMENT REQUIREMENT (P6.18 D-8: OS-level site protection until a license-audited crypto dependency closes it) | `<EMPTY>` | `<EMPTY>` |
| C11 | Secrets-manager integration | References-only rule enforced structurally (P7.05 `SECRET_REFERENCE_SCHEMES` = `env:`/`file:`/`secret-store:`; plugin config refuses plain values in secret-shaped fields); the actual store integration is deployment design (P9.09) | `<EMPTY>` | `<EMPTY>` |
| C12 | Audit logging | IMPLEMENTED: append-only platform `AuditSink` (P1.10) + read-only API action (P4.36); edge `FileAuditBuffer` with in-order at-least-once upload (P6.18; P9.32) | `<EMPTY>` | `<EMPTY>` |
| C13 | Edge secure boot / update where supported | Update verification surface WRITTEN-UNRUN (P9.34); secure boot per-profile determinations PENDING PILOT HARDWARE SPEC (P6.34) — no claim either way | `<EMPTY>` | `<EMPTY>` |
| C14 | Backup / restore | §21.2 design position; the rehearsal is P9.24 (human execution) | `<EMPTY>` | `<EMPTY>` |
| C15 | Vulnerability management | Cadence template below (§11) — EMPTY until the assessor/owner records it for the deployed versions | `<EMPTY>` | `<EMPTY>` |
| C16 | Incident response | §17.3–§17.4 position; the procedure + tabletop are P9.23 (human execution) | `<EMPTY>` | `<EMPTY>` |

## 11. Pilot-period vulnerability-management cadence (TEMPLATE — EMPTY)

Ledger step 6. All three cells are deployment/organization decisions;
a cadence without an owner is the ledger's named failure mode:

| Item | Recorded value | Owner |
|---|---|---|
| Scanning frequency (dependency + image + host scans, for the deployed versions, whole pilot period) | `<EMPTY>` | `<EMPTY>` |
| Patch window (max time from patched release to deployed update, per component class; interacts with the P9.34 freeze window during acceptance) | `<EMPTY>` | `<EMPTY>` |
| Escalation rule when a critical CVE lands inside the freeze window | `<EMPTY>` | `<EMPTY>` |

## 12. Findings register (TEMPLATE — EMPTY)

Ledger step 7. Every finding carries severity, owner, and due state
(must-fix before pilot vs accepted residual carried to P9.27); a
compensating control without an owner and expiry is the ledger's named
failure mode:

| Finding ID | Source (threat row / control row / inspection) | Description | Severity | Must-fix before pilot? | Owner | Due state / expiry | Residual carried to P9.27? |
|---|---|---|---|---|---|---|---|
| `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

## 13. Reviewer-verification procedure (human)

1. Confirm §1 states that no assessment was executed and every
   assessment cell (status, evidence, severity, likelihood, impact,
   residual, cadence, findings) is `<EMPTY>`.
2. Confirm no row in §9/§10 asserts a control is "implemented" for a
   deployment; implemented/partial claims may appear only as cited
   CODE-level facts (e.g. "implemented in code: P1.11 RBAC"), never as
   deployed verdicts.
3. Confirm Rev. 4 appears only in §7 as draft input, never as a
   requirement, and that no IEEE 1547.3 content is asserted (§8 is
   EMPTY/UNREAD).
4. Trace each §6.2 leaf to the named P9.31/P6.15 rejection and each
   §9/§10 citation to the real seam (`command_verify.rs`,
   `command_path.rs`, `pilot_rejection_proof.rs`, `clock_quality.rs`,
   `uplink_mtls.rs`, `no_inbound_listener.rs`, `manifest.rs`,
   `signing.rs`, `update_verification.rs`, `auth.rs`,
   `flexcore-authz/src/*`).
5. Confirm every UNTESTED recording (T2, T6 telemetry-broker mapping,
   T8 SBOM) is present and that no testing subtask is implied to have
   run.
6. Confirm the ledger tick wording is "implemented, awaiting human
   build/test".

## 14. Filling commands (for the human assessor)

- After the pilot deployment exists: fill §3 (deployment, versions,
  inspected config sources), classify §4 assets, fill §5 deployed
  crossings from the LIVE firewall/IdP state, reconcile §6.1 actors
  against §17.1, and author §6.3 trees.
- Walk §9 row by row: for each threat, inspect the deployed evidence,
  confirm the named testing subtask's execution record exists, then set
  status and likelihood/impact/residual. Any row without evidence stays
  UNTESTED/partial.
- Walk §10 row by row against the deployed configuration; record every
  partial/missing control's compensation WITH owner and expiry (§12).
- Read IEEE 1547.3-2023; fill §8. Read the Rev. 4 draft; fill §7 (draft
  label mandatory).
- Record the §11 cadence with named owners; transfer every
  must-fix/accepted finding into §12; carry the residual column to
  P9.27.
- File the completed report + findings list as the P9.07 evidence named
  in `CHECKLIST.md` ("Security assessment complete") only after the
  independent second-reviewer signature (§3).

## 15. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| NO security assessment exists — no deployment, no assessor, no evidence; P9.28's "no must-fix finding open" check cannot be evaluated | §1, §3, §12 |
| Production crypto absent at the code level: command signature is a keyed checksum, nonce/update digests are FNV-1a-64 test-grade — all production replacements are open DEPLOYMENT REQUIREMENTS | §6.2; §10 C5/C6; P9.09 §6 |
| Encryption at rest unimplemented (P6.18 D-8); OpenEMS REST hop cleartext inside the OT boundary (P6.26 §9 item 5) | §10 C10; §5 |
| Plugin-compromise and SBOM controls have NO pilot testing subtask — recorded UNTESTED by construction of the current ledger | §9 T2, T8 |
| Broker-duplication testing on the telemetry path has no named execution — reviewer must name one or accept UNTESTED | §9 T6 |
| Secure boot undetermined (no hardware spec); update verification WRITTEN-UNRUN — the pilot must not run without the P9.34 control verified | §10 C13 |
| Rev. 4 draft pressure: a final Rev. 4 during the pilot may move baseline expectations — tracked as draft input only | §7 |
| Telemetry-broker and head-gap-sequence design positions (monotonic window admits forward gaps) must be consciously accepted, not inherited | §6.2; §9 T6 |

Status: **WRITTEN-UNRUN** — an assessment template cannot be "run"; the
assessment itself (deployment inspection, ratings, findings, second
reviewer) is the human gate and has not happened and is not scheduled.
No deployment was inspected; no pilot exists and none is claimed.
