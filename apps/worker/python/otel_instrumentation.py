# FIM-F09.01 — Shared OpenTelemetry trace-context instrumentation for the
#         Python workers under apps/worker/python/: W3C traceparent
#         propagation, span-carrying log-line emission (P11.07 schema v1),
#         and emit-time redaction (P11.08 posture).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. IMPLEMENTED UNDER FIM-F09 (the Final Implementation Milestone wave for
#    feature F-09, docs/releases/final-implementation-before-publication.md
#    §5). WRITTEN, NEVER RUN as a worker dependency: the pure-logic core
#    (parse/serialize/extract/redact/format) is stdlib-only and carries
#    in-module hand-computed self-tests (`--self-test`), but NO worker has
#    ever imported this module in a live process, and NO emitted line below
#    has ever been observed in a running deployment. The first human run is
#    the gate (the FIM program's standing hard boundary: no build, no test
#    execution, no installation — everything is WRITTEN).
# 2. THE NO-OP FALLBACK IS THE DEFAULT POSTURE. The OpenTelemetry Python
#    SDK is OPTIONAL and NOT INSTALLED (never added to any requirements
#    file in this program). Every SDK touchpoint is a guarded import
#    (the torch.load-guard idiom of gnn_worker/gnn_inference_worker.py):
#    with the SDK absent the module is fully functional in its no-op mode —
#    trace context comes from W3C `traceparent` parsing, not from the SDK —
#    and the emitted log-line STRUCTURE is identical to the SDK-present
#    path. The SDK-present OTLP export path is UNPROVEN until a human runs
#    a worker with the SDK installed and a Collector endpoint configured;
#    every SDK call is marked `UNVERIFIED — otel API` exactly like the
#    P11.05 reference module
#    (`python/workers/worker_template/otel_tracing.py`).
# 3. HUMAN GATES (F-09 acceptance criteria that need live infra): a RUNNING
#    Collector gateway and FIRING SLO alerts are HUMAN GATES — acceptance
#    criteria 1-3 of the F-09 feature entry need live infrastructure this
#    program never had (no Collector has ever run; no span has ever been
#    exported). This module does NOT claim any of them. What it provides is
#    the Python half of acceptance criterion 1's propagation chain
#    (traceparent header → worker log line), written and self-tested.
# 4. HEADER NAMES — discovered, not invented. The JetStream header name the
#    Rust side uses is the W3C standard `traceparent`:
#    - SET side: `crates/adapters/nats/src/publisher.rs` (the P11.05
#      TraceHeaderProvider seam) injects the pairs the global W3C
#      `TraceContextPropagator` writes (`apps/worker/src/otel.rs::
#      trace_header_pairs`) as JetStream message headers on every publish.
#    - READ side: `crates/adapters/nats/src/consumer.rs` (DeliveredMessage)
#      reads the header by the name `traceparent` with NATS's
#      case-insensitive header lookup.
#    No Rust code declares a `trace_id`-named header. The Python side
#    therefore looks up EXACTLY `traceparent`, case-insensitively, and
#    consumes no other context header (`tracestate`, when the propagator
#    writes one, is not needed for correlation and is ignored — recorded).
# 5. TRANSPORT REALITY (honest channel statement): no JetStream header dict
#    reaches a Python worker in this repository's recorded contracts — the
#    Python workers speak the JSON-document discipline (ONE request JSON on
#    stdin → ONE result envelope on stdout) and the federation scripts read
#    a config FILE. The extraction helpers below implement the consumer
#    side of the P11.05 propagation contract for every carrier a Python
#    process can actually see today, in documented precedence:
#    (a) an explicit header dict passed by the caller (the JetStream read,
#        for any Python component that receives message headers),
#    (b) an OPTIONAL `traceparent` string field or `headers` object in the
#        JSON envelope (additive read; the closed request contracts do not
#        declare these keys today and validators ignore unknown request
#        keys — absence is the steady state, never required),
#    (c) the `TRACEPARENT` environment variable (RECORDED CHOICE for this
#        tree, derived from the W3C header name; NOT claimed as a
#        cross-tool standard). An absent or unparseable carrier means "no
#        inherited context" — NEVER a synthesized trace id (the P11.05
#        posture, verbatim).
# 6. LOG LINES follow the P11.07 structured-log schema v1
#    (records/07_p11_07_log_schema_v1.md): one JSON line per event,
#    `flexcore.log.v1`, SeverityNumber band bases, mandatory
#    tenant_id/command_id/trace_id attributes (empty = not applicable).
#    REDACTION AT EMIT (P11.08, records/08_p11_08_redaction.md): the
#    message and every extra attribute pass the closed rule set BEFORE the
#    line is serialized. NO redaction-at-read promise is made anywhere.
#    The redaction vocabulary below is a MIRROR of the closed sets that
#    live in `python/workers/worker_template/redaction.py` (KEY_DENYLIST,
#    VALUE_PATTERNS, the email pattern, the fixed `[REDACTED]` marker)
#    extended with the workers' own proposal-only vocabulary
#    (FORBIDDEN_RESULT_KEYS of e.g. gnn_worker/gnn_inference_worker.py) as
#    EXTRA-attribute keys — byte-faithful mirrors, both sets CLOSED:
#    amending them is a recorded amendment to the P11.08 record and the
#    F-09 ledger, never a silent change. The mandatory `command_id` schema
#    field keeps its P11.07 semantics (empty = not applicable) and is NOT
#    treated as an extra (see §REDACTION NOTE below).
# 7. LOG STREAM: schema-v1 lines from this module go to STDERR, never
#    stdout — the worker transport contract reserves stdout for EXACTLY
#    ONE result envelope (apps/worker/python/README.md). The P11.07
#    template emitter's stdout default applies to the server-shaped
#    worker template, not to these stdin/stdout workers (recorded
#    difference, not an oversight).
# 8. STDLIB-ONLY: imports json/logging/re/os/sys/dataclasses/datetime/
#    typing only. No third-party package is imported at module level or in
#    any pure-logic path. The SDK imports live INSIDE the guarded init
#    function and only run when an OTLP endpoint is configured AND the SDK
#    is installed.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import json
import logging
import os
import re
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Any, Mapping, Optional, TextIO

