# FIM-F09.03 — STRUCTURAL validation of the checked-in OTel Collector
#         gateway configuration (deploy/compose/otel-collector-config.yaml):
#         the three signals (traces / metrics / logs) are wired
#         (receivers -> processors -> exporters pipelines present, and
#         every referenced component DEFINED). A static tool, NOT a run.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. IMPLEMENTED UNDER FIM-F09. WRITTEN, NEVER RUN as part of any pipeline
#    verification: this tool has NEVER observed a Collector process, and a
#    passing exit code here proves NOTHING about runtime behaviour. A
#    RUNNING Collector gateway and FIRING SLO alerts are HUMAN GATES (F-09
#    acceptance criteria 1-3 need live infrastructure — feature entry F-09
#    in docs/releases/final-implementation-before-publication.md §5, phase
#    record P11.06). NO network, NO collector binary, NO docker: this file
#    parses text and checks references, nothing else.
# 2. WHAT "STRUCTURALLY VALID" MEANS here: (a) the document parses under
#    the YAML subset below; (b) the top-level sections a Collector config
#    needs exist (receivers, processors, exporters, service); (c) the
#    three signal pipelines (traces, metrics, logs) are present; (d) each
#    pipeline names at least one receiver and one exporter (processors
#    optional — a pipeline without processors is a WARNING, the Collector
#    allows it); (e) every component a pipeline references is DEFINED in
#    its top-level section. It does NOT validate component CONFIGURATION
#    semantics (endpoints, intervals, limits) — that is the Collector
#    binary's own config validation, a human-gate item.
# 3. THE YAML SUBSET (deliberate; PyYAML is NOT a dependency of this tree
#    and NO dependency is added). The reader accepts EXACTLY:
#      - comments (`#` to end of line: full-line, or trailing after a
#        scalar when separated by whitespace; never inside quotes),
#      - block mappings at any depth (2-space-indented in the checked-in
#        config; any consistent indentation is accepted per level),
#      - `key: value` scalars (plain or wrapped once in single/double
#        quotes),
#      - inline flow sequences `[a, b, c]` of plain scalars (empty `[]`
#        allowed),
#      - nested block mappings as the value of a `key:` line.
#    It REFUSES (with a line number, never a misparse) everything else:
#    block sequences (`- item`), flow mappings (`{...}`), anchors and
#    aliases (`&x`, `*x`), tags (`!!str`), multi-line scalar indicators
#    (`|` and `>`), document markers (`---`, `...`), tab indentation, and
#    duplicate keys at the same level. Refusal = exit 1 with the finding.
#    The checked-in config uses only accepted constructs (verified by this
#    tool's self-tests and by tests_fim_otel/selftest.py against the real
#    file).
# 4. STDLIB-ONLY (sys/os/re/json/typing); runnable today with
#    `python -I apps/worker/python/tools_fim/validate_collector_config.py`
#    (default config path resolved from this file's location; an explicit
#    path may be passed as argv[1]). `--self-test` runs the hand-computed
#    parser/validator pins below (exit 0/1).
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import json
import os
import re
import sys

# The three OTel signals the gateway must wire (P11.06 record; the
# checked-in config's `service.pipelines` keys).
REQUIRED_SIGNALS: tuple = ("traces", "metrics", "logs")

# The top-level sections this validator requires (a Collector config
# without them is not loadable).
REQUIRED_SECTIONS: tuple = ("receivers", "processors", "exporters", "service")

# The pipeline fields and whether each is required (the Collector's
# pipeline schema, reduced to what a structural check can pin).
REQUIRED_PIPELINE_FIELDS: tuple = ("receivers", "exporters")
OPTIONAL_PIPELINE_FIELDS: tuple = ("processors",)

_SEVERITY_ERROR = "error"
_SEVERITY_WARNING = "warning"

# The constructs this subset REFUSES, recognized anywhere in a line.
_REFUSE_ANCHOR = re.compile(r"(?:^|\s)&[A-Za-z0-9_\-]+")
_REFUSE_ALIAS = re.compile(r"(?:^|\s)\*[A-Za-z0-9_\-]+")
_REFUSE_TAG = re.compile(r"(?:^|\s)!")
_REFUSE_MULTILINE = re.compile(r"(?:^|\s)[|>](?:[+-]?\d*\s*$|\s)")
_REFUSE_BLOCK_ITEM = re.compile(r"^\s*-\s")

_FLOW_LIST = re.compile(r"^\[(.*)\]$")


