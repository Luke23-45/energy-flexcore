# Neryva FlexCore

**FlexCore** is an open, modular Virtual Power Plant (VPP) orchestration and
distribution-aware DER coordination platform.

This repository holds the **implemented FlexCore codebase**: 29 Cargo
workspace members (15 core/infrastructure crates, 10 protocol and
persistence/broker adapters, and 4 application binaries), the TypeScript
operator console, Python numerical workers, and the deploy, security,
harness, and documentation trees that surround them.

## What state this is in — read this first

- **WRITTEN-UNRUN.** The code in this repository has never been compiled,
  tested, or run. The first human `cargo build --workspace` (and then
  `cargo test`, `cargo clippy`, and the console/npm test suite) is the real
  gate: every verification record in the ledger is UNRUN/EMPTY until then,
  and no claim in any document upgrades that state.
- **Simulation-first, by construction.** The entire 0.x line is
  simulation-only. No command has ever been issued to a device by this
  platform. Field write authority is a v1.0.0 release-ladder property with
  operator-approved control — and the v1.0.0 operator-mode qualification
  record is NOT QUALIFIED today
  (`ledger/phase-10-v1-release-hardening/15_operator_mode_qualification.md`).
- **v1.0.0 is the release-ladder TARGET, pending the human gates.** The
  release notes draft
  (`docs/releases/v1.0.0-release-notes-draft.md`) records each capability's
  actual state; no release candidate has been built. Treat "pilot-ready" as
  the target position, not an achieved state.
- Not a certification, not a safety case, and not permission for live grid
  control. FlexCore is a supervisory orchestration layer: it performs no
  primary frequency response, synthetic inertia, relay protection, inverter
  inner-loop control, or sub-cycle control.

## Repository map

| Path | Contents |
|---|---|
| `crates/flexcore-contracts` … `crates/flexcore-grid-api` | The core stack: frozen contracts, domain vocabulary, ports, application services, events, storage, audit, authz, optimization and grid APIs (hexagonal dependency rule; see `tools/check_dependency_rule.py`) |
| `crates/flexcore-simulator` | The grid/DER simulator the 0.x line runs against |
| `crates/edge-agent`, `crates/flexcore-kernel` | The edge agent and the plugin kernel (manifest validation, dependency resolution) |
| `crates/flexcore-protocol-ports`, `crates/adapters/*` | The protocol adapter crates: OCPP CSMS, Modbus, DNP3, MQTT/Sparkplug B, IEEE 2030.5, OpenADR, CIM/CGMES import, ENTSO-E price ingestion — each with its declared unwired transport seam (a seam is a design boundary, not a working connection) |
| `apps/control-plane`, `apps/sim-runner`, `apps/worker`, `apps/plugin-host` | The application binaries: REST control plane, scenario runner, numerical worker host, WASM plugin host |
| `console/` | The operator console (TypeScript/Vite; see `console/README.md` and `console/SUPPORT_MATRIX.md`) |
| `python/workers/` | The Python numerical workers |
| `deploy/` (compose, edge, k8s, load-test) | Deployment/upgrade artifacts — every run record holds its human gate |
| `security/` | Dependency-audit suppressions, SBOM freshness tooling |
| `proto/`, `wit/`, `plugins/` | Frozen Protobuf contracts, WIT plugin interfaces, plugin manifests/profiles/scenarios |
| `benchmarks/`, `scenarios/` | The forecast-eval benchmark set and the scenario library/synthetic portfolio |
| `harness/site-integration/` | The site-integration test harness pack (never built or run) |

## Documentation

