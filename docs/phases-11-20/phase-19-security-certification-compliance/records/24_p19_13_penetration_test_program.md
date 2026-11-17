<!-- P19.13 — Penetration-test program. HONEST RECORD: a PROGRAM record
     only — scope, frequency, vendor-selection criteria (P19-E-3
     registered OPEN with DEFAULT (a)), and remediation SLAs. The test
     is NOT executed and NOT scheduled as done; the engagement, its
     vendor, its findings, and its dates are human gates; every
     engagement cell is NOT-STARTED [S-44]. No certification,
     attestation, or audit outcome is claimed anywhere. -->
# P19.13 — Penetration-test program: scope, frequency, vendor-selection criteria (P19-E-3 OPEN, DEFAULT (a)), remediation SLAs — engagement NOT-STARTED

| Field | Value |
|---|---|
| Subtask | P19.13 — Penetration-test program (`ledger_phase_19.md` §P19.13) |
| Wave / agent | Phase 19 wave 1, Agent B |
| Status | **PROGRAM RECORD — WRITTEN, NEVER EXECUTED, NEVER SCHEDULED AS DONE.** The engagement is a HUMAN GATE; every engagement cell in §7 is NOT-STARTED. |
| Sources | [S-44] (Uplight security page — the commercial precedent that year-after-year SOC 2 attestations are published alongside penetration tests; cited as market posture, no claim about FlexCore); P19.14 threat-model refresh (`records/25_p19_14_threat_model_refresh.md` — the scope derivation source); P19.12 SLA tiers (`records/23_p19_12_vulnerability_management_program.md` — remediation SLAs consumed by citation); plan §6 (P19-E-3); conventions §4 (agents never close an E-decision) |
| Not-a-certification | Per `ledger/phase-10-v1-release-hardening/25_security_consolidation.md` §7 (verbatim discipline, AGENTS.md §3 rules 2–3): FlexCore is not certified, not a safety case, and not permission for live grid control. A penetration test, if one is ever performed, produces *findings* — it is not a certification, not a clearance, and not evidence that a system is secure. |

## 1. Honest status — the plain statement

**No penetration test has been executed, contracted, scheduled, or
scoped-as-done anywhere in this program.** The target system itself
does not yet exist as a runnable artifact (the Phase 0–10 workspace is
WRITTEN-UNRUN; program README hard status boundary 1), so no honest
engagement is even possible yet. This record delivers only what a
program must have *before* an engagement can be sensibly commissioned:
the scope (§2, derived from the P19.14 threat-model surfaces), the
frequency policy (§3), the vendor-selection criteria with P19-E-3
registered OPEN (§4), the rules-of-engagement skeleton (§5), the
remediation SLAs consumed from P19.12 (§6), and the engagement register
with every cell NOT-STARTED (§7).

## 2. Scope — derived from the P19.14 threat-model refresh surfaces

Scope derives from the P19.14 surface registry (`records/25_p19_14_threat_model_refresh.md`
§3–§4) — cited, not restated. The four in-scope target classes:

| # | Target class | Why in scope (per P19.14 surfaces) |
|---|---|---|
| SC-1 | Control plane (API, command lifecycle, authz, audit surfaces) | The identity (P13) and command-integrity (P0.11) surfaces concentrate here; the three-gate lifecycle is the highest-value integrity target |
| SC-2 | Edge agent (update verification, attestation, offline behavior) | The OTA surface (P18.02 bundle verification, P18.13 attestation) and the §18.2 edge trust boundary (P0.17 §7) |
| SC-3 | Operator console (TypeScript) | The enterprise-IT-zone browser surface (P0.17 zone 1); session and step-up seams (P13.16/P13.04) |
| SC-4 | Protocol adapters (aggregator server, adapter permission boundary) | The P14 surfaces: mTLS requirements (P14.02), PKI design (P14.03), the command-relay-only boundary (P14.16) |

