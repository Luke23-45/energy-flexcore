# P6.02 — License/dependency audit for the Phase 6 stack (D4 follow-through): STATIC inventory

Date: 2026-11-12. Subtask: P6.02, Phase 6 ledger (`ledger_phase_6.md` §P6.02).
Owner role: Legal + engineering (ledger P6.02). **This is a STATIC inventory,
not a D4 decision and not a scan result.** D4 remains OPEN
(`.agent/DECISIONS.md`); the core license is a RECOMMENDATION (Apache-2.0),
not a decision, and the workspace asserts no `license` field
(root `Cargo.toml` header comment; `ledger/phase-01-rust-foundation/28_dependency_license_inventory.md`).

## 1. Method and its limits (stated honestly)

- Nothing was installed, built, or run in this loop (`ENTRY_NOTE.md` loop
  rules). **No `cargo generate-lockfile`, no `cargo tree`, no
  `cargo deny check licenses`, no `gitleaks`** — these are the human's gate
  inputs. A `Cargo.lock` does not exist in this loop's state, so the
  TRANSITIVE dependency set is unverified, exactly as recorded in the
  Phase 1 baseline (`28_dependency_license_inventory.md` §Method).
- The inventory below covers the direct dependencies Phase 6 uses or plans,
  plus the recorded external-system candidates, with license families
  **quoted from in-repo research records** (cited per row). No license is
  stated from memory.
- Note on sources: the subtask brief names `.agent/RESEARCH.md`; no such
  file exists in `.agent/` (checked this date — `.agent/` contains
  CONTEXT/DECISIONS/PROVENANCE/README/WRITING + roles/skills). License
  findings are cited from the phase RESEARCH files instead:
  `ledger/phase-06-edge-protocol/RESEARCH.md`,
  `ledger/phase-01-rust-foundation/RESEARCH.md` and `28_dependency_license_inventory.md`,
  `ledger/phase-03-flexibility-scheduling/RESEARCH.md`,
  `ledger/phase-05-operator-console-simulator/RESEARCH.md`.

## 2. Rust dependencies Phase 6 uses (already in the workspace — no new crates planned)

| Dependency | Version pin | License family (source) | Verdict | Action |
|---|---|---|---|---|
| tokio | 1.54.1 (workspace `Cargo.toml`; never 1.54.0, CVE note in Phase 1 RESEARCH) | MIT — `ledger/phase-01-rust-foundation/28_dependency_license_inventory.md` (marked "verify at build") | PASS (permissive), unverified transitively | Human's first `cargo deny check licenses` confirms |
| axum | 0.8.9 | MIT — same P1.28 table | PASS (permissive), unverified | Same |
| tower / tower-http | 0.5.3 / 0.7.0 | MIT (tower) and MIT/Apache-2.0 (tower-http) — P1.28 table | PASS (permissive), unverified | Same |
| sqlx | 0.9.0 (features: runtime-tokio, **tls-rustls**) | MIT/Apache-2.0 — P1.28 table | PASS (permissive), unverified | Same |
| serde / serde_json / time | resolved at first lockfile | MIT/Apache-2.0 — P1.28 table | PASS (permissive), unverified | Same |
| tracing / tracing-subscriber | 0.1.44 / 0.3.23 | MIT — P1.28 table | PASS (permissive), unverified | Same |
| utoipa | 5.5.0 | MIT/Apache-2.0 per crates.io metadata — P1.28 table | PASS (permissive), unverified | Same |
| async-nats | 0.50.0 | MIT/Apache-2.0 — P1.28 table | PASS (permissive), unverified | Same |
| clarabel | =0.11.1 | Apache-2.0 — `ledger/phase-03-flexibility-scheduling/RESEARCH.md` | PASS (permissive) | Confirmed in Phase 3 research |
| **rustls** | **no direct pin — pulled via sqlx's `tls-rustls` feature; Phase 6's mTLS channel (P6.07) builds on it** (`ENTRY_NOTE.md` boundary 3: "no new crate additions without the D4 audit note") | **UNVERIFIED in this repo** — no in-repo source records rustls's license; recorded as an open verification item, to be closed by the human's `cargo deny check licenses` run against the first lockfile | FLAG — unknown until the scan | Named action: add to the human's cargo-deny input list; owner: legal + engineering (D4 owners). Do not add a direct rustls dependency before the D4 audit records it |

**Copyleft check:** no copyleft dependency is known in the direct set
(P1.28 §Flagged items 1) — but the transitive set is UNVERIFIED and that
gap is closed only by the human's first `cargo deny` run. Phase 6 plans
**no new third-party crates**; the only new code is first-party
(`crates/edge-agent/**`, adapter crates per the wave plan). Any future
addition (e.g. a direct rustls/tokio-rustls pin for P6.07, an OpenEMS
client library) requires the D4 audit note first (`ENTRY_NOTE.md`
boundary 3).

## 3. Runtime components (not workspace crates)

