# P8.24 — Fault-injection scaffolding for the research-module robustness
#         drill (pure stdlib, over INJECTED callables).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. THIS SCAFFOLDING WAS WRITTEN, NEVER EXECUTED. The ledger P8.24 drill
#    (ledger/phase-08-advanced-coordination/ledger_phase_8.md §P8.24) is a
#    HUMAN GATE input: every fault/module cell in
#    `24_failure_drill.md` §3 is EMPTY (UNRUN). This module is the
#    scaffolding the human gate uses to inject the faults — nothing here
#    has ever been run, and no drill outcome exists anywhere.
# 2. WHAT THIS IS: a pure-stdlib fault-injection wrapper over injected
#    callables (the same port shape the research modules already use:
#    `crates/flexcore-research/src/dmpc.rs::AdmmLocalWorker` is the
#    call-return precedent; the Python workers are one-JSON-document
#    callables). The drill plan targets these wrappers AT the worker
#    boundary: a target callable stands in for ONE worker/participant
#    reply, and the injected fault stands in for the transport/worker
#    fault named in the ledger's step-1 fault list.
# 3. FAULTS IMPLEMENTED (closed vocabulary, validated fail-closed at
#    construction): `none` (pass-through), `error` (the worker answers
#    with an injected error — models an unresponsive/failed worker),
#    `drop` (the reply never arrives — models message loss),
#    `crash_before` / `crash_after` (the worker dies before serving /
#    after computing, reply lost with the process),
#    `scripted_reply` (a canned reply is served WITHOUT calling the
#    target — models the honest stale re-serve: the canned reply carries
#    its own `stale: True` flag exactly like
#    `LocalSolveReply.stale`), `replay_last` (the last successful reply
#    is delivered AGAIN without calling the target — models a duplicated
#    message), `reorder_reversed` (scripted replies are served in
#    REVERSE order — models out-of-order delivery on a message-queue
#    transport), `clock_skew` (a clock callable wrapped with a fixed
#    offset — models clock skew against the injected clock of the
#    validity window), `oversize` (a payload builder that exceeds the
#    recorded cap — models the oversized-output/input fault).
# 4. WHAT THIS IS NOT: no subprocess is started, no worker process is
#    killed, no network socket is opened, no file is written, no sleep
#    is performed — every fault is a pure data-flow transformation over
#    the injected callable, so every drill expectation below is
#    HAND-COMPUTED arithmetic on toy targets (the self-tests), never an
#    observed run. Real-process fault injection (kill -9 on a worker,
#    SIGKILL mid-iteration) is the composition root's supervision domain
#    (the Phase 7 P7.18 precedent) and is NOT modeled here — recorded in
#    `24_failure_drill.md` §2 F6.
# 5. UNIT DISCIPLINE (report §10.2): `clock_skew` offsets are in
#    milliseconds (the validity-window field `valid_until_ms` is epoch
#    ms); `oversize` caps are in bytes (the harness cap
#    `MAX_PROPOSAL_PAYLOAD_BYTES` = 1 MiB = 1048576 bytes); power values
#    inside toy replies are MW with `P > 0` = export. No unit is
#    invented beyond the recorded ones.
# 6. Run instruction (human gate ONLY; never run in the writing loop):
#      python -I apps/worker/python/drill/fault_injection.py
#    (the pytest suite mirrors these cases in tests/test_fault_injection.py).
# 7. P12.09 EXTENSION (written, never executed; the same honesty rules
#    apply unchanged): the closed vocabulary gained `partial_write` (the
#    target runs, the delivered reply is TRUNCATED to `keep_bytes` —
#    models a torn/partial write at the seam; a reply type that cannot
#    be sliced fails closed) and `slow_consumer` (the target runs and
#    the reply is delivered UNCHANGED, with the injected delay carried
#    as DATA in `CallRecord.delay_ms` — the scaffolding never sleeps,
#    so the drill's timeout arithmetic is hand-computed against
#    `SUPERVISION_DEFAULT_CALL_TIMEOUT_MS`, transcribed from
#    `apps/worker/src/supervision.rs::DEFAULT_CALL_TIMEOUT_MS` = 30 000 ms).
#    Two new injectors extend the same pure-data discipline:
#    `PartitionInjector` (a bounded network-partition WINDOW over a call
#    sequence — before the window: pass-through; during: the named
#    `NetworkPartitionError`, target unreached; after: pass-through —
#    models the NATS/Postgres outage windows C-NATS-1 drills at the
#    transport level without any socket being opened) and
#    `scheduled_skew_clock` (the generalization of `skewed_clock`: a
#    PER-CALL offset schedule; call i uses offsets_ms[i] while the
#    schedule holds, then the LAST offset persists — a drift that
#    plateaus; negative offsets legal; no clock read of its own).
#    Nothing real is partitioned, written, delayed, or skewed: every new
#    fault is a transformation over the injected callable, and every new
#    self-test expectation is hand-computed arithmetic on toy targets.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

# The recorded harness payload cap the `oversize` helper targets
# (verbatim from `crates/flexcore-research/src/validation.rs`:
# `pub const MAX_PROPOSAL_PAYLOAD_BYTES: usize = 1024 * 1024;`).
MAX_PROPOSAL_PAYLOAD_BYTES = 1024 * 1024

