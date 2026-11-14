# The good minimal example — the echo worker (Class C)

| Field | Value |
|---|---|
| Subtask | P7.24 step 2 (`ledger/phase-07-plugin-sdk/ledger_phase_7.md` §P7.24) |
| What this example IS | a reference guide to the shipped minimal worker — the code of record is `python/workers/worker_template/example_echo_worker.py`; nothing here duplicates it |
| Companion bad example | [../bad-plugin/README.md](../bad-plugin/README.md) |
| Honest status | WRITTEN-UNRUN: the echo logic is stdlib-only and runnable by inspection; `main()` needs grpcio (never installed in this loop); the pytest suite is written, never executed |

## 1. What the echo worker demonstrates

`example_echo_worker.py` is the smallest concrete worker on the template
(`python/workers/README.md` is the template guide of record):

- It accepts exactly one payload type, `flexcore.example.echo_request.v1`
  (the `ACCEPTED_PAYLOAD_TYPES` frozenset) — and refuses any other with
  the template's structured `unsupported_payload_type` refusal. Refusing
  what it cannot honour honestly is the first lesson: never approximate.
- Its `EchoWorkerLogic.evaluate(request) -> (payload_bytes, provenance)`
  computes a DETERMINISTIC result: the echo of the input payload plus its
  SHA-256 digest. Same input, same output bytes — exactly the property
  the P7.42 seeded replay compares.
- It holds no caches, imports nothing from the platform, and has no
  authority: its output is a proposal by construction (ADR-010).
- Its `main()` wires the template pieces: config (`config.from_env`),
  the health machine (`HealthStateMachine`: `STARTING → READY → DRAINING
  → STOPPING`), the shutdown sequence, the SIGTERM hook
  (`server.make_signal_hook`), and a `service_addition_cb` that would
  register the generated `flexcore.worker.v1` servicer into `serve()` —
  the generated import happens in the concrete worker, never in the
  template.

## 2. What a manifest for it looks like

The shipped Class C manifest
(`plugins/manifests/forecast-worker.manifest.json`) is the shape to copy:
`"class": "grpc-worker"`, entry-point key `target`, permissions from the
§5.7 vocabulary, secret REFERENCES (never values), and resource bounds —
all covered by [../../manifest-guide.md](../../manifest-guide.md).

## 3. Running it (PREPARED-UNRUN)

The echo LOGIC is stdlib-only and genuinely runnable today (by
inspection, or via the template's pure-logic tests). The `main()`
entrypoint needs grpcio, which was never installed in this loop; the
template raises the named deployment requirement when run without it.
The human pass (from `python/workers/README.md` §7):

```sh
python -m venv .venv-workers            # never committed
# replace the UNPINNED-REVERIFY entries in python/workers/requirements.txt
# with verified versions first (procedure in that file), then:
.venv-workers/Scripts/pip install -r python/workers/requirements.txt
.venv-workers/Scripts/pip install pytest   # record the version
PYTHONPATH=python/workers .venv-workers/Scripts/python -m pytest python/workers/tests/ -v
# buf gates once the worker proto is registered with the proto/ module:
buf lint proto/
buf breaking proto/ --against '.git#branch=main,subdir=proto'
```

The FIRST human build step for any concrete worker is wiring the
buf-generated `flexcore.worker.v1` servicer into `serve()`'s
`service_addition_cb` (the cb shape is fixed in `server.py`).

## 4. Conformance (P7.21)

The conformance suite (`crates/flexcore-kernel`/host contract tests,
P7.21 — written-unrun, owned by its own wave) is the contract every
plugin must pass: result provenance (report §9.2), the closed status
vocabularies, units-with-values, and the proposal-only boundary. The echo
worker is the minimal passing fixture by design; the
[bad plugin](../bad-plugin/README.md) is the deliberately failing
counterpart.
