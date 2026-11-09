# P0.30 — What Phase 0 explicitly did NOT build, decide, or claim

Provenance: subtask P0.30 (ledger_phase_0.md). Date: 2026-11-07. Status: draft for the Stage 0 gate.

## 1. What was not built

No API server, no database or migrations, no NATS streams, no telemetry
ingestion, no asset registry, no simulator, no operator console, no plugin
host, no solver integration, no CI pipeline, no Docker/Compose files, no
Protobuf contracts, no OpenAPI document. No crate other than
`crates/flexcore-domain` exists; no crate beyond it was scaffolded "to be
ready".

**Recorded deviation (read this before citing the ledger criterion):** the
original ledger stance was "no code files added by Phase 0 work"
(`ledger_phase_0.md` P0.30 acceptance criterion 2; `PLAN.md` §1 non-goals).
The implementation coordinator's loop rules explicitly override this: "Phase
0 … implement them as code contracts (types, enums, state machine, validation,
constants)". Phase 0 therefore created a minimal Cargo workspace with ONE
crate, `crates/flexcore-domain`, holding the frozen contracts (units, quality,
time model, command state machine, event vocabulary, API path constants) with
their tests. This is the single deviation from the ledger's documents-only
position, and the criterion below is left UNTICKED for that reason. Phase 1
owns the full workspace (report §8 layout); `flexcore-domain` is its seed, not
a substitute.

## 2. What was not decided

D1 (jurisdiction/market), D2 (first portfolio and service), D3 (network
class), D4 (license audit), D5 (protocol/asset mix), D6 (cyber/OT boundary) —
all recorded as OPEN or SCHEDULED with owner roles and venues; no ADR decides
any of them. No jurisdiction, market programme, protocol, or license is named
as chosen anywhere in the Phase 0 artefacts. Named gate owners and calendar
dates are assigned at the Stage 0 gate meeting (the ledger forbids inventing
them). No benchmark acceptance thresholds were set (§19.4). No RPO/RTO values
were invented (§21.2).

## 3. What was not claimed

No safety, certification, or protection-grade claim; no protocol-conformance
claim; no solver-capability claim beyond report §7.4 (HiGHS is LP/QP/MIP, not
SOCP; Clarabel is convex conic, not MISOCP; AC power flow is not AC-OPF; IPOPT
is local, not global); no performance or scale claim; no 0.x field-control
promise (the entire 0.x line is simulation-only; field write authority begins
at v1.0.0 with operator approval). ADR-001…ADR-012 are "Proposed", not
"Accepted" — the Stage 0 review has not been held (see
`29_exit_gate_review.md`).

## 4. Repository state

- `docs/foundations/*` — untouched (read-only reference; read for P0.26 only).
- `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` — untouched.
- `.agent/DECISIONS.md` — updated only to record the P0.03–P0.06/P0.28
  scheduled-gate status; no decision row added.
- New files: `ledger/phase-00-product-freeze/01…30` freeze documents (this
  directory), `docs/architecture/adr/ADR-001…012.md`, and the
  `crates/flexcore-domain` workspace (the recorded deviation above).
- Nothing committed or pushed by this work (per task instruction); the human
  owns all commits.

The P0.30 acceptance criterion "no new code files" is therefore NOT met as
written — see the deviation note in §1. The criterion "the list is referenced
from the Phase 1 handover pack (P0.27)" is met: `27_handover_pack.md` row 24.
