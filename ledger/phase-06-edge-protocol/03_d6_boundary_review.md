# P6.03 — D6 cyber/OT boundary review: PREPARED (not signed, not closed)

Date: 2026-11-12. Subtask: P6.03, Phase 6 ledger (`ledger_phase_6.md` §P6.03).
Owner role for D6: **Security** (`.agent/DECISIONS.md`; ledger P6.03).

**No named security engineer exists yet and no sign-off is recorded here.**
The ledger forbids inventing owners and dates (`CONTEXT.md` §9; `.agent/DECISIONS.md`
gate-table rule). Every engineering position in this document is a
**RECOMMENDATION pending the named security engineer's review and sign-off**.

## 1. Status: D6 is STILL OPEN

| Field | Value |
|---|---|
| Decision | D6 — Cyber and OT boundary review |
| Status | **OPEN — recorded, not closed.** This file does not update `.agent/DECISIONS.md` and does not sign anything. |
| Gate | Before any read-only pilot (report §25.2; `.agent/DECISIONS.md`) |
| Owner | Security |
| Site | **PLACEHOLDER `pilot-site-A`** — no real site exists; see `01_d5_disposition.md` §4. D6 is inherently site-specific; a review without a real site cannot close it. |
| Review status | **PREPARED** — the boundary question checklist (§3) and the recommended positions (§4–§5) are written for the reviewer. The review itself is NOT conducted and D6 is NOT closed. |

Why not closed (acceptance criteria of P6.03, quoted honestly): the exit
check requires a dated review that names the site, lists all five §17.2
zones, lists permitted and prohibited flows, names the outbound connection
pattern, and carries security sign-off with a date. The site is a
placeholder and the sign-off does not exist — so none of that is claimed
here. P6.24 (lab integration) stays blocked on D6 per the ledger.

Governing reference: **NIST SP 800-82 Rev. 3 (September 2023)** is the
current final baseline; Rev. 4 (initial public draft, 21 September 2026)
is a DRAFT and is not cited as a requirement (`RESEARCH.md` §6).

## 2. What this document is

The report states the OT boundary is site-specific and not derivable from
the architecture alone (`CONTEXT.md` §1; ledger P6.03 risk row). What an
agent CAN prepare is the question set the reviewer must answer per site,
plus the architecture-level positions the report already fixes. That is
what follows. What an agent CANNOT produce is the site description, the
permitted-flow list for a real network, or the sign-off.

## 3. Boundary-question checklist (PREPARED for the D6 reviewer)

Questions the named security engineer must answer for the actual pilot
site. Sources: `CONTEXT.md` §2 rows for §17.1–§17.5, §3.3, §18.1–§18.3,
§21.2–§21.4; `CONTEXT.md` §9 open question 6.

### 3.1 Zones (report §17.2 — all five zones must be enumerated for the site)

| # | Question | Report anchor |
|---|---|---|
| Z1 | Does the site operate the full five-zone model — Enterprise IT, Central application, Customer integration/DMZ, OT/site control, Field/device — or fewer? | §17.2 (zones listed in ledger P6.03 inputs; `CONTEXT.md` §2) |
| Z2 | Where exactly does the FlexCore edge agent sit? Report topology: in the OT DMZ, talking to the site EMS (OpenEMS), which talks to device gateways and assets. Does the site's actual DMZ match this? | §3.3; `CONTEXT.md` §1 |
| Z3 | Does the site run its EMS (OpenEMS) inside the OT zone with the edge agent as the only crossing point? | §3.3 |

### 3.2 Flows (permitted and prohibited)

| # | Question | Report anchor |
|---|---|---|
| F1 | Is the edge→central connection outbound-only (edge dials out to central or a customer-controlled broker/gateway)? Confirm NO inbound internet access into OT. | §3.3 (outbound from customer edge; inbound internet access into OT to be avoided) |
| F2 | Prohibited flows to confirm per site: no direct database/NATS/plugin network access from any field-device network. | §17.2 (quoted in ledger P6.03 step 1) |
| F3 | What does the site NOT permit? The review must record prohibitions explicitly, not only permissions. | §17.3; ledger P6.03 step 3 |
| F4 | Which connection pattern does the site require: customer-controlled broker/gateway, DMZ, unidirectional gateway, or jump host? (Site-specific; `CONTEXT.md` §9 open question 6.) | §3.3 |
| F5 | Which flows does the site EMS (OpenEMS) accept? Note: OpenEMS Edge↔Backend authentication is API-key over WebSocket and the Backend.Edge.App↔Edge.Manager link is trusted-network without peer authentication — a FlexCore channel is a separate path, not a substitute (R6-C1, `RESEARCH.md` §7). | `RESEARCH.md` §1, §7 |

