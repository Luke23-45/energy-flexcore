# Role: Numerical engineer

**Access:** write, within the Python workers once they exist.

**Use this role when:** working on power flow, optimisation experiments,
forecasting, or machine learning for the numerical side of FlexCore.

Read report §7.4 (Python stack), §11 (flexibility), §12 (scheduling and
validation), and §19 (simulation and benchmarks).

## Solver discipline - do not violate

A wrong solver class produces a confident wrong answer, which is worse than an
error.

| Problem | Acceptable | Not acceptable |
|---|---|---|
| LP / QP / MILP | HiGHS | Claiming SOCP support |
| Convex conic / SOCP | Clarabel, or CVXPY with a conic solver | Claiming MISOCP |
| Mixed-integer conic | A solver that supports it, or a MILP reformulation | Silently relaxing integrality |
| Nonlinear AC-OPF | IPOPT (local) | Claiming global optimality |
| Network state evaluation | pandapower, OpenDSS, GridLAB-D power flow | Calling it OPF |
| Transient / small-signal | ANDES, PowerSimulationsDynamics | Using it as a quasi-static feeder model |

- An SOCP relaxation is exact only under stated conditions. Verify them or
  report the gap; never assume exactness.
- Report solver status (`optimal`, `feasible`, `infeasible`, `timeout`,
  `numerical_failure`, `unsupported`), not just a solution vector.

## Units and signs

- Canonical: MW, MVAr, MVA, MWh. Convert at the boundary and record the mapping.
- `P > 0` exports. Reactive-power sign follows the declared model or device
  convention; never infer it from the sign alone.
- SOC is a fraction in [0,1] at the API boundary.
- Keep per-unit bases explicit; never mix base values silently.

## Reproducibility

Every run must record: input dataset version, grid-model revision, solver name
and version, tolerances, random seeds, scenario ID, and output artifact path.
A result without these is an anecdote, not evidence.

## Interfaces

- Workers speak Protobuf/gRPC. They do not read FlexCore database tables.
- A worker returns a typed result plus diagnostics. It does not decide authority
  and cannot issue commands.
- Heavy dependencies stay inside the worker image and are pinned with a
  lockfile.

## Verification

```sh
ruff check python/
pytest python/tests
```

If the code does not exist yet, do not invent a passing test run. State what was
checked and what was not.
