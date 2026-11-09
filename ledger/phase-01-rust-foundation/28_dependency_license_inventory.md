# P1.28 — Dependency / license baseline (D4 input — NOT a decision)

Date: 2026-11-07. Status: input prepared for the D4 owners (legal +
engineering). **D4 remains OPEN** — this inventory is not a license
decision, and no license field is asserted anywhere in the workspace
(root `Cargo.toml` deliberately omits `license`; see P1.03).

## Method and its limit

`Cargo.lock` does not exist yet (generating it requires a cargo run, which
the loop bans forbid). The inventory below therefore covers the **direct
workspace dependencies** with the license families recorded in
`ledger/phase-01-rust-foundation/RESEARCH.md` and the projects' own
metadata — every row must be re-verified by the human's first
`cargo tree` + `cargo deny check licenses` run, which will also enumerate
the TRANSITIVE set (P1.28 acceptance requires transitive coverage; that
evidence does not exist yet and is recorded as an open task).

## Direct dependency inventory

| Dependency | Version pin (RESEARCH.md) | License family | Verified? |
|---|---|---|---|
| tokio | 1.54.1 | MIT | No — verify at build |
| axum | 0.8.9 | MIT | No — verify at build |
| tower | 0.5.3 | MIT | No — verify at build |
| tower-http | 0.7.0 | MIT/Apache-2.0 | No — verify at build |
| sqlx | 0.9.0 | MIT/Apache-2.0 | No — verify at build |
| async-nats | 0.50.0 | MIT/Apache-2.0 | No — verify at build |
| serde / serde_json | resolved at first lockfile | MIT/Apache-2.0 | No — verify at build |
| time | resolved at first lockfile | MIT/Apache-2.0 | No — verify at build |
| tracing / tracing-subscriber | 0.1.44 / 0.3.23 | MIT | No — verify at build |
| utoipa | 5.5.0 | MIT/Apache-2.0 (per crates.io metadata) | No — verify at build |

All are permissive dual-licensed families; **none is copyleft**, but the
transitive set is unverified — that is the gap the first `cargo deny` run
closes.

## Runtime components (not workspace crates)

| Component | Version | License | Note |
|---|---|---|---|
| NATS server | 2.15.2 | Apache-2.0 (CNCF) | License-history note (P1.28 step 3): Synadia's April 2025 Business-Source-License move was reversed by the 1 May 2025 CNCF agreement, and the episode is closed decisively by NATS graduating in CNCF on 2 November 2026 (`RESEARCH.md` §5, primary announcements). The D4 review should NOT re-litigate this from stale news. |
| PostgreSQL server | 18.6 | PostgreSQL License (permissive) | Server-side deployment component. |

## Flagged items for the D4 review

1. **No copyleft dependency is known** in the direct set — but transitive
   coverage is pending the first lockfile (recorded open task; P1.28
   acceptance criterion 1 is NOT yet met as written).
2. **No core license file exists** (`.agent/PROVENANCE.md`); Apache-2.0 is
   the recommendation, pending D4 — the workspace asserts no license.
3. **Python/worker stack (Phase 7)** and **console (Phase 5)** dependency
   audits are out of Phase 1 scope and will be added by their phases'
   inventory work.
4. **Solver/numerical stack (Phase 3+) is a live D4 issue** per
   `MASTER_LEDGER.md` §9.2 (e.g. ANDES GPLv3, PyPSA GPL) — flagged here so
   the D4 review expects it in a later increment; nothing of that stack is
   present in Phase 1.

## Handover

This file is the delivery to the D4 owners (legal + engineering). The
mechanical re-verification the human runs before the D4 review:

```sh
cargo generate-lockfile
cargo tree --workspace --edges normal,build > /tmp/tree.txt
cargo deny check licenses      # after the deny.toml the CI notes require
cargo cyclonedx --all          # SBOM artefact (also produced per CI build)
```
