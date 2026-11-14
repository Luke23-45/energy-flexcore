<!-- P10.14 — Browser/OS support-matrix test. MATRIX + PROCEDURE record: the
     matrix from report §21.4 is recorded as TARGETS, never as verified
     support — every result cell is EMPTY because no browser/OS run has ever
     happened (nothing installed, nothing built, nothing run in this loop).
     The console build-toolchain constraints are FACTS read from
     console/package.json (secondary-source pins, re-verify at first
     install). The companion pointer doc is console/SUPPORT_MATRIX.md
     (additive; no existing console source touched). -->
# P10.14 — Browser/OS Support-Matrix Test (targets + procedure; every result cell EMPTY — no browser run has happened)

Date: 2026-11-14. Subtask: P10.14 (`ledger_phase_10.md` §P10.14). Owner role:
Platform engineering (with frontend engineering for the console cells). Report
trace: §22.3 release gate 10 ("the supported browser/OS matrix in §21.4 is
tested"); §21.4 (build/distribution form). Inputs: P10.01 scope freeze; the
§21.4 matrix (quoted verbatim below); the actual console toolchain facts read
from `console/package.json` (P5.01 scaffold, never installed/built); P10.18
(the release candidate — concurrent wave-3 sibling, cited by expected path
`ledger/phase-10-v1-release-hardening/18_release_candidate_build.md`; it does
not exist yet and no artifact hash is taken from it); P10.21 (the published
support matrix consumes this record's results). Deliverable: this record +
`console/SUPPORT_MATRIX.md` (additive; no existing console source touched).

## 1. Honest status — the plain statement

**No browser has ever loaded this console; no approval flow has ever been
exercised on any browser; no CLI binary exists and none has been run on any
OS; no OCI image has ever been built or pulled; no matrix cell has ever been
tested.** Every result cell in §5 is EMPTY. The matrix in §2 records the
**targets** §21.4 declares — it is not a claim of support, and nothing in it
may be read as tested (§22.3's own rule: "no untested cell is listed as
supported" — P10.21 publishes the support matrix ONLY from filled results
here). The console itself is the Phase 5 scaffold state: complete source,
never installed, never built, no lockfile, no node_modules
(`console/README.md`). Per ENTRY_NOTE.md boundary 3 this is "implemented,
awaiting human build/test"; zero observed failures are NOT claimed.

| Field | Value |
|---|---|
| Release-candidate identifier | `<PENDING P10.18 — no candidate build exists>` |
| Browser/OS versions tested | `<EMPTY — UNRUN>` |
| Artifact hashes tested | `<EMPTY — no candidate artifacts exist>` |

## 2. The §21.4 matrix, recorded as TARGETS (every result cell EMPTY)

### 2.1 The build/distribution form (§21.4, verbatim)

> | Surface | Build artifact | Runtime | Distribution |
> |---|---|---|---|
> | Operator Console | Static React/TypeScript bundle served by the control plane or a CDN | Current and previous major versions of Chrome, Edge, Firefox, and Safari | Bundled with the server release; no native mobile or desktop client |
> | Control Plane | Linux x86-64 and ARM64 OCI image plus native binary | Linux container preferred; Windows/macOS only for development | Signed OCI image; Helm chart from v1.0.0 |
> | WASM Plugin Host | Linux x86-64/ARM64 container | Linux container with sandbox and capability policy | OCI image; enabled from v0.5.0 |
> | Numerical / Research Workers | Linux Python container images pinned by lockfile | Linux containers | OCI images matching the control-plane release |
> | Site Edge Agent | Linux x86-64/ARM64 container or systemd service | Linux at the site or in the OT DMZ | Versioned package/OCI image with signed updates |
> | CLI / Simulation Lab | Cross-platform Rust CLI plus containerized simulator environment | CLI on Linux, macOS, and Windows; simulators in Linux containers | Release archives for the CLI; OCI images for the simulation environment |

### 2.2 The matrix cells to fill (the TARGETS; every Result cell `<EMPTY — UNRUN>`)

| Surface | Cell (target from §21.4) | Exact version tested | Result | Disposition |
|---|---|---|---|---|
| Operator Console | Chrome, current major | `<EMPTY — UNRUN>` | | |
| Operator Console | Chrome, previous major | `<EMPTY — UNRUN>` | | |
| Operator Console | Edge, current major | `<EMPTY — UNRUN>` | | |
| Operator Console | Edge, previous major | `<EMPTY — UNRUN>` | | |
| Operator Console | Firefox, current major | `<EMPTY — UNRUN>` | | |
| Operator Console | Firefox, previous major | `<EMPTY — UNRUN>` | | |
| Operator Console | Safari, current major | `<EMPTY — UNRUN>` | | |
| Operator Console | Safari, previous major | `<EMPTY — UNRUN>` | | |
| Control Plane | Linux x86-64 OCI image | `<EMPTY — UNRUN>` | | |
| Control Plane | Linux ARM64 OCI image | `<EMPTY — UNRUN>` | | |
| Control Plane | native binary on Linux | `<EMPTY — UNRUN>` | | |
| WASM Plugin Host | Linux x86-64 container | `<EMPTY — UNRUN>` | | |
| WASM Plugin Host | Linux ARM64 container | `<EMPTY — UNRUN>` | | |
| Numerical / Research Workers | Linux Python container (lockfile-pinned) | `<EMPTY — UNRUN>` | | |
| Site Edge Agent | Linux x86-64 container | `<EMPTY — UNRUN>` | | |
| Site Edge Agent | Linux ARM64 container | `<EMPTY — UNRUN>` | | |
| Site Edge Agent | systemd service on Linux | `<EMPTY — UNRUN>` | | |
| CLI / Simulation Lab | CLI on Linux | `<EMPTY — UNRUN>` | | |
| CLI / Simulation Lab | CLI on macOS | `<EMPTY — UNRUN>` | | |
| CLI / Simulation Lab | CLI on Windows | `<EMPTY — UNRUN>` | | |
| CLI / Simulation Lab | simulators in Linux containers | `<EMPTY — UNRUN>` | | |

The concrete "current" and "previous" major version numbers are determined AT
RUN TIME by the executor (the matrix names majors, not versions, by design);
a cell that cannot be executed (e.g. Safari on a machine without macOS) is
recorded with its blocker, never assumed, never silently dropped.

## 3. The console build-toolchain constraints — FACTS from `console/package.json`

Read from the actual config (no install, no build — these are the declared
pins, verbatim values):

| Item | Value in `package.json` | Note recorded in the file itself |
|---|---|---|
| build script | `tsc -b && vite build` | typecheck-then-bundle; the artifact a matrix run consumes |
| typecheck / lint / test | `tsc -b` / `eslint .` / `vitest run` | the §22.2 "frontend lint/type-check/test/build" gate items |
| react / react-dom | `19.3.1` (exact) | SECONDARY-SOURCE PIN — re-verify against react.dev/versions at first install (the file's `x-pin-policy`) |
| typescript | `~7.0.0` | same secondary-source pin; the known typescript-eslint peer-cap risk is recorded in the file |
| vite | `^8.2.1` (caret) | same pin policy |
| @vitejs/plugin-react | `^5.0.0` (caret) | same pin policy |
| vitest / eslint / typescript-eslint / @types/* | `^3.0.0` / `^9.0.0` / `^8.0.0` / `^19.0.0` | UNPINNED in the file's own words — resolved at first install, then recorded |
| lockfile / node_modules | **none exist** | "NOTHING was installed, built, or run in the Phase 5 loop" (the file's own x-pin-policy) |

Consequence for the matrix (recorded, not a promise): every console cell runs
against the bundle produced by the candidate build (P10.18) from this exact
toolchain state — and because there is no lockfile, the first install
resolves the caret/tilde ranges; the resolved versions MUST be recorded with
the matrix results (a matrix run on different resolved versions than the
candidate build used is a finding, not a pass).

## 4. The test procedure — what a matrix run exercises per cell (WRITTEN, UNRUN)

Console cells (ledger step 2), once per browser cell:

1. Serve the candidate console bundle (the P10.18 artifact; its hash is
   recorded with the results — §22.3 verification method: "confirm the
   artifact hashes tested match the candidate").
2. Load the §20.1 pilot workflows (the workspace registry the console
   implements — every workspace the candidate exposes, exercised, with any
   not-yet-implemented workspace rendered as its honest not-implemented
   state, never as data).
3. Exercise an approval flow end-to-end: propose → the approver sees the
   command with its validation/feasibility evidence and scope/limits BEFORE
   approving (§20.2; the P9.26 C-5/C-6 separation-of-duties posture in the
   UI) → approve → the state renders as approved, never as executed
   (§20.2: "Never display a proposed setpoint as if it were an executed
   command"). The write path stays operator-approved only (§21.4); the
   console never adds one.
4. Verify the §20.2 state distinctions render correctly: candidate, feasible,
   approved, issued, acknowledged, and measured states visually distinct;
   measured, estimated, stale, and simulated values distinguished; each
   rejected command shown with the failing constraint and supporting
   evidence; high-impact actions require explicit confirmation; emergency
   actions visually separate.
5. Record the exact browser version (user-agent string), the OS, and the
   per-check results in §5.

Non-console cells (ledger step 3): the control-plane, worker, and edge
artifacts run on their stated runtimes (x86-64 and ARM64 where declared);
the CLI runs on Linux, macOS, and Windows; simulators run in Linux
containers. Each cell records the exact runtime version and result. All of
it depends on candidate artifacts that do not exist (P10.18).

## 5. The run record — ALL CELLS EMPTY (UNRUN)

Every cell of §2.2 is `<EMPTY — UNRUN>`. No browser/OS run has happened; no
execution evidence exists; no disposition has been made. The table is the
one §2.2 defines (not duplicated here): 21 target cells, all empty.

## 6. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that every §2.2 Result/Disposition cell
   is `<EMPTY — UNRUN>` — the matrix records targets, not support.
2. Confirm §2.1 quotes §21.4 verbatim (build/distribution form) and that
   §22.3 gate 10's rule ("no untested cell is listed as supported") is
   stated in §1 as binding for P10.21's published matrix.
3. Confirm §3's toolchain values match `console/package.json` exactly
   (read the file; nothing was installed — no lockfile, no node_modules).
4. Confirm §4's procedure names the §20.1/§20.2 checks and the
   artifact-hash requirement of the ledger's verification method, and that
   the approval flow is described as operator-approved only.
5. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the matrix and procedure are written; no browser run, no
   build, no install happened.

## 7. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) the console build gate (the first install/build; record the resolved
#     dependency versions for §3):
cd console && npm install && npm run build
#     (the first human install re-verifies the §3 secondary-source pins per
#     the package.json x-pin-policy)

# (c) Obtain the P10.18 candidate artifacts (console bundle, control-plane
#     OCI images, worker images, edge artifact, CLI archives); record their
#     hashes with the matrix results.
# (d) Run the §4 procedure per §2.2 cell; fill each cell FROM THE RUN ONLY
#     (exact browser/OS/runtime versions, per-check results). A cell that
#     cannot execute is recorded with its blocker.
# (e) Record dispositions (fix, or remove the cell from the supported matrix
#     with a P10.26 release-notes entry).
# (f) Hand the filled matrix to P10.21 (published support matrix) and P10.27
#     (go/no-go).
```

## 8. Findings (recorded, not fixed)

- **F-P10.14-1 (no lockfile — reproducibility risk):** the console has no
  lockfile and no node_modules; the first install resolves the caret/tilde
  ranges at that moment. Two matrix-cell runs on different days could
  execute different resolved toolchains unless the resolved versions are
  pinned (lockfile) at the candidate build. Recorded as a dependency of the
  matrix's evidential value on the P10.18 build discipline.
- **F-P10.14-2 (candidate dependency):** every cell needs a candidate
  artifact (P10.18) that does not exist; the matrix cannot begin until the
  candidate build exists. Nothing here pre-declares a result.
- **F-P10.14-3 (Safari cells):** the Safari cells require macOS hardware;
  the run plan must provision it or record the cells' blocker — a Safari
  cell is never inferred from a WebKit-engine argument.
- **F-P10.14-4 (secondary-source pins):** the §3 toolchain pins are dated
  third-party round-ups (the package.json x-pin-policy), not
  registry-verified values; the first install re-verifies them against the
  official release pages before any matrix cell is meaningful.

## 9. Non-claims

No browser ran the console; no approval flow was exercised anywhere; no CLI
was built or run on any OS; no OCI image was built or pulled; no support of
any browser or OS is claimed; no compatibility percentage, baseline, or
"works on" statement appears. The §2.2 matrix is the §21.4 TARGETS with empty
results. Nothing here establishes browser support, OS support, or release
readiness — the runs are the human gates and have not happened and are not
scheduled. The entire evidence class is simulation/lab-grade (AGENTS.md
rules 2–5; ENTRY_NOTE boundaries 1 and 3).

Status: **WRITTEN-UNRUN** — a support-matrix test cannot "run" by being
written; the candidate build, the browsers/OSes, and the executions are the
human gates and have not happened and are not scheduled.