# ---------------------------------------------------------------------------
# The W3C `traceparent` grammar (W3C Trace Context Recommendation, the
# version-00 format). STRICT validation:
#   `version(2 lowercase hex) - trace-id(32) - parent-id/span-id(16) -
#    trace-flags(2)`, exactly 55 chars, dash-separated.
# Per the Recommendation: version `ff` is forbidden; an all-zero trace-id
# is invalid; an all-zero span-id is invalid; the hex grammar (HEXDIGLC)
# is LOWERCASE-only and a vendor MUST ignore a traceparent whose fields
# contain non-lowercase hex. Recorded strictness decision: this parser
# implements the lowercase-only grammar (the Rust
# TraceContextPropagator emits lowercase, so the recorded interop is
# unaffected) and REFUSES other versions (an unparseable header means
# "no inherited context" — never a guessed one); the repo-root reference
# `python/workers/worker_template/otel_tracing.py` is lenient on
# uppercase — the difference is this module's recorded strict posture,
# not a contradiction of the reference.
# Trace-flags: NO flags value is invalid per the Recommendation (`ff`
# means "all 8 flags set"); only the least-significant bit is defined
# (sampled); other bits are reserved and are propagated verbatim, never
# set by this module (serialization emits only what it parsed or the
# default `01` — no reserved bit is ever fabricated here).
# ---------------------------------------------------------------------------
_TRACEPARENT_PATTERN = re.compile(
    r"^([0-9a-f]{2})-([0-9a-f]{32})-([0-9a-f]{16})-([0-9a-f]{2})$"
)

# The forbidden version byte (W3C Trace Context: "version ff is forbidden").
_FORBIDDEN_VERSION = "ff"

# The only version whose full grammar the Recommendation defines (and the
# one the Rust propagator writes); other versions mean "no inherited
# context" for this strict parser (recorded decision, header item 5).
_SUPPORTED_VERSION = "00"

_TRACE_ID_ZERO = "0" * 32
_SPAN_ID_ZERO = "0" * 16

# The JetStream header name (exact W3C standard name; the lookup is
# case-insensitive — NATS header names are case-insensitive and the W3C
# processing model expects the header name in any case).
TRACEPARENT_HEADER_NAME = "traceparent"

# The environment-variable carrier (RECORDED CHOICE — honesty header item 5).
TRACEPARENT_ENV_VAR = "TRACEPARENT"

# The default trace-flags value this module serializes when it originates a
# context (sampled bit set; no reserved bits — header item 4 note).
DEFAULT_TRACE_FLAGS = "01"

# The SDK status strings the module reports (the no-op fallback is the
# DEFAULT posture — honesty header item 2).
SDK_STATUS_UNAVAILABLE = "sdk-absent-noop"
SDK_STATUS_DISABLED = "endpoint-unset-noop"
SDK_STATUS_INIT_FAILED = "sdk-init-failed-noop"
SDK_STATUS_ACTIVE = "sdk-active-unproven"


@dataclass(frozen=True)
class TraceContext:
    """The W3C trace context this module carries.

    `trace_id` and `span_id` are lowercase hex (32/16 chars). `span_id`
    is the span id the context carries: when the context was INHERITED
    from a `traceparent` carrier it is the upstream (remote parent) span
    id — with the SDK absent this module never generates a local span id
    (no fabrication, the P11.05 posture); when the guarded SDK path is
    active (UNPROVEN until the human gate) the span id would come from
    the SDK's active span. `trace_flags` is the verbatim 2-hex flags
    byte (reserved bits propagated, never set).
    """

    trace_id: str
    span_id: str
    trace_flags: str = DEFAULT_TRACE_FLAGS

    def traceparent(self) -> str:
        """The W3C `traceparent` header value of this context.

        Refuses to serialize an invalid context (never fabricates a
        malformed header) — returns an empty string instead, which every
        emitter here treats as "no context to propagate".
        """
        return serialize_traceparent(self)


def parse_traceparent(value: str) -> Optional[TraceContext]:
    """Strict W3C parse of a `traceparent` header value.

    Returns None (no inherited context — NEVER a synthesized id) when the
    value is not exactly the version-00 grammar in lowercase hex, or when
    the version is the forbidden `ff` (any other non-`00` version is
    refused under the strict posture — header item 4), or when the
    trace-id or span-id is all zeros. Pinned vectors live in this
    module's self-tests and in tests_fim_otel/selftest.py.
    """
    if not isinstance(value, str):
        return None
    match = _TRACEPARENT_PATTERN.match(value.strip())
    if match is None:
        return None
    version, trace_id, span_id, flags = match.groups()
    if version == _FORBIDDEN_VERSION:
        return None
    if version != _SUPPORTED_VERSION:
        return None
    if trace_id == _TRACE_ID_ZERO or span_id == _SPAN_ID_ZERO:
        return None
    return TraceContext(trace_id=trace_id, span_id=span_id, trace_flags=flags)


def serialize_traceparent(context: TraceContext) -> str:
    """The W3C `traceparent` value of a context, or "" when the context
    does not satisfy the strict grammar (never fabricates a malformed or
    spec-forbidden header)."""
    if not isinstance(context, TraceContext):
        return ""
    candidate = "-".join(
        (context.trace_id, context.span_id, context.trace_flags)
    )
    candidate = "%s-%s" % (_SUPPORTED_VERSION, candidate)
    match = _TRACEPARENT_PATTERN.match(candidate)
    if match is None:
        return ""
    _, trace_id, span_id, flags = match.groups()
    if trace_id == _TRACE_ID_ZERO or span_id == _SPAN_ID_ZERO:
        return ""
    return candidate


def trace_context_from_headers(headers: Optional[Mapping]) -> Optional[TraceContext]:
    """The trace context carried by a HEADER dict (a JetStream message
    header map — the exact consumer-side read of the P11.05 contract).

    Looks up the W3C standard name `traceparent` CASE-INSENSITIVELY (the
    convention the Rust consumer's NATS HeaderMap lookup uses); absent or
    unparseable → None (no inherited context, never synthesized).
    """
    if not isinstance(headers, Mapping):
        return None
    for key in headers.keys():
        if isinstance(key, str) and key.strip().lower() == TRACEPARENT_HEADER_NAME:
            return parse_traceparent(str(headers[key]))
    return None


def trace_context_from_envelope(envelope: Optional[Mapping]) -> Optional[TraceContext]:
    """The trace context carried by a JSON envelope (a request document or
    federation config), read ADDITIVELY (honesty header item 5b):

    - an optional top-level `traceparent` string field;
    - an optional top-level `headers` object carrying `traceparent`.

    Neither key is part of any closed request contract today; absence is
    the steady state and yields None (no inherited context). Never
    raises, never synthesizes — a malformed carrier is exactly "no
    context".
    """
    if not isinstance(envelope, Mapping):
        return None
    direct = envelope.get(TRACEPARENT_HEADER_NAME)
    if isinstance(direct, str):
        context = parse_traceparent(direct)
        if context is not None:
            return context
    nested = envelope.get("headers")
    if isinstance(nested, Mapping):
        return trace_context_from_headers(nested)
    return None


