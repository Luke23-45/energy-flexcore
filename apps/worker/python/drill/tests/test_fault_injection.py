# P8.24 — pytest mirror of the fault-injection scaffolding self-tests.
#
# HONESTY HEADER: this suite mirrors, one-for-one, the hand-computed
# self-tests inside `apps/worker/python/drill/fault_injection.py`
# (`run_self_tests`). Every expectation is derived on paper from the
# constructed inputs. This suite was WRITTEN, NEVER RUN (the loop bans;
# ENTRY_NOTE.md) — running it is a human-gate input for the P8.24 drill:
#   python -m pytest apps/worker/python/drill/tests/test_fault_injection.py -v

import sys
from pathlib import Path

import pytest

# Import the scaffolding module under pytest with NOTHING installed (the
# scaffolding is pure stdlib; the path append is the same pattern the
# other worker suites use for their sibling modules).
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import fault_injection as fi  # noqa: E402


@pytest.fixture()
def counting_toy():
    """A toy target: counts its own calls, returns call_count * 2 (MW-
    shaped toy numbers are NOT used here — the toy reply is unit-free)."""
    state = {"n": 0}

    def target():
        state["n"] += 1
        return state["n"] * 2

    return target


def test_none_plan_passes_through(counting_toy):
    injector = fi.FaultInjector(counting_toy, [fi.Fault("none")] * 3)
    replies = [injector.call() for _ in range(3)]
    assert replies == [2, 4, 6]
    assert injector.stats() == {
        "calls": 3,
        "target_called": 3,
        "faults_injected": 0,
        "errors_delivered": 0,
        "replies_delivered": 3,
    }


def test_injected_error_raised_target_skipped():
    state = {"n": 0}

    def target():
        state["n"] += 1
        return state["n"]

    injected = RuntimeError("worker down")
    injector = fi.FaultInjector(
        target,
        [fi.Fault("none"), fi.Fault("error", error=injected), fi.Fault("none")],
    )
    assert injector.call() == 1
    with pytest.raises(RuntimeError) as excinfo:
        injector.call()
    assert excinfo.value is injected
    assert injector.call() == 2
    assert injector.stats()["target_called"] == 2
    assert injector.stats()["faults_injected"] == 1


def test_message_drop_then_recover():
    state = {"n": 0}

    def target():
        state["n"] += 1
        return state["n"]

    injector = fi.FaultInjector(target, [fi.Fault("drop"), fi.Fault("none")])
    with pytest.raises(fi.MessageDropped):
        injector.call()
    assert injector.call() == 1
    assert injector.stats()["target_called"] == 1


def test_crash_after_target_ran_reply_lost():
    state = {"n": 0}

    def target():
        state["n"] += 1
        return state["n"]

    injector = fi.FaultInjector(
        target,
        [fi.Fault("none"), fi.Fault("crash_after", phase="after", detail="SIGKILL"), fi.Fault("none")],
    )
    injector.call()
    with pytest.raises(fi.WorkerCrashError) as excinfo:
        injector.call()
    assert excinfo.value.phase == "after"
    injector.call()
    assert injector.stats()["target_called"] == 3
    assert injector.stats()["replies_delivered"] == 2


def test_crash_before_target_never_ran():
    state = {"n": 0}

    def target():
        state["n"] += 1
        return state["n"]

    injector = fi.FaultInjector(target, [fi.Fault("crash_before", phase="before"), fi.Fault("none")])
    with pytest.raises(fi.WorkerCrashError) as excinfo:
        injector.call()
    assert excinfo.value.phase == "before"
    assert injector.call() == 1
    assert injector.stats()["target_called"] == 1


def test_scripted_reply_stale_flag_travels_verbatim(counting_toy):
    canned = {"value_mw": 2.0, "stale": True}
    injector = fi.FaultInjector(counting_toy, [fi.Fault("scripted_reply", reply=canned), fi.Fault("none")])
    served = injector.call()
    assert served is canned
    assert served["stale"] is True
    assert injector.call() == 2
    assert injector.stats()["target_called"] == 1


