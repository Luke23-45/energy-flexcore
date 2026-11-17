# P11.02 — SLO register and error-budget policy

| Field | Value |
|---|---|
| Subtask | P11.02 (SLO register + error-budget policy) |
| Wave / agent | Phase 11 wave 1, Agent A |
| Status | WRITTEN RECOMMENDATIONS ONLY: every target below is a RECOMMENDATION pending operator sign-off. No observed latency, traffic, or error distribution exists anywhere in this program (the base is WRITTEN-UNRUN); every threshold is registered BEFORE any run that could fill it (X-8 pre-registration, conventions §5). |
| Keyed to | The P11.01 metric taxonomy (`records/01_p11_01_metric_taxonomy.md`) — no SLO below references a family the taxonomy does not name. |
| Sources | [S-61] (Google SRE Workbook, alerting on SLOs — SLI/SLO definitions, error budgets, burn-rate alerting, low-traffic synthetic-traffic guidance) |

## 1. The register

SLI definitions are ratios over the P11.01 families. A "good" event is
defined per SLI; the measurement window is 28 days [S-61] (rolling
four-week windows are the workbook's shape; the window choice is part of
the same RECOMMENDATION label).

| # | Service | SLI | Good-event definition (threshold = RECOMMENDATION) | Target (RECOMMENDATION) | Family (P11.01) |
|---|---|---|---|---|---|
| 1 | `control-plane-api` | Availability | Served requests with status < 500 / all served requests | 99.9% over 28 days | `flexcore_http_requests_total{status}`, `flexcore_http_errors_total` |
| 2 | `control-plane-api` | Latency | Requests with duration ≤ 1000 ms / all served requests | 99.0% over 28 days | `flexcore_api_request_duration_milliseconds` (bucket `le="1000"`) |
| 3 | `command-lifecycle` | Accept latency | Command submissions accepted (PROPOSED committed) within 500 ms / all accepted | 99.0% over 28 days | `flexcore_command_accept_duration_milliseconds` (bucket `le="500"`) |
| 4 | `command-lifecycle` | Terminal-state ratio | Commands reaching a terminal state / commands reaching `APPROVED` (the last gate a command cannot bypass) | 99.0% over 28 days; the terminal-state DISTRIBUTION (COMPLETED vs REJECTED vs SUPERSEDED vs CANCELLED vs FALLBACK) is recorded, not targeted | `flexcore_command_state_transitions_total{from,to}` (terminal `to` values: COMPLETED, REJECTED, CANCELLED, SUPERSEDED, FALLBACK) |
| 5 | `command-lifecycle` | Stuck-state count | Commands parked in a non-terminal state beyond the registered stuck threshold | ZERO tolerated above the registered threshold; the threshold itself is registered by P11.20 (wave 2) before the gauge can fill | `flexcore_commands_stuck` (named in P11.01, implemented wave 2) |

Not in the register: the P2.21 telemetry families (queue depth, delay
summary, quarantine) — they are operational health signals with their own
P11.03 alert treatment, not user-facing-service SLIs. Inventing an SLI
for them would put a target on a signal that has never been observed.

## 2. Error-budget policy (what happens at exhaustion)

The error budget of an SLO is 1 − target [S-61]: e.g. SLO 1 tolerates
0.1% of 28-day availability miss. The policy below is a WRITTEN
RECOMMENDATION pending operator sign-off (it constrains future work
prioritization — an operator decision, never an agent one):

1. **Budget computation.** Burn rate = observed bad-event rate ÷
   tolerated bad-event rate, computed over the multi-window pairs of the
   P11.03 alert rules (14.4× / 1h+5m page, 6× / 6h+30m page, 1× / 3d
   ticket) [S-61]. The rules are B's checked-in files; this register is
   what they burn against.
2. **While the budget is unspent** (burn rate < 1 over the 28-day
   window): feature and hardening work proceeds normally.
3. **Budget under burn** (a page or ticket fires): the on-call response
   is the operator runbook — "who is paged" is an operator answer (the
   P11-E-3 DEFAULT routes alerts; it stays OPEN).
4. **Budget exhausted** (the 28-day budget is spent): the RECOMMENDED
   policy is that reliability work takes priority over feature work for
   the affected service until the burn rate is back under 1 — this is
   the [S-61] posture. It binds FUTURE program planning only after an
   operator signs it; nothing in this repository enforces it.
5. **No SLO gaming.** Thresholds (1000 ms API latency, 500 ms accept
   latency) are registered here BEFORE any run; observed values live
   structurally separate (conventions §5 / X-8). No threshold may be
   edited in the same commit that observes it.

## 3. Low-traffic services and synthetic traffic

[S-61] is explicit that SLO alerting on low-traffic services is
unreliable: with few events, either the windows rarely fill or the burn
rate is noise. The `command-lifecycle` service is exactly this case in
the 0.x simulation-only program (commands exist only when a simulation
drives them). The recorded approach, per [S-61]:

- Services whose event rate is too low to fill the P11.03 windows get
  SYNTHETIC TRAFFIC: a scheduled probe workload (e.g. a periodic
  simulated command submission and a periodic API request) that keeps
  the SLI denominators moving. The synthetic workload is a wave-2+ /
  operator decision — it runs in the simulation-only environment, adds
  no field authority, and is recorded here as the REGISTERED INTENT,
  not implemented in wave 1 (no traffic generator exists in this
  workspace; the honest status is that SLO 3 and SLO 5 cannot be
  measured at all until either real or synthetic traffic exists).
- The `control-plane-api` service in the Compose dev stack has the same
  problem at development scale; its SLOs are recommendations for the
  OPERATED deployment, not for a developer laptop.

## 4. Honesty boundary

- No target below has ever been measured against anything. The first
  human build/run is the gate; until then this register is a set of
  written recommendations with pre-registered thresholds.
- [S-61] is cited for the SLO/error-budget/burn-rate/synthetic-traffic
  concepts; it makes no claim about FlexCore.
