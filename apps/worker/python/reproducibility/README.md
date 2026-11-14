# P8.20 — Reproducibility tooling (pure stdlib, over injected data)

Subtask: P8.20 (`ledger/phase-08-advanced-coordination/ledger_phase_8.md`
§P8.20). The ledger deliverable is the **reproducibility standard +
artifact store + rerun-test records**; the decision record lives at
`ledger/phase-08-advanced-coordination/20_reproducibility.md`. This
directory holds the pure-Python part of that standard: the record-format
validator, the recorded seed-derivation implementation, and the rerun
comparator. All three are WRITTEN, NEVER RUN (Phase 8 loop bans:
nothing installed, built, or executed; no pytest) — the hand-computed
self-tests are the human gate's first check.

**Nothing here has ever run, and no reproducibility record or rerun-test
record exists anywhere.** Every measured/executed cell in the P8.20
record is EMPTY; no rerun has happened. The tooling is pure stdlib over
INJECTED data: it performs no I/O, stores nothing, and never touches a
dispatch-shaped concept (reporting-only, ADR-010).

## Modules

| Module | What it pins | Self-test entrypoint |
|---|---|---|
| `seed_derivation.py` | The frozen `flexcore-research-benchmark-set-v1` seed derivation `seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))` — FNV-1a 64 against the PUBLISHED test vectors, splitmix64 against hand-computed/theorem-derived expectations | `python apps/worker/python/reproducibility/seed_derivation.py` |
| `record_schema.py` | The versioned record schema `flexcore-research/reproducibility-record.v1` — what every run MUST record (versions, seeds + derivation, config digest, input snapshots + artifact digests, artifact-store refs, environment, tolerance class + P8.38 pre-registration ref) and the named refusals for every missing/placeholder/dishonest field | `python apps/worker/python/reproducibility/record_schema.py` |
| `rerun_check.py` | The rerun test's comparator: identity check before comparison, output comparison within the INJECTED pre-registered tolerance (never derived here — X-8), the exclusion marking for non-reproducing results | `python apps/worker/python/reproducibility/rerun_check.py` |

## How it is verified (the human gate)

```sh
# (a) the in-module hand-computed self-tests (no framework, no pytest):
python apps/worker/python/reproducibility/seed_derivation.py
python apps/worker/python/reproducibility/record_schema.py
python apps/worker/python/reproducibility/rerun_check.py

# (b) the pytest mirror:
PYTHONPATH=apps/worker/python/reproducibility python -m pytest \
  apps/worker/python/reproducibility/tests -v

# (c) the documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py
```

A green (a)/(b) earns a one-line human build/test gate record in
`20_reproducibility.md`; the measured cells there stay EMPTY until real
runs and reruns exist.