def trace_context_from_environment(environ: Optional[Mapping] = None) -> Optional[TraceContext]:
    """The trace context carried by the process environment under the
    recorded `TRACEPARENT` carrier name (honesty header item 5c). Absent
    or unparseable → None. `environ` defaults to the real process env;
    the parameter exists so tests pin the behavior without touching the
    process env."""
    source = os.environ if environ is None else environ
    try:
        value = source.get(TRACEPARENT_ENV_VAR)
    except Exception:  # noqa: BLE001 — a broken environ mapping is "no context"
        return None
    if not isinstance(value, str):
        return None
    return parse_traceparent(value)


def resolve_trace_context(
    *,
    headers: Optional[Mapping] = None,
    envelope: Optional[Mapping] = None,
    environ: Optional[Mapping] = None,
) -> Optional[TraceContext]:
    """The documented-precedence context resolution of this module
    (honesty header item 5): explicit `headers` (the JetStream read) →
    the JSON `envelope` → the `TRACEPARENT` environment variable → None
    (no inherited context). First PARSEABLE carrier wins; an unparseable
    carrier falls through to the next one rather than poisoning the
    resolution (absence at every level is the honest no-context
    outcome)."""
    context = trace_context_from_headers(headers)
    if context is not None:
        return context
    context = trace_context_from_envelope(envelope)
    if context is not None:
        return context
    return trace_context_from_environment(environ)


# ---------------------------------------------------------------------------
# REDACTION AT EMIT (P11.08) — the closed rule sets, mirrored from
# `python/workers/worker_template/redaction.py` (the file the P11.08
# record 08_p11_08_redaction.md names) and extended with the workers'
# proposal-only vocabulary (FORBIDDEN_RESULT_KEYS, first declared in
# apps/worker/python/gnn_worker/gnn_inference_worker.py and re-declared
# per worker). BOTH SETS ARE CLOSED MIRRORS: no entry added or removed
# here; amending them is a recorded amendment to the P11.08 record (and
# the F-09 ledger), never a silent change.
#
# REDACTION NOTE (the P11.07 seam): the schema-v1 MANDATORY attributes
# (tenant_id, command_id, trace_id) keep their recorded semantics (empty
# = not applicable) and are emitted as named schema fields, NOT as
# extras — the denylist below applies to EXTRA attributes only. This is
# what keeps the mandatory `command_id` correlation field alive (P11.07
# §1) while the proposal-only vocabulary (`command`, `command_id`,
# `dispatch`, `approval`, ...) still cannot travel as arbitrary extra
# log attributes.
#
# The boundary is EMIT, never READ: these filters run inside the emission
# path before a line leaves the process. A line already emitted is a
# historical record; no function here claims retroactive safety (the
# P11.08 record states this explicitly).
# ---------------------------------------------------------------------------

REDACTION_MARKER = "[REDACTED]"

# Closed key denylist — P11.08 KEY_DENYLIST verbatim.
REDACTION_KEY_DENYLIST: frozenset = frozenset(
    {
        "authorization",
        "www authenticate",
        "proxy authorization",
        "password",
        "passphrase",
        "secret",
        "client secret",
        "access token",
        "refresh token",
        "id token",
        "bearer",
        "cookie",
        "set cookie",
        "api key",
        "apikey",
        "private key",
        "database url",
        "connection string",
        "session id",
        "credentials",
    }
)

# The proposal-only result vocabulary (FORBIDDEN_RESULT_KEYS, mirrored
# from the workers) as EXTRA-attribute keys — the same closed set the
# workers' assert_proposal_only refuses in result documents; see the
# REDACTION NOTE above for the mandatory-field seam.
FORBIDDEN_EXTRA_KEYS: frozenset = frozenset(
    {
        "authorization_token",
        "auth_token",
        "command",
        "command_id",
        "command_ids",
        "issue",
        "issued",
        "dispatch",
        "dispatchable",
        "approval",
        "approved",
        "approval_token",
    }
)

# Separator normalization — P11.08 verbatim (any run of "_", "-", or " "
# collapses to a single space before comparison).
_KEY_SEPARATOR = re.compile(r"[_\-\s]+")


def _normalized_key(key: str) -> str:
    return _KEY_SEPARATOR.sub(" ", key.strip().lower())


def is_denied_extra_key(key: str) -> bool:
    """True when an EXTRA-attribute key is on one of the closed denylists:
    the P11.08 credential vocabulary (compared with the separator
    normalization the P11.08 record defines) OR the workers' proposal-only
    vocabulary (compared as the exact lowercase spelling the workers'
    assert_proposal_only uses — `key.strip().lower()`). Each set keeps the
    comparison regime its source declares; both are closed mirrors."""
    if not isinstance(key, str):
        return False
    if _normalized_key(key) in REDACTION_KEY_DENYLIST:
        return True
    return key.strip().lower() in FORBIDDEN_EXTRA_KEYS


# Closed value patterns — P11.08 VALUE_PATTERNS verbatim (bearer tokens,
# JWT shape, AWS access-key-id shape, URL credentials, key=value leaks,
# long hex blobs ≥ 40 chars — deliberately above the 32-hex
# correlation-handle length so trace ids and UUIDs survive).
VALUE_PATTERNS: tuple = tuple(
    re.compile(pattern)
    for pattern in (
        r"(?i)\bbearer\s+[A-Za-z0-9\-\._~\+/=]{8,}",
        r"\beyJ[A-Za-z0-9\-_]{10,}\.[A-Za-z0-9\-_]{10,}\.[A-Za-z0-9\-_]{10,}\b",
        r"(?i)\bAKIA[0-9A-Z]{16}\b",
        r"(?i)\b(?:postgres|postgresql|mysql|redis)://[^\s:@/]+:[^\s@/]+@",
        r"(?i)(?:password|passwd|pwd|secret|token|api[_\-]?key)\s*[=:]\s*\S+",
        r"\b[0-9a-fA-F]{40,}\b",
    )
)

# Email shape (the PII class the P11.08 record records) — verbatim.
_EMAIL_PATTERN = re.compile(
    r"\b[A-Za-z0-9\.\_\%\+\-]+@[A-Za-z0-9\.\-]+\.[A-Za-z]{2,}\b"
)


def redact_string(text: str) -> str:
    """The P11.08 value rule for one string (pattern-matching substrings
    become the fixed marker; no partial masking, no reversibility)."""
    if not isinstance(text, str):
        return text  # pragma: no cover - defensive, typed callers only
    redacted = text
    for pattern in VALUE_PATTERNS:
        redacted = pattern.sub(REDACTION_MARKER, redacted)
    return _EMAIL_PATTERN.sub(REDACTION_MARKER, redacted)


