# Phase 14 — Protocol and Device Connectivity Expansion: Exit-Gate Checklist

**Exit gate (plan §6, the phase's own acceptance criteria): every subtask
has a decision record; the deliverables are model/schema designs,
written-unrun test files, simulator profile specifications, and
decision/assessment records — no protocol is ever connected, no device
is ever reached, no message is ever exchanged; every adapter carries a
scope declaration with a mandatory non-empty `not_modelled` list and a
capability manifest, with the never-granted set byte-identical; the
dependency checker stays green with every new crate an additive member;
P14-E-1, P14-E-2, and P14-E-3 remain OPEN with DEFAULTs; no
certification, conformance, or safety claim appears anywhere.**

Each item states the evidence and its status. A checked item without
evidence is not checked. **Nothing in this phase has been built, run,
connected, enrolled, polled, published, parsed, or certified** — the
first human build and the record-101 lab paths are the program gates;
every "prepared" status below awaits its named human gate, and none of
these items claims runtime behavior.

## Plan §6 acceptance criteria (one-for-one)

- [x] **Criterion 1 — every subtask has a decision record citing the
  files touched and the [S-n] sources for every external claim.**
  Evidence: records 01–04 + 05 (wave 1 A), 21–24 + 25 (wave 1 B),
  41–44 + 45 (wave 2 A), 61–64 + 65 (wave 2 B), 81–84 + 85 (wave 3 A),
  101–103 + 104 (wave 3 B, the close) exist under `records/`. The
  ledger's P14.01–P14.16 ticks carry citations that all resolve to
  files on disk (close record §3.1); the P14.17–P14.24 ticks are the
  ORCHESTRATOR's post-close action, expected to cite records 81, 82,
  83, 84, 101, 102, 103, and 104. Status: **satisfied by written
  artifacts**.
- [x] **Criterion 2 — the deliverables are designs, written-unrun
  tests, simulator profile specifications, and decision/assessment
  records; no protocol connected, no device reached, no message
  exchanged.** Evidence: the honesty sweep (close record §3.4) over
  every phase artifact and the eight `docs/protocols/` documents; every
  transport seam is UNWIRED (`SeamNotWired` / `UnwiredTransport` /
  the refusing defaults); every execution-shaped statement in every
  record is negated or labeled WRITTEN-UNRUN / UNRUN / EMPTY /
  NOT-STARTED. Status: **satisfied by the audit** as of the close
  record's date (an inspection, not a tool run).
- [x] **Criterion 3 — every adapter carries a scope declaration with a
  mandatory non-empty `not_modelled` list and a capability manifest;
  the never-granted set is byte-identical before and after.**
  Evidence: the common `ProtocolScopeDeclaration` +
  `ProtocolAdapterManifest` (records 01, 21 §2, 23 §2, 24 §2, 43 §2,
  44 §2) with fail-closed `assert_supports` and the one-variant
  `SupervisoryRelayOnly` authority; `git status` shows
  `crates/flexcore-kernel/src/manifest.rs` and
  `crates/flexcore-domain/src/command_state.rs` NOT in any batch's
  modified-file set (close record §3.5 quotes both invariants
  verbatim). Status: **satisfied by the quoted evidence**.
- [x] **Criterion 4 — the dependency checker stays green after every
  batch; every new crate is an additive workspace member.** Evidence:
  the checker output quoted verbatim in every batch close-out (05, 25,
  45, 65, 85) and in close record §2 — 22 → 25 → 27 members across the
  waves, 27 at close; seven additive member entries total (the common
  ports crate + six adapter crates; waves 2B and 3 added none), all
  landed in the batch that gave them code. Status: **satisfied by the quoted
  evidence** (the tool checks the dependency rule, not runtime
  behavior).
- [x] **Criterion 5 — P14-E-1, P14-E-2, and P14-E-3 remain OPEN with
  DEFAULTs recorded; no certification, conformance, or safety claim
  appears anywhere.** Evidence: P14-E-1 registered OPEN (plan §4;
  record 102 §1 — the DEFAULT's priority application is recorded, not
  applied as a close), P14-E-2 OPEN (record 22 §3), P14-E-3 OPEN
  (record 42 §6); the ledger's decision register reads OPEN for all
  three; the zero-claim boundary is stated in every adapter record,
  every operator doc, the evidence plan (101 §5–§6), and verified by
  the §3.4 sweep. Status: **satisfied by the register entries**; no
  agent closed any decision.

## Human-gate inputs still pending (none of them closable by an agent)

| # | Gate input | Unblocks |
|---|---|---|
| 1 | First human `cargo build --workspace` / clippy / `cargo test --workspace` (compiles the six adapter crates, the simulator counterparts, the backpressure bindings, the edge seam modules, and every written-unrun test file) | every WRITTEN-UNRUN claim in the phase |
| 2 | The standard-text verification passes (records 02 §4, 03 §3, 21 §5, 23 §4, 24 §4, 41 §5, 42 §1/§4, 43 §4, 44 §4, 61 §4, 63 §4) — procurement of the IEEE/IEC/OpenADR/SunSpec/DNP texts first | every UNVERIFIED protocol detail; the implementation-time edits a mismatch triggers |
| 3 | Human closers for P14-E-1 (record 102 §3), P14-E-2 (record 22 §3), P14-E-3 (record 42 §6) — the named DR program, the named utility SCADA integration, the named EU DSO integration | the three OPEN decision points and the implementation order they set |
| 4 | The certification paths (record 101 §2–§3): lab selection, fees, harness procurement, the running implementations the labs test | 2030.5/CSIP, OpenADR, DNP3 certification — never an agent act |
| 5 | Operator sign-off of every registered threshold (the 2030.5 caps, the Modbus budgets, the record-84 alert thresholds and placeholder budget) and the first live Prometheus evaluation + alert-routing dry run (UNRUN) | the X-8 pre-registered values; the alert rules' first real evaluation |
| 6 | The MA TSRG transcription pass (record 61 §6) — a human transcribes the jurisdiction values with citations | the settings repository's first populated jurisdiction table |
| 7 | The reviewed common-layer publish-vocabulary decision (record 43 §2 / record 85 §5) and, when a first protocol adapter's edge-side implementation is proposed, its registry flip with a mapping-table plan and a P6.02 audit note (record 82 §6) | the MQTT manifest under-declaration; the PLANNED → Implemented registry transitions |
| 8 | The orchestrator's ledger ticks for P14.17–P14.24, citing records 81–84, 101–103, and 104, plus the ledger status-line update | the expected 24/24 end state (close record §1) |

Full close position and the consistency-audit results:
`records/104_p14_24_phase_close.md`.
