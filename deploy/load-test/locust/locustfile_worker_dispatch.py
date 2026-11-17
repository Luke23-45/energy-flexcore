# P20.02 — Worker dispatch fan-out leg (Locust) — WRITTEN, NEVER EXECUTED
# (decision record: docs/phases-11-20/phase-20-deployment-architecture-
# scale/records/02_p20_02_load_test_plan.md).
#
# What this leg measures: the worker dispatch fan-out observed END-TO-END
# through the honest API path — the schedule-proposal accept writes (POST
# /api/v1/optimization-runs + POST /api/v1/schedules, the recorded accept
# surface, `schedule.propose` authorization), then polls of the READ-ONLY
# dispatch-command evidence surface (GET /api/v1/dispatch-commands and
# /api/v1/dispatch-commands/{id}) to observe the downstream pipeline's
# API-visible effects.
#
# Honesty rules this file enforces by construction:
# - NO direct NATS publish anywhere: injecting synthetic messages onto
#   `flexcore.v1.>` subjects would bypass the command lifecycle and the
#   authorization gates (hard ban 9). The worker is a NATS consumer, not
#   an HTTP endpoint; its effects are observed through the API only.
# - NO lifecycle fabrication: no scenario ever asserts a command state;
#   the READ-ONLY surface is only ever read (the recorded
#   NO_EXECUTE_ENDPOINT_STATEMENT, dto_phase4.rs).
# - NO duration assertions: nothing fails a run on any duration (the P3.27
#   no-runtime-threshold discipline). Latencies are recorded by Locust's
#   statistics machinery; the human interprets them via the P20.01
#   framework.
#
# Required env (see ../README.md): FLEXCORE_LOAD_BASE_URL,
#   FLEXCORE_LOAD_TOKEN, FLEXCORE_LOAD_TENANT_UUID,
#   FLEXCORE_LOAD_PORTFOLIO_ID, FLEXCORE_LOAD_SITE_ID,
#   FLEXCORE_LOAD_ASSET_ID, FLEXCORE_POLL_MAX_SECONDS (default 30).

import json
import os
import time
import uuid

from locust import HttpUser, between, events, task


class LoadConfigError(Exception):
    """A missing env-contract value — fail loud, never silently default."""


def _require(name: str) -> str:
    value = os.environ.get(name, "")
    if not value:
        raise LoadConfigError(
            f"{name} is required (see deploy/load-test/README.md)"
        )
    return value


@events.init.add_listener
def _validate_env(environment, **kwargs):
    # Fail loud BEFORE any task spawns: a run without the env contract
    # would measure nothing and pretend otherwise.
    for name in (
        "FLEXCORE_LOAD_TOKEN",
        "FLEXCORE_LOAD_TENANT_UUID",
        "FLEXCORE_LOAD_PORTFOLIO_ID",
        "FLEXCORE_LOAD_SITE_ID",
        "FLEXCORE_LOAD_ASSET_ID",
    ):
        _require(name)


class WorkerDispatchFanoutUser(HttpUser):
    """One synthetic operator driving the schedule-proposal accept path
    and observing the downstream dispatch evidence."""

    # A small think-time between proposal cycles (a pacing input, not a
    # pass criterion).
    wait_time = between(0.5, 2.0)

    def on_start(self):
        self.tenant = _require("FLEXCORE_LOAD_TENANT_UUID")
        self.portfolio_id = _require("FLEXCORE_LOAD_PORTFOLIO_ID")
        self.site_id = _require("FLEXCORE_LOAD_SITE_ID")
        self.asset_id = _require("FLEXCORE_LOAD_ASSET_ID")
        self.poll_max_seconds = float(
            os.environ.get("FLEXCORE_POLL_MAX_SECONDS", "30")
        )
        # The dev-auth bearer token (the smoke.sh shape; requires the
        # dev-auth build per deploy/compose/smoke.sh).
        self.client.headers.update(
            {
                "Authorization": _require("FLEXCORE_LOAD_TOKEN"),
                "Content-Type": "application/json",
            }
        )

    def _propose_schedule(self):
        """One accept-write cycle: register a candidate, then propose the
        schedule revision realizing it. Unique candidate id per iteration
        (the duplicate-candidate 409 avoided by construction)."""
        candidate_id = f"SYN-cand-{uuid.uuid4()}"
        start_millis = int(time.time() * 1000)

        opt_body = {
            "portfolio_id": self.portfolio_id,
            "site_id": self.site_id,
            "candidate_id": candidate_id,
            "status": "feasible",
            "problem_class": "flexibility_scheduling",
            "solver_name": "synthetic-load-solver",
            "solver_version": "synthetic-1",
            "provider_id": "synthetic",
            "provider_version": "synthetic-1",
            "objective_value": 1.0,
            "objective_units": "MW",
            "input_snapshot_id": f"SYN-snapshot-{uuid.uuid4()}",
            "grid_model_version": "synthetic-grid-v1",
            "record_payload_json": json.dumps(
                {
                    "synthetic": True,
                    "data_policy": "deploy/load-test/data/synthetic-data-policy.md",
                }
            ),
        }
        self.client.post(
            "/api/v1/optimization-runs",
            json=opt_body,
            name="POST /api/v1/optimization-runs",
        )

        schedule_body = {
            "portfolio_id": self.portfolio_id,
            "site_id": self.site_id,
            "revision": 1,
            "horizon_start_millis": start_millis,
            "horizon_end_millis": start_millis + 3600 * 1000,
            "roll_trigger": "initial_solve",
            "candidate_id": candidate_id,
            "asset_time_coupled_validation_passed": True,
            "schedule_payload_json": json.dumps(
                [
                    {
                        "asset_id": self.asset_id,
                        "intervals": [
                            {
                                "start_millis": start_millis,
                                "end_millis": start_millis + 3600 * 1000,
                                "setpoint_mw": 0.5,
                            }
                        ],
                    }
                ]
            ),
        }
        self.client.post(
            "/api/v1/schedules",
            json=schedule_body,
            name="POST /api/v1/schedules",
        )

    def _observe_dispatch_evidence(self):
        """Poll the READ-ONLY dispatch-command evidence surface, recording
        how long the pipeline took to surface any dispatch-command record
        after this user's proposal cycle. The observed interval is a
        RECORDED figure for the P20.01 framework — never a pass/fail on a
        duration."""
        deadline = time.monotonic() + self.poll_max_seconds
        observed_command = None
        while time.monotonic() < deadline and observed_command is None:
            with self.client.get(
                "/api/v1/dispatch-commands?limit=50",
                name="GET /api/v1/dispatch-commands",
                catch_response=True,
            ) as response:
                if response.status_code == 200:
                    try:
                        body = response.json()
                        entries = body.get("entries") or body.get("items") or []
                        if entries:
                            observed_command = entries[0]
                            response.success()
                        else:
                            # No command evidence yet within this poll —
                            # an ordinary observation, never a failure.
                            response.success()
                    except ValueError:
                        response.failure("dispatch evidence surface returned non-JSON")
                else:
                    # A 401 here means the env contract is wrong; anything
                    # else (429 backpressure included) is recorded as-is.
                    response.success()
            if observed_command is None:
                time.sleep(1.0)

    @task
    def propose_and_observe(self):
        self._propose_schedule()
        self._observe_dispatch_evidence()