def redact_value(value: Any) -> Any:
    """The P11.08 emit-time value rule: strings through the pattern set;
    dicts and lists walked recursively; non-string leaves never rewritten
    (a number or bool cannot carry a secret shape this module claims to
    catch)."""
    if isinstance(value, str):
        return redact_string(value)
    if isinstance(value, dict):
        return redact_pairs(value)
    if isinstance(value, (list, tuple)):
        return [redact_value(item) for item in value]
    return value


def redact_pairs(attributes: dict) -> dict:
    """The P11.08 emit-time attribute rule for EXTRA attributes:
    key-denylist redaction FIRST (the whole value becomes the marker,
    whatever it is — covering the credential vocabulary AND the
    proposal-only vocabulary), then the value rule on the survivors.
    Original key spellings are preserved in the output."""
    redacted: dict = {}
    for key, value in attributes.items():
        if isinstance(key, str) and is_denied_extra_key(key):
            redacted[key] = REDACTION_MARKER
            continue
        redacted[key] = redact_value(value)
    return redacted


# ---------------------------------------------------------------------------
# Schema v1 (P11.07) — SeverityNumber band bases [S-62] for stdlib logging
# level names (mirrored from python/workers/worker_template/log_schema.py;
# unknown level → 0 UNSPECIFIED, never a guessed band).
# ---------------------------------------------------------------------------

LOG_SCHEMA_VERSION = "flexcore.log.v1"

SEVERITY_NUMBERS: dict = {
    "trace": 1,
    "debug": 5,
    "info": 9,
    "notice": 9,
    "warning": 13,
    "warn": 13,
    "error": 17,
    "exception": 17,
    "critical": 21,
    "fatal": 21,
}

# The attribute keys every schema-v1 line carries (present even when the
# value is the empty string — the P11.07 absent-vs-empty semantics).
MANDATORY_ATTRIBUTES: tuple = ("tenant_id", "command_id", "trace_id")


def severity_number_for_level(level: str) -> int:
    """The SeverityNumber band base for a stdlib-logging level name
    (case-insensitive, `warn` alias included); 0 (UNSPECIFIED) for
    anything the closed table does not name."""
    if not isinstance(level, str):
        return 0
    return SEVERITY_NUMBERS.get(level.strip().lower(), 0)


def _rfc3339_utc_now() -> str:
    """The emit timestamp (RFC 3339 UTC, the OTel data model's Timestamp
    field) — operational metadata, never a result payload (the P11.07
    determinism note)."""
    return datetime.now(timezone.utc).isoformat()


def build_log_line(
    level: str,
    message: str,
    *,
    tenant_id: str = "",
    command_id: str = "",
    context: Optional[TraceContext] = None,
    sdk_status: str = SDK_STATUS_UNAVAILABLE,
    **attributes: Any,
) -> str:
    """Build ONE schema-v1 line (P11.07 shape), with emit-time redaction
    (P11.08) applied BEFORE serialization, and the trace context of the
    current resolution stamped into the mandatory `trace_id` attribute
    (empty string when there is no inherited context — never a
    synthesized id) plus the `span_id`/`trace_flags`/`otel_sdk_status`
    extras. Returns the serialized line (sorted-key JSON, one line, no
    trailing newline) — the caller or the logging formatter writes it.

    The mandatory `command_id` field keeps its P11.07 semantics (empty =
    not applicable); if a caller passes command-scoped identity as an
    EXTRA it is denied by the closed proposal-only vocabulary (the
    REDACTION NOTE seam)."""
    trace_id = context.trace_id if context is not None else ""
    span_id = context.span_id if context is not None else ""
    trace_flags = context.trace_flags if context is not None else ""
    line: dict = {
        "schema_version": LOG_SCHEMA_VERSION,
        "severity_number": severity_number_for_level(level),
        "severity_text": str(level).strip().upper(),
        "timestamp": _rfc3339_utc_now(),
        "message": redact_string(str(message)),
        "tenant_id": tenant_id,
        "command_id": command_id,
        "trace_id": redact_string(trace_id),
        "span_id": span_id,
        "trace_flags": trace_flags,
        "otel_sdk_status": sdk_status,
    }
    for key, value in attributes.items():
        if isinstance(key, str) and is_denied_extra_key(key):
            line[key] = REDACTION_MARKER
            continue
        line[key] = redact_value(value)
    return json.dumps(line, sort_keys=True)


# The standard LogRecord attribute names — the attrs a RedactionFilter
# leaves alone (everything else on a record is caller-supplied and goes
# through the emit-time rule).
_LOGRECORD_STANDARD_ATTRS: frozenset = frozenset(
    {
        "name",
        "msg",
        "args",
        "levelname",
        "levelno",
        "pathname",
        "filename",
        "module",
        "exc_info",
        "exc_text",
        "stack_info",
        "lineno",
        "funcName",
        "created",
        "msecs",
        "relativeCreated",
        "thread",
        "threadName",
        "processName",
        "process",
        "taskName",
        "message",
        "asctime",
    }
)


class TraceContextFilter(logging.Filter):
    """Stamps the worker's resolved trace context onto every LogRecord:
    `trace_id` / `span_id` / `trace_flags` (empty strings when there is
    no inherited context — the record structure is stable either way) and
    `otel_sdk_status` (the no-op/SDK posture string). The context is the
    one resolved at `instrument_worker` time; a per-record re-resolution
    does not exist in the no-op posture (recorded limitation: without the
    SDK there is no context storage — a long-lived worker carries the
    context it started with)."""

    def __init__(
        self,
        context: Optional[TraceContext],
        sdk_status: str = SDK_STATUS_UNAVAILABLE,
        name: str = "",
    ) -> None:
        super().__init__(name=name)
        self._context = context
        self._sdk_status = sdk_status

    def filter(self, record: logging.LogRecord) -> bool:
        context = self._context
        record.trace_id = context.trace_id if context is not None else ""
        record.span_id = context.span_id if context is not None else ""
        record.trace_flags = context.trace_flags if context is not None else ""
        record.otel_sdk_status = self._sdk_status
        return True


class RedactionFilter(logging.Filter):
    """The P11.08 emit-time filter at the logging seam: the record's
    rendered message and every CALLER-SUPPLIED (non-standard) record
    attribute pass the closed rule set before the record reaches any
    handler/formatter. Key-denylist extras become the fixed marker
    wholesale; string values go through the pattern set; dicts/lists are
    walked; non-string leaves are untouched. No redaction-at-read promise
    (P11.08 §1): this runs at EMIT, inside the process."""

    def filter(self, record: logging.LogRecord) -> bool:
        try:
            rendered = record.getMessage()
            record.msg = redact_string(rendered)
            record.args = ()
        except Exception:  # noqa: BLE001 — a broken format must never crash emit
            record.msg = redact_string(str(record.msg))
            record.args = ()
        for attr in list(vars(record).keys()):
            if attr in _LOGRECORD_STANDARD_ATTRS or attr.startswith("_"):
                continue
            value = getattr(record, attr, None)
            if isinstance(value, str) and is_denied_extra_key(attr):
                setattr(record, attr, REDACTION_MARKER)
            elif isinstance(value, (str, dict, list, tuple)):
                setattr(record, attr, redact_value(value))
        return True