# The recorded supervision call timeout the `slow_consumer` drill
# arithmetic is hand-computed against (transcribed verbatim from
# `apps/worker/src/supervision.rs`:
# `pub const DEFAULT_CALL_TIMEOUT_MS: u64 = 30_000;` — a COPY for the
# drill, never a second authority; if the Rust constant changes, this
# transcription is re-diffed and re-cited).
SUPERVISION_DEFAULT_CALL_TIMEOUT_MS = 30_000

# The closed fault vocabulary. Anything else is a construction-time
# configuration refusal (fail-closed, like every research module).
# P12.09 adds `partial_write`, `slow_consumer`, and `network_partition`
# (the last one is carried ONLY by `PartitionInjector`, the window
# injector — see the honesty header, item 7).
FAULT_KINDS: frozenset = frozenset(
    {
        "none",
        "error",
        "drop",
        "crash_before",
        "crash_after",
        "scripted_reply",
        "replay_last",
        "reorder_reversed",
        "partial_write",
        "slow_consumer",
        "network_partition",
    }
)


class DrillConfigurationError(ValueError):
    """A malformed drill plan — refused at construction, before any call."""


class MessageDropped(RuntimeError):
    """The injected reply never arrived (message loss at the seam)."""


class WorkerCrashError(RuntimeError):
    """The injected worker died. `phase` is "before" (died before
    serving — the target was never called) or "after" (died after
    computing — the reply was lost with the process)."""

    def __init__(self, phase: str, detail: str = "") -> None:
        if phase not in ("before", "after"):
            raise DrillConfigurationError(
                "crash phase must be 'before' or 'after' (got %r)" % (phase,)
            )
        super().__init__(
            "injected worker crash (%s)%s" % (phase, ": %s" % detail if detail else "")
        )
        self.phase = phase


class ReplayUnavailable(RuntimeError):
    """A `replay_last` fault fired with no prior successful reply to
    replay — a duplicated message cannot exist before the first message."""


class NetworkPartitionError(RuntimeError):
    """The injected partition window is CLOSED at this call — no traffic
    passes the seam (modeled data-flow loss; no socket exists)."""


class PartialWriteUnsupported(DrillConfigurationError):
    """A `partial_write` fault fired against a reply type that cannot be
    sliced (not bytes/str) — refused at call time, fail-closed."""


class Fault:
    """ONE fault in a plan. Construct with `fault(kind, **payload)` —
    the constructor validates the kind and the required payload fields
    (fail-closed; an invalid plan never reaches a target)."""

    __slots__ = ("kind", "error", "reply", "phase", "detail", "keep_bytes", "delay_ms")

    def __init__(self, kind: str, *, error=None, reply=None, phase: str = "", detail: str = "",
                 keep_bytes=None, delay_ms=None) -> None:
        if kind not in FAULT_KINDS:
            raise DrillConfigurationError(
                "fault kind %r is not one of %s" % (kind, sorted(FAULT_KINDS))
            )
        if kind == "error" and not isinstance(error, BaseException):
            raise DrillConfigurationError(
                "the 'error' fault requires an injected error instance"
            )
        if kind == "scripted_reply" and reply is None:
            raise DrillConfigurationError(
                "the 'scripted_reply' fault requires a canned reply "
                "(the honest stale re-serve carries its own stale flag)"
            )
        if kind in ("crash_before", "crash_after") and not phase:
            raise DrillConfigurationError(
                "the %r fault requires a crash phase" % (kind,)
            )
        if kind == "partial_write":
            if not isinstance(keep_bytes, int) or isinstance(keep_bytes, bool) \
                    or keep_bytes < 0:
                raise DrillConfigurationError(
                    "the 'partial_write' fault requires a non-negative integer "
                    "keep_bytes (got %r)" % (keep_bytes,)
                )
        if kind == "slow_consumer":
            if not isinstance(delay_ms, int) or isinstance(delay_ms, bool) \
                    or delay_ms < 0:
                raise DrillConfigurationError(
                    "the 'slow_consumer' fault requires a non-negative integer "
                    "delay_ms (got %r)" % (delay_ms,)
                )
        self.kind = kind
        self.error = error
        self.reply = reply
        self.phase = phase
        self.detail = detail
        self.keep_bytes = keep_bytes
        self.delay_ms = delay_ms


def fault(kind: str, **payload) -> Fault:
    """The plan-entry constructor (see `Fault`)."""
    return Fault(kind, **payload)


class CallRecord:
    """The record of ONE call through the injector (the drill's evidence
    unit: which fault fired, whether the target was reached, what came
    back). `reply` is the delivered value on success; `error` is the
    delivered exception on failure; exactly one of them is set.
    `delay_ms` carries a `slow_consumer` fault's injected delay as DATA
    (the scaffolding never sleeps); it is 0 for every other fault."""

    __slots__ = ("index", "fault_kind", "target_called", "reply", "error", "delay_ms")

    def __init__(self, index: int, fault_kind: str, target_called: bool, reply=None, error=None,
                 delay_ms: int = 0) -> None:
        self.index = index
        self.fault_kind = fault_kind
        self.target_called = target_called
        self.reply = reply
        self.error = error
        self.delay_ms = delay_ms


