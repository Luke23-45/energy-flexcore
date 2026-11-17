# P16.15 — Program event management: OpenADR event lifecycle linkage, opt-in/out handling, M&V report delivery

| Field | Value |
|---|---|
| Subtask | P16.15 (Program event management — OpenADR event lifecycle linkage to the command lifecycle: an external event becomes PROPOSED commands per asset, NEVER an issued command; opt-in/out handling; report delivery for M&V [S-116][S-117]. Consumes the P14.05 mapping) |
| Wave / agent | Phase 16 wave 2, Agent A |
| Status | WRITTEN-UNRUN: the event records, the fail-closed opt-in gate, the PROPOSED-entry linkage type, and the M&V delivery records are written, never compiled, never executed. **No market or program event has ever arrived: no VTN is contacted, no event is received, no command candidate is opened, no report is transmitted.** The stores are empty; test fixtures are synthetic (ADR-011; conventions §7.3). |
| Sources | [S-116] (OpenADR DR Program Guide — the event/reporting posture), [S-117] (OpenLeadr reporting pattern); the P14.05 mapping (`crates/adapters/protocol-openadr/src/event_mapping.rs` — the PROPOSED-entry pattern, consumed by citation); `crates/flexcore-domain/src/command_state.rs` (read-only — the frozen 14-state/27-edge set untouched); ADR-010 (external events never pass PROPOSED without the three gates) |
| Files touched | `crates/flexcore-application/src/program_event_management.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive module line) |

## 1. What landed

| Item | Content |
|---|---|
| `ProgramEventRecord` | The external program event as RECORDED data: verbatim external ref, the DECLARED event-kind label (ADR-011 — no universal taxonomy), the end-exclusive event window (the domain `ParticipationWindow` reused), declared key/value data |
| `ParticipantDecisionRecord` | Opt-in/opt-out per asset with scope {ProgramWide, EventScoped} and an end-exclusive effective window; a change of mind is a NEW decision |
| `ProposedCommandLinkage` | The event → command-candidate linkage per asset. Its entry state is a CONSTANT function returning `CommandState::Proposed` — **not a field** — mirroring the P14.05 `ExternalCommandProposal::entry_state` pattern, so no code path can record a later entry state from external input. The constructible state vocabulary is exactly {PROPOSED_ENTERED, REJECTED_BY_GATE}; every forward state is the protected gates'. The candidate interval must fall INSIDE the event window (refused, never clipped) |
| Opt-in gate | FAIL-CLOSED: an asset is included only with an active opt-in whose effective window covers the event start; NO decision = NO candidate (`PROGRAM_EVENT_NO_PARTICIPANT_DECISION`). The latest decision by (recorded timestamp, decision id) wins; a recorded opt-out overrides an opt-in and produces a named `OptOutAppliedRecord` — never a silent omission. An opt-out-applied record for a non-opted-out asset is refused (`PROGRAM_EVENT_OPT_OUT_APPLIED_REALITY_MISMATCH`) |
| `MvReportDeliveryRecord` | The closed two-state delivery record [S-116][S-117]: AWAITING_REPORT → REPORT_RECORDED (the report reference is mandatory; the report itself is P16.05's artifact, consumed by citation). No TRANSMITTED/ACKNOWLEDGED state — transmission is not performed by this module |
| `ProgramEventStore` | BTreeMap-backed, register-only, deterministic decision resolution |

## 2. Recorded decisions

1. **External events enter at PROPOSED, structurally** (`EXTERNAL_EVENTS_PROPOSED_ONLY_STATEMENT`, pinned by tests): the linkage's `entry_state()` is a constant; the test suite re-verifies against the frozen machine that no PROPOSED → later shortcut edge exists (the P14.05 test pattern, mirrored). The P14.05 adapter mapping is consumed by citation — this module stores the candidate's identity (command id, source event ref, source subsystem) and provenance, never a re-mapping.
2. **Opt-in is fail-closed** (`OPT_IN_IS_FAIL_CLOSED_STATEMENT`, pinned by tests): silence is never consent; the negative tests pin no-decision, expired-window, and opt-out-wins refusals.
3. **A gate rejection is recorded as data, never as this module's decision**: REJECTED_BY_GATE records the lifecycle's own PROPOSED → REJECTED outcome; the module never advances a candidate.
4. **ADR-010 and the hard bans hold verbatim**: the frozen 14-state/27-edge set is byte-identical (read-only consumption of `command_state::try_transition`); external events never pass PROPOSED without the three gates; no new event-vocabulary entries are added (the frozen 26 untouched; P16-E-2 stays open with Agent B).
5. **No reactive target, no sign flipping, no unit conversion**: those belong to the P14.05 adapter boundary, consumed — never duplicated here.

## 3. Honest boundaries

1. No event has arrived; no candidate has been opened in any lifecycle; no report has been delivered or transmitted; every fixture value is synthetic.
2. The P16.05 M&V report linkage is by citation (reference only); the composition root binds report identities.
3. Every test is WRITTEN-UNRUN; execution is the human gate. The permitted-tool outputs are quoted in the close-out record (47).
