# P7.17 — The worker health state machine: STARTING -> READY -> DRAINING
#         -> STOPPING, with FAILED reported by the supervisor (P7.18).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. STDLIB-ONLY, genuinely importable/runnable today. No grpcio anywhere.
# 2. Lifecycle vocabulary consistency seam (recorded): the state NAMES
#    (STARTING/READY/DRAINING/STOPPING, plus FAILED as reported by the
#    supervisor) mirror `crates/flexcore-kernel/src/lifecycle.rs`
#    `LifecycleState` (READY, DRAINING, ...) so the same concept never
#    carries two names across the platform. This module does NOT claim to
#    implement the kernel state machine — it is the worker-side health
#    view that the P7.18 supervisor probes and maps.
# 3. Transition rules are CLOSED: only the listed transitions succeed;
#    anything else raises `IllegalHealthTransition`. There is no
#    DRAINING -> READY recovery here: once a worker drains, it is on its
#    way out (the supervisor's recovery path is a supervisor decision,
#    recorded in its policy — P7.18).
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

# The worker lifecycle states (proto `WorkerHealthState` names).
HEALTH_STATES: tuple[str, ...] = ("STARTING", "READY", "DRAINING", "STOPPING")

# The closed transition table. Keys are (from, to); anything not listed
# is illegal. STARTING may fail startup by going straight to STOPPING.
LEGAL_TRANSITIONS: frozenset[tuple[str, str]] = frozenset(
    {
        ("STARTING", "READY"),  # initialization done; accept work.
        ("STARTING", "STOPPING"),  # startup failed; exit cleanly.
        ("READY", "DRAINING"),  # shutdown requested; finish in-flight work.
        ("READY", "STOPPING"),  # immediate stop (no in-flight work to finish).
        ("DRAINING", "STOPPING"),  # in-flight work finished (or grace elapsed).
    }
)


class IllegalHealthTransition(Exception):
    """An attempt to make a transition outside the closed table.

    Carries the offending (from, to) pair; the worker answers such an
    attempt with a structured error rather than corrupting its state.
    """


class HealthStateMachine:
    """The worker's own lifecycle state machine.

    Not thread-safe by itself: the gRPC server seam (server.py) guards
    it behind its own concurrency mechanism. The pure transition logic
    lives here so tests exercise it without a server.
    """

    def __init__(self) -> None:
        # Every worker begins in STARTING: it is not ready until it says so.
        self._state: str = "STARTING"

    @property
    def state(self) -> str:
        """The current state (one of `HEALTH_STATES`)."""
        return self._state

    def is_ready(self) -> bool:
        """Readiness: True only in READY (the probe's `ready` field)."""
        return self._state == "READY"

    def accepts_work(self) -> bool:
        """Whether a new Execute request may start now.

        Only READY accepts work. DRAINING/STOPPING refuse with the
        structured UNAVAILABLE error (errors.py); STARTING refuses
        because initialization is not complete.
        """
        return self._state == "READY"

    def can_transition(self, to_state: str) -> bool:
        """Whether (current, `to_state`) is a legal transition."""
        if to_state not in HEALTH_STATES:
            return False
        return (self._state, to_state) in LEGAL_TRANSITIONS

    def transition(self, to_state: str) -> str:
        """Apply a transition and return the new state.

        Raises `IllegalHealthTransition` for any transition outside the
        closed table — fail-closed, never a silent no-op.
        """
        if to_state not in HEALTH_STATES:
            raise IllegalHealthTransition(
                f"unknown health state {to_state!r}; known: {HEALTH_STATES}"
            )
        if not self.can_transition(to_state):
            raise IllegalHealthTransition(
                f"illegal health transition {_state_name(self._state)} -> "
                f"{to_state}; legal transitions: {sorted(LEGAL_TRANSITIONS)}"
            )
        self._state = to_state
        return self._state


def _state_name(state: str) -> str:
    """Human-safe state name (identity; kept for message stability)."""
    return state


def readiness_response(
    machine: HealthStateMachine,
    contract_version: str,
    detail: str = "",
) -> dict[str, object]:
    """The pure shape of a Check response (mirrors the proto
    `HealthCheckResponse`). The server seam fills the wire fields from
    this dict; the logic (ready only in READY) is testable here.
    """
    state = machine.state
    return {
        "contract_version": contract_version,
        "state": state,
        "ready": state == "READY",
        "detail": detail,
    }