class FaultInjector:
    """Wraps ONE target callable with a per-call fault plan.

    `plan` is a list of `Fault` entries, applied in call order; calls
    beyond the plan use `default` (a `Fault("none")` unless overridden).
    Every call — faulted or not — appends one `CallRecord`; the drill
    reads the records, never memory.
    """

    def __init__(self, target, plan: list, default: Fault | None = None) -> None:
        if not callable(target):
            raise DrillConfigurationError(
                "the drill target must be a callable (got %r)" % (target,)
            )
        if default is None:
            default = Fault("none")
        if not isinstance(default, Fault):
            raise DrillConfigurationError("default must be a Fault")
        for entry in plan:
            if not isinstance(entry, Fault):
                raise DrillConfigurationError(
                    "every plan entry must be a Fault (got %r)" % (entry,)
                )
        self._target = target
        self._plan = list(plan)
        self._default = default
        self.records: list = []
        self._last_reply = None
        self._has_last_reply = False
        self._reorder_queue: list = []

    def call(self, *args, **kwargs):
        """ONE call through the injector. Applies the next plan entry,
        records the outcome, and returns the reply or raises the fault's
        error. The target is called at most once per call (a duplicate
        delivery is `replay_last`, which never re-calls the target — the
        modelled duplicate is a TRANSPORT event, not a re-execution)."""
        index = len(self.records)
        fault_entry = self._plan[index] if index < len(self._plan) else self._default

        if fault_entry.kind == "none":
            reply = self._target(*args, **kwargs)
            self._remember(reply)
            self._append(index, fault_entry, target_called=True, reply=reply)
            return reply

        if fault_entry.kind == "error":
            # The worker answers with a failure — the target is never
            # reached (the injected error stands in for the whole reply).
            self._append(index, fault_entry, target_called=False, error=fault_entry.error)
            raise fault_entry.error

        if fault_entry.kind == "drop":
            # Message loss: no reply ever arrives.
            self._append(index, fault_entry, target_called=False, error=None)
            raise MessageDropped(
                "injected message drop at call %d (the reply never arrived)" % index
            )

        if fault_entry.kind == "crash_before":
            error = WorkerCrashError("before", fault_entry.detail)
            self._append(index, fault_entry, target_called=False, error=error)
            raise error

        if fault_entry.kind == "crash_after":
            # The target DID run — its reply is lost with the process.
            self._target(*args, **kwargs)
            error = WorkerCrashError("after", fault_entry.detail)
            self._append(index, fault_entry, target_called=True, error=error)
            raise error

        if fault_entry.kind == "scripted_reply":
            # A canned reply served WITHOUT the target (the honest stale
            # re-serve shape: the canned reply itself carries its stale
            # flag; the injector never fabricates one).
            self._append(index, fault_entry, target_called=False, reply=fault_entry.reply)
            return fault_entry.reply

        if fault_entry.kind == "replay_last":
            if not self._has_last_reply:
                self._append(index, fault_entry, target_called=False, error=None)
                raise ReplayUnavailable(
                    "replay_last fired at call %d with no prior successful "
                    "reply to replay" % index
                )
            self._append(index, fault_entry, target_called=False, reply=self._last_reply)
            return self._last_reply

        # reorder_reversed is delivered by its own injector
        # (`ReorderInjector` below). A `reorder_reversed` entry inside a
        # plain FaultInjector plan is a drill-plan defect: it would reach
        # the fail-closed guard at the end of this method, never the
        # target.

        if fault_entry.kind == "partial_write":
            # A torn/partial write: the target DID run (its side effect
            # happened), but only the first keep_bytes of the reply
            # arrive. A reply type that cannot be sliced fails CLOSED
            # (the truncation is never guessed, and the error is recorded
            # with target_called=True — the side effect must not vanish
            # from the record).
            reply = self._target(*args, **kwargs)
            if not isinstance(reply, (bytes, str)):
                error = PartialWriteUnsupported(
                    "partial_write fired at call %d against an unsliceable "
                    "reply type %s" % (index, type(reply).__name__)
                )
                self._append(index, fault_entry, target_called=True, error=error)
                raise error
            delivered = reply[:fault_entry.keep_bytes]
            self._append(index, fault_entry, target_called=True, reply=delivered)
            return delivered

        if fault_entry.kind == "slow_consumer":
            # The worker serves — eventually. The injected delay travels
            # as DATA (record.delay_ms); the scaffolding performs NO
            # sleeping and reads NO clock, so the drill's timeout
            # arithmetic (does the delay exceed the supervision call
            # timeout?) is hand-computed by the caller against
            # SUPERVISION_DEFAULT_CALL_TIMEOUT_MS.
            reply = self._target(*args, **kwargs)
            self._append(index, fault_entry, target_called=True, reply=reply,
                         delay_ms=fault_entry.delay_ms)
            return reply

        raise DrillConfigurationError(  # pragma: no cover — construction refuses this
            "unreachable: kind %r survived construction validation" % fault_entry.kind
        )

    def _remember(self, reply) -> None:
        self._last_reply = reply
        self._has_last_reply = True

    def _append(self, index: int, fault_entry: Fault, *, target_called: bool, reply=None, error=None) -> None:
        self.records.append(
            CallRecord(index, fault_entry.kind, target_called, reply=reply, error=error)
        )

    def stats(self) -> dict:
        """Hand-checkable counters over the records (the drill's
        per-cell arithmetic input)."""
        records = self.records
        return {
            "calls": len(records),
            "target_called": sum(1 for r in records if r.target_called),
            "faults_injected": sum(1 for r in records if r.fault_kind != "none"),
            "errors_delivered": sum(1 for r in records if r.error is not None),
            "replies_delivered": sum(1 for r in records if r.reply is not None),
        }


