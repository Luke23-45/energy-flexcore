# P0.18 — Control-authority modes and the simulation-only rule freeze

| Field | Value |
|---|---|
| Subtask | P0.18 — Freeze control-authority modes and the simulation-only rule |
| Report trace | §21.4 (control-authority deployment modes), §0 (executive decision, release path), §2.1 (release definition), §22.3 (release gates), §14.5 (dispatch is not hard real-time) |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (product baseline; foundations specification governs on conflict per AGENTS.md §2) |
| Inputs | P0.01 (scope/non-goals), P0.11 (command state machine), P0.17 (threat model) |
| Status | Draft for Stage 0 review |
| Scope limit | This note freezes requirements on releases and deployments; it grants no field authority, implies no regulatory certification, and makes no protection-grade or hard-real-time claim (§0.3; §14.5; AGENTS.md §3). |

---

## 1. Purpose

Freeze the four control-authority modes and the rule that mode is a deployment/site setting, never a plugin choice (ledger P0.18). This note is cited in every phase gate; Phase 4 dispatch work is simulation-only by construction (ledger P0.18 risk/mitigation note).

## 2. The four control-authority modes (§21.4)

| Mode | Authority | Writes to devices | Default use |
|---|---|---|---|
| Simulation | None | No | v0.1.0; development, demonstration, benchmarking |
| Read-only / shadow | None; computes and compares proposed actions against measurements | No | Early pilots and commissioning |
| Operator-approved | Each command requires an authorized human approval | Yes, after approval | v1.0.0 default |
| Bounded automation | Pre-approved policy envelope with limits, expiry, and automatic fallback | Yes, inside the envelope only | Post-1.0, per deployment, site, and asset class after qualification |

Validation, feasibility, and authorization remain distinct gates in every mode; an operator approval is not a feasibility result, and an acknowledgement is not physical delivery (§14; P0.11).

## 3. Mode rule (§21.4)

- Control-authority mode is a **deployment and per-site setting, not something a plugin can select** (§21.4; §5.7 — the platform kernel owns final authorization and dispatch state transitions).
- A deployment **may run mixed modes by site or asset class** (§21.4).
- **Promotion to a higher-authority mode requires the release gates in §22.3 plus site-specific acceptance** (§21.4). Per §22.3, a production field pilot additionally requires site owner approval, device/vendor integration tests, local safety review, commissioning, and any applicable market/interconnection qualification; unresolved §25.2 gates must be closed for the deployment jurisdiction, network class, and protocol mix before that deployment is promoted beyond read-only mode.

## 4. Release-ladder rule (§0, §2.1)

- The **entire 0.x line is simulation-only**: it can model, schedule, validate, and demonstrate the full decision chain, but it cannot write to physical devices (§0). v0.3.0 adds shadow alongside simulation; v0.4.0 is read-only or test-lab control by default (§2.1). No 0.x release grants field write authority.
- **Field write authority begins at v1.0.0 with operator approval**: operator-approved control is the v1.0.0 default, and production dispatch only after customer-specific acceptance and commissioning (§2.1; §21.4).
- **Bounded automation is post-1.0**, granted per deployment, site, and asset class after qualification; no release level by itself grants field authority (§0; §2.1).
- **A version number does not imply regulatory certification or operational qualification** (§2.1). The mode actually enabled at a site is governed by §21.4 and the §22.3 release gates.
- Dispatch is not hard real-time: no primary-frequency, synthetic-inertia, relay, inverter inner-loop, or sub-cycle control capability is claimed in any mode (§14.5; §0.3).

Per-release control status (§2.1):

| Release | Control status |
|---|---|
| v0.1.0 | Simulation only; no device writes |
| v0.2.0 | Simulation only |
| v0.3.0 | Simulation and shadow mode |
| v0.4.0 | Read-only or test-lab control by default |
| v0.5.0 | Simulation/shadow; no plugin can bypass dispatch gates |
| v1.0.0 | Operator-approved control is the default; production dispatch only after customer-specific acceptance and commissioning |
| v1.x | Per-feature qualification; bounded automation may be granted per deployment |
| v2.x | Shadow/simulation first; no automatic field authority by default |

## 5. Terminology rule (`.agent/WRITING.md`; ledger P0.18 step 4)

| Use | Do not use |
|---|---|
| "operator-approved control" | "manual control", "human-in-the-loop" |
| "read-only / shadow mode" | "passive mode" |

"Certified" and "conformant" require a test result; implementing an adapter is neither (`.agent/WRITING.md`; §2.3).

## 6. What this document does not do

- It grants no field control in any 0.x release, and no Phase 0 artefact may promise field control in 0.x (ledger acceptance criterion; verification note below).
- It enables no mode: mode selection is a later-phase, per-deployment act gated by §22.3 plus site-specific acceptance.
- It makes no safety, protection-grade, hard-real-time, or certification claim (§0.3; §14.5; AGENTS.md §3 rules 2–3, 5).

## 7. Acceptance-criteria coverage (ledger P0.18)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| All four modes tabulated with authority, writes, and default use | §2 |
| Mode-is-a-deployment-setting rule and mixed-modes rule stated | §3 |
| 0.x simulation-only rule and v1.0.0 operator-approved default stated | §4 |
| No Phase 0 artefact promises field control in any 0.x release | §4 per-release table; §6; verification note below |

Additional ledger steps covered: promotion rule with §22.3 gates and site-specific acceptance (§3), §2.1 "version number ≠ certification" rule (§4), terminology rule (§5). Verification method (ledger): comparison against §21.4 and §2.1; grep Phase 0 artefacts for field-control promises in 0.x — zero hits required.
