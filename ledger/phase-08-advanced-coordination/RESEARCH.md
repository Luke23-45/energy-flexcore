# Phase 8 — Advanced Coordination Research: Research

| Field | Value |
|---|---|
| Phase | 8 — Stage 8 Advanced coordination research |
| Checked | 7 November 2026 |
| Method | `browser.search` + `browser.open` on primary sources only (official docs, repos, standards bodies) where available; peer-reviewed literature via publisher/arXiv pages where noted. Secondary summaries are marked as such. |
| Rule | Do not invent versions. Anything not verified here is labelled **ASSUMPTION** or listed as an open verification task (§7). |

## 1. ADMM / DMPC — convergence and the convexity boundary

| Item | Finding (checked 2026-11-07) | Source |
|---|---|---|
| Classical guarantees | ADMM convergence to the global optimum holds under standard assumptions (convexity, closedness, Slater's condition), with primal/dual residuals and objective gap decaying as O(1/k) in the standard analysis. | Standard ADMM theory as restated in recent applied work; report §13.2 states the same boundary. |
| Nonconvex / mixed-integer | With binary variables (or nonconvex AC-OPF), the problem is nonconvex and standard guarantees do not apply; ADMM is treated as a practical coordination heuristic with warm-starting and iteration limits. Under packet loss, the same work uses a lossy-update variant and an execution policy implementing the consensus setpoint with local feasibility. | Alghamdi (2026), "Resilient distributed model predictive control for cooperative microgrids under communication loss with demand response integration", PLOS ONE 21(4): e0345857 — https://journals.plos.org/plosone/article?id=10.1371%2Fjournal.pone.0345857 |
| Convexity-preserving learning | A 2026 distributed learning-based MPC study preserves convexity (input-convex encoder-only transformer embedded by recursive substitution) precisely so that the resulting constraint-coupled problem is compatible with the established convergence conditions of Tracking-ADMM; the authors note distributed approaches guarantee optimality/convergence/feasibility "only in the presence of convexity". | arXiv 2605.30763v2 (2026) — https://arxiv.org/pdf/2605.30763v2 (preprint, not peer-reviewed at time of check) |
| Report position | Report §13.2: "Classical ADMM guarantees rely on convexity and stated assumptions. For nonconvex AC-OPF or mixed-integer local problems, do not claim global convergence or global optimality without a specific theorem and experiment." | Report §13.2 |

**Implication:** the report's boundary is confirmed by the 2026 literature.
P8.07/P8.09 must treat nonconvex/mixed-integer ADMM as a heuristic and
report residuals, iteration counts, and fallback behaviour — never a global
optimality claim.

## 2. GNN for power systems — PyTorch and PyTorch Geometric versions

| Item | Finding (checked 2026-11-07) | Source |
|---|---|---|
| PyTorch (current) | **PyTorch 2.15, released 28 October 2026** (3,117 commits from 583 contributors since 2.14). Supports Python 3.11–3.15 (incl. free-threaded 3.14t/3.15t); default wheel moves to CUDA 13.2 (13.2.2 toolkit), with CUDA 13.4 GA wheels alongside. | PyTorch 2.15 release blog — https://pytorch.org/blog/pytorch-2-15-release-blog/ |
| PyTorch Geometric (current found) | **PyG 2.8.0, released 5 June 2026** (CHANGELOG: [2.8.0] – 2026-06-05). Supports **PyTorch 2.9, 2.10, 2.11, and 2.12**, Python 3.10–3.14; `pyg-lib==0.7.0`; `torch-cluster` and `torch-spline-conv` deprecated/ignored. Dropped PyTorch 2.8. | PyG releases — https://github.com/pyg-team/pytorch_geometric/releases ; PyG CHANGELOG (via GitHub) |
| Version skew | PyG 2.8.0 (June 2026) predates PyTorch 2.13, 2.14, and 2.15. Its stated PyTorch support tops out at **2.12**, while the current PyTorch is **2.15**. No PyG release supporting PyTorch 2.15 was found in this check. | Comparison of the two rows above |

**Implication (R8-C3, see §6):** do **not** pin PyTorch 2.15 + PyG 2.8.0
together on assumption. P8.03/P8.10 must either pin a verified-compatible
pair (e.g. PyTorch 2.12 + PyG 2.8.0, if that pair is verified at
implementation time) or re-check for a newer PyG release and record the
result. A pair that installs but changes numerics silently is worse than a
pair that fails loudly.

## 3. MARL — RLlib / Ray and Gymnasium

| Item | Finding (checked 2026-11-07) | Source |
|---|---|---|
| RLlib position | RLlib is the reinforcement-learning library inside Ray (Anyscale); first-class multi-agent support via `MultiAgentEnv`; consumes Gymnasium environments. The **new API stack** (RLModule, Learner, EnvRunner, ConnectorV2) is the default in recent releases, replacing ModelV2/Policy; **TensorFlow support was dropped — RLlib is now PyTorch-only**. The legacy `rllib_contrib` collection (A2C, DDPG, TD3, MADDPG, QMIX, etc.) was removed; the new stack ships a focused set (PPO, APPO, IMPALA, DQN/Rainbow, SAC, DreamerV3, plus offline/imitation methods). | Ray/RLlib documentation — https://docs.ray.io/en/latest/rllib/index.html ; 2026 tooling survey (secondary, corroborating) |
| Versions found | **Ray 2.56.1 (17 July 2026)** in a July 2026 snapshot; **Ray 2.57.0** seen in an August 2026 commit (pins `gymnasium==1.2.2` per release). **No Ray version current as of 7 November 2026 was verified from a primary source in this check** — see open verification task. | Secondary snapshot, July–August 2026 (see §7) |
| Gymnasium | **Gymnasium 1.3.0, uploaded 22 April 2026** (requires Python 3.10+); the maintained successor to OpenAI Gym and the environment API targeted by SB3/RLlib. | PyPI Gymnasium record, via July 2026 secondary snapshot |
| ANDES RL | ANDES v2.0 adds an **`andes.rl` module with `AndesEnv`, a Gymnasium-compatible environment** for RL on power-system dynamics (configurable observations, group/model-level action setpoints, custom reward/disturbance functions, fast reset via `TDS.reinit()`; install with `pip install andes[rl]`). | ANDES release notes — https://docs.andes.app/en/latest/reference/release-notes.html |

**Implication:** P8.13 must record the RLlib API stack explicitly (new vs
old) and the exact Ray version pinned at implementation time; the new stack's
reduced algorithm set (no MADDPG/QMIX in the new stack) constrains which
MARL algorithms are available without the removed contrib collection.

## 4. ANDES dynamic simulation

| Item | Finding (checked 2026-11-07) | Source |
|---|---|---|
| Latest release | **ANDES 2.0.0, released 11 March 2026** (PyPI files uploaded 12 March 2026). v1.10.0 released 5 January 2026 (pyproject.toml migration, pandapower 3.x compatibility, docs overhaul). The live docs currently show a development version (1.10.1.post50+). | ANDES release notes — https://docs.andes.app/en/latest/reference/release-notes.html ; PyPI — https://pypi.org/project/andes/ |
| Capabilities | Open-source Python package for power-system modelling and simulation: power flow (Newton–Raphson), time-domain simulation (implicit trapezoidal), eigenvalue analysis, continuation power flow, state estimation. 100+ dynamic models (GENROU/GENCLS, TGOV1/IEEEG1/HYGOV, EXST1/ESST3A/SEXS/AC8B, IEEEST/ST2CUT, REGCA1/REECA1/REPCA1/WTDTA1, PVD1/ESD1 DER models, ZIP/FLoad loads), with limiters/saturation. DAE formulation with symbolically derived Jacobians. | ANDES docs — https://docs.andes.app/en/latest/ ; PyPI (above) |
| License | **GPLv3.** "ANDES is licensed under the GPL v3 License." | PyPI — https://pypi.org/project/andes/ |
| RL environment | `andes.rl` / `AndesEnv` (Gymnasium-compatible) added in v2.0.0 (see §3). | ANDES release notes (above) |

**Implication:** ANDES is the right tool for the dynamic track (§19.1,
§7.4), but its GPLv3 license is the single most consequential licensing
fact in this phase: P8.03 must record the isolation design and get legal
confirmation before ANDES is depended on. The report recommends Apache-2.0
for the core (§25.1) and D4 is open; linking GPLv3 code into the core would
force a license decision after the fact.

## 5. HELICS co-simulation

| Item | Finding (checked 2026-11-07) | Source |
|---|---|---|
| What it is | Modular cyber-physical energy co-simulation framework with APIs for multiple languages and simulators; synchronizes separate simulator federates and time domains. | Report §7.4 [R16]; HELICS docs — https://docs.helics.org/en/latest/ |

**Open verification task:** the HELICS version current as of 7 November 2026
was not verified from a primary source in this check (only the framework's
role, which the report already states). P8.18 must pin and record the HELICS
version at implementation time. **ASSUMPTION:** none made about the HELICS
version.

## 6. Contradictions and traps found (recorded honestly)

| ID | Finding |
|---|---|
| **R8-C1 — No contradiction in §13's honesty boundaries** | The report's §13.1–§13.3 limits (same physical validation for GNN; ADMM convexity boundary; MARL finite-test-set limit; safety-filter relative-degree requirement) are confirmed, not contradicted, by the 2026 literature checked here (notably the PLOS ONE RDMPC paper's explicit "with binary variables … standard guarantees do not apply" and the arXiv convexity-preserving design). No correction to the report is proposed on these points. |
| **R8-C2 — ANDES GPLv3 vs the Apache-2.0 core recommendation** | Report §7.4 lists ANDES as the dynamic-stability tool and §25.1 recommends Apache-2.0 for the core, while D4 (license audit) is open. ANDES 2.0.0 is GPLv3 (PyPI). This is not a contradiction in the report — the report never claims ANDES is Apache-2.0, and §8.2 already requires research workers to communicate via Protobuf/gRPC rather than importing internal tables — but it is a live decision: ANDES must be isolated behind the worker boundary (P8.03/P8.16), and any distribution of ANDES-derived code must go through legal. An implementer who adds ANDES as a direct dependency of a core crate would create the conflict. |
| **R8-C3 — PyTorch/PyG version skew (new finding)** | PyTorch 2.15 (28 Oct 2026) is current, but the newest PyG found, 2.8.0 (5 Jun 2026), supports PyTorch only up to 2.12. Pinning "latest PyTorch + latest PyG" is therefore not a verified-compatible pair. This is an ecosystem finding, not a report contradiction (the report pins no versions), but it must be resolved by P8.03 before any GNN work starts. |
| **R8-C4 — RLlib's new API stack narrows the algorithm set** | The report (§7.4) names "RLlib or an explicitly selected research framework" without naming an API stack. Since the report's cut-off, the new API stack is default and PyTorch-only, and MADDPG/QMIX live only in the removed `rllib_contrib` collection. A MARL design that assumes MADDPG/QMIX availability in current RLlib would be wrong; P8.13 must select the framework and stack explicitly. This is a staleness risk in the report's loose wording, recorded here rather than silently assumed. |
| **R8-C5 — ANDES docs version vs release version** | The live ANDES docs show a development version string (1.10.1.post50+), while PyPI's latest release is 2.0.0 (March 2026). The docs' "latest" build does not obviously correspond to the 2.0.0 release. P8.16 must pin ANDES 2.0.0 (or a verified later release) and record which docs were consulted, rather than citing "the docs" generically. |

## 7. Open verification tasks (not facts)

1. Current Ray/RLlib version as of 7 November 2026 — check Ray's releases
   or PyPI directly (only 2.56.1 / 2.57.0 from July–August 2026 secondary
   snapshots were found here).
2. Newer PyG release supporting PyTorch 2.13–2.15 — re-check
   https://github.com/pyg-team/pytorch_geometric/releases at implementation
   time; if none exists, pin the verified-compatible pair per P8.03.
3. HELICS version current as of 7 November 2026 — check the HELICS
   releases/docs directly before P8.18.
4. GHSA/advisory pages for any PyTorch/PyG/RLlib/ANDES security issues
   disclosed near the cut-off — not checked here; P8.25 should confirm the
   pinned versions carry no known critical issue at implementation time.
5. Whether a newer ANDES release exists after 2.0.0 (March 2026) — re-check
   PyPI at implementation time; the docs' development version string does
   not establish one.
