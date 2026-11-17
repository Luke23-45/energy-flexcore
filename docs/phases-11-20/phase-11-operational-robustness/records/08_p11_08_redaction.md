# P11.08 — PII/secret redaction at emit

| Field | Value |
|---|---|
| Subtask | P11.08 (PII/secret redaction at emit; redaction unit tests written complete and UNRUN) |
| Wave / agent | Phase 11 wave 1, Agent A |
| Status | WRITTEN-UNRUN: the redaction module, its integration into the emit path, and its test suite are written; NOTHING has ever executed. The test suite's execution is a human gate (conventions §1.3). |
| Sources | [S-86] (GDPR Art. 17 — the erasure/PII boundary Phase 13's data-governance work will consume; NOT exercised in this phase), [S-62]/[S-63] (the emit path is the log-schema record's emitter) |
| Files touched | `python/workers/worker_template/redaction.py` (new), `python/workers/worker_template/log_schema.py` (the emit path calls redaction before serialization), `python/workers/tests/test_redaction.py` (new, complete, UNRUN) |

## 1. The boundary: EMIT, never READ

Redaction runs INSIDE the emission path — `log_schema.emit_log_line`
applies the redaction rules to the message and every attribute value
BEFORE the line is serialized and written. There is NO
redaction-at-read promise anywhere: a line already emitted is a
historical record; retention and erasure on downstream stores are the
Phase 13 data-governance scope (the DSAR/erasure boundary is recorded
in the phase context glossary, [S-86]). No function or comment in the
module claims retroactive safety — the module header states the
boundary explicitly.

## 2. The rules (closed, deny-by-default on known-benign content)

1. **Key denylist** (closed set, `KEY_DENYLIST`): an attribute whose
   normalized key matches is redacted WHOLESALE — the value becomes the
   fixed marker `[REDACTED]` (`REDACTION_MARKER`), whatever it is.
   Normalization collapses `_`/`-`/space separators and lowercases, so
   `database_url`, `DatabaseURL`, and `database url` are the same
   entry. The set is CLOSED: adding an entry is a recorded amendment
   (this record), never a silent change.
2. **Value patterns** (closed set, `VALUE_PATTERNS`): bearer tokens,
   JWT shape, AWS access-key-id shape, URL credentials
   (`postgres://user:pass@…`), key-value leaks (`password=…`,
   `api_key: …`), long hex blobs (≥40 chars — deliberately above the
   32-hex correlation-handle length so trace ids and UUIDs survive),
   and email addresses (the PII class recorded in the phase context).
3. **Fixed marker**: every replacement is `[REDACTED]` — no partial
   masking, no reversibility.
4. **Deny-by-default on over-redaction**: the tests pin that ordinary
   operational content (command ids, tenant ids, asset states, 32-hex
   handles) is NOT redacted — over-redaction would destroy the
   correlation value the P1.20/P11.07 lines exist for.
5. **Recursive walk**: dict/list attribute values are walked; non-string
   leaves (numbers, bools) are never rewritten.

## 3. Recorded limitations (what the rule set does NOT catch)

- Free-text PII that is not an email shape (names, phone numbers,
  addresses) is NOT detected — no NLP heuristic is claimed or wanted
  here; the honest boundary is the closed pattern list.
- Secrets shorter than the pattern minimums (e.g. an 8-char bearer
  token) may pass through — the patterns are deliberately conservative.
- Non-Python emitters (the Rust `tracing` path) do NOT call this module:
  the Rust correlation line's no-secrets rule (P1.20: Authorization
  never logged, no body content) remains the Rust-side boundary; wiring
  the same closed rule set into the Rust emitter is a recorded HANDOFF
  NOTE for wave 2 (the `track_metrics` line already carries only the
  closed field set, but a generic `redact_string` Rust twin does not
  exist yet).
- The Python worker template had no log emitter before P11.07, so there
  is no legacy emit path to retrofit; `emit_log_line` is the single
  Python entry point and always redacts (no toggle — a toggle would
  invite emit-without-redaction paths).

## 4. The tests (complete, UNRUN)

`python/workers/tests/test_redaction.py` (mirroring the P7.17 test
suite's honesty header and run procedure): key denylist (exact +
normalized spellings, original key preserved), value patterns (each
closed pattern with a synthetic adversarial input, plus the
known-benign must-survive set, including the 32-hex/UUID boundary),
recursive walk, non-string leaves untouched, and the emit boundary
(`emit_log_line` output contains the marker and none of the injected
secrets; exactly one line written; mandatory attributes present). All
inputs are synthetic test strings, not discovered credentials.

## 5. Honesty boundary

- No redaction rule has ever executed against real or synthetic input
  in this program; the suite pins INTENDED behavior, and the first
  human run is the gate.
- No GDPR/privacy-conformance claim of any kind is made: this is an
  emit-time filter with a recorded scope, not a data-protection
  control [S-86] is cited for the boundary context only.