class SchemaV1Formatter(logging.Formatter):
    """Formats each record as ONE schema-v1 JSON line (P11.07): the
    mandatory attribute set at the recorded empty-string semantics, the
    trace context the TraceContextFilter stamped, and the caller-supplied
    non-standard record attributes as redacted extras. Errors render as
    the message plus the exception type name — a traceback is an extra
    the closed rule set does not study, so it is NOT serialized here
    (recorded limitation, not a completeness claim)."""

    def format(self, record: logging.LogRecord) -> str:
        attributes: dict = {}
        for attr in sorted(vars(record).keys()):
            if attr in _LOGRECORD_STANDARD_ATTRS or attr.startswith("_"):
                continue
            if attr in ("trace_id", "span_id", "trace_flags", "otel_sdk_status"):
                continue  # carried in the named fields below
            attributes[attr] = getattr(record, attr)
        if record.exc_info:
            attributes["exc_type"] = type(record.exc_info[1]).__name__
        level = record.levelname.lower()
        return build_log_line(
            level,
            record.getMessage(),
            tenant_id=str(getattr(record, "tenant_id", "") or ""),
            command_id=str(getattr(record, "command_id", "") or ""),
            context=_context_of(record),
            sdk_status=str(getattr(record, "otel_sdk_status", "") or SDK_STATUS_UNAVAILABLE),
            **attributes,
        )


def _context_of(record: logging.LogRecord) -> Optional[TraceContext]:
    """Rebuilds the TraceContext the TraceContextFilter stamped (the
    formatter cannot see the filter's private field)."""
    trace_id = str(getattr(record, "trace_id", "") or "")
    span_id = str(getattr(record, "span_id", "") or "")
    if not trace_id or not span_id:
        return None
    flags = str(getattr(record, "trace_flags", "") or DEFAULT_TRACE_FLAGS)
    return TraceContext(trace_id=trace_id, span_id=span_id, trace_flags=flags)


# ---------------------------------------------------------------------------
# The guarded SDK touchpoint (the torch.load-guard idiom): the ONLY
# opentelemetry import in this module lives INSIDE this function. With the
# SDK absent (the DEFAULT environment) the import raises ImportError and
# the recorded fallback reason is returned — the no-op posture, log-line
# structure unchanged. With an endpoint configured AND the SDK installed
# the provider build follows the P11.05 reference shape; every SDK call is
# UNVERIFIED — otel API until the human gate runs it.
# ---------------------------------------------------------------------------

OTEL_DEPLOYMENT_REQUIREMENT = (
    "flexcore worker OTLP export requires the opentelemetry SDK "
    "(opentelemetry-sdk plus an OTLP exporter), which is OPTIONAL in "
    "this tree and NOT installed (never added to requirements in the "
    "FIM program); the worker runs in the no-op posture without it"
)


def try_init_otlp_export(
    *,
    service_name: str,
    endpoint: Optional[str] = None,
    environ: Optional[Mapping] = None,
) -> tuple:
    """The guarded OTLP export init. Returns `(provider, status, reason)`:

    - endpoint unset/empty (the DEFAULT) → `(None, "endpoint-unset-noop",
      "")` — export disabled, the honest no-op, no SDK import attempted;
    - SDK import fails → `(None, "sdk-absent-noop", <requirement text>)`
      — the recorded fallback, log-line structure unchanged;
    - the SDK is importable but the provider build fails →
      `(None, "sdk-init-failed-noop", <reason>)` — fail-open to no-op,
      never a crash of the worker;
    - the provider builds → `(provider, "sdk-active-unproven", "")` —
      UNPROVEN until a human runs it (honesty header item 2); the caller
      passes the provider to `shutdown_otlp_export` at worker stop.

    UNVERIFIED — otel API (the P11.05 reference's marker): TracerProvider
    with Resource service name, BatchSpanProcessor over
    OTLPSpanExporter(endpoint), ParentBased(TraceIdRatioBased) sampling
    (the P11.06 policy), global propagator TraceContextTextMapPropagator.
    The endpoint env key mirrors the Rust worker's
    `FLEXCORE_OTEL_EXPORTER_OTLP_ENDPOINT` (apps/worker/src/otel.rs)."""
    source = os.environ if environ is None else environ
    if endpoint is None:
        try:
            endpoint = source.get("FLEXCORE_OTEL_EXPORTER_OTLP_ENDPOINT")
        except Exception:  # noqa: BLE001 — a broken environ is "unset"
            endpoint = None
    if not endpoint or not str(endpoint).strip():
        return (None, SDK_STATUS_DISABLED, "")
    try:
        import opentelemetry  # noqa: PLC0415 — deliberate guarded import
    except ImportError as error:
        return (None, SDK_STATUS_UNAVAILABLE, "%s (%s)" % (OTEL_DEPLOYMENT_REQUIREMENT, error))
    try:
        # UNVERIFIED — otel API: the deferred SDK imports (P11.05 shape).
        from opentelemetry import propagate, trace  # noqa: PLC0415
        from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import (  # noqa: PLC0415
            OTLPSpanExporter,
        )
        from opentelemetry.sdk.resources import Resource  # noqa: PLC0415
        from opentelemetry.sdk.trace import TracerProvider  # noqa: PLC0415
        from opentelemetry.sdk.trace.export import BatchSpanProcessor  # noqa: PLC0415
        from opentelemetry.sdk.trace.sampling import (  # noqa: PLC0415
            ParentBased,
            TraceIdRatioBased,
        )

        ratio_raw = source.get("FLEXCORE_OTEL_TRACE_SAMPLE_RATIO") if environ is None else source.get(
            "FLEXCORE_OTEL_TRACE_SAMPLE_RATIO"
        )
        try:
            ratio = float(ratio_raw) if ratio_raw else 1.0
        except (TypeError, ValueError):
            ratio = 1.0
        provider = TracerProvider(
            resource=Resource.create({"service.name": service_name}),
            sampler=ParentBased(TraceIdRatioBased(max(0.0, min(1.0, ratio)))),
        )
        provider.add_span_processor(BatchSpanProcessor(OTLPSpanExporter(endpoint=str(endpoint).strip())))
        trace.set_tracer_provider(provider)
        propagate.set_global(propagate.TraceContextTextMapPropagator())
        return (provider, SDK_STATUS_ACTIVE, "")
    except Exception as error:  # noqa: BLE001 — fail-open to the no-op posture
        return (None, SDK_STATUS_INIT_FAILED, "%s: %s" % (type(error).__name__, error))


