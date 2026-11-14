# P5.16 — Shared plumbing for the HELICS co-simulation federate scripts
#         (config loading, fail-closed helpers, exit-report emission).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER (read before trusting anything below)
#
# 1. THIS MODULE WAS WRITTEN, NEVER EXECUTED. The Phase 5 loop bans
#    install/build/run; helics==3.7.0 (see requirements-helics.txt) was
#    NEVER installed and NOTHING in this directory was ever run. Every
#    helics API call in these scripts follows the DOCUMENTED 3.7.0 API as
#    of the 2026-11-07 research cut-off and is **UNVERIFIED** until the
#    human's build/test pass executes the federation against the pinned
#    version. Nothing here is evidence that any co-simulation ever ran.
# 2. HELICS COORDINATES; IT DOES NOT REPLACE PHYSICS (report §12.4, §19.1):
#    HELICS is the federation's time coordinator and message bus. The
#    physics stay in the EXISTING models — the Phase 2 asset simulator's
#    behaviour profiles and the Phase 4 pandapower AC power-flow pipeline
#    (driven through the UNMODIFIED P5.15 grid_worker.py protocol). No
#    federate script in this directory computes any physics itself, and no
#    completed co-simulation run is a power-system validation by itself.
# 3. FAIL-CLOSED (the P4.08 policy this pattern inherits from
#    grid_worker.py): a missing helics binding, a malformed config, a
#    protocol mismatch, or a federate-level error is answered with
#    NOTHING meaningful on stdout + a reason on stderr + a non-zero exit.
#    The Rust orchestrator (`apps/sim-runner/src/helics_orchestration.rs`)
#    records every such failure as a fail-closed outcome; there is NO path
#    from a federate failure to a successful co-simulation record.
# 4. TRANSPORT CONTRACT: on SUCCESS each federate prints ONE JSON exit
#    report document (protocol `flexcore-helics-federate/1`) on stdout —
#    the document the Rust orchestrator parses and validates (protocol
#    tag, federate name, ok flag, finiteness). A failed federate prints
#    no report at all (fail-closed), never a false one.
# ══════════════════════════════════════════════════════════════════════════
#
# helics API usage table (DOCUMENTED 3.7.0 forms; every row UNVERIFIED —
# VERIFY-BUILD items, checked at the human's first federation run):
#
# | Purpose                      | Documented call (as written below)          |
# |------------------------------|---------------------------------------------|
# | binding version probe        | `helicsGetVersion()` (read defensively)     |
# | broker creation              | `helicsCreateBroker(key, init_string)`      |
# | broker connectivity probe    | `helicsBrokerIsConnected(broker)`           |
# | broker teardown              | `helicsBrokerDisconnect(broker)` / `helicsBrokerFree` / `helicsCloseLibrary` |
# | value federate creation      | `helicsCreateValueFederate(name)`           |
# | core init (broker address)   | `helicsFederateSetCoreInitString(fed, "--broker=<address>")` — recorded ASSUMPTION: the zmq core accepts `--broker=` in its init string |
# | pinned period                | `helicsFederateSetTimeProperty(fed, HELICS_PROPERTY_TIME_PERIOD, seconds)` |
# | publication (double + unit)  | `helicsFederateRegisterPublication(fed, topic, HELICS_DATA_TYPE_DOUBLE, unit)` |
# | subscription                 | `helicsFederateRegisterSubscription(fed, topic, unit)` |
# | execution mode               | `helicsFederateEnterExecutingMode(fed)`     |
# | time request                 | `helicsFederateRequestTime(fed, requested)` — returns the granted time |
# | publish                      | `helicsPublicationPublishDouble(pub, value)` |
# | read                         | `helicsInputGetDouble(sub)`                 |
# | teardown                     | `helicsFederateDisconnect(fed)` / `helicsFederateFree(fed)` / `helicsCloseLibrary()` |
#
# Everything this federation produces is simulation-only (the 0.x line,
# §21.4): no field read, no field write, no control authority, no safety
# or certification implication, and no transient-stability claim (a
# steady-state tool is not a stability tool, §19.1).

from __future__ import annotations

import json
import sys

# The federation-config document tag. The Rust orchestrator
# (`helics_orchestration.rs` HELICS_FEDERATION_PROTOCOL_ID) validates this
# tag BEFORE any process is launched; the scripts re-check it fail-closed.
FEDERATION_PROTOCOL_ID = "flexcore-helics-federation/1"

# The exit-report document tag (`helics_orchestration.rs`
# HELICS_FEDERATE_REPORT_PROTOCOL_ID).
FEDERATE_REPORT_PROTOCOL_ID = "flexcore-helics-federate/1"

REPORT_KIND_FEDERATE = "federate"
REPORT_KIND_BROKER = "broker"

# The recorded version pin (requirements-helics.txt; RESEARCH.md §1). The
# scripts report the INSTALLED binding version; they never refuse a
# mismatch (the pin discipline lives with the orchestrator/adapter, as in
# grid_worker.py).
PINNED_HELICS_VERSION = "3.7.0"

