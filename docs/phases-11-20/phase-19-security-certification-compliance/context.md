# Phase 19 — Security Certification and Compliance Readiness — Context

Date: 2026-11-14. Status: **NOT STARTED** (0/18 — see `ledger_phase_19.md`).
This directory is PLANNING ONLY: no code, crates, or manifests are created by
this program's planning step (program conventions §3). Citations `[S-n]`
resolve in `../00_GAP_ANALYSIS.md` §11; the rules in
`../01_PROGRAM_CONVENTIONS.md` bind everything below.

## 1. Why this phase exists

Commercial table-stakes #10 (gap analysis §3) is the driver: enterprise
procurement demands security attestations, and FlexCore has none. Enode
advertises SOC 2 accreditation [S-45]; Uplight publishes year-after-year SOC 2
plus penetration tests [S-44]. The gap analysis records FlexCore's posture
for that row as "Nothing: security consolidation verdict NOT APPROVED
(P10.25)" and assigns Phase 19. The honest target: build the evidence
programs and mappings a certification process consumes — never the
certification itself, which is a human/organizational act (constraint 1).

The Phase 0–10 records this phase upgrades in posture (each cited at its
path in §2; P19 supersedes their *template* posture with standing
*programs* — an upgrade record, never a rewrite of the frozen files):

| 0–10 record | Verdict as recorded there | Phase 19 relationship |
|---|---|---|
| P10.25 security review consolidation | **NOT APPROVED** — input pack for a review never convened; sign-off UNSIGNED | The review posture stands; P19 adds the program layer above it |
| P10.19 SBOM/license closure | **NOT CLOSED** — no SBOM exists, no scan ever ran | P19.08/P19.09 turn the procedure into a standing SBOM/dependency program |
| P10.20 vulnerability scan closure | **NOT CLOSED** — zero scans; EMPTY result table | P19.12 upgrades the template into a vulnerability-management program with SLA tiers |
| P10.21 signed artifacts | **NOT CLOSED** — nothing built or signed; no key material | P19.07 designs the provenance/signing posture (SLSA, cosign keyless) |
| `deny.toml` (P6.27) | **WRITTEN-NEVER-RUN** | P19.09 writes rollout configs around it; the file stays byte-identical; runs stay human gates |

Standards landscape (gap analysis §4 Track B rows):

- **IEC 62443** [S-22]: 4-1 (secure development lifecycle process), 4-2
  (component security requirements) [S-163], 3-2/3-3 (zone/conduit and
  system requirements); ISASecure certification schemes. The gap analysis
  records FlexCore's 62443 posture as "Partial (secure SDLC practices exist
  informally)" → Phase 19.
- **NERC CIP** [S-23][S-24][S-25]: CIP currently does not apply to DERAs — a
  voluntary CIP-003 Attachment 12 baseline is the recorded posture;
  CIP-013-style supply-chain flow-down to suppliers; the NERC CIP roadmap
  (Jan 2026) is monitored for DERA applicability changes.
- **SOC 2 Type II** [S-80]: an attestation over an observation window —
  evidence of control operation over time, collected and audited; an
  organizational process, not a build item.
- **Supply chain** [S-78][S-79]: SLSA L1→L2 build provenance, cosign signing,
  SBOM in SPDX/CycloneDX, cargo-deny/cargo-audit/pip-audit scanning.

## 2. What exists today (the real seams)

All paths verified 2026-11-14. The Phase 0–10 workspace is WRITTEN-UNRUN —
written modules with records, not verified behavior.

| Seam | Path | What it honestly contains |
|---|---|---|
| Threat model (P0.17) | `ledger/phase-00-product-freeze/17_threat_model.md` | Five security zones, 16 §17.3 controls, 10 §17.4 threats, 10 §17.5 invariants; NIST SP 800-82 Rev. 3 baseline (Rev. 4 draft). Frozen at Phase 0 — covers none of the Phase 11–18 surfaces; P19.14 refreshes as design, without editing the frozen file |
| Security consolidation (P10.25) | `ledger/phase-10-v1-release-hardening/25_security_consolidation.md` | VERDICT: NOT APPROVED; sign-off UNSIGNED; consolidates P10.19/20/21 (all NOT CLOSED) plus the P9.27 register, trace-only; the verbatim not-a-certification discipline (its §7) binds Phase 19 |
| SBOM/license closure (P10.19) | `ledger/phase-10-v1-release-hardening/19_sbom_license_closure.md` | NOT CLOSED; no lockfiles, no SBOM, no legal review; generation procedure recorded never run; LLVM-exception allowlist question carried as a RECORDED DECISION |
| Vuln scan closure (P10.20) | `ledger/phase-10-v1-release-hardening/20_vuln_scan_closure.md` | NOT CLOSED; zero scans in the repository's history; an unqueried advisory database is an UNKNOWN, never "no issues found" |
| Signed artifacts (P10.21) | `ledger/phase-10-v1-release-hardening/21_signed_artifacts.md` | NOT CLOSED; artifact set described, never built; no production signing key exists; test-grade FNV-1a-64 digests are NOT release signatures |
| Deny policy (P6.27) | `deny.toml` | WRITTEN-NEVER-RUN; `unlicensed = "deny"`, copyleft families not allowed, `yanked = "deny"` with empty ignore list; first human `cargo deny check` is the verification point |
| CI workflow | `.github/workflows/ci.yml` | 24 jobs including `security-and-supply-chain`, `edge-license-gate`, `edge-supply-chain`, `no-dylib-gate`, `release-candidate-build` — all UNRUN (nothing has ever been built) |
| Limitations register (P9.27) | `ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md` | LR-040/LR-055 production crypto absent (keyed checksum; test-grade digests); LR-062 D1 OPEN — no jurisdiction, no residency constraint, no privacy regime, no retention basis |
| Audit crate (P1.10) | `crates/flexcore-audit/` | Append-only `AuditSink` and `AuditRecord`; chain digests test-grade FNV-1a (LR-203) until the P11.15 production-grade upgrade |
| ADR-010 | `docs/architecture/adr/ADR-010.md` | ML/RL outputs are proposals only, never dispatch — inherited verbatim; the certification posture never relaxes it |
| Audit chain upgrade (forward seam) | `../phase-11-operational-robustness/plan.md` P11.15 | Production-grade chain (SHA-256/HMAC or better; INSERT-only role; per-tenant verification) — P19 consumes its records as evidence, never duplicates |
| Drills (forward seam) | `../phase-12-backup-dr-resilience/plan.md` P12.07/P12.08 | Chaos program and game-day runbooks — drill records are Availability evidence P19 cites |
| Identity (forward seam) | `../phase-13-enterprise-identity-integration/plan.md` | IdP (P13.01, P13-E-1), OIDC (P13.02), SCIM (P13.03), step-up (P13.04), GDPR engineering (P13.13), jurisdiction registry (P13.14) — P19 builds the access-review and evidence processes on top |
| Update manifests (forward seam) | `../phase-18-edge-fleet-lifecycle-ota/plan.md` P18.02 | Signed update-bundle format — the IEC 62443-4-2 CR 3.4 software-integrity evidence anchor [S-163] |