def test_replay_last_duplicate_delivery_no_reexecution(counting_toy):
    injector = fi.FaultInjector(counting_toy, [fi.Fault("none"), fi.Fault("none"), fi.Fault("replay_last")])
    r1 = injector.call()
    r2 = injector.call()
    r3 = injector.call()
    assert (r1, r2, r3) == (2, 4, 4)
    assert injector.stats()["target_called"] == 2


def test_replay_without_prior_success_refused(counting_toy):
    injector = fi.FaultInjector(counting_toy, [fi.Fault("replay_last")])
    with pytest.raises(fi.ReplayUnavailable):
        injector.call()
    assert injector.stats()["target_called"] == 0


def test_reorder_delivers_out_of_order_then_passes_through(counting_toy):
    injector = fi.ReorderInjector(counting_toy, ["r1", "r2", "r3"])
    d1 = injector.call()
    d2 = injector.call()
    d3 = injector.call()
    d4 = injector.call()
    assert (d1, d2, d3) == ("r3", "r2", "r1")
    assert d4 == 2
    assert injector.stats()["target_called"] == 1


def test_empty_reorder_plan_refused(counting_toy):
    with pytest.raises(fi.DrillConfigurationError):
        fi.ReorderInjector(counting_toy, [])


def test_clock_skew_offset_both_directions():
    base = lambda: 1000  # noqa: E731 — a fixed toy clock
    assert fi.skewed_clock(base, 5000)() == 6000
    assert fi.skewed_clock(base, -250)() == 750


def test_oversized_payload_exactly_cap_plus_one():
    assert len(fi.oversized_payload()) == 1048577
    assert len(fi.oversized_payload(3)) == 4
    assert set(fi.oversized_payload(3)) == {"x"}


def test_oversized_payload_negative_cap_refused():
    with pytest.raises(fi.DrillConfigurationError):
        fi.oversized_payload(-1)


@pytest.mark.parametrize(
    "offender",
    [
        lambda: fi.Fault("explode"),
        lambda: fi.Fault("error", error="not an exception"),
        lambda: fi.Fault("scripted_reply"),
        lambda: fi.Fault("crash_before"),
        lambda: fi.FaultInjector(42, [fi.Fault("none")]),
        lambda: fi.WorkerCrashError("sideways"),
    ],
    ids=[
        "unknown_kind",
        "error_without_instance",
        "scripted_without_reply",
        "crash_without_phase",
        "non_callable_target",
        "bad_crash_phase",
    ],
)
def test_construction_refusals_fail_closed(offender):
    with pytest.raises((fi.DrillConfigurationError, ValueError)):
        offender()


def test_default_beyond_plan_is_pass_through():
    state = {"n": 0}

    def target():
        state["n"] += 1
        return state["n"]

    injector = fi.FaultInjector(target, [fi.Fault("drop")])
    with pytest.raises(fi.MessageDropped):
        injector.call()
    for _ in range(3):
        assert injector.call() == state["n"]
    assert injector.stats()["target_called"] == 3


def test_drill_cell_records_faults_verbatim():
    def exploding():
        raise fi.MessageDropped("boom")

    cell_ok = fi.run_drill_cell("cell-ok", "returns 42", lambda: {"value": 42})
    cell_fault = fi.run_drill_cell("cell-fault", "raises", exploding)
    assert cell_ok["completed"] is True
    assert cell_ok["outcome"] == {"value": 42}
    assert cell_fault["completed"] is False
    assert "MessageDropped" in cell_fault["error"]
    assert fi.summarize([cell_ok, cell_fault]) == {"cells": 2, "completed": 1, "errored": 1}


def test_empty_summary_is_honest_zeros():
    assert fi.summarize([]) == {"cells": 0, "completed": 0, "errored": 0}