Explicitly out of scope for a FlexCore-commissioned engagement: site
OT devices and local controllers (zone 4–5 customer assets —
P0.17 §3; site-local safety remains with local certified controllers,
§17.5 item 10), customer IdPs (third-party operated), and any live
grid path — no field write authority exists at any 0.x release
(AGENTS.md §3 rule 5; simulation-only ceiling).

## 3. Frequency — RECOMMENDATION registered before any engagement

| Policy | Registered value | Status |
|---|---|---|
| Periodic cadence | **Annual** (RECOMMENDATION — registered before any engagement that could set a precedent) | REGISTERED; no engagement exists |
| Release trigger | Before each v1.0-class release claiming operator-approved control posture | REGISTERED |
| Material-change trigger | After any material architecture change (new protocol surface, new authn path, new trust boundary) | REGISTERED |

## 4. Vendor selection — P19-E-3 registered OPEN, DEFAULT (a)

| Field | Value |
|---|---|
| ID | **P19-E-3** (phase-19 decision register; ledger §Decision points) |
| Question | What criteria govern penetration-test vendor selection? |
| Options | (a) criteria-based with recorded minimums; (b) incumbent/cheapest qualifying vendor; (c) broker-managed |
| **DEFAULT** | **(a) — criteria-based, with the minimum criteria recorded here:** (1) methodology disclosure (the vendor names its methodology and its evidence-capture practice); (2) OT/grid competence (demonstrable experience testing OT-adjacent or energy-sector software — the §17.1 posture makes this non-optional); (3) retest terms (a priced or included retest of remediated findings); (4) reporting quality (findings reported with severity, exploitability reasoning, and reproduction steps — the same fields the P19.12 register consumes) |
| Status | **OPEN — never closed by this record or by any agent** (conventions §4). The vendor, the engagement date, and the findings remain human gates. Closer: human owner. |

## 5. Rules-of-engagement skeleton — ALL CELLS EMPTY (human-gated inputs)

| Field | Value |
|---|---|
| Target environment (lab deployment identifier) | `<EMPTY>` — a lab that does not exist yet; never production; never customer sites |
| Engagement window | `<EMPTY>` |
| Vendor (selected under §4 criteria) | `<EMPTY>` |
| Rules of engagement (attack surface allowlist, prohibited actions) | `<EMPTY>` |
| Escalation contact (both sides) | `<EMPTY>` |
| Findings report reference | `<EMPTY>` |
| Retest reference | `<EMPTY>` |

## 6. Remediation SLAs — consumed from P19.12 (citation, not restatement)

Findings from any engagement are triaged under the P19.12 registered
tiers (`records/23_p19_12_vulnerability_management_program.md` §3) and
any not-fixed finding enters the P19.12 §5 signed, time-bounded
exception process. A pen-test finding and a scanner finding follow ONE
program, so no engagement can create a second, weaker remediation path.

## 7. Engagement register — ALL CELLS NOT-STARTED [S-44]

| Field | Value |
|---|---|
| Engagement | **NOT-STARTED** |
| Vendor | **NOT-STARTED** |
| Scope confirmation | **NOT-STARTED** |
| Dates | **NOT-STARTED** |
| Findings | **NOT-STARTED** |
| Retest | **NOT-STARTED** |
| Disposition of findings (per §6) | **NOT-STARTED** |

The commercial precedent [S-44] records what the market expects
*published* (SOC 2 attestations accompanied by penetration tests); it
records nothing about FlexCore, and this register claims nothing.

## 8. Reviewer-verification procedure (human)

1. Confirm §7's cells all read NOT-STARTED and no date, vendor, or
   finding appears anywhere in this record.
2. Confirm P19-E-3 is OPEN with DEFAULT (a) recorded and no closer
   named except the human owner.
3. Confirm §2's scope cites the P19.14 surfaces by path and §6 cites
   the P19.12 tiers by path — neither restated.
4. Confirm §5's cells are all `<EMPTY>`.

## 9. Status

**WRITTEN / PROGRAM-ONLY.** Scope, frequency, criteria, and SLAs are
recorded; the engagement is a human gate; every engagement cell is
NOT-STARTED. No test is executed or scheduled as done, and no test
outcome exists or is claimed.