## 3. Constraints (critical)

1. **Certification is a human/organizational process.** Agents prepare
   evidence structures, mappings, policies, and configs. **NO certification,
   attestation, scan result, or audit outcome is ever claimed.** The
   verbatim not-a-certification discipline of
   `ledger/phase-10-v1-release-hardening/25_security_consolidation.md` §7
   (quoting AGENTS.md §3 rules 2–3) carries forward unchanged.
2. **Every scan/tool run is a human gate.** No scanner runs in planning or
   implementation loops (conventions §1). X-8 carries over: any thresholds
   (SLA tiers, freshness windows, exception expiries) are registered BEFORE
   any run that could fill them, and dynamic results stay structurally
   separate (conventions §5).
3. **This phase COLLECTS evidence; it does not duplicate producers.** P11
   (audit chain, logs), P12 (drills), P13 (access reviews, IdP) own their
   artifacts. Every P19 mapping traces by path, cites the producer record,
   and never restates or re-decides (gap analysis §9: cross-phase seams are
   defined once in the earlier phase and consumed).
4. **ISO 27001 Annex A specifics are UNVERIFIED in research** (gap analysis
   §11 UNVERIFIED list, marked). The mapping work is done against the SOC 2
   Trust Services Criteria [S-80], which the research covered; every ISO
   27001-side statement is recorded as REQUIRING THE STANDARD TEXT and is
   marked UNVERIFIED wherever it appears.
5. **No penetration test is executed or scheduled as done.** The program
   defines scope, frequency, vendor-selection criteria, and remediation SLAs
   only (P19.13); the engagement, its vendor, and its findings are
   human/organizational acts, recorded as NOT-STARTED.

## 4. Glossary (compliance and supply-chain terms used in this phase)

| Term | Meaning |
|---|---|
| SOC 2 Type II / Trust Services Criteria | AICPA attestation reporting on control operation over an observation window [S-80]; the five TSC categories: Security (mandatory), Availability, Confidentiality, Processing Integrity, Privacy |
| ISO 27001 ISMS / Annex A | The ISMS is the audited management system (policy, risk assessment, Statement of Applicability); Annex A is its control catalogue — Annex A specifics UNVERIFIED in this program's research (marked; mapping requires the standard text) |
| IEC 62443 zones / conduits / SL1–4 | Partitioning of assets into zones connected by conduits, each assigned a security level SL1–SL4 [S-22] |
| IEC 62443-4-1 SDL | The secure development lifecycle process requirements (practices) a product developer must operate [S-22] |
| IEC 62443-4-2 CR / FR mapping | Component requirements (CR) grouped by foundational requirements (FR), mapped per component with justification [S-163] |
| ISASecure SDLA / EDSA / SSA | ISASecure schemes: SDLA (secure development), EDSA (embedded device), SSA (system security) [S-22] |
| NERC CIP registration | CIP applicability follows registration for Bulk Electric System functions; aggregator triggers are assessed, never assumed [S-23] |
| CIP-013 (supply chain) | The supply-chain risk-management standard; its vendor flow-down style is adopted voluntarily where CIP does not yet apply [S-24] |
| CIP-003 Attachment 12 | The voluntary cybersecurity baseline for entities not subject to full CIP — the recorded DERA posture [S-23] |
| SLSA levels | Supply-chain Levels for Software Artifacts: L1 provenance exists, L2 hosted build platform with signed provenance [S-78] (register note: v1.2 current — verify) |
| Keyless signing | Sigstore cosign signing with short-lived certificates issued from an OIDC identity — no long-lived key held [S-79] |
| SBOM | Software Bill of Materials in SPDX or CycloneDX, generated from lockfiles/resolved environments, never hand-written (P10.19 rule) |
| VEX | Vulnerability Exploitability eXchange — a statement of whether a listed vulnerability actually affects a product |
| Pen test scope | The assets, paths, and rules of engagement a test covers — defined in P19.13; the test itself is never executed here |
| DPIA | Data protection impact assessment — structured analysis of high-risk processing; template prepared in P19.15, legal sign-off human-gated [S-86][S-87] |
| Access review | Periodic re-verification of who holds what access; quarterly cadence here; evidence recorded for audit |
| Vulnerability SLA tiering | Remediation time targets set by severity and exploitability, pre-registered before any scan run (X-8) |
