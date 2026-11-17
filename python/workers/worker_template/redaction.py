# P11.08 — PII/secret REDACTION AT EMIT for Python worker log paths.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. STDLIB-ONLY and genuinely importable/runnable today — every function
#    is pure string/dict logic over closed rule sets, exercised by the
#    UNRUN test suite (`python/workers/tests/test_redaction.py`).
# 2. THE REDACTION BOUNDARY IS EMIT: this module runs inside the
#    log-emission path (`log_schema.emit_log_line`) BEFORE a line leaves
#    the process. There is NO redaction-at-read promise anywhere: a line
#    already emitted is a historical record; downstream stores do the
#    retention/erasure work (the GDPR data-governance boundary is Phase
#    13 scope — the P11.08 record states this explicitly). No function
#    here may be described as making logs "safe" retroactively.
# 3. Closed rule sets, deny-by-default: a KEY is redacted when it matches
#    the closed denylist (§KEY_DENYLIST); a VALUE is redacted when it
#    matches one of the closed value patterns (§VALUE_PATTERNS). Unknown
#    shapes are NOT guessed at — the patterns are deliberately
#    conservative, and the record lists what is NOT caught (recorded
#    limitation, never a completeness claim).
# 4. Every replacement is the fixed marker "[REDACTED]" — no partial
#    masking, no reversibility, no "looks like" heuristics beyond the
#    listed patterns.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import re
from typing import Any

# The fixed replacement marker (single constant — the tests pin it).
REDACTION_MARKER = "[REDACTED]"

# ---------------------------------------------------------------------------
# Closed key denylist: a log attribute whose KEY matches (exact, after
# case-normalization and separator normalization: "_"/"-"/" " are the same
# separator) is redacted wholesale, whatever its value. The list is CLOSED:
# adding an entry is a recorded amendment (the P11.08 record), never a
# silent change.
# ---------------------------------------------------------------------------
KEY_DENYLIST: frozenset[str] = frozenset(
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

# Separator normalization: any run of "_", "-", or " " inside a key
# collapses to a single space before the denylist comparison (so
# "database_url", "database-url", "DatabaseURL-ish variants, and
# "database url" all match).
_KEY_SEPARATOR = re.compile(r"[_\-\s]+")


def _normalized_key(key: str) -> str:
    return _KEY_SEPARATOR.sub(" ", key.strip().lower())


# ---------------------------------------------------------------------------
# Closed value patterns: a string VALUE matching one of these is redacted
# wherever it appears (message or attribute). Conservative by design.
# ---------------------------------------------------------------------------
VALUE_PATTERNS: tuple[re.Pattern[str], ...] = tuple(
    re.compile(pattern)
    for pattern in (
        r"(?i)\bbearer\s+[A-Za-z0-9\-\._~\+/=]{8,}",  # Bearer tokens.
        r"\beyJ[A-Za-z0-9\-_]{10,}\.[A-Za-z0-9\-_]{10,}\.[A-Za-z0-9\-_]{10,}\b",  # JWT shape.
        r"(?i)\bAKIA[0-9A-Z]{16}\b",  # AWS access key id shape.
        r"(?i)\b(?:postgres|postgresql|mysql|redis)://[^\s:@/]+:[^\s@/]+@",  # URL credentials.
        r"(?i)(?:password|passwd|pwd|secret|token|api[_\-]?key)\s*[=:]\s*\S+",  # key=value leaks.
        r"\b[0-9a-fA-F]{40,}\b",  # Long hex blobs (keys/digests).
    )
)

# Email shape (PII class — recorded in the P11.08 record; a worker log
# line has no reason to carry a raw address).
_EMAIL_PATTERN = re.compile(
    r"\b[A-Za-z0-9\.\_\%\+\-]+@[A-Za-z0-9\.\-]+\.[A-Za-z]{2,}\b"
)


def redact_string(text: str) -> str:
    """Redact pattern-matching substrings from one string (the value rule).

    Applied to messages and string attribute values at emit. Non-string
    values are handled by [`redact_value`].
    """
    if not isinstance(text, str):
        return text  # pragma: no cover - defensive, typed callers only
    redacted = text
    for pattern in VALUE_PATTERNS:
        redacted = pattern.sub(REDACTION_MARKER, redacted)
    return _EMAIL_PATTERN.sub(REDACTION_MARKER, redacted)


def redact_value(value: Any) -> Any:
    """The emit-time value rule: strings go through the pattern set; any
    OTHER type passes through unchanged EXCEPT inside dicts and lists,
    which are walked recursively (the attribute maps the log-schema
    emitter carries may nest). Non-string leaves are never rewritten —
    a number or bool cannot carry a secret shape this module claims to
    catch."""
    if isinstance(value, str):
        return redact_string(value)
    if isinstance(value, dict):
        return redact_pairs(value)
    if isinstance(value, (list, tuple)):
        return [redact_value(item) for item in value]
    return value


def redact_pairs(attributes: dict) -> dict:
    """The emit-time attribute rule: key-denylist redaction FIRST (the
    whole value becomes the marker, whatever it is), then the value rule
    on the survivors. Keys are normalized before comparison (see the
    denylist note); original key spellings are preserved in the output."""
    redacted: dict = {}
    for key, value in attributes.items():
        if isinstance(key, str) and _normalized_key(key) in KEY_DENYLIST:
            redacted[key] = REDACTION_MARKER
            continue
        redacted[key] = redact_value(value)
    return redacted


def is_denied_key(key: str) -> bool:
    """True when a key is on the closed denylist (exposed for the tests
    and for callers that drop rather than redact a field)."""
    return _normalized_key(key) in KEY_DENYLIST