class ReorderInjector(FaultInjector):
    """The reorder fault as its own injector: a plan of scripted replies
    served in REVERSE order (out-of-order delivery on a message-queue
    transport). Every call serves the LAST queued reply first; the target
    is never called while scripted replies remain. Calls beyond the queue
    pass through to the target.

    Construct with the ordered list of canned replies; the delivery order
    is reversed. A plan argument is refused — reorder is the whole plan.
    """

    def __init__(self, target, scripted_replies: list) -> None:
        super().__init__(target, plan=[], default=Fault("none"))
        if not scripted_replies:
            raise DrillConfigurationError(
                "a reorder drill needs at least one scripted reply"
            )
        self._queue = list(scripted_replies)

    def call(self, *args, **kwargs):
        index = len(self.records)
        if self._queue:
            reply = self._queue.pop()  # LAST queued reply first — out of order
            self._append(index, Fault("reorder_reversed"), target_called=False, reply=reply)
            return reply
        reply = self._target(*args, **kwargs)
        self._remember(reply)
        self._append(index, Fault("none"), target_called=True, reply=reply)
        return reply


def skewed_clock(base_clock, offset_ms: int):
    """Wraps a clock callable with a FIXED offset (milliseconds, per the
    validity-window field `valid_until_ms`). Negative offsets are legal
    (skew runs both ways); the offset is a drill parameter, never read
    from any wall clock — the scaffolding performs no sleeping and no
    time reads of its own."""

    def skewed(*args, **kwargs):
        return base_clock(*args, **kwargs) + offset_ms

    return skewed


def scheduled_skew_clock(base_clock, offsets_ms: list):
    """The P12.09 generalization of `skewed_clock`: a PER-CALL offset
    schedule. Call i reads base + offsets_ms[i] while i is inside the
    schedule; beyond the schedule the LAST offset PERSISTS (a drift that
    plateaus — the explicit modelling choice, documented here so no
    ambiguity hides in the drill arithmetic). Negative offsets are legal;
    an empty schedule is a construction refusal (fail-closed, like the
    empty reorder plan). No clock is read and no sleep is performed by
    the scaffolding itself."""
    if not offsets_ms:
        raise DrillConfigurationError(
            "a scheduled-skew drill needs at least one offset"
        )
    schedule = list(offsets_ms)
    calls = {"n": 0}

    def skewed(*args, **kwargs):
        index = calls["n"]
        calls["n"] += 1
        offset = schedule[index] if index < len(schedule) else schedule[-1]
        return base_clock(*args, **kwargs) + offset

    return skewed


class PartitionInjector(FaultInjector):
    """The P12.09 network-partition fault as its own injector (the
    `ReorderInjector` shape): a bounded partition WINDOW over the call
    sequence. Calls before the window pass through; calls INSIDE the
    window raise the named `NetworkPartitionError` with the target
    UNREACHED (no traffic passes the seam — modeled data-flow loss, no
    socket exists); calls after the window pass through again (the
    partition healed).

    Construct with `open_after_calls` (the number of healthy calls before
    the window) and `closed_for_calls` (the window length, >= 1). The
    window bounds are drill parameters, registered by the drill plan —
    never derived from any runtime observation.
    """

    def __init__(self, target, open_after_calls: int, closed_for_calls: int) -> None:
        super().__init__(target, plan=[], default=Fault("none"))
        if not isinstance(open_after_calls, int) or isinstance(open_after_calls, bool) \
                or open_after_calls < 0:
            raise DrillConfigurationError(
                "open_after_calls must be a non-negative integer (got %r)"
                % (open_after_calls,)
            )
        if not isinstance(closed_for_calls, int) or isinstance(closed_for_calls, bool) \
                or closed_for_calls < 1:
            raise DrillConfigurationError(
                "closed_for_calls must be a positive integer (got %r)"
                % (closed_for_calls,)
            )
        self._open_after = open_after_calls
        self._closed_for = closed_for_calls

    def call(self, *args, **kwargs):
        index = len(self.records)
        if self._open_after <= index < self._open_after + self._closed_for:
            error = NetworkPartitionError(
                "injected network partition at call %d (the seam is closed: "
                "no traffic passes)" % index
            )
            self._append(index, Fault("network_partition"), target_called=False, error=error)
            raise error
        reply = self._target(*args, **kwargs)
        self._remember(reply)
        self._append(index, Fault("none"), target_called=True, reply=reply)
        return reply