| Path | Contents |
|---|---|
| `docs/foundations/Neryva_FlexCore_System_Design_Specification_v1.0.pdf` / `.docx` | Canonical system-design specification (source of truth; do not edit) |
| `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` | Product definition, architecture, delivery plan, open decisions |
| `docs/phases-11-20/` | The phase 11–20 program: conventions, per-phase records, and the final audit (`02_PROGRAM_FINAL_AUDIT.md`) |
| `ledger/` | The phase 0–10 implementation ledger: every subtask's status, verification cell, and the release-scope freeze |
| `docs/protocols/` | Per-protocol adapter design/status notes (OCPP, DNP3, MQTT, CIM/CGMES, ENTSO-E, …) |
| `docs/releases/v1.0.0-release-notes-draft.md` | The v1.0.0 release-notes DRAFT with the known-limitations register (DRAFT — not a release) |
| `docs/reviews/final_review_for_v1/` | The final-review program for v1: per-area review findings and their fix waves |

Read the report first. Section 0 is the executive decision; §2 is scope by
release; §25.2 lists the decisions that are still open.

## Canonical units and vocabulary (summary)

- **Units:** MW, MVAr, MVA, MWh. `P > 0` means export at the point of
  connection. Reactive-power sign is declared per grid model and device
  profile, never assumed. Voltage is kV at the nominal base, per-unit in
  solver representation. SOC is a fraction in [0, 1] at the API boundary.
- **Telemetry quality states** (exactly one per value): `GOOD`, `SUSPECT`,
  `BAD`, `MISSING`, `ESTIMATED`, `STALE`.
- **Command lifecycle:** `PROPOSED → VALIDATED → FEASIBLE → APPROVED →
  ISSUED → ACKNOWLEDGED → TRACKING → COMPLETED`, plus `REJECTED`, `FAILED`,
  `EXPIRED`, `CANCELLED`, `SUPERSEDED`, `FALLBACK`. Validation, feasibility,
  and authorization are distinct gates; acknowledgement is not physical
  delivery.
- **Control-authority modes:** Simulation, Read-only/shadow,
  Operator-approved, Bounded automation. Mode is a deployment and per-site
  setting; a plugin can never select it.

## Agent harness

The harness is tool-neutral. `AGENTS.md` is the single instruction file; the
files under `.agent/` are read on demand. None of it requires a specific client.

| File | Purpose |
|---|---|
| `AGENTS.md` | Instructions for any coding agent (open AGENTS.md format) |
| `.agent/README.md` | Harness layout and per-tool wiring |
| `.agent/CONTEXT.md` | Product and architecture digest |
| `.agent/DECISIONS.md` | Open decisions and recommended defaults |
| `.agent/WRITING.md` | Evidence, citation, units, and terminology rules |
| `.agent/skills/` | On-demand procedures in Agent Skills format |
| `.agent/roles/` | Portable role prompts: architect, docs, Rust, numerical, security |
| `.agent/PROVENANCE.md` | Sources used and the license audit |

Thin adapters, only where a tool does not read `AGENTS.md`:

| File | Tool |
|---|---|
| `CLAUDE.md` | Claude Code (imports `AGENTS.md`; never loads it twice) |
| `.gemini/settings.json` | Gemini CLI (`context.fileName` set to `AGENTS.md`) |
| `.github/copilot-instructions.md` | GitHub Copilot repository-wide instructions |

Cursor, OpenCode, Codex, and Claude Code v2.1.277+ read `AGENTS.md` directly and
need no adapter.

## Checks

Documentation checks require Python 3.11+; no third-party packages.

```sh
python tools/verify_docs.py      # headings, § refs, tables, fences, citations, TOC
python tools/validate_agent.py   # skills, roles, and README index
python tools/setup_agent.py --all  # optional native skill-discovery links
```

The Rust and console suites (`cargo build/test/clippy`, `npm test`) are the
human execution gate — see "What state this is in" above: nothing has been
built or run yet, and the workspace's own verification records stay
UNRUN/EMPTY until a human runs them.

## License

The repository has no license yet. The workspace `license` field is
deliberately unset and no `LICENSE` file exists: the license and dependency
audit is an open decision (D4, `.agent/DECISIONS.md`). Apache-2.0 is the
report's recommendation for core software — a recommendation, not a decision.
Resolving D4 and adding the license file is a recorded human gate ahead of any
v1.0.0 publication.
