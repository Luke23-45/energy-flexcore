# P14.04 — IEEE 2030.5 conformance test suite (WRITTEN-UNRUN)

| Field | Value |
|---|---|
| Subtask | P14.04 — CSIP use-case matrix incl. DERControl scheduling, response, and log-event flows; primacy / event-prioritization tests [S-156]. Tests written COMPLETE and UNRUN; execution and any lab correlation are HUMAN GATES |
| Wave / agent | Phase 14 wave 1, Agent A |
| Status | **WRITTEN-UNRUN — execution is a human gate** (conventions §1 item 3). **No conformance is claimed, implied, or preparable-by-agents**: CSIP certification is a human-gated lab process (conventions §7.3; QualityLogic/CSIP labs [S-5]). This suite is in-repo evidence preparation for the P14.19 evidence plan, nothing more. |
| Sources | [S-1] IEEE 2030.5-2018, [S-3] SunSpec CSIP Implementation Guide v2.1, [S-5] QualityLogic Intro to 2030.5, [S-156] SunSpec IEEE 2030.5 in production; the P14.02 record (02) |
| Files touched | `crates/adapters/protocol-20305/tests/csip_usecase_matrix.rs` (NEW), `crates/adapters/protocol-20305/tests/primacy_and_lifecycle.rs` (NEW) |

## 1. What was written

| File | Content | Honest status |
|---|---|---|
| `tests/csip_usecase_matrix.rs` | Ten resource-tree use cases U1–U10 (§2), each one test function, each building a full server tree through the public API only | Written complete; UNRUN |
| `tests/primacy_and_lifecycle.rs` | Four primacy use cases P1–P4 and four lifecycle use cases L1–L4 (§3), including the frozen-table gate-skipping refusals | Written complete; UNRUN |

## 2. Use-case matrix, part 1 — the CSIP resource tree

| # | Use case (secondary-source basis, UNVERIFIED detail) | Test function | What it pins |
|---|---|---|---|
| U1 | Device enrollment appears in the EndDeviceList [S-3] | `u1_enrollment_appears_in_the_end_device_list` | SFDI + LFDI carried; FSA/DERP links present |
| U2 | FunctionSetAssignments binds DERP and RSPS to a device [S-3] | `u2_fsa_binds_the_program_and_response_lists` | mRID, declared primacy, both links |
| U3 | DERProgram exposes DefaultDERControl and DERControlList [S-3] | `u3_program_exposes_default_control_and_control_list` | The program's two links |
| U4 | DefaultDERControl is the no-active-control state [S-3] | `u4_default_control_is_served_as_the_no_active_control_state` | The fallback base payload |
| U5 | DERControl scheduling: start/duration/randomization [S-1][S-3] | `u5_control_scheduling_windows_answer_active_at_correctly` | Half-open windows; declared randomization shift |
| U6 | Device posts a Response for an applied control [S-3] | `u6_device_response_is_recorded_and_served` | POST → Created → served list |
| U7 | Device posts LogEvents; the aggregator retains them [S-3] | `u7_device_log_events_are_recorded_bounded_and_served` | Bounded retention, served contents |
| U8 | PricingList present in the served tree [S-3] | `u8_pricing_list_is_present_in_the_served_tree` | Presence and shape of OUR declared minimal type (never protocol pricing semantics) |
| U9 | List paging semantics (limit/offset, meta recount) [S-3] | `u9_list_paging_recounts_meta_and_slices_entries` | Exact `all`/`results`, sliced entries |
| U10 | Undeclared paths refused indistinguishably from missing ones | `u10_undeclared_and_uninstalled_paths_are_indistinguishable` | 404 uniformity; NotAllowed only for declared read-only collections |

## 3. Use-case matrix, part 2 — primacy and the lifecycle entry rule

| # | Use case | Test function | What it pins |
|---|---|---|---|
| P1 | Higher-priority (lower DECLARED primacy) control wins while competing [S-156] | `p1_lower_declared_primacy_wins` | Rule 1 of `resolve_primacy` |
| P2 | Equal primacy: earlier creation wins; then deterministic mRID | `p2_equal_primacy_breaks_on_creation_then_mrid_deterministically` | Rules 2–3; order-independence; empty set → no winner |
| P3 | Utility override (1547-2018 Clause 10 [S-20]) as DECLARED enrollment | `p3_utility_override_wins_the_direct_comparison` | The utility side wins when enrolled with a strictly stronger primacy; the `utility_authority` flag is a declaration, never inferred |
| P4 | Non-overlapping controls never compete | `p4_non_overlapping_controls_never_compete` | Overlap as the competition precondition |
| L1 | External DERControls enter at PROPOSED — always | `l1_external_controls_enter_at_proposed_always` | The CONST entry state |
| L2 | The adapter records PROPOSED or PROPOSED-gate REJECTED, nothing else | `l2_adapter_records_proposed_or_gate_rejected_nothing_else` | Every other PROPOSED-edge refused |
| L3 | Gate-skipping (PROPOSED → ISSUED/APPROVED/…) refused by the frozen table | `l3_gate_skipping_is_refused_by_the_frozen_table` | The frozen machine is the authority, not the wrapper |
| L4 | Undeclared primacy is never defaulted into competition | `l4_undeclared_primacy_is_never_defaulted_into_competition` | Declared-never-inferred |

## 4. What this suite is NOT

1. **Not a conformance suite in the certification sense.** The QualityLogic/
   CSIP lab process tests the WIRE behaviour of a running server against
   the standard's test procedures [S-5]; this suite exercises the DESIGN
   SURFACE of an unrun skeleton. It prepares evidence structure; it cannot
   produce a conformance result.
2. **Not a claim about real devices.** Every fixture is in-process model
   data; no device, server, or network exists in this program (plan §6
   acceptance 2).
3. **Not executed, not passing.** The tests have never compiled. The first
   human build may surface defects (conventions §7.1); recorded failures
   are then fixed as code edits — the record updates with them, honestly.

## 5. UNVERIFIED / UNRUN register

1. **UNRUN** — both files, entirely. Execution is the human gate.
2. **UNVERIFIED** — every use-case detail transcribed from [S-1][S-3][S-5]
   [S-156] (spellings, hrefs, list semantics, primacy convention); the
   UNVERIFIED labels live at the type/constant definitions (model.rs,
   addressing.rs, server.rs) and the file headers of both test files.
3. **Lab correlation is a separate human process** — when the P14.19 lab
   path runs a real CSIP test harness, its results are human-gate inputs
   recorded per conventions §5; they neither validate nor invalidate this
   suite mechanically.

## 6. Handoff notes

1. **P14.19**: this suite is the in-repo evidence skeleton for the CSIP
   lab conversation; the lab's own procedures govern certification.
2. **Wave 3 (P14.17)**: the simulated 2030.5 client can execute this
   suite's flows end-to-end once it lands; until then the suite stays
   UNRUN.
3. **The first human build** runs both files as part of `cargo test`;
   failures are recorded and fixed as code edits (honesty rule; no
   silent reinterpretation).