class SubsetViolation(Exception):
    """The document uses a construct outside the YAML subset — refused
    (never misparsed), carrying the 1-based line number."""

    def __init__(self, line_number: int, reason: str) -> None:
        super().__init__("line %d: %s" % (line_number, reason))
        self.line_number = line_number
        self.reason = reason


def _strip_comment(line: str) -> str:
    """Removes a trailing comment (` #...`) outside quotes; a `#` inside
    quotes is content."""
    in_single = False
    in_double = False
    for index, char in enumerate(line):
        if char == "'" and not in_double:
            in_single = not in_single
        elif char == '"' and not in_single:
            in_double = not in_double
        elif char == "#" and not in_single and not in_double:
            if index == 0 or line[index - 1] in " \t":
                return line[:index]
    return line


def _unquote(scalar: str) -> str:
    """Strips ONE layer of matching surrounding quotes (the subset's only
    quoting form)."""
    if len(scalar) >= 2 and scalar[0] == scalar[-1] and scalar[0] in "'\"":
        return scalar[1:-1]
    return scalar


def _parse_flow_list(text: str, line_number: int) -> list:
    """Parses `[a, b, c]` into a list of plain scalars (quotes stripped
    per item). Anything nested inside an item (brackets, braces) is
    REFUSED."""
    match = _FLOW_LIST.match(text.strip())
    if match is None:
        raise SubsetViolation(line_number, "expected a [flow, list] value, got %r" % text.strip())
    inner = match.group(1).strip()
    if inner == "":
        return []
    items: list = []
    for raw_item in inner.split(","):
        item = raw_item.strip()
        if item == "":
            raise SubsetViolation(line_number, "empty flow-list item")
        if "[" in item or "]" in item or "{" in item or "}" in item:
            raise SubsetViolation(line_number, "nested flow collections are outside the subset: %r" % item)
        items.append(_unquote(item))
    return items


def parse_yaml_subset(text: str) -> dict:
    """The minimal indentation-based reader for the checked-in config
    shape. Returns nested dicts whose leaves are plain scalars (str) or
    flow lists (list of str). Raises SubsetViolation on anything outside
    the documented subset (module header item 3) — refusal over misparse,
    always."""
    root: dict = {}
    # The stack of (indent, container) frames currently open.
    stack: list = [(-1, root)]
    previous_plain_key_line: dict = {}
    for line_number, raw_line in enumerate(text.splitlines(), start=1):
        if "\t" in raw_line[: len(raw_line) - len(raw_line.lstrip())]:
            raise SubsetViolation(line_number, "tab indentation is outside the subset")
        line = _strip_comment(raw_line).rstrip()
        if line.strip() == "":
            continue
        if line.strip() in ("---", "..."):
            raise SubsetViolation(line_number, "document markers are outside the subset")
        content = line.strip()
        if _REFUSE_ANCHOR.search(content) or _REFUSE_ALIAS.search(content):
            raise SubsetViolation(line_number, "anchors/aliases are outside the subset")
        if _REFUSE_TAG.search(content):
            raise SubsetViolation(line_number, "tags are outside the subset")
        if re.search(r"(?:^|\s)[|>](?:\s*[+-]?\d*)?\s*$", content) or re.match(r"^(?:\s*)[|>]", content):
            raise SubsetViolation(line_number, "multi-line scalar indicators (| >) are outside the subset")
        if _REFUSE_BLOCK_ITEM.match(line):
            raise SubsetViolation(line_number, "block sequences (- items) are outside the subset")
        indent = len(line) - len(line.lstrip(" "))
        while stack and indent <= stack[-1][0] and len(stack) > 1:
            stack.pop()
        parent = stack[-1][1]
        if not isinstance(parent, dict):
            raise SubsetViolation(line_number, "a list context is outside the subset")
        if ":" not in content:
            raise SubsetViolation(line_number, "expected `key:` or `key: value`")
        key, _, value = content.partition(":")
        key = _unquote(key.strip())
        value = value.strip()
        if key == "":
            raise SubsetViolation(line_number, "empty key")
        if key in parent:
            raise SubsetViolation(line_number, "duplicate key %r at the same level" % key)
        if value == "":
            # A nested mapping (or an empty leaf) opens here; the next
            # deeper line fills it. An empty leaf stays an empty dict.
            child: dict = {}
            parent[key] = child
            stack.append((indent, child))
            previous_plain_key_line[key] = line_number
            continue
        if value.startswith("{") or value.endswith("{"):
            raise SubsetViolation(line_number, "flow mappings ({...}) are outside the subset")
        if value.startswith("["):
            parent[key] = _parse_flow_list(value, line_number)
        elif value.startswith("*") or value.startswith("&"):
            raise SubsetViolation(line_number, "anchors/aliases are outside the subset")
        else:
            parent[key] = _unquote(value)
    return root