def shutdown_otlp_export(provider: object) -> None:
    """Best-effort shutdown of a provider from `try_init_otlp_export`
    (UNVERIFIED — otel API: TracerProvider.shutdown()); a None provider
    (any no-op posture) is a no-op."""
    if provider is None:
        return
    try:
        provider.shutdown()  # type: ignore[attr-defined]  # UNVERIFIED — otel API.
    except Exception:  # noqa: BLE001 — shutdown must never crash the worker
        pass


# ---------------------------------------------------------------------------
# instrument_worker — the additive entrypoint hook (FIM-F09.02). One call
# per worker process start: resolves the trace context, attaches the
# TraceContextFilter + RedactionFilter + a stderr SchemaV1 handler to the
# worker's logger (idempotently — a second call never duplicates
# handlers), and emits the worker-start line carrying the trace context
# (F-09 acceptance criterion 1's worker-log leg).
# ---------------------------------------------------------------------------


class WorkerInstrumentation:
    """The instrumentation state `instrument_worker` returns: the worker's
    logger (filters attached), the resolved context (None = no inherited
    context), and the recorded SDK posture. `event` emits one schema-v1
    line through the filtered logger; `start_line` is the serialized
    worker-start line (kept for callers and self-tests to pin)."""

    def __init__(
        self,
        worker_name: str,
        logger: logging.Logger,
        context: Optional[TraceContext],
        sdk_status: str,
        sdk_fallback_reason: str,
        provider: object,
        start_line: str,
    ) -> None:
        self.worker_name = worker_name
        self.logger = logger
        self.context = context
        self.sdk_status = sdk_status
        self.sdk_fallback_reason = sdk_fallback_reason
        self.provider = provider
        self.start_line = start_line

    def event(self, level: str, message: str, **attributes: Any) -> None:
        """One schema-v1 event line through the filtered logger (redaction
        runs twice by design: once here at the call boundary, once at the
        RedactionFilter — the P11.08 boundary is emit, and layered
        re-redaction is idempotent on already-redacted content)."""
        extra = {
            "event_worker": self.worker_name,
            "sdk_status": self.sdk_status,
        }
        for key, value in attributes.items():
            if isinstance(key, str) and is_denied_extra_key(key):
                extra["redacted_" + _normalized_key(key).replace(" ", "_")] = REDACTION_MARKER
            else:
                extra[key] = redact_value(value)
        self.logger.log(
            _level_for(level),
            redact_string(str(message)),
            extra=extra,
        )


def _level_for(level: str) -> int:
    """The stdlib logging numeric level for a schema-v1 level name
    (unknown names log at INFO — the schema's SeverityNumber still
    reports 0 UNSPECIFIED via severity_number_for_level)."""
    numeric = {
        "trace": logging.DEBUG,
        "debug": logging.DEBUG,
        "info": logging.INFO,
        "notice": logging.INFO,
        "warning": logging.WARNING,
        "warn": logging.WARNING,
        "error": logging.ERROR,
        "exception": logging.ERROR,
        "critical": logging.CRITICAL,
        "fatal": logging.CRITICAL,
    }
    if not isinstance(level, str):
        return logging.INFO
    return numeric.get(level.strip().lower(), logging.INFO)


def instrument_worker(
    worker_name: str,
    *,
    headers: Optional[Mapping] = None,
    envelope: Optional[Mapping] = None,
    stream: Optional[TextIO] = None,
    init_otlp: bool = False,
) -> WorkerInstrumentation:
    """The additive entrypoint hook (FIM-F09.02): resolve the trace
    context (precedence per `resolve_trace_context`), attach the
    filters + the stderr schema-v1 handler to the worker's logger
    (idempotent), and emit the worker-start line with the trace context.

    NO behavior change beyond that one stderr line: worker logic, result
    vocabulary, and FORBIDDEN_RESULT_KEYS are untouched by every caller.

    `init_otlp` (DEFAULT False — the no-op posture is the default): when
    True, attempts the guarded OTLP init (endpoint from
    FLEXCORE_OTEL_EXPORTER_OTLP_ENDPOINT); every failure degrades to the
    recorded no-op status with the fallback reason kept on the returned
    object. The SDK-present path is UNPROVEN until the human gate."""
    resolved_stream = sys.stderr if stream is None else stream
    context = resolve_trace_context(headers=headers, envelope=envelope)
    provider, sdk_status, fallback_reason = try_init_otlp_export(
        service_name=worker_name,
        environ=None if envelope is None else _envelope_environ(envelope),
    ) if init_otlp else (None, SDK_STATUS_UNAVAILABLE, OTEL_DEPLOYMENT_REQUIREMENT)

    logger = logging.getLogger(worker_name)
    logger.setLevel(logging.INFO)
    logger.propagate = False
    has_context_filter = any(isinstance(f, TraceContextFilter) for f in logger.filters)
    has_redaction_filter = any(isinstance(f, RedactionFilter) for f in logger.filters)
    if not has_context_filter:
        logger.addFilter(TraceContextFilter(context, sdk_status, name=worker_name + ".trace"))
    if not has_redaction_filter:
        logger.addFilter(RedactionFilter(name=worker_name + ".redact"))
    handler_is_missing = not any(
        isinstance(h, logging.StreamHandler) and getattr(h, "_flexcore_schema_v1", False)
        for h in logger.handlers
    )
    if handler_is_missing:
        handler = logging.StreamHandler(resolved_stream)
        handler.setFormatter(SchemaV1Formatter())
        handler._flexcore_schema_v1 = True  # type: ignore[attr-defined] — the idempotency marker
        logger.addHandler(handler)

    instrumentation = WorkerInstrumentation(
        worker_name=worker_name,
        logger=logger,
        context=context,
        sdk_status=sdk_status,
        sdk_fallback_reason=fallback_reason,
        provider=provider,
        start_line="",
    )
    start_attributes = {
        "event": "worker_start",
        "worker": worker_name,
    }
    if context is None:
        start_attributes["trace_context"] = "absent"
    else:
        start_attributes["trace_context"] = "inherited_traceparent"
    start_line = build_log_line(
        "info",
        "worker start",
        context=context,
        sdk_status=sdk_status,
        **start_attributes,
    )
    instrumentation.start_line = start_line
    logger.log(logging.INFO, "worker start", extra=start_attributes)
    return instrumentation