# The canonical unit vocabulary a publication/subscription may declare
# (the mirror of `helics_orchestration.rs`'s allowed-unit set; MW, MVAr,
# MVA, MWh per §10.2; voltage pu in solver representation; A on an
# explicit base is declared per grid model).
CANONICAL_UNITS = ("MW", "MVAr", "MVA", "MWh", "kV", "A", "pu", "percent")

# The role statement carried in every exit report's status text, so no
# run record can lose it (§12.4, §19.1).
HELICS_ROLE_STATEMENT = (
    "HELICS coordinates the federation's time steps and value exchange; "
    "it does NOT replace the federates' physics models (report 12.4, 19.1). "
    "The physics stay in the existing models: the Phase 2 asset simulator "
    "profiles and the Phase 4 pandapower AC power-flow pipeline."
)

HELICS_UNVERIFIED_NOTE = (
    "UNVERIFIED-API federate: helics==3.7.0 was never installed in this "
    "loop; the script follows the documented 3.7.0 API and was never "
    "executed. First execution belongs to the human build/test pass."
)


def fail(message: str) -> None:
    """Answers a failure FAIL-CLOSED: a reason on stderr, NOTHING on
    stdout (no partial, no fabricated exit report), non-zero exit. The
    Rust orchestrator maps this to a fail-closed outcome — never a
    best-effort report."""
    sys.stderr.write("flexcore-helics-federate: " + message + "\n")
    sys.exit(2)


def load_config(path: str) -> dict:
    """Loads and pre-validates the federation config document (the SAME
    document the Rust orchestrator validated; the scripts re-check the
    parts they depend on, fail-closed)."""
    try:
        with open(path, "r", encoding="utf-8") as handle:
            config = json.load(handle)
    except OSError as error:
        fail("the federation config could not be read: %s" % error)
        return {}  # unreachable; keeps type-checkers honest
    except json.JSONDecodeError as error:
        fail("the federation config is not a valid JSON document: %s" % error)
        return {}
    if config.get("protocol") != FEDERATION_PROTOCOL_ID:
        fail(
            "federation config protocol %r is not %r"
            % (config.get("protocol"), FEDERATION_PROTOCOL_ID)
        )
    return config


def section_for(config: dict, federate_name: str) -> dict:
    """Returns THIS federate's section of the config, fail-closed."""
    for entry in config.get("federates", []):
        if entry.get("name") == federate_name:
            return entry
    fail("no federate named %r in the federation config" % federate_name)
    return {}  # unreachable


def import_helics():
    """Imports the helics binding or fails closed with the honest
    message. This is the gate that keeps the loop honest: WITHOUT the
    installed pinned binding these scripts can NEVER produce an exit
    report, so a co-simulation record can never exist without helics
    having actually run."""
    try:
        import helics as h  # noqa: PLC0415 — deferred: a missing binding is a fail-closed startup answer
    except ImportError as error:
        fail(
            "the helics Python binding is not installed (%s). helics==%s is the recorded "
            "pin (requirements-helics.txt; NEVER installed in this loop). No co-simulation "
            "can run without it — fail-closed, never a simulated federation."
            % (error, PINNED_HELICS_VERSION)
        )
        return None  # unreachable
    return h


def helics_version(h) -> str | None:
    """The installed binding's version, read defensively (the probe must
    never crash the report; UNVERIFIED surface)."""
    try:
        getter = getattr(h, "helicsGetVersion", None)
        if getter is None:
            return None
        return str(getter())
    except Exception:  # noqa: BLE001 — the version probe must never crash the worker
        return None


def emit_report(report: dict) -> None:
    """Prints the ONE exit-report document (one line of JSON) on stdout —
    the transport contract with the Rust orchestrator."""
    sys.stdout.write(json.dumps(report))
    sys.stdout.write("\n")
    sys.stdout.flush()


def federate_report(
    *,
    federation: str,
    federate: str,
    role: str,
    granted_steps: int,
    final_granted_time_seconds: float,
    published_values: int,
    received_values: int,
    skipped_publications: int,
    helics_version_reported: str | None,
) -> dict:
    """Builds the successful federate exit report (ok: true is asserted
    ONLY by a federate that actually completed its pinned time-step loop
    against a real broker — the fail-closed paths above never reach
    here)."""
    return {
        "protocol": FEDERATE_REPORT_PROTOCOL_ID,
        "kind": REPORT_KIND_FEDERATE,
        "federation": federation,
        "federate": federate,
        "role": role,
        "granted_steps": granted_steps,
        "final_granted_time_seconds": final_granted_time_seconds,
        "published_values": published_values,
        "received_values": received_values,
        "skipped_publications": skipped_publications,
        "ok": True,
        "status_text": "%s %s" % (HELICS_ROLE_STATEMENT, HELICS_UNVERIFIED_NOTE),
        "helics_version_reported": helics_version_reported,
    }