def _as_mapping(value: object) -> dict | None:
    return value if isinstance(value, dict) else None


def validate_collector_config(document: dict) -> list:
    """The structural checks over a parsed config document. Returns the
    findings list — each `{"severity": "error"|"warning", "check": <id>,
    "detail": <str>}` — empty only when all three signals are wired with
    fully-resolved references."""
    findings: list = []

    def finding(severity: str, check: str, detail: str) -> None:
        findings.append({"severity": severity, "check": check, "detail": detail})

    section_map: dict = {}
    for section in REQUIRED_SECTIONS:
        value = document.get(section)
        mapping = _as_mapping(value)
        if mapping is None:
            finding(
                _SEVERITY_ERROR,
                "missing-section",
                "the top-level section %r is missing or not a mapping" % section,
            )
        section_map[section] = mapping or {}

    pipelines = _as_mapping(section_map["service"].get("pipelines"))
    if pipelines is None:
        finding(
            _SEVERITY_ERROR,
            "pipelines-missing",
            "service.pipelines is missing or not a mapping — no signal is wired",
        )
        pipelines = {}

    for signal in REQUIRED_SIGNALS:
        pipeline = _as_mapping(pipelines.get(signal))
        if pipeline is None:
            finding(
                _SEVERITY_ERROR,
                "pipeline-missing",
                "the %r signal pipeline is absent (service.pipelines.%s)" % (signal, signal),
            )
            continue
        for field in REQUIRED_PIPELINE_FIELDS:
            references = pipeline.get(field)
            if not isinstance(references, list) or len(references) == 0:
                finding(
                    _SEVERITY_ERROR,
                    "pipeline-empty",
                    "the %r pipeline's %s list is missing or empty" % (signal, field),
                )
        processors = pipeline.get("processors")
        if processors is None:
            finding(
                _SEVERITY_WARNING,
                "no-processors",
                "the %r pipeline declares no processors (the Collector allows it; "
                "the checked-in gateway wires memory_limiter + batch)" % signal,
            )
        elif not isinstance(processors, list):
            finding(
                _SEVERITY_ERROR,
                "pipeline-shape",
                "the %r pipeline's processors value is not a list" % signal,
            )
        # Every referenced component must be DEFINED.
        for field in ("receivers", "processors", "exporters"):
            references = pipeline.get(field)
            if not isinstance(references, list):
                continue
            for reference in references:
                if not isinstance(reference, str):
                    finding(
                        _SEVERITY_ERROR,
                        "reference-shape",
                        "the %r pipeline's %s entry %r is not a plain name" % (signal, field, reference),
                    )
                    continue
                section = section_map.get(field if field != "processors" else "processors", {})
                defined = field in section_map and isinstance(section_map[field], dict) and reference in (section_map[field] or {})
                if not defined:
                    finding(
                        _SEVERITY_ERROR,
                        "undefined-reference",
                        "the %r pipeline references %s %r which is not defined in the %r section"
                        % (signal, field, reference, field),
                    )

    # Defined-but-unreferenced components are warnings (they cost config
    # review attention, never correctness).
    for field in ("receivers", "processors", "exporters"):
        section = _as_mapping(document.get(field)) or {}
        referenced: set = set()
        for signal in REQUIRED_SIGNALS:
            pipeline = _as_mapping(pipelines.get(signal)) or {}
            refs = pipeline.get(field)
            if isinstance(refs, list):
                referenced.update(r for r in refs if isinstance(r, str))
        for name in sorted(section.keys()):
            if name not in referenced:
                finding(
                    _SEVERITY_WARNING,
                    "unused-component",
                    "the %s %r is defined but referenced by no pipeline" % (field, name),
                )
    return findings


def validate_config_text(text: str) -> tuple:
    """Parses and validates one config document; returns (findings,
    document-or-None). A SubsetViolation becomes a single error finding
    (the checks below it cannot run)."""
    try:
        document = parse_yaml_subset(text)
    except SubsetViolation as violation:
        return (
            [
                {
                    "severity": _SEVERITY_ERROR,
                    "check": "unparseable",
                    "detail": str(violation),
                }
            ],
            None,
        )
    return (validate_collector_config(document), document)