def exceeds_supervision_timeout(delay_ms: int, timeout_ms: int = SUPERVISION_DEFAULT_CALL_TIMEOUT_MS) -> bool:
    """The hand-computed `slow_consumer` arithmetic helper: whether an
    injected delay exceeds the supervision call timeout (transcribed from
    `apps/worker/src/supervision.rs::DEFAULT_CALL_TIMEOUT_MS` = 30 000 ms;
    the copy rule in the module header applies). Pure integer comparison —
    no clock, no sleep."""
    return delay_ms > timeout_ms


def oversized_payload(cap_bytes: int = MAX_PROPOSAL_PAYLOAD_BYTES) -> str:
    """A string payload of exactly cap_bytes + 1 characters — the
    smallest hostile payload that exceeds the recorded cap (the
    validation harness Stage-1 refusal
    `PROPOSAL_ENVELOPE_INVALID` fires above `MAX_PROPOSAL_PAYLOAD_BYTES`).
    The payload content is inert ('x' repeats); nothing is written or
    transmitted by building it."""
    if cap_bytes < 0:
        raise DrillConfigurationError("cap must be non-negative")
    return "x" * (cap_bytes + 1)


def run_drill_cell(name: str, expectation: str, action):
    """Runs ONE drill cell and records the outcome honestly.

    `action` is a zero-argument callable that performs the injected calls
    and returns the observed summary dict (or raises). The cell record
    carries the expectation TEXT verbatim and whether the action
    completed — an exception is recorded AS the outcome, never absorbed.
    The record is data for the human gate; NO verdict is computed here
    beyond `completed`.
    """
    if not callable(action):
        raise DrillConfigurationError("a drill cell needs a callable action")
    try:
        outcome = action()
        return {
            "name": name,
            "expectation": expectation,
            "completed": True,
            "outcome": outcome,
            "error": None,
        }
    except Exception as error:  # noqa: BLE001 — the drill records faults verbatim
        return {
            "name": name,
            "expectation": expectation,
            "completed": False,
            "outcome": None,
            "error": "%s: %s" % (type(error).__name__, error),
        }


def summarize(cells: list) -> dict:
    """Counts the cell records (honest zeros for an empty list — an
    empty drill is zero cells, never a fabricated pass)."""
    return {
        "cells": len(cells),
        "completed": sum(1 for c in cells if c["completed"]),
        "errored": sum(1 for c in cells if not c["completed"]),
    }


# ---------------------------------------------------------------------------
# In-module self-tests for the pure scaffolding (hand-computed; NEVER RUN
# in this loop — the human gate runs `python -I ...fault_injection.py`).
# Every expectation below is derived on paper from the constructed inputs.
# ---------------------------------------------------------------------------