def _envelope_environ(envelope: Mapping) -> Mapping:
    """The mapping the guarded OTLP init reads env keys from when a
    caller passed an envelope: the process environment (the envelope is
    not an env source — recorded; the parameter exists to keep the
    signature honest about which mapping is authoritative)."""
    return os.environ


# The status string when the one-call hook itself failed (recorded on the
# last note_worker_start call; the worker keeps running untraced).
SDK_STATUS_INSTRUMENTATION_FAILED = "instrumentation-failed-noop"

# The recorded failure of the most recent note_worker_start call (empty
# when it succeeded or never ran). Exists so a self-test can pin the
# degradation without any output on the worker's stderr beyond the
# ordinary contract.
LAST_NOTE_FAILURE = ""


def note_worker_start(
    worker_name: str,
    *,
    headers: Optional[Mapping] = None,
    envelope: Optional[Mapping] = None,
    stream: Optional[TextIO] = None,
) -> Optional[WorkerInstrumentation]:
    """The ONE-CALL additive hook worker entrypoints use (FIM-F09.02):
    everything `instrument_worker` does, PLUS the guarantee that this
    function NEVER raises — any failure inside the instrumentation degrades
    to the recorded no-op posture (`LAST_NOTE_FAILURE` carries the reason;
    the worker runs EXACTLY as before, untraced). The worker's own
    contract (one JSON result envelope on stdout, fail-closed refusals)
    always wins over telemetry — recorded, deliberate silence on failure.

    Returns the WorkerInstrumentation, or None when instrumentation
    failed (the recorded fallback; never a crash, never a fabricated
    trace id)."""
    global LAST_NOTE_FAILURE
    try:
        LAST_NOTE_FAILURE = ""
        return instrument_worker(
            worker_name, headers=headers, envelope=envelope, stream=stream
        )
    except Exception as error:  # noqa: BLE001 — telemetry never breaks a worker
        LAST_NOTE_FAILURE = "%s: %s" % (type(error).__name__, error)
        return None


# ---------------------------------------------------------------------------
# In-module self-tests for the PURE-LOGIC core (hand-computed; NEVER RUN
# in this program — execution is the human gate). Pinned vectors: the W3C
# spec's example traceparent, malformed shapes, all-zero refusals, the
# forbidden version byte, reserved-flags propagation, header-name
# case-insensitivity, redaction of a seeded secret-bearing record, and
# the no-op posture's stable line structure.
# ---------------------------------------------------------------------------

_W3C_EXAMPLE_TRACEPARENT = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"


