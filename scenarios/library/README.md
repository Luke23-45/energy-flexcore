# `scenarios/library/` — the Phase 5 scenario library (§19.3 benchmark cases)

**Everything in this directory is SYNTHETIC.** No IEEE feeder model is used
or claimed anywhere: the per-case IEEE record (§19.3 step 2) is part of
every record and reads `included: false` with the reason — no IEEE
13/34/123-bus distribution or 39/118-bus transmission model is available
in this repository and neither licensing nor model availability is
confirmed (the same honest entry as the `14_15_adapters.md` feeder-case
record). A future inclusion requires the confirmed license/availability
recorded in the case's `ieee_case_record` FIRST.

## What a record is

Each `lib-*.json` file is a **ScenarioRecord v3** (P5.18): a pinned,
replayable record carrying the pinned telemetry scenario (path + id +
version), the pinned forecast fixture when one is consumed, the named
synthetic grid-model fixture, the objective configuration (from the
declared set), the dispatch/tracking parameters, the backend selection
with the provider pin, the master seed with the recorded derivation rule,
the clock configuration, the pre-registered tolerances, the pre-registered
expected-outcome slot, and the nondeterminism-source inventory.

- Paths in `inputs.scenario.path` / `inputs.forecast_fixture.path` resolve
  **relative to the record file's directory**; the worker
  `script_path` is **repo-root-relative** (the CLI convention).
- Replay (`flexcore-sim-runner replay-run --record <path> …`) uses ONLY
  the recorded inputs; any input that cannot be reproduced from the record
  is an error, never a silent default (P5.18 step 2).
- Expected outcomes are **pre-registered and UNRUN** (the P3.26
  anti-fitting discipline): written before any run; when the scenarios are
  first run, a mismatch is a FINDING — never a reason to edit the
  expectations.
- Runtime classes in `INDEX.md` are DECLARED classes, not measured
  figures (measured runtimes are the P5.23 benchmark's obligation, under
  the P4.35 measurement rules).

## The scale limit (§19.3, mandatory statement)

**The first feeder tests do not establish capability for a 10,000-bus
utility network.** The library starts small (one asset), adds the
two-site fixture scale, then the meshed two-feeder fixture — all
synthetic, all balanced, all at fixture scale. No scaling claim of any
kind follows from these cases (report §19.3; the P4.35 raw-numbers rule
applies to any future measured figures too).

## Reproducibility (P5.19)

Same scenario + same versions + same seeds ⇒ same results within the
pre-registered tolerance: **byte-identical** for the scripted
deterministic legs; **relative 1e-9** (floor 1e-12, DEFAULT) for
floating-point worker legs. The tolerance table and the nondeterminism
inventory live in each record and in the decision record
(`ledger/phase-05-operator-console-simulator/18_20_replay_library.md`).
The double-run proof itself is UNRUN — it is the human build/test pass's
pre-registered run.