def run_self_tests() -> list:
    """Runs every hand-computed self-test; returns one record per test."""
    results: list = []

    def check(name: str, condition: bool, detail: str) -> None:
        results.append({"name": name, "passed": bool(condition), "detail": detail})

    # ---- Test 1: a none-plan passes through ----------------------------
    # HAND CHECK: target returns call_count*2 → replies [2, 4, 6]; target
    # called 3 times; 0 faults injected.
    state = {"n": 0}

    def counting_toy():
        state["n"] += 1
        return state["n"] * 2

    injector = FaultInjector(counting_toy, [Fault("none")] * 3)
    replies = [injector.call() for _ in range(3)]
    check(
        "none_plan_passes_through",
        replies == [2, 4, 6] and injector.stats() == {
            "calls": 3, "target_called": 3, "faults_injected": 0,
            "errors_delivered": 0, "replies_delivered": 3,
        },
        "three pass-through calls: replies [2, 4, 6], target called 3x, 0 faults",
    )

    # ---- Test 2: an injected error is raised verbatim, target skipped --
    # HAND CHECK: plan [none, error, none] → call 2 raises the SAME
    # injected instance; the target is called on calls 1 and 3 only.
    state2 = {"n": 0}

    def counting_toy2():
        state2["n"] += 1
        return state2["n"]

    injected = RuntimeError("worker down")
    injector2 = FaultInjector(counting_toy2, [Fault("none"), Fault("error", error=injected), Fault("none")])
    first = injector2.call()
    error_seen = None
    try:
        injector2.call()
    except RuntimeError as e:
        error_seen = e
    third = injector2.call()
    check(
        "injected_error_raised_target_skipped",
        first == 1 and error_seen is injected and third == 2
        and injector2.stats()["target_called"] == 2
        and injector2.stats()["faults_injected"] == 1,
        "call 2 raised the injected instance (identity), target called only on 1 and 3",
    )

    # ---- Test 3: message drop ------------------------------------------
    # HAND CHECK: plan [drop, none] → call 1 raises MessageDropped, target
    # called 0 times; call 2 passes through (target called once).
    injector3 = FaultInjector(counting_toy2, [Fault("drop"), Fault("none")])
    dropped = None
    try:
        injector3.call()
    except MessageDropped as e:
        dropped = e
    ok = injector3.call()
    check(
        "message_drop_then_recover",
        dropped is not None and ok == 1 and injector3.stats()["target_called"] == 1,
        "call 1 dropped (no reply, target unreached); call 2 recovered normally",
    )

    # ---- Test 4: crash_after — the target ran, the reply was lost ------
    # HAND CHECK: plan [none, crash_after, none] over a 3-call sequence:
    # the target IS called on call 2 (so 3 target calls total), the caller
    # sees WorkerCrashError with phase "after", and only 2 replies are
    # delivered.
    injector4 = FaultInjector(
        counting_toy2,
        [Fault("none"), Fault("crash_after", phase="after", detail="SIGKILL"), Fault("none")],
    )
    injector4.call()
    crash = None
    try:
        injector4.call()
    except WorkerCrashError as e:
        crash = e
    injector4.call()
    check(
        "crash_after_target_ran_reply_lost",
        crash is not None and crash.phase == "after"
        and injector4.stats()["target_called"] == 3
        and injector4.stats()["replies_delivered"] == 2,
        "call 2: target executed, reply lost; caller saw the named crash",
    )

    # ---- Test 5: crash_before — the target never ran -------------------
    injector5 = FaultInjector(
        counting_toy2,
        [Fault("crash_before", phase="before"), Fault("none")],
    )
    crash5 = None
    try:
        injector5.call()
    except WorkerCrashError as e:
        crash5 = e
    ok5 = injector5.call()
    check(
        "crash_before_target_never_ran",
        crash5 is not None and crash5.phase == "before" and ok5 == 1
        and injector5.stats()["target_called"] == 1,
        "call 1 crashed before serving (target unreached); call 2 served",
    )

    # ---- Test 6: scripted_reply — the honest stale re-serve shape ------
    # HAND CHECK: a canned reply {"value_mw": 2.0, "stale": True} served
    # at call 1 WITHOUT calling the target; the flag travels INSIDE the
    # reply (the injector never fabricates staleness).
    canned = {"value_mw": 2.0, "stale": True}
    injector6 = FaultInjector(counting_toy2, [Fault("scripted_reply", reply=canned), Fault("none")])
    served = injector6.call()
    normal = injector6.call()
    check(
        "scripted_reply_stale_flag_travels_verbatim",
        served is canned and served["stale"] is True and normal == 1
        and injector6.stats()["target_called"] == 1,
        "call 1 served the canned reply verbatim (target unreached); "
        "call 2 called the target",
    )

    # ---- Test 7: replay_last — a duplicated delivery, no re-execution --
    # HAND CHECK: plan [none, none, replay_last]: call 3 returns call 2's
    # reply (2) WITHOUT calling the target → target called exactly twice.
    injector7 = FaultInjector(counting_toy2, [Fault("none"), Fault("none"), Fault("replay_last")])
    r1 = injector7.call()
    r2 = injector7.call()
    r3 = injector7.call()
    check(
        "replay_last_duplicate_delivery_no_reexecution",
        (r1, r2, r3) == (1, 2, 2) and injector7.stats()["target_called"] == 2,
        "call 3 re-delivered call 2's reply (2) with no target call — a "
        "transport duplicate, not a re-execution",
    )

    # ---- Test 8: replay with no prior success is refused ---------------
    injector8 = FaultInjector(counting_toy2, [Fault("replay_last")])
    refused = None
    try:
        injector8.call()
    except ReplayUnavailable as e:
        refused = e
    check(
        "replay_without_prior_success_refused",
        refused is not None and injector8.stats()["target_called"] == 0,
        "a duplicate cannot precede the first message — named refusal, "
        "target unreached",
    )

    # ---- Test 9: reorder — scripted replies served in reverse ----------
    # HAND CHECK: scripted [r1, r2, r3] → delivery order r3, r2, r1; the
    # target is never called while the queue holds; a 4th call passes
    # through (target called once).
    injector9 = ReorderInjector(counting_toy2, ["r1", "r2", "r3"])
    d1 = injector9.call()
    d2 = injector9.call()
    d3 = injector9.call()
    d4 = injector9.call()
    check(
        "reorder_delivers_out_of_order_then_passes_through",
        (d1, d2, d3) == ("r3", "r2", "r1") and d4 == 1
        and injector9.stats()["target_called"] == 1,
        "out-of-order delivery r3, r2, r1; the 4th call reached the target",
    )

    # ---- Test 10: empty reorder plan refused ---------------------------
    def empty_reorder() -> None:
        ReorderInjector(counting_toy2, [])

    try:
        empty_reorder()
        refused10 = False
    except DrillConfigurationError:
        refused10 = True
    check(
        "empty_reorder_plan_refused",
        refused10,
        "an empty scripted list is a construction refusal (fail-closed)",
    )

    # ---- Test 11: clock skew — fixed offset, both directions -----------
    # HAND CHECK: base 1000 + 5000 = 6000; base 1000 - 250 = 750.
    base = lambda: 1000  # noqa: E731 — a fixed toy clock
    check(
        "clock_skew_offset_both_directions",
        skewed_clock(base, 5000)() == 6000 and skewed_clock(base, -250)() == 750,
        "offsets are added verbatim (ms), positive and negative",
    )

    # ---- Test 12: oversize — exactly one byte over the cap -------------
    # HAND CHECK: 1048576 + 1 = 1048577 characters.
    payload = oversized_payload()
    tiny = oversized_payload(3)
    check(
        "oversized_payload_exactly_cap_plus_one",
        len(payload) == 1048577 and len(tiny) == 4 and set(tiny) == {"x"},
        "the payload exceeds MAX_PROPOSAL_PAYLOAD_BYTES by exactly one byte; "
        "content inert",
    )

    # ---- Test 13: construction refusals (fail-closed plan validation) --
    def unknown_kind() -> None:
        Fault("explode")

    def error_without_instance() -> None:
        Fault("error", error="not an exception")

    def scripted_without_reply() -> None:
        Fault("scripted_reply")

    def crash_without_phase() -> None:
        Fault("crash_before")

    def non_callable_target() -> None:
        FaultInjector(42, [Fault("none")])

    def bad_phase() -> None:
        WorkerCrashError("sideways")

    refused13 = True
    for fn in (unknown_kind, error_without_instance, scripted_without_reply,
               crash_without_phase, non_callable_target, bad_phase):
        try:
            fn()
            refused13 = False
        except (DrillConfigurationError, ValueError):
            pass
    check(
        "construction_refusals_fail_closed",
        refused13,
        "unknown kind / missing payload / non-callable target / bad crash "
        "phase all refused at construction",
    )

    # ---- Test 14: default beyond the plan ------------------------------
    # HAND CHECK: plan of length 1, four calls → calls 2–4 use the
    # default (none) → target called 4 times, 1 fault injected.
    injector14 = FaultInjector(counting_toy2, [Fault("drop")])
    outcome14 = {"dropped": 0, "ok": 0}
    try:
        injector14.call()
    except MessageDropped:
        outcome14["dropped"] += 1
    for _ in range(3):
        injector14.call()
        outcome14["ok"] += 1
    check(
        "default_beyond_plan_is_pass_through",
        outcome14 == {"dropped": 1, "ok": 3}
        and injector14.stats()["target_called"] == 3,
        "call 1 dropped; calls 2–4 defaulted to pass-through (3 target calls)",
    )

    # ---- Test 15: run_drill_cell records honestly ----------------------
    def exploding_action() -> None:
        raise MessageDropped("boom")

    cell_ok = run_drill_cell("cell-ok", "returns 42", lambda: {"value": 42})
    cell_fault = run_drill_cell("cell-fault", "raises", exploding_action)
    summary = summarize([cell_ok, cell_fault])
    check(
        "drill_cell_records_faults_verbatim",
        cell_ok["completed"] is True and cell_ok["outcome"] == {"value": 42}
        and cell_fault["completed"] is False
        and "MessageDropped" in cell_fault["error"]
        and summary == {"cells": 2, "completed": 1, "errored": 1},
        "a fault is recorded as the outcome, never absorbed; the summary "
        "counts 2 cells / 1 completed / 1 errored",
    )

    # ---- Test 16: empty summary is honest zeros ------------------------
    check(
        "empty_summary_is_honest_zeros",
        summarize([]) == {"cells": 0, "completed": 0, "errored": 0},
        "an empty drill is zero cells — never a fabricated pass",
    )

    # ---- Test 17 (P12.09): partial_write truncates a sliceable reply ---
    # HAND CHECK: target returns b"PAYLOAD-BYTES" (13 bytes);
    # keep_bytes=7 → the caller receives b"PAYLOAD" (7 bytes); the target
    # WAS called (1 target call); the record carries fault_kind
    # "partial_write" and reply of length 7.
    def bytes_toy() -> bytes:
        return b"PAYLOAD-BYTES"

    injector17 = FaultInjector(bytes_toy, [Fault("partial_write", keep_bytes=7)])
    delivered17 = injector17.call()
    record17 = injector17.records[0]
    check(
        "partial_write_truncates_sliceable_reply",
        delivered17 == b"PAYLOAD" and record17.target_called is True
        and record17.fault_kind == "partial_write" and len(delivered17) == 7,
        "the target ran, the delivered reply is the first 7 of 13 bytes "
        "(a torn write, recorded — never silently completed)",
    )

    # ---- Test 18 (P12.09): partial_write against an unsliceable reply --
    # HAND CHECK: a dict reply is refused AT CALL TIME with
    # PartialWriteUnsupported, target_called=True recorded (the side
    # effect happened — the record must not vanish it).
    injector18 = FaultInjector(lambda: {"ok": True},
                               [Fault("partial_write", keep_bytes=3)])
    refused18 = None
    try:
        injector18.call()
    except PartialWriteUnsupported as e:
        refused18 = e
    check(
        "partial_write_unsliceable_reply_fails_closed",
        refused18 is not None and injector18.records[0].target_called is True,
        "an unsliceable reply is a named call-time refusal; the target "
        "call stays in the record (fail-closed, nothing guessed)",
    )

    # ---- Test 19 (P12.09): slow_consumer carries the delay as DATA -----
    # HAND CHECK: plan [none, slow_consumer(45_000), none] over the
    # counting target: replies [1, 2, 3]; the target is called 3 times;
    # record 1 (0-indexed) carries delay_ms = 45_000; the helper
    # classifies 45_000 > 30_000 as exceeding the supervision timeout and
    # 29_999 as not. NO sleeping happens anywhere.
    injector19 = FaultInjector(
        counting_toy2,
        [Fault("none"), Fault("slow_consumer", delay_ms=45_000), Fault("none")],
    )
    r19 = [injector19.call() for _ in range(3)]
    check(
        "slow_consumer_delay_travels_as_data",
        r19 == [1, 2, 3] and injector19.records[1].delay_ms == 45_000
        and injector19.records[0].delay_ms == 0
        and injector19.stats()["target_called"] == 3
        and exceeds_supervision_timeout(45_000) is True
        and exceeds_supervision_timeout(29_999) is False,
        "the delay is recorded data (45_000 ms > the 30_000 ms supervision "
        "timeout, 29_999 ms is not) — no sleeping, no clock read",
    )

    # ---- Test 20 (P12.09): partition window — closed, then healed -------
    # HAND CHECK: PartitionInjector(open_after=2, closed_for=2) over the
    # counting target: calls 0,1 pass (replies 1,2); calls 2,3 raise
    # NetworkPartitionError (target unreached, fault_kind
    # "network_partition"); call 4 passes (reply 3 — the target count is
    # 3 total).
    injector20 = PartitionInjector(counting_toy2, open_after_calls=2, closed_for_calls=2)
    replies20 = [injector20.call() for _ in range(2)]
    partition_errors = 0
    for _ in range(2):
        try:
            injector20.call()
        except NetworkPartitionError:
            partition_errors += 1
    healed20 = injector20.call()
    kinds20 = {r.fault_kind for r in injector20.records}
    check(
        "partition_window_closes_then_heals",
        replies20 == [1, 2] and partition_errors == 2 and healed20 == 3
        and injector20.stats()["target_called"] == 3
        and kinds20 == {"none", "network_partition"},
        "calls 2-3 are the closed seam (named error, target unreached); "
        "call 4 healed and reached the target",
    )

    # ---- Test 21 (P12.09): partition construction refusals --------------
    def bad_window_a() -> None:
        PartitionInjector(counting_toy2, open_after_calls=-1, closed_for_calls=1)

    def bad_window_b() -> None:
        PartitionInjector(counting_toy2, open_after_calls=0, closed_for_calls=0)

    refused21 = True
    for fn in (bad_window_a, bad_window_b):
        try:
            fn()
            refused21 = False
        except DrillConfigurationError:
            pass
    check(
        "partition_construction_refusals_fail_closed",
        refused21,
        "a negative open_after and a zero-length window are construction "
        "refusals (fail-closed)",
    )

    # ---- Test 22 (P12.09): scheduled skew — per-call, then plateau ------
    # HAND CHECK: base 1000, schedule [0, 500, -250]: reads 1000, 1500,
    # 750; the 4th read holds the LAST offset: 1000 + (-250) = 750.
    reads = []
    clock22 = scheduled_skew_clock(lambda: 1000, [0, 500, -250])
    for _ in range(4):
        reads.append(clock22())
    check(
        "scheduled_skew_per_call_then_plateaus",
        reads == [1000, 1500, 750, 750],
        "per-call offsets 0/500/-250, then the last offset persists "
        "(the documented plateau choice)",
    )

    # ---- Test 23 (P12.09): empty skew schedule refused ------------------
    def empty_schedule() -> None:
        scheduled_skew_clock(lambda: 1000, [])

    refused23 = False
    try:
        empty_schedule()
    except DrillConfigurationError:
        refused23 = True
    check(
        "empty_skew_schedule_refused",
        refused23,
        "an empty schedule is a construction refusal (fail-closed)",
    )

    # ---- Test 24 (P12.09): partial_write / slow_consumer construction ---
    def negative_keep() -> None:
        Fault("partial_write", keep_bytes=-1)

    def missing_keep() -> None:
        Fault("partial_write")

    def negative_delay() -> None:
        Fault("slow_consumer", delay_ms=-5)

    def missing_delay() -> None:
        Fault("slow_consumer")

    refused24 = True
    for fn in (negative_keep, missing_keep, negative_delay, missing_delay):
        try:
            fn()
            refused24 = False
        except DrillConfigurationError:
            pass
    check(
        "partial_write_and_slow_consumer_refusals_fail_closed",
        refused24,
        "missing or negative keep_bytes / delay_ms are construction "
        "refusals (fail-closed)",
    )

    return results


if __name__ == "__main__":  # pragma: no cover — human-gate entrypoint
    _records = run_self_tests()
    _failures = [r for r in _records if not r["passed"]]
    for _r in _records:
        print(("PASS" if _r["passed"] else "FAIL"), _r["name"], "-", _r["detail"])
    print("%d/%d self-tests passed" % (len(_records) - len(_failures), len(_records)))
    raise SystemExit(1 if _failures else 0)
