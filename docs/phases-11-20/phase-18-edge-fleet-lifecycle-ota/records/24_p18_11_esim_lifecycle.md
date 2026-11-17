# P18.11 — eSIM/cellular lifecycle tracking: carrier-agnostic data model only

| Field | Value |
|---|---|
| Subtask | P18.11 — eSIM/cellular lifecycle tracking [S-167] |
| Wave / agent | Phase 18 wave 1, Agent B |
| Status | **IMPLEMENTED, AWAITING HUMAN BUILD/TEST** (WRITTEN-UNRUN). DATA MODEL only — no SIM exists, no profile state was ever recorded, no carrier relationship exists |
| Sources | [S-167] IDEMIA eSIM IoT Manager overview (gap analysis §11); keyed to the P18.12 schema (`records/25_p18_12_fleet_registry_hardening.md`, migration 0023) |
| Files touched | `crates/edge-agent/src/esim_lifecycle.rs` (new), `crates/edge-agent/src/lib.rs` (additive module declaration only) |

## 1. Scope (stated twice — module header and here)

**DATA MODEL ONLY.** The module tracks profile state as recorded data
riding the migration-0023 fleet-state journal (`ESIM_PROFILE_STATE` rows).
It does NOT manage profiles, talk to any carrier system, or implement any
eSIM standard. **Carrier specifics are out of scope**: no SM-DP+ address,
no activation code, no confirmation code, no carrier-specific policy
field — only carrier-AGNOSTIC observations (state, carrier LABEL,
identifier REFERENCES, recorded instants).

## 2. Provenance and the UNVERIFIED register

The profile-state vocabulary is inspired by the eSIM lifecycle concepts
named in the [S-167] commercial overview (download / enable / disable /
delete). The exact state names, the transitions a real eUICC enforces, and
the SGP.22/SGP.32 semantics are **UNVERIFIED** — this program holds no
standard text. Consequences, recorded deliberately:

1. The vocabulary is **closed but deliberately coarse** (`NoProfile`,
   `Enabled`, `Disabled`, `Unknown`, `Deleted`) — a reporter that cannot
   characterize a real state maps it to `Unknown`, never invents a value.
2. Identifier fields are opaque REFERENCES with NO format validation —
   this module cannot verify ICCID/IMSI/eID semantics without a standard
   text, so it validates none and trusts none.
3. **Named human-gate check (inherited by wave 2 / a future deployment):**
   any real eSIM fleet tracking must verify the vocabulary mapping against
   the SGP.22/SGP.32 texts before relying on it.

## 3. The model

| Element | Shape | Journal mapping |
|---|---|---|
| Profile state | `EsimeProfileState` — closed 5-value vocabulary with exact `state_value` strings; unknown strings are the named `FLEET_STATE_VALUE_UNKNOWN` refusal | The `state_value` of `ESIM_PROFILE_STATE` rows |
| Observation | `EsimeObservation` — device id, profile state, carrier LABEL (free text), profile REFERENCE (opaque), observed instant | `state_value` = the state string; `detail` = `carrier:<label>\|profile:<reference-or-unrecorded>`; single-line-printable refusal on both text fields |
| Reachability reading | `ExpectedCellularReachability` — `ExpectedReachable` iff the latest state is `Enabled`; absence of evidence is `ExpectedUnreachable` (unknown-is-not-healthy discipline) | A READING AID for fleet views / P18.06 visualization data — never a guarantee (a device can be unreachable for reasons a data model cannot see) |

Fail-closed validation rule worth naming: an `Enabled`/`Disabled`/`Unknown`
profile with NO profile reference is refused (`FLEET_STATE_RECORD_INVALID`)
— an unidentifiable profile is untrackable. `NoProfile` legally carries no
reference.

## 4. What the platform does NOT do

- It never CHANGES a profile state (the journal is append-only; the
  platform records what was OBSERVED).
- It never reads or stores cellular credentials of any kind — `secret.read`
  stays in the never-granted set (byte-identical).
- It never integrates with a carrier or eSIM management platform — that
  would be a deployment integration (and, per the phase-18 non-goals,
  carrier-specific eSIM integration is out of scope for this phase
  entirely).

## 5. Compliance mapping (mapping, never a certification claim)

| Deliverable aspect | Mapped requirement | Basis |
|---|---|---|
| Fleet-visibility of link expectations (input to health interpretation) | IEC 62443-4-1 SAR 3.4 (process — operations evidence) [S-164] | As cited by the phase-18 plan; clause-content interpretation **UNVERIFIED**. The eSIM model itself has no direct 62443 component-requirement mapping; the honest mapping is the operational-visibility one |

## 6. Honesty register

1. **WRITTEN-UNRUN** — never built, never executed; the in-module tests are
   complete and UNRUN (human gate).
2. **No SIM, no profile, no carrier** — no cellular profile state was ever
   recorded; every exercised surface is an in-memory test double.
3. **UNVERIFIED** — [S-167] overview specifics; SGP.22/SGP.32 semantics;
   identifier formats; the 62443 clause-content interpretation.
