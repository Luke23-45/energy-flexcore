# P11.08 — The emit-time redaction test suite: complete and WRITTEN,
#         NEVER RUN (the execution is a human gate — program conventions
#         §1).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WRITTEN, NEVER RUN. pytest has never been installed in this loop; no
#    test below has ever executed. Run procedure (the recorded P7.17
#    pattern): scratch venv, `pip install pytest` (version recorded
#    then), then `python -m pytest python/workers/tests/ -v` from the
#    repository root with `python/workers/` on `PYTHONPATH`.
# 2. Coverage (what the suite pins, P11.08 record): the closed key
#    denylist (exact + normalized spellings), the closed value patterns
#    (bearer/JWT/AWS/url-credentials/key-value/long-hex/email), recursive
#    dict/list walking, the fixed marker, deny-by-default on unknown
#    shapes (an ordinary sentence, an identifier, a UUID, and a
#    tenancy-relevant id are NOT redacted), and the emit boundary
#    (redaction happens inside `emit_log_line` before serialization —
#    asserted against a captured stream; NO redaction-at-read promise is
#    tested or claimed anywhere).
# 3. The adversarial inputs below are SYNTHETIC test strings, not
#    discovered credentials.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import io

import pytest

from worker_template import log_schema, redaction


class TestKeyDenylist:
    def test_exact_denylist_keys_are_redacted_wholesale(self) -> None:
        for key in ("authorization", "password", "api_key", "database_url"):
            redacted = redaction.redact_pairs({key: "whatever-the-value-is"})
            assert redacted[key] == redaction.REDACTION_MARKER

    def test_normalized_spellings_match(self) -> None:
        # Separator and case normalization: the same concept under
        # different spellings is the same denylist entry.
        for key in ("Authorization", "AUTHORIZATION", "database-url",
                    "DATABASE_URL", "database url", "api-key", "Api_Key"):
            assert redaction.is_denied_key(key), key
            redacted = redaction.redact_pairs({key: "sensitive-value"})
            assert redacted[key] == redaction.REDACTION_MARKER

    def test_benign_keys_pass_through_with_value_rule_only(self) -> None:
        redacted = redaction.redact_pairs({"asset_id": "asset-42", "count": 3})
        assert redacted == {"asset_id": "asset-42", "count": 3}

    def test_original_key_spelling_is_preserved(self) -> None:
        redacted = redaction.redact_pairs({"DatabaseURL": "postgres://u:p@h/db"})
        assert "DatabaseURL" in redacted
        assert redacted["DatabaseURL"] == redaction.REDACTION_MARKER


class TestValuePatterns:
    def test_bearer_tokens_are_redacted(self) -> None:
        assert redaction.redact_string(
            "uplink used Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9 Signature"
        ) != "uplink used Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9 Signature"
        assert redaction.REDACTION_MARKER in redaction.redact_string(
            "Bearer abcd1234efgh5678"
        )

    def test_jwt_shape_is_redacted(self) -> None:
        jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NSJ9.c2lnbmF0dXJlLXBhcnQ"
        redacted = redaction.redact_string(f"token {jwt} rejected")
        assert jwt not in redacted
        assert redaction.REDACTION_MARKER in redacted

    def test_url_credentials_are_redacted(self) -> None:
        redacted = redaction.redact_string(
            "connect failed: postgres://flexcore_user:s3cret@db:5432/flexcore"
        )
        assert "s3cret" not in redacted
        assert redaction.REDACTION_MARKER in redacted

    def test_key_value_leaks_are_redacted(self) -> None:
        for leak in ("password=hunter2", "API_KEY: ab12cd34", "secret=xyzzy"):
            redacted = redaction.redact_string(f"config bad: {leak}")
            assert leak not in redacted
            assert redaction.REDACTION_MARKER in redacted

    def test_long_hex_blobs_are_redacted(self) -> None:
        blob = "a1b2" * 20  # 80 hex chars
        redacted = redaction.redact_string(f"digest {blob} mismatch")
        assert blob not in redacted

    def test_email_addresses_are_redacted(self) -> None:
        redacted = redaction.redact_string("notify operator@example.com on failure")
        assert "operator@example.com" not in redacted
        assert redaction.REDACTION_MARKER in redacted

    def test_ordinary_content_is_not_redacted(self) -> None:
        # Deny-by-default: known-benign operational content must survive
        # (over-redaction would destroy the correlation value of the logs).
        text = "command cmd-1234 for tenant 7f3e asset BATTERY-01 state VALIDATED, delay 120ms"
        assert redaction.redact_string(text) == text

    def test_uuid_and_short_ids_survive(self) -> None:
        # 32/36-char identifiers are correlation handles, not secrets:
        # the long-hex rule starts at 40 chars and the tests pin that
        # boundary deliberately.
        uuid_text = "018f3c4e-7a1b-6c00-9d2e-3f4a5b6c7d8e"
        assert redaction.redact_string(f"trace for {uuid_text}") == f"trace for {uuid_text}"
        hex32 = "ab" * 16  # 32 hex chars — an id-length handle, not caught
        assert redaction.redact_string(f"key {hex32}") == f"key {hex32}"


class TestRecursiveWalk:
    def test_nested_dicts_and_lists_are_walked(self) -> None:
        attributes = {
            "context": {
                "note": "Bearer abcd1234efgh5678 attached",
                "nested": ["mailto: user@host.example", 42, True],
            },
            "authorization": "Bearer top-secret-token-value",
        }
        redacted = redaction.redact_pairs(attributes)
        assert redacted["authorization"] == redaction.REDACTION_MARKER
        inner = redacted["context"]
        assert redaction.REDACTION_MARKER in inner["note"]
        assert inner["nested"][0] == redaction.REDACTION_MARKER
        assert inner["nested"][1] == 42
        assert inner["nested"][2] is True

    def test_non_string_leaves_are_never_rewritten(self) -> None:
        assert redaction.redact_value(7) == 7
        assert redaction.redact_value(1.5) == 1.5
        assert redaction.redact_value(True) is True
        assert redaction.redact_value(None) is None


class TestEmitBoundary:
    def test_emit_log_line_redacts_before_serialization(self) -> None:
        # The P11.08 boundary: redaction happens INSIDE the emitter, so
        # the serialized line the caller returns/never contains the
        # secret — this is the emit-time guarantee and nothing more.
        stream = io.StringIO()
        serialized = log_schema.emit_log_line(
            "info",
            "uplink failed with Bearer abcd1234efgh5678",
            tenant_id="tenant-1",
            command_id="",
            trace_id="a" * 32,
            auth="Bearer zz-top-secret-token",
            stream=stream,
        )
        written = stream.getvalue()
        assert serialized.endswith("}")
        for text in (serialized, written):
            assert "zz-top-secret-token" not in text
            assert "abcd1234efgh5678" not in text
            assert redaction.REDACTION_MARKER in text
        # The written stream form carries exactly one line.
        assert written.count("\n") == 1

    def test_mandatory_attributes_are_always_present(self) -> None:
        import json

        serialized = log_schema.emit_log_line(
            "error", "boom", stream=io.StringIO()
        )
        line = json.loads(serialized)
        for attribute in log_schema.MANDATORY_ATTRIBUTES:
            assert attribute in line, attribute
        assert line["schema_version"] == log_schema.LOG_SCHEMA_VERSION


if __name__ == "__main__":  # pragma: no cover — manual local check only.
    pytest.main([__file__])