### 3.3 Channel and identity controls (report §17.3–§17.4)

| # | Question | Report anchor |
|---|---|---|
| C1 | mTLS in both directions on the edge↔central channel; central never dials into the site; no listener socket on the edge for inbound central connections. | §17.3, §3.3, §18.2; P6.07 acceptance criteria |
| C2 | TLS minimum version and cipher policy for the edge mTLS channel — the P6.03 record must carry this before P6.07 can pin it (ledger P6.03 step 3). Site-specific; must come from the reviewer, not from code defaults. | §17.3; ledger P6.03 step 3 |
| C3 | Certificate/identity decisions: enrollment flow, per-edge identity bound to one tenant and the approved site/asset set, expiry/rotation/revocation. | §17.3, §18.3, §16.2 |
| C4 | Firewall allowlist and segmentation decisions for the edge path; what ports/hosts are reachable from the DMZ. | §17.3 |
| C5 | Clock-quality/time-sync position for expiry checks at the edge. | §17.4, §14.3 |
| C6 | Audit logging of every cross-boundary event (enrollment, command rejection, health). | §17.3 (audit logging) |
| C7 | Secrets management: no secrets in logs, config files, plugin config, or source control. OpenEMS Getting Started defaults (`admin`/`admin`, `Metadata.Dummy`, `ws://`) must never be the pilot model (R6-C2, `RESEARCH.md` §7). | §17.3; `RESEARCH.md` §1, §7 |

### 3.4 Command-path and fallback boundary (report §21.2–§21.4)

| # | Question | Report anchor |
|---|---|---|
| P1 | Confirm the read-only / test-lab default for this site: the command path is DISABLED by default; enabling is only via the written P6.14 gate (test environment, named approvers, expiry, audit). | §21.4; §2.1 v0.4.0; P6.14 |
| P2 | Confirm a central outage does not disable site-local protective controls or force an unsafe device state; the edge issues no new commands while disconnected. | §21.2; P6.18 |
| P3 | Confirm FlexCore's supervisory boundary: no primary frequency response, synthetic inertia, relay protection, inverter inner-loop control, or sub-cycle control; local EMS remains authoritative for fast safety behaviour. | §17.5; §14.5; AGENTS.md rule 3 |
| P4 | Per-asset fallback policy (if any test-env command path is later approved) is operator-reviewed and per-site — no universal "set power to zero". | §14.4; P6.16 |

## 4. Architecture-level positions the report already fixes (RECOMMENDATIONS restated for the reviewer)

These are restatements of report positions carried by this phase's
`CONTEXT.md`/`ENTRY_NOTE.md`, offered as the recommended baseline the
reviewer starts from — each still requires site-specific confirmation:

1. **Outbound-only channel principle** — **RECOMMENDATION**: the
   edge-to-central connection is initiated outward from the customer edge;
   the central platform never dials into the site; there is no inbound
   internet access into the OT network. This matches P6.07's design (edge
   initiates, mTLS both directions, no inbound listener on the edge).
   (`CONTEXT.md` §1; §3.3; ENTRY_NOTE boundary on the outbound mTLS channel.)
2. **Test-lab / read-only default** — **RECOMMENDATION**: v0.4.0 control
   status is "Read-only or test-lab control by default" (release ladder,
   ledger header); the command path stays disabled by default and may only
   be enabled in a test environment under the P6.14 written gate. No field
   write authority before v1.0.0 with operator approval (AGENTS.md rule 5).
3. **Edge agent placement** — **RECOMMENDATION**: a separate Rust daemon in
   the site's OT DMZ, not a library inside the control plane, with an
   independent lifecycle (§8.3, §18.1).
4. **No OpenEMS-native-auth conflation** — **RECOMMENDATION**: treat the
   OpenEMS API-key WebSocket as adapter-internal; FlexCore's own channel
   stays mTLS and is not downgraded (R6-C1).
5. **Unidirectional gateway / jump host / broker** — **OPEN**: which of
   these the site requires cannot be recommended without the site; it is
   `CONTEXT.md` §9 open question 6 and must be answered by the reviewer.

## 5. Sign-off block (NOT SIGNED — gate input)

| Field | Value |
|---|---|
| Reviewer (named security engineer) | NOT ASSIGNED — to be named at the D6 gate meeting per `.agent/DECISIONS.md` |
| Review date | NOT SET |
| Site reviewed | None — `pilot-site-A` is PLACEHOLDER |
| Verdict | NOT RECORDED — D6 remains OPEN |

Until this block is filled by the named reviewer against a real site
description, **no pilot-site connection work proceeds** (ledger P6.03
exit check) and P6.07 pins no channel policy from a signed record — its
policy parameters remain RECOMMENDATION-level until then.
