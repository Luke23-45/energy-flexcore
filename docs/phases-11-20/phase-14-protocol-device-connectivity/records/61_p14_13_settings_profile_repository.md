# P14.13 — IEEE 1547-2018 settings-profile repository

| Field | Value |
|---|---|
| Subtask | P14.13 — function catalog (volt-var, volt-watt, freq-watt, constant-PF, ramp rates, enter/exit service, ride-through categories), jurisdiction default tables (MA TSRG first [S-19]), per-device settings state; a document store, never a claim that any device accepts any function [S-19][S-20] |
| Wave / agent | Phase 14 wave 2, Agent B (P14.13–P14.16) |
| Status | WRITTEN-UNRUN: the repository module and its tests are written, never compiled, never executed (conventions §1; the first human build is the program gate). No settings were read from or written to any device; no device exists in this program. |
| Sources | [S-19] MA TSRG Inverter Source Requirements Document (mass.gov), [S-20] IEEE 1547-2018 (standards.ieee.org/standard/1547-2018.html). **Neither text was consulted** — every function name, category name, and table-shape detail carries an UNVERIFIED label at its definition (§4). |
| Files touched | `crates/flexcore-domain/src/ieee1547_settings.rs` (NEW), `crates/flexcore-domain/src/lib.rs` (additive module registration only) |

## 1. What landed (deliverable class: domain vocabulary + document store)

| Piece | Where | Honest status |
|---|---|---|
| The function catalog: `SettingFunctionId` — closed 9-entry vocabulary (ConstantPf, VoltVar, VoltWatt, FreqWatt, RampRate, EnterService, ExitService, FrequencyRideThrough, VoltageRideThrough) with canonical names and the declared `is_curve` property | `ieee1547_settings.rs` | Implemented; UNRUN. Function set UNVERIFIED against [S-20] (§4 item 1) |
| The ride-through category vocabulary `RideThroughCategory` (CategoryI/II/III) | same | Implemented; UNVERIFIED against [S-20] (§4 item 2) |
| Curve-point axis vocabulary `CurveQuantity` (per-unit voltage/frequency, percent-of-nameplate axes) — units declared next to every number, never inferred | same | Implemented; UNVERIFIED (§4 item 3) |
| Setting values `SettingValue` (Curve / Scalar / Category / Enabled) with the validating `FunctionSetting::new` (curve functions must carry curves; finite numbers; mandatory quantity notes; mandatory source citations) | same | Implemented; UNRUN |
| Jurisdiction default tables as DOCUMENTS: `JurisdictionDefaultTable` (jurisdiction id + source citation + transcribed entries) and the insert-only `SettingsDocumentStore` (BTreeMap keyed by jurisdiction id; duplicate registration refused) | same | Implemented; UNRUN. **The repository ships NO transcribed values** (§2) |
| Per-device settings state: `DeviceSettingsState` with declared function support, `CertifiedBaselineRecord`, `AggregatorScheduleRecord`, `UtilityOverrideRecord`; conflicting/duplicate state writes refused with named errors | same | Implemented; UNRUN |
| Tests: catalog closedness, curve-function rules, insert-only store, no-fabricated-defaults, declared-support, state-conflict refusals, deterministic ordering | `#[cfg(test)] mod tests` | Written complete; UNRUN |

## 2. The document-store posture (the load-bearing rule)

**The repository records declared settings DATA. It never claims that any
device accepts any function [S-19][S-20].** Enforced at three points:

1. **Device function support is declared data** (`DeviceSettingsState::
   declared_function_support`), entered from an interconnection record or a
   device profile. The P14.14 workflow refuses every change to a function
   the device has not declared (`FunctionNotDeclared`). There is no
   default function set anywhere in the module.
2. **Jurisdiction tables ship with structure, not numbers.** The MA TSRG
   table [S-19] is the FIRST intended registration, and the store's shape
   anticipates it — but the module ships zero transcribed values: every
   value in a jurisdiction table is entered data with a citation, recorded
   at implementation time from the primary document. Pre-filling numbers
   from secondary material would be exactly the hallucinated-fact risk
   AGENTS.md §3 rule 1 forbids; an empty registered table with its source
   citation is the honest starting state, and `default_setting` returns
   `None` — never a fabricated default — for anything unregistered or
   untranscribed.
3. **Every setting record carries `source` + `provenance` +
   `source_citation`**, all mandatory (a citation-less setting is not a
   reviewed record). Jurisdiction-table entries must carry
   `SettingSource::JurisdictionDefault` (a construction check refuses
   foreign sources inside a jurisdiction document).

## 3. Placement and dependency rule

Settings vocabulary is domain data (the subtask's placement note), so the
module lives in `crates/flexcore-domain` and depends on nothing
infrastructure — the crate's dependency line is untouched
(`flexcore-contracts` only; no serde, no protocol-crate types). Protocol
adapters (the 2030.5 and Modbus settings surfaces) map their native
spellings INTO these types at their own boundary; the domain module never
imports protocol vocabulary (phase-14 context §2: adapters never define
parallel models — and the domain never mirrors them either). The checker
(`python -I tools/check_dependency_rule.py`) needs no new entries: no new
crate, no new edge (output quoted in the batch close-out, record 65).

## 4. UNVERIFIED register (implementation-time checks against the primary texts)

| # | Detail | Where |
|---|---|---|
| 1 | The exact [S-20] Clause 5/6 function names and the completeness of the catalog (e.g. reactive-power-priority, voltage-reference functions) | `SettingFunctionId` |
| 2 | The ride-through category set and the per-level mandatory categories | `RideThroughCategory` |
| 3 | Which quantities each curve's axes actually carry (per-unit vs volts; percent-of-nameplate vs W/var) | `CurveQuantity` |
| 4 | The MA TSRG table shape: which functions the SRD specifies, which categories it selects | `JurisdictionDefaultTable` (intended first registration, [S-19]) |
| 5 | Whether "enter/exit service" settings are a single function or several in the standard | `SettingFunctionId::EnterService` / `ExitService` |

A confirmed mismatch at that gate is a recorded edit to this module (a
rename, an additive variant, or a transcription), never a silent
reinterpretation (plan §7 note 6).

## 5. Honest boundaries

No device was read, written, or reached; no settings state exists outside
in-process test data; the entire 0.x line is simulation-only. This module
claims NO conformance with IEEE 1547-2018, IEEE 1547.1-2020, or UL 1741 SB
(the scope statement of `inverter_pq_model.rs` P3.08 carried forward
verbatim; conventions §7.3).

## 6. Handoff notes

1. **P14.14 (this batch)** consumes the repository types: the workflow's
   declared-support gate is `DeviceSettingsState::declares_support`, and
   the state records (`CertifiedBaselineRecord`,
   `AggregatorScheduleRecord`, `UtilityOverrideRecord`) are the workflow's
   write targets.
2. **Phase 15 (interconnection-record ingestion)**: the declared-support
   set and the certified baseline are the natural landing shape for UL
   1741 SB COC / 1547.1 test-report data [S-21][S-169][S-170].
3. **Human gates**: first build; the standard-text verification pass (§4);
   the MA TSRG transcription pass (a human transcribes [S-19] values with
   citations — an agent must not invent them).
