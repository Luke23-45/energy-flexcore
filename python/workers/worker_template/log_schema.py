# P11.07 — Structured-log schema v1 emitter for Python workers, mapped to
#         the OTel logs data model [S-62].
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. STDLIB-ONLY and genuinely importable/runnable today. The emitter
#    writes ONE JSON line per call (the same one-line-per-event shape the
#    P1.20 control-plane correlation line uses), with the schema v1
#    contract of the phase-11 record
#    `records/07_p11_07_log_schema_v1.md`.
# 2. REDACTION IS AT EMIT (P11.08): every message string and attribute
#    value passes through `redaction` INSIDE `emit_log_line`, before the
#    line is serialized. No redaction-at-read promise is made anywhere
#    (see the redaction module header).
# 3. SeverityNumber ([S-62]: TRACE 1–4, DEBUG 5–8, INFO 9–12, WARN
#    13–16, ERROR 17–20, FATAL 21–24): the mapping table below covers
#    stdlib `logging` and `structlog` level NAMES (structlog uses the
#    logging levels; both spellings map to the band base). Unknown level
#    names map to 0 (UNSPECIFIED) — never a guessed band. The Rust
#    (`tracing`) half of the mapping lives in
#    `apps/control-plane/src/metrics.rs::severity_number_for_level` (same
#    numeric table, pinned by that module's tests).
# 4. Determinism note (P7.42 posture): log lines are operational
#    metadata, NOT result payloads — the UTC timestamp here never enters
#    a worker result. A deterministic-mode worker's results are
#    unaffected.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import json
import sys
from datetime import datetime, timezone
from typing import Any, Optional, TextIO

from . import redaction

# The schema version string carried by every line (single source with the
# Rust emitter, which carries the same constant).
LOG_SCHEMA_VERSION = "flexcore.log.v1"

# The OTel SeverityNumber band bases [S-62], for stdlib logging AND
# structlog level names (structlog's levels are the stdlib logging
# levels; "warn" is accepted as the structlog alias of "warning").
SEVERITY_NUMBERS: dict[str, int] = {
    "trace": 1,
    "debug": 5,
    "info": 9,
    "notice": 9,  # RFC 5424-ish alias some emitters use; INFO band.
    "warning": 13,
    "warn": 13,
    "error": 17,
    "exception": 17,  # logging.exception logs at ERROR.
    "critical": 21,
    "fatal": 21,
}

# The attribute keys every schema-v1 line carries (present even when the
# value is the empty string — see the record for absent-vs-empty
# semantics).
MANDATORY_ATTRIBUTES: tuple[str, ...] = ("tenant_id", "command_id", "trace_id")


def severity_number_for_level(level: str) -> int:
    """The SeverityNumber band base for a level name (stdlib/structlog
    spellings, case-insensitive); 0 (UNSPECIFIED) for anything the table
    does not name — the same deny-by-default rule as the Rust mapping."""
    if not isinstance(level, str):
        return 0
    return SEVERITY_NUMBERS.get(level.strip().lower(), 0)


def emit_log_line(
    level: str,
    message: str,
    *,
    tenant_id: str = "",
    command_id: str = "",
    trace_id: str = "",
    stream: Optional[TextIO] = None,
    **attributes: Any,
) -> str:
    """Build, redact, serialize, and write ONE schema-v1 log line.

    The line shape (record 07 is the contract):
      {"schema_version": "flexcore.log.v1",
       "severity_number": <band base per [S-62]>,
       "severity_text": <the caller's level, uppercased>,
       "timestamp": <RFC 3339 UTC, emit time>,
       "message": <redacted>,
       "tenant_id": ..., "command_id": ..., "trace_id": ...,
       <extra attributes, redacted>}

    Redaction (P11.08) runs at emit, BEFORE serialization: the message
    through the value-pattern set, the attributes through the key
    denylist and the value rule. Returns the serialized line (so a caller
    may pin its shape); writes it with a trailing newline to `stream`
    (DEFAULT stdout; tests pass an `io.StringIO` to capture).
    """
    if stream is None:
        stream = sys.stdout
    severity = severity_number_for_level(level)
    line: dict[str, Any] = {
        "schema_version": LOG_SCHEMA_VERSION,
        "severity_number": severity,
        "severity_text": str(level).strip().upper(),
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "message": redaction.redact_string(str(message)),
        "tenant_id": tenant_id,
        "command_id": command_id,
        "trace_id": trace_id,
    }
    for key, value in attributes.items():
        line[key] = redaction.redact_value(value)
    serialized = json.dumps(line, sort_keys=True)
    stream.write(serialized + "\n")
    return serialized