| Component | Version | License (source) | Verdict |
|---|---|---|---|
| NATS server | 2.15.2 | Apache-2.0 (CNCF) — P1.28 table + `ledger/phase-01-rust-foundation/RESEARCH.md` (Synadia BSL episode closed; NATS graduated in CNCF 2 Nov 2026) | PASS |
| PostgreSQL server | 18.6 | PostgreSQL License (permissive) — P1.28 table | PASS |

## 4. External systems and recorded candidates (Phase 6 scope)

| Item | Version / state | License (source) | Verdict / action |
|---|---|---|---|
| **OpenEMS** (recommended-first site EMS, `01_d5_disposition.md` §3) | 2026.11.0 released 31 Oct 2026 (Zenodo record) — `ledger/phase-06-edge-protocol/RESEARCH.md` §1 | **UNVERIFIED — not stated in any in-repo source.** `RESEARCH.md` §1–§7 records architecture, releases, and auth model but does NOT record OpenEMS's license. It is NOT recorded as AGPL here because no in-repo source says so. | **FLAG — D4 open item.** Named action: the D4 owners verify OpenEMS's license (and any client-library licensing) from the project's primary source before any client library is vendored or linked. Integration as planned uses the documented REST API (EdgeConfig) over the network, which the ENTRY_NOTE treats as an external-system boundary — but the license question is still recorded, not assumed away. Owner: legal + engineering (D4). |
| OpenEMS client libraries | Not present; none planned without the D4 note (`ENTRY_NOTE.md` boundary 3) | n/a — nothing vendored | No action until a candidate exists |
| OpenADR 3.0 conformance tooling | members-only reference implementation; certification requires an Alliance license (~US $1,250 per the Alliance's certification page) — `RESEARCH.md` §2, §8 task 3 | Access terms for the test tool: membership-gated, UNVERIFIED in detail | FLAG for P6.19: confirm access terms before writing tests |
| OCPP 2.1 Edition 2 materials | dates conflict between secondary sources — `RESEARCH.md` §3, §8 task 2 | n/a — not selected (D5 OPEN) | Only relevant if D5 selects OCPP |
| SunSpec / CSIP test procedures | SunSpec publishes guides + conformance procedure; certification via SunSpec Authorized Test Laboratories — `RESEARCH.md` §4–§5 | Access terms UNVERIFIED | FLAG for P6.19 if ever in scope (later adapter per §2.3) |

## 5. Carried pins from prior phases (context for the D4 review)

| Item | Version | License (source) | Note |
|---|---|---|---|
| pandapower | 3.5.6 — `ledger/phase-05-operator-console-simulator/RESEARCH.md` §pins | Not recorded in the sources checked for this inventory — carried as a Phase 5/Phase 1 flagged D4 input (P1.28 flagged the solver/numerical Python stack as a live D4 issue) | NOT installed in this loop |
| HELICS | 3.7.0 — same source | BSD-3-Clause — `ledger/phase-05-operator-console-simulator/RESEARCH.md` §1 | helics API usage UNVERIFIED |
| HiGHS (highspy) | 1.15.1 — Phase 3/4 records | MIT — `ledger/phase-03-flexibility-scheduling/RESEARCH.md` | Tier 1 worker path |
| Ipopt | 3.14.20 — Phase 3 RESEARCH | Eclipse Public License (EPL) — same source | NOT a Phase 3/6 component; recorded so D4 expects EPL if Stage 8 ever uses it |
| Console: TypeScript / Vite / React | per Phase 5 RESEARCH | TypeScript Apache-2.0, Vite MIT, React MIT — `ledger/phase-05-operator-console-simulator/RESEARCH.md` §82 note | Formal D4 audit still pending |

## 6. SBOM input list (P6.02 step 4)

The audit table above (§2–§5) is the Phase 6 input to the release SBOM per
§9.3/§22.2. The mechanical SBOM production remains the human's pipeline
(`cargo cyclonedx --all` per CI, `ledger/phase-01-rust-foundation/28_dependency_license_inventory.md` §Handover);
this loop does not run it.

## 7. Scans NOT RUN — human gate inputs

| Scan | Status | Why |
|---|---|---|
| `cargo deny check licenses` | **NOT RUN** | Loop bans forbid running tools/builds (`ENTRY_NOTE.md`); transitive license evidence cannot exist without a lockfile |
| `gitleaks` (secret scan) | **NOT RUN** | Same; recorded in HANDOVER.md as a human gate input |
| `cargo tree` transitive enumeration | **NOT RUN** | No lockfile in this loop's state |

## 8. Verdict

- Direct Rust set: permissive families per the recorded research; zero
  known copyleft in the direct set; **zero unknown-license items in the
  direct set as recorded** — but "zero unknown" holds only against the
  recorded sources, not a scan.
- Open items carried to D4: (1) rustls license verification; (2) OpenEMS
  license verification; (3) transitive-set verification via the first
  `cargo deny` run; (4) conformance-suite access terms (P6.19);
  (5) pandapower/solver-stack Python audit carried from Phase 1.
- This audit does not close or narrow D4 by itself. When the human's scan
  runs, the verification method of P6.02 applies: re-run the dependency
  enumeration from the lockfiles and diff against this table; any
  unlisted dependency fails the check.