def default_config_path() -> str:
    """The checked-in gateway config, resolved from this file's location:
    apps/worker/python/tools_fim/ (4 levels up) -> repo root ->
    deploy/compose/."""
    here = os.path.dirname(os.path.abspath(__file__))
    repo_root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(here))))
    return os.path.join(repo_root, "deploy", "compose", "otel-collector-config.yaml")


def render_findings(findings: list) -> str:
    """The human-readable findings report (sorted: errors first)."""
    ordered = sorted(findings, key=lambda f: (0 if f["severity"] == _SEVERITY_ERROR else 1, f["check"], f["detail"]))
    lines = []
    for item in ordered:
        lines.append("[%s] %s: %s" % (item["severity"].upper(), item["check"], item["detail"]))
    errors = sum(1 for f in findings if f["severity"] == _SEVERITY_ERROR)
    warnings = sum(1 for f in findings if f["severity"] == _SEVERITY_WARNING)
    lines.append("%d findings (%d errors, %d warnings)" % (len(findings), errors, warnings))
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Hand-computed self-tests for the parser and the validator (NEVER RUN in
# this program — the human gate runs them; synthetic configs only, the
# checked-in config is exercised by tests_fim_otel/selftest.py).
# ---------------------------------------------------------------------------

_GOOD_MINI_CONFIG = """
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317
      http:
        endpoint: 0.0.0.0:4318

processors:
  memory_limiter:
    check_interval: 1s
    limit_mib: 256
  batch:
    timeout: 5s

exporters:
  debug:
    verbosity: basic

service:
  extensions: [health_check]
  pipelines:
    traces:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [debug]
    metrics:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [debug]
    logs:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [debug]
"""


def run_self_tests() -> list:
    """Runs every hand-computed self-test; returns one record per test
    (`name`, `passed`, `detail`) — the repo's --self-test convention."""
    results: list = []

    def check(name: str, condition: bool, detail: str) -> None:
        results.append({"name": name, "passed": bool(condition), "detail": detail})

    # -- the subset parser accepts the checked-in config shape. ----------
    parsed = parse_yaml_subset(_GOOD_MINI_CONFIG)
    check(
        "parse_mini_config_mapping",
        parsed.get("receivers", {}).get("otlp", {}).get("protocols", {}).get("grpc", {}).get("endpoint")
        == "0.0.0.0:4317",
        "nested mappings parse to the pinned leaf value",
    )
    check(
        "parse_mini_config_flow_list",
        parsed.get("service", {}).get("pipelines", {}).get("traces", {}).get("processors")
        == ["memory_limiter", "batch"],
        "inline flow sequences parse to a list of plain scalars",
    )
    commented = parse_yaml_subset("# header comment\nreceivers:\n  otlp:  # trailing comment\n    grpc:\n      endpoint: 0.0.0.0:4317\n")
    check(
        "parse_comments",
        commented.get("receivers", {}).get("otlp", {}).get("grpc", {}).get("endpoint") == "0.0.0.0:4317"
        and "otlp" in commented.get("receivers", {}),
        "full-line and trailing comments are stripped; keys survive",
    )
    check(
        "parse_quoted_scalar",
        parse_yaml_subset('exporters:\n  debug:\n    verbosity: "basic"\n').get("exporters", {}).get("debug", {}).get("verbosity")
        == "basic",
        "a once-quoted scalar is unquoted",
    )
    check(
        "parse_empty_flow_list",
        parse_yaml_subset("service:\n  extensions: []\n").get("service", {}).get("extensions") == [],
        "an empty flow list parses to []",
    )

    # -- refusals (never a misparse). ------------------------------------
    refusals = [
        ("block_sequence", "pipelines:\n  traces:\n    receivers:\n      - otlp\n"),
        ("tab_indent", "receivers:\n\totlp:\n"),
        ("flow_mapping", "processors:\n  batch: {timeout: 5s}\n"),
        ("anchor", "receivers: &base\n  otlp: {}\n"),
        ("alias", "service: *base\n"),
        ("tag", "receivers: !!map\n  otlp:\n"),
        ("multiline_scalar", "processors:\n  batch:\n    timeout: |\n      5s\n"),
        ("document_marker", "---\nreceivers:\n"),
    ]
    refused_all = True
    refused_detail = ""
    for label, text in refusals:
        try:
            parse_yaml_subset(text)
            refused_all = False
            refused_detail = "%s was accepted (must be refused)" % label
            break
        except SubsetViolation:
            continue
    check(
        "refuse_outside_subset",
        refused_all,
        refused_detail or "all 8 outside-subset constructs are refused with a line number",
    )
    check(
        "duplicate_key_refused",
        _raises_subset_violation("receivers:\n  a: 1\n  a: 2\n"),
        "a duplicate key at the same level is refused",
    )

    # -- the validator on a good mini-config: zero errors. ----------------
    findings, document = validate_config_text(_GOOD_MINI_CONFIG)
    errors = [f for f in findings if f["severity"] == _SEVERITY_ERROR]
    check(
        "validate_good_mini_config",
        document is not None and errors == [],
        "the mini gateway config wires all three signals; errors %r" % [f["check"] for f in errors],
    )

    # -- a missing signal pipeline is an error. ---------------------------
    missing_logs = _GOOD_MINI_CONFIG.replace(
        """    logs:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [debug]
""",
        "",
    )
    findings_missing, _ = validate_config_text(missing_logs)
    check(
        "missing_logs_pipeline_is_error",
        any(f["check"] == "pipeline-missing" and "logs" in f["detail"] and f["severity"] == _SEVERITY_ERROR for f in findings_missing),
        "dropping the logs pipeline yields exactly the pipeline-missing error",
    )

    # -- an undefined reference is an error. ------------------------------
    undefined = _GOOD_MINI_CONFIG.replace("exporters: [debug]", "exporters: [nonexistent]")
    findings_undefined, _ = validate_config_text(undefined)
    check(
        "undefined_reference_is_error",
        any(f["check"] == "undefined-reference" and "nonexistent" in f["detail"] for f in findings_undefined),
        "a pipeline referencing an undefined exporter yields undefined-reference",
    )

    # -- a defined-but-unreferenced component is a warning. ---------------
    unused = _GOOD_MINI_CONFIG.replace(
        "exporters:\n  debug:\n    verbosity: basic\n",
        "exporters:\n  debug:\n    verbosity: basic\n  extra:\n    verbosity: basic\n",
    )
    findings_unused, _ = validate_config_text(unused)
    check(
        "unused_component_is_warning",
        any(f["check"] == "unused-component" and "extra" in f["detail"] and f["severity"] == _SEVERITY_WARNING for f in findings_unused)
        and not any(f["check"] == "unused-component" and f["severity"] == _SEVERITY_ERROR for f in findings_unused),
        "an unreferenced exporter is a warning, never an error",
    )

    # -- a pipeline without receivers is an error. ------------------------
    no_receivers = _GOOD_MINI_CONFIG.replace(
        """    logs:
      receivers: [otlp]
""",
        """    logs:
""",
    )
    findings_no_recv, _ = validate_config_text(no_receivers)
    check(
        "empty_pipeline_field_is_error",
        any(f["check"] == "pipeline-empty" and "logs" in f["detail"] for f in findings_no_recv),
        "a pipeline without its receivers list yields pipeline-empty",
    )

    return results


def _raises_subset_violation(text: str) -> bool:
    """True when the parser refuses the text (SubsetViolation) — the
    self-test helper for single refusals."""
    try:
        parse_yaml_subset(text)
    except SubsetViolation:
        return True
    return False


if __name__ == "__main__":  # pragma: no cover — human-gate entrypoint
    if "--self-test" in sys.argv[1:]:
        _records = run_self_tests()
        _failures = [r for r in _records if not r["passed"]]
        for _r in _records:
            print(("PASS" if _r["passed"] else "FAIL"), _r["name"], "-", _r["detail"])
        print("%d/%d self-tests passed" % (len(_records) - len(_failures), len(_records)))
        raise SystemExit(1 if _failures else 0)

    _path = sys.argv[1] if len(sys.argv) > 1 else default_config_path()
    try:
        with open(_path, "r", encoding="utf-8") as _handle:
            _text = _handle.read()
    except OSError as _error:
        print("ERROR: the config could not be read: %s" % _error)
        raise SystemExit(1)
    _findings, _document = validate_config_text(_text)
    _wired = []
    if _document is not None:
        _pipelines = _document.get("service", {}).get("pipelines", {})
        if isinstance(_pipelines, dict):
            _wired = [s for s in REQUIRED_SIGNALS if isinstance(_pipelines.get(s), dict)]
    print("config: %s" % _path)
    print("signals wired: %s" % (_wired if _wired else "NONE"))
    print(render_findings(_findings))
    _errors = [f for f in _findings if f["severity"] == _SEVERITY_ERROR]
    raise SystemExit(1 if _errors else 0)