def run_self_tests() -> list:
    """Runs every hand-computed self-test; returns one record per test
    (`name`, `passed`, `detail`) — the repo's --self-test convention."""
    results: list = []

    def check(name: str, condition: bool, detail: str) -> None:
        results.append({"name": name, "passed": bool(condition), "detail": detail})

    # -- parse: the W3C spec's example is valid, round-trips exactly. ----
    context = parse_traceparent(_W3C_EXAMPLE_TRACEPARENT)
    check(
        "parse_w3c_example_valid",
        context is not None
        and context.trace_id == "4bf92f3577b34da6a3ce929d0e0e4736"
        and context.span_id == "00f067aa0ba902b7"
        and context.trace_flags == "01",
        "the W3C spec example parses to its pinned ids",
    )
    check(
        "serialize_round_trip",
        context is not None and context.traceparent() == _W3C_EXAMPLE_TRACEPARENT,
        "parse->serialize round-trips the W3C example byte-exactly",
    )
    reparsed = parse_traceparent(context.traceparent()) if context is not None else None
    check(
        "round_trip_reparse_equal",
        reparsed == context,
        "serialize->parse yields an equal context",
    )

    # -- malformed shapes are refused (None = no inherited context). -----
    malformed = [
        "",
        "not-a-traceparent",
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7",  # missing flags
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-extra",  # trailing
        "0-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",  # short version
        "00-4bf92f3577b34da6a3ce929d0e0e473-00f067aa0ba902b7-01",  # 31-hex trace id
        "00-4bf92f3577b34da6a3ce929d0e0e473g-00f067aa0ba902b7-01",  # non-hex byte
        "00-4bf92f3577b34da6a3ce929d0e0e4736_00f067aa0ba902b7-01",  # wrong separator
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-0z",  # non-hex flags
    ]
    check(
        "parse_malformed_refused",
        all(parse_traceparent(value) is None for value in malformed),
        "all %d malformed shapes parse to None" % len(malformed),
    )

    # -- all-zero ids refused (W3C: invalid values). ---------------------
    check(
        "parse_all_zero_trace_id_refused",
        parse_traceparent(
            "00-00000000000000000000000000000000-00f067aa0ba902b7-01"
        )
        is None,
        "the all-zero trace-id is refused",
    )
    check(
        "parse_all_zero_span_id_refused",
        parse_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01"
        )
        is None,
        "the all-zero span-id is refused",
    )

    # -- version bytes: ff forbidden (W3C), non-00 refused (strict). -----
    check(
        "parse_version_ff_refused",
        parse_traceparent(
            "ff-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
        )
        is None,
        "version ff is forbidden",
    )
    check(
        "parse_version_01_refused",
        parse_traceparent(
            "01-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
        )
        is None,
        "non-00 versions mean no inherited context (strict posture)",
    )

    # -- trace-flags: no value is invalid; reserved bits propagate. ------
    flags_ctx = parse_traceparent(
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-03"
    )
    check(
        "parse_reserved_flags_propagate",
        flags_ctx is not None and flags_ctx.trace_flags == "03",
        "flags other than 00/01 are valid and propagate verbatim",
    )
    flags_ff = parse_traceparent(
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-ff"
    )
    check(
        "parse_flags_ff_valid_no_synthesis",
        flags_ff is not None and flags_ff.trace_flags == "ff",
        "ff flags parse (all 8 set is legal per the W3C grammar)",
    )
    check(
        "serialize_never_fabricates",
        serialize_traceparent(TraceContext(trace_id=_TRACE_ID_ZERO, span_id="00f067aa0ba902b7")) == "",
        "serializing an all-zero-trace-id context yields \"\" (never a malformed header)",
    )

    # -- non-lowercase hex refused (W3C HEXDIGLC; the strict posture). ---
    check(
        "parse_uppercase_refused",
        parse_traceparent(
            "00-4BF92F3577B34DA6A3CE929D0E0E4736-00F067AA0BA902B7-01"
        )
        is None,
        "uppercase hex is refused (lowercase-only grammar; recorded strict posture)",
    )

    # -- header extraction: the exact Rust-side header name, any case. ---
    check(
        "extract_headers_exact_name",
        trace_context_from_headers({"traceparent": _W3C_EXAMPLE_TRACEPARENT}) == context,
        "the JetStream header dict under the exact Rust-side name parses",
    )
    check(
        "extract_headers_case_insensitive",
        trace_context_from_headers({"TraceParent": _W3C_EXAMPLE_TRACEPARENT}) == context
        and trace_context_from_headers({"TRACEPARENT": _W3C_EXAMPLE_TRACEPARENT}) == context,
        "header-NAME lookup is case-insensitive (NATS/W3C convention)",
    )
    check(
        "extract_headers_absent_none",
        trace_context_from_headers({"Nats-Msg-Id": "abc"}) is None,
        "an absent header means no inherited context",
    )
    check(
        "extract_headers_unparseable_none",
        trace_context_from_headers({"traceparent": "garbage"}) is None,
        "an unparseable header means no inherited context",
    )

    # -- envelope extraction (additive optional keys). -------------------
    check(
        "extract_envelope_direct_key",
        trace_context_from_envelope({"protocol": "p", "traceparent": _W3C_EXAMPLE_TRACEPARENT})
        == context,
        "an optional envelope traceparent field parses",
    )
    check(
        "extract_envelope_nested_headers",
        trace_context_from_envelope({"headers": {"traceparent": _W3C_EXAMPLE_TRACEPARENT}})
        == context,
        "an optional envelope headers object parses",
    )
    check(
        "extract_envelope_absent_none",
        trace_context_from_envelope({"protocol": "p"}) is None,
        "an envelope without the optional keys means no inherited context",
    )

    # -- environment carrier (recorded choice). --------------------------
    check(
        "extract_env_carrier",
        trace_context_from_environment({"TRACEPARENT": _W3C_EXAMPLE_TRACEPARENT}) == context,
        "the TRACEPARENT env carrier parses",
    )
    check(
        "resolve_precedence_headers_first",
        resolve_trace_context(
            headers={"traceparent": _W3C_EXAMPLE_TRACEPARENT},
            envelope={"traceparent": "garbage"},
            environ={"TRACEPARENT": "garbage"},
        )
        == context,
        "headers win over envelope and env",
    )
    check(
        "resolve_all_absent_none",
        resolve_trace_context(headers=None, envelope=None, environ={}) is None,
        "no carrier anywhere means None (never synthesized)",
    )

    # -- redaction of a seeded secret-bearing record (P11.08 posture). ---
    seeded = redact_pairs(
        {
            "password": "hunter2-super-secret",
            "api_key": "AKIAIOSFODNN7EXAMPLE",
            "note": "bearer abcdefgh12345678 and ops@example.com",
            "auth_token": "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2ln",
            "tenant_ref": "tenant-7",
        }
    )
    check(
        "redact_denied_keys_wholesale",
        seeded["password"] == REDACTION_MARKER
        and seeded["api_key"] == REDACTION_MARKER
        and seeded["auth_token"] == REDACTION_MARKER,
        "denylist extras (P11.08 + proposal-only) become the marker wholesale",
    )
    check(
        "redact_value_patterns",
        "abcdefgh12345678" not in seeded["note"]
        and "ops@example.com" not in seeded["note"]
        and REDACTION_MARKER in seeded["note"],
        "bearer-token and email shapes inside a string value are masked",
    )
    check(
        "redact_preserves_benign",
        seeded["tenant_ref"] == "tenant-7",
        "ordinary operational content survives (no over-redaction)",
    )
    line = build_log_line(
        "info",
        "processing request with password=hunter2-super-secret",
        context=context,
        tenant_ref="tenant-7",
        secret="value-never-emitted",
    )
    check(
        "redact_at_emit_in_line",
        "hunter2-super-secret" not in line
        and "value-never-emitted" not in line
        and REDACTION_MARKER in line,
        "the message and denied extras are redacted BEFORE serialization",
    )

    # -- schema-v1 line structure (stable across the SDK posture). -------
    parsed_line = json.loads(line)
    expected_keys = {
        "schema_version",
        "severity_number",
        "severity_text",
        "timestamp",
        "message",
        "tenant_id",
        "command_id",
        "trace_id",
        "span_id",
        "trace_flags",
        "otel_sdk_status",
        "tenant_ref",
        "secret",
    }
    check(
        "log_line_schema_v1_keys",
        expected_keys.issubset(parsed_line.keys())
        and parsed_line["schema_version"] == LOG_SCHEMA_VERSION
        and parsed_line["severity_number"] == 9
        and parsed_line["severity_text"] == "INFO"
        and parsed_line["trace_id"] == context.trace_id,
        "the no-op line carries the schema-v1 field set and the inherited trace id",
    )
    no_context_line = json.loads(
        build_log_line("info", "worker start", context=None, event="worker_start")
    )
    check(
        "log_line_no_context_empty_not_fabricated",
        no_context_line["trace_id"] == "" and no_context_line["span_id"] == "",
        "absent context emits EMPTY ids, never synthesized ones",
    )
    check(
        "severity_unknown_is_zero",
        severity_number_for_level("loud") == 0,
        "an unknown level maps to 0 UNSPECIFIED, never a guessed band",
    )

    # -- the guarded SDK posture (the no-op fallback is the default). ----
    provider, status, reason = try_init_otlp_export(
        service_name="self-test", endpoint="", environ={}
    )
    check(
        "noop_default_endpoint_unset",
        provider is None and status == SDK_STATUS_DISABLED and reason == "",
        "endpoint unset means the honest no-op, no SDK import attempted",
    )
    provider_absent, status_absent, reason_absent = try_init_otlp_export(
        service_name="self-test",
        endpoint="http://127.0.0.1:4317",
        environ={},
    )
    check(
        "noop_when_sdk_absent",
        provider_absent is None
        and status_absent == SDK_STATUS_UNAVAILABLE
        and "opentelemetry" in reason_absent,
        "endpoint set but SDK absent means the recorded no-op fallback reason",
    )
    check(
        "shutdown_none_is_noop",
        shutdown_otlp_export(None) is None,
        "shutting down a None provider is a no-op",
    )

    return results


if __name__ == "__main__":  # pragma: no cover — human-gate entrypoint
    if "--self-test" in sys.argv[1:]:
        _records = run_self_tests()
        _failures = [r for r in _records if not r["passed"]]
        for _r in _records:
            print(("PASS" if _r["passed"] else "FAIL"), _r["name"], "-", _r["detail"])
        print("%d/%d self-tests passed" % (len(_records) - len(_failures), len(_records)))
        raise SystemExit(1 if _failures else 0)
    raise SystemExit(
        "otel_instrumentation is a library module; run it with --self-test"
    )
