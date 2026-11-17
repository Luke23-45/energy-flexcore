# P12.09 — Fault-injection expansion (network partition, clock-skew schedule, partial write, slow consumer)

| Field | Value |
|---|---|
| Subtask | P12.09 (fault-injection expansion of `apps/worker/python/drill/fault_injection.py` — broader fault taxonomy as injected callables, stdlib-only pattern preserved; complete test files written-unrun) |
| Wave / agent | Phase 12 wave 1, Agent B |
| Status | **WRITTEN-UNRUN.** The extension and its test mirrors are written; nothing has been executed — not the self-tests, not the pytest suite (the loop bans; running them is a human gate). The P8.24 discipline is unchanged: every fault is a pure data-flow transformation over an injected callable; no socket is opened, no file is written, no sleep is performed, no clock is read. |
| Sources | The P8.24 scaffolding itself (`apps/worker/python/drill/fault_injection.py` — the pattern followed exactly); `apps/worker/src/supervision.rs` (`DEFAULT_CALL_TIMEOUT_MS` = 30 000, transcribed with the copy rule); plan §2.2 (the fault taxonomy the chaos experiments need) |
| Files touched | `apps/worker/python/drill/fault_injection.py` (extended, additive), `apps/worker/python/drill/tests/test_fault_injection.py` (extended, additive) |

## 1. What was added (the P12.09 taxonomy)

| Fault | Mechanism (all pure data-flow over the injected callable) | Modelled failure |
|---|---|---|
| `partial_write` (plan kind) | the target RUNS (the side effect happened), the delivered reply is TRUNCATED to the first `keep_bytes` (bytes/str replies only); an unsliceable reply raises the named `PartialWriteUnsupported` AT CALL TIME with `target_called=True` recorded — the side effect never vanishes from the record | a torn/partial write at the seam |
| `slow_consumer` (plan kind) | the target runs, the reply is delivered UNCHANGED, and the injected `delay_ms` travels as DATA in `CallRecord.delay_ms`; the scaffolding NEVER sleeps and reads NO clock — the timeout arithmetic (`exceeds_supervision_timeout`, vs the transcribed 30 000 ms) is hand-computed by the drill | a consumer whose replies arrive inside the timeout window or beyond it |
| `network_partition` (window kind, `PartitionInjector`) | a bounded partition WINDOW over the call sequence: before the window pass-through; inside the window the named `NetworkPartitionError` with the target UNREACHED; after the window pass-through (healed). Window bounds (`open_after_calls` ≥ 0, `closed_for_calls` ≥ 1) are validated at construction (fail-closed) | a NATS/Postgres outage window (the C-NATS-1 shape) at the callable seam |
| `scheduled_skew_clock` (helper) | the generalization of `skewed_clock`: a PER-CALL offset schedule; call i uses `offsets_ms[i]` while inside the schedule, then the LAST offset PERSISTS (a drift that plateaus — the documented modelling choice); negative offsets legal; an empty schedule is a construction refusal | clock drift that changes over the drill (interplays with the P6.31 hold-on-drift rule's test fixtures) |

Additive-only changes to the existing module: two new `Fault` payload fields (`keep_bytes`, `delay_ms`) with fail-closed validation; one new `CallRecord` slot (`delay_ms`, default 0 — every existing record is unchanged); two new exception classes; the vocabulary gained `network_partition` (carried ONLY by `PartitionInjector`, never a plan entry); one transcribed constant (`SUPERVISION_DEFAULT_CALL_TIMEOUT_MS`, the copy rule stated at the definition). `stats()` keys are UNCHANGED byte-for-byte — all P8.24 self-tests and pytest mirrors still compare the same dicts.

## 2. Tests (written, never run)

The pytest suite (`tests/test_fault_injection.py`) gained mirrors of the new self-tests 17–24, plus the zero-delay legality case, the transcription-copy check, and two parametrized refusal groups (partition windows; partial_write/slow_consumer payloads): truncation arithmetic, the unsliceable fail-closed path, delay-as-data with the 45 000 ms vs 29 999 ms timeout classification, the partition close→heal window, construction refusals, the per-call-then-plateau skew schedule, and the constant transcription check. Execution command (human gate only):

```sh
python -I apps/worker/python/drill/fault_injection.py        # self-tests
python -m pytest apps/worker/python/drill/tests/test_fault_injection.py -v  # mirrors
```

## 3. Honest boundaries

- No fault reaches any real system: `partial_write` truncates an in-memory return value; `slow_consumer` records a number; `PartitionInjector` raises a Python exception in-process; `scheduled_skew_clock` adds integers. Nothing is partitioned, written, delayed, or skewed outside the injected callable.
- No sleep, no thread, no socket, no file I/O was added — the module remains stdlib-only and import-pure.
- The new faults model the four taxonomy rows plan §2.2 needs at the CALLABLE seam; real-process fault injection (killing containers, iptables rules, tc delays) remains the chaos experiment scripts' lab-control domain (record 21), never this module's.
- §13.3 semantics: when the human gate runs these tests, zero failures is a test outcome of a finite suite, never a guarantee about any runtime.

## 4. ASSUMPTION / UNRUN labels

- **UNRUN**: every new self-test and pytest mirror; the module has never been imported by anything outside its own human-gate entrypoint.
- **ASSUMPTION**: the transcribed `SUPERVISION_DEFAULT_CALL_TIMEOUT_MS` matches `apps/worker/src/supervision.rs` at reconciliation time (the copy rule — a mismatch is a FINDING, never a silent re-pin).
- **ASSUMPTION**: `bytes`/`str` slicing covers the toy reply types the drills use; a structured reply type would fail closed (named), which is the intended behavior.

## 5. Handoff notes

1. **P12.07 scripts**: `PartitionInjector` and `scheduled_skew_clock` are the callable-seam vocabulary the chaos experiments' lab-control faults mirror at the process level; the two stay separate by design (pure data-flow vs real lab control).
2. **Human gate**: the module's `__main__` self-test runner and the pytest suite both run ONLY at the human gate, with `python -I`.
3. **Reconciliation**: if any Phase 0–10 code change moves `DEFAULT_CALL_TIMEOUT_MS`, the transcription and this record's citation must be re-diffed.
