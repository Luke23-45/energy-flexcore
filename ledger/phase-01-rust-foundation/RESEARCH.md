# RESEARCH — Phase 1: Modular Rust Foundation

Research cut-off for this ledger: **7 November 2026** (same as report §27).
Checked: **7 November 2026**. Sources are primary only (the project's own
release notes, docs.rs/crates.io registry pages, or official documentation),
except where explicitly labelled secondary. Anything unverified is labelled
**ASSUMPTION** or written as an open verification task. Re-verify every
version at build time with `cargo --version`, `cargo tree`, and the registry
pages — a newer patch release is adopted only by an explicit, recorded pin
change.

## 1. Rust toolchain

| Item | Finding | Source |
|---|---|---|
| Current stable (7 Nov 2026) | **Rust 1.99.1** (point release 15 October 2026; 1.99.0 released 1 October 2026). Fixes: missing `wcslen` symbol on some targets, FFI bool-return miscompilation on x86_64, rust-lld issue on macOS | Rust blog, https://blog.rust-lang.org/ (Announcing Rust 1.99.1, 15 Oct 2026); Rust Forge release table, https://forge.rust-lang.org/ (Stable 1.99, 1 Oct 2026; Beta 1.100, stable 12 Nov 2026) |
| Next stable | **Rust 1.100.0, planned 12 November 2026** — after this ledger's cut-off. Do NOT pin 1.100 on 7 November 2026; re-evaluate after it ships | Rust Forge (above); report §5.4 [R3] (same date) |
| Edition | **2024 edition** is the current/default edition since Rust 1.85 (February 2025) | Rust Forge / release notes (above) |
| Release cadence | New stable every six weeks | Rust release process, https://forge.rust-lang.org/ |

**Pin for Phase 1:** Rust **1.99.1**, edition **2024** (P1.02).

## 2. Tokio

| Item | Finding | Source |
|---|---|---|
| Current version (7 Nov 2026) | **Tokio 1.54.1** (21 October 2026); 1.54.0 (19 October 2026) | docs.rs, https://docs.rs/crate/tokio/latest (CHANGELOG: 1.54.1, 21 Oct 2026); lib.rs, https://lib.rs/crates/tokio (1.54.1, 21 Oct 2026) |
| MSRV | **Rust 1.85** (raised in 1.54.0); rolling MSRV policy of at least 6 months | docs.rs Tokio page (above) |
| LTS lines | 1.51.x LTS until March 2027 (MSRV 1.71); 1.53.x LTS until September 2027 (MSRV 1.71) | docs.rs Tokio page (above) |
| Release cadence | Roughly one minor per month; patches for bugfixes | docs.rs Tokio page (above) |

**Note:** a secondary article claims a "Tokio 2.0" (dev.to, March 2026). No
Tokio 2.0 release exists in the registry/docs.rs record as of this check —
treat the "2.0" claim as **unverified; do not use it**. Pin Tokio **1.54.1**.

**Security note (iteration-02 review, checked 7 November 2026):** the 1.54.1
pin is security-motivated, not just "latest". CVE-2026-115567 (disclosed
23 October 2026) is a race between `tokio::time::Interval::poll_tick`
(including `.tick().await`) and the multi-threaded runtime's time driver in
1.54.0 that could drop an internal timer entry and cause a use-after-free on
the timer's waker and task state in safe Rust (CWE-362/CWE-416); 1.54.1
reverts the timer change that introduced it (docs.rs CHANGELOG, 1.54.1,
21 October 2026). Severity labels differ by source (CNA advisory: critical;
CVSS 4.0 base 5.8 per Rapid7) — do not assert a single score; the pin
itself is what matters. Never pin 1.54.0.

## 3. Axum

| Item | Finding | Source |
|---|---|---|
| Current version (7 Nov 2026) | **Axum 0.8.9** (14 April 2026); previous 0.8.8 (20 December 2025) | lib.rs, https://lib.rs/crates/axum (89 releases; 0.8.9, 14 Apr 2026); rustfinity Axum guide (Sep 2026): "Axum 0.8.9 (the latest release as of September 2026)" |
| MSRV | **Rust 1.80** | lib.rs Axum page (above) |
| Status | Still 0.x — minor bumps (0.7 → 0.8) can contain breaking changes; patch releases within 0.8 are backwards compatible | rustfinity guide (above) |
| Home | Tokio project (`tokio-rs` GitHub organisation); built on hyper 1.x, tower 0.5, tower-http | lib.rs / rustfinity (above) |

**Pin for Phase 1:** Axum **0.8.9** (pin the minor: `axum = "0.8.9"`).

## 4. SQLx

| Item | Finding | Source |
|---|---|---|
| Current version (7 Nov 2026) | **SQLx 0.9.0** (crates.io date 21 May 2026; changelog header "0.9.0 – 2026-05-06" — the two dates differ; the crates.io publication date, 21 May 2026, is the one to cite for the release) | docs.rs, https://docs.rs/crate/sqlx/latest (0.9.0); CHANGELOG, https://raw.githubusercontent.com/launchbadge/sqlx/main/CHANGELOG.md |
| MSRV | **Rust 1.94.0** for the 0.9.0 cycle | SQLx CHANGELOG (above) |
| Repository move | Shortly after 0.9.0 the repository moved to a new GitHub organisation, https://github.com/transact-rs/ (from `launchbadge`) | SQLx CHANGELOG (above) |
| Databases | PostgreSQL, MySQL/MariaDB, SQLite. MSSQL was removed in 0.7 pending a rewrite (SQLx Pro initiative) | docs.rs SQLx page (above) |
| Offline mode | "Offline mode is now always enabled" in 0.9.0; `sqlx.toml` per-crate configuration supported; `sqlx-cli` 0.9.0 for migrations | docs.rs SQLx page; docs.rs sqlx-cli page, https://docs.rs/crate/sqlx-cli/latest |
| Edition | SQLx 0.9.0 itself is still edition 2021 (its Cargo.toml notes "TODO: upgrade to edition 2024") — this does not constrain the workspace edition | docs.rs SQLx Cargo.toml, https://docs.rs/crate/sqlx/latest/source/Cargo.toml.orig |

**Pin for Phase 1:** SQLx **0.9.0** (+ `sqlx-cli` 0.9.0). MSRV 1.94.0 is
satisfied by the Rust 1.99.1 pin.

## 5. NATS server and async-nats

| Item | Finding | Source |
|---|---|---|
| NATS server, latest minor | **2.15**, released **17 September 2026** ("Latest"); 2.14 (30 April 2026) "Maintained"; 2.12 end-of-life (final 2.12.15, 12 August 2026) | NATS docs, https://docs.nats.io/release-notes/ |
| NATS server, latest patch | **v2.15.2** (published 26 October 2026; "5 November 2026" in the earlier draft was a secondary-source crawl date, corrected 7 November 2026 in the final assembly pass). **CORRECTED 7 November 2026 (iteration-02 review):** v2.15.2 does NOT fix CVE-2026-33222. CVE-2026-33222 (JetStream stream-restore endpoint auth bypass, GHSA-9983-vrx2-fg9c, published 24 March 2026, CVSS 4.9) was fixed in **2.11.15 and 2.12.6**; every 2.15.x release merely includes that fix. The v2.15.2 changelog item is CVE-2026-111943 (eMudhra certificate distrust, Go 1.27.3). Sibling **CVE-2026-33223** (Nats-Request-Info header spoof, CVSS 6.4, same fix versions 2.11.15 / 2.12.6) — added 7 November 2026 (iteration-09 review). The earlier wording ("2.15.2 fixes CVE-2026-33222") was wrong. | CVE records for CVE-2026-33222 (Wiz, Vulert, Feedly — all agreeing the fixed releases are 2.11.15 / 2.12.6); NATS release notes, https://docs.nats.io/release-notes/ |
| JetStream semantics | Durable streams, consumers, acknowledgements, replay, at-least-once delivery | NATS docs, https://docs.nats.io/concepts/jetstream (report [R6]) |
| License | NATS stays under **Apache-2.0** in the CNCF. Synadia's April 2025 move to the Business Source License was reversed by agreement with the CNCF on 1 May 2025 (Synadia/CNCF joint announcement, https://www.synadia.com/press/cncf-and-synadia-align-on-securing-the-future-of-nats), and the episode is closed decisively: NATS **graduated in CNCF on 2 November 2026** (CNCF project page, https://www.cncf.io/projects/nats/, checked 7 November 2026). The earlier "verify before the D4 review" qualifier is retired — the position is confirmed from primary announcements. |
| async-nats (Rust client) | **0.50.0** (20 July 2026) per lib.rs release list (70 releases; 0.49.0, 25 May 2026; 0.47.0, 31 March 2026). Note: docs.rs "latest" pages for older versions (e.g. 0.21.0) are stale snapshots — use the lib.rs/crates.io release list. Still 0.x: "will introduce breaking changes"; API described as stable but versioned <1.0 | lib.rs, https://lib.rs/crates/async-nats ; docs.rs, https://docs.rs/crate/async-nats/0.21.0/source/README.md (stale snapshot, do not use as the version source) |
| Sync `nats` crate | Deprecated; use `async-nats` | lib.rs async-nats page (above) |

**Pins for Phase 1:** NATS server **2.15.x** (re-check the latest 2.15 patch
at build time; resolve the exact patch — 2.15.2 at the cut-off — and record
it; do not run a version affected by CVE-2026-33222 without the fix, i.e.
nothing before 2.11.15 / 2.12.6 on its line); async-nats **0.50.0**.

**Durability caution (design input, P1.14):** Jepsen tested NATS JetStream
2.12.1 and found it lost writes if data files were truncated or corrupted on
a minority of nodes, and documented the default fsync policy (flush every two
minutes rather than before acknowledgement) as a risk; NATS has since
documented the default fsync policy. Source: Jepsen, https://jepsen.io/analyses/nats-2.12.1.pdf
(December 2025 analysis of 2.12.1). This is why Phase 1 claims at-least-once
delivery and never claims zero-loss durability from the broker alone — the
outbox + idempotency keys are the durable guarantee, and the fsync policy
must be a deployment decision, not an assumption.

## 6. PostgreSQL

| Item | Finding | Source |
|---|---|---|
| Current major | **PostgreSQL 18**, released **25 September 2025** (35th major release); new async I/O subsystem; wire-protocol version 3.2 (first new protocol version since PostgreSQL 7.4, 2003); `md5` password authentication deprecated | PostgreSQL press release, https://www.postgresql.org/about/news/postgresql-18-released-3142/ ; press FAQ, https://www.postgresql.org/about/press/faq/ |
| Latest minor | **18.6, released 13 August 2026** (18.5 was never released — a regression found after wrap); PostgreSQL 19 at RC 1 since 15 October 2026, not yet final | **CONFIRMED 7 November 2026 (final review), primary source:** PostgreSQL announcement, https://www.postgresql.org/about/news/postgresql-186-1711-1615-1519-1424-and-19-beta-3-released-3365/ (18.6, 17.11, 16.15, 15.19, 14.24; 18.5 skipped). The earlier secondary-only qualifier is retired; keep the build-time `SELECT version()` as a cheap sanity check, not as an open question. Next scheduled minor: 12 November 2026 — re-check at build time |
| Support | Each major supported 5 years after first release; PostgreSQL 18 supported until 14 November 2030; PostgreSQL 14 stops receiving fixes 12 November 2026 | postgresql.org versioning/FAQ (above); secondary lifecycle summary |
| Partitioning | Declarative partitioning (since PG 10): `PARTITION BY RANGE/LIST/HASH`; parent is virtual, rows route to partitions; PK/unique constraints must include the partition key; `DETACH PARTITION ... CONCURRENTLY` (PG 14+); no automatic partition creation — pre-create partitions ahead of the ingest frontier | PostgreSQL docs (current = 18), https://www.postgresql.org/docs/current/ddl-partitioning.html — **opened and read 6 November 2026** |
| RLS | Table owners typically bypass RLS unless configured otherwise (`FORCE ROW LEVEL SECURITY`) | PostgreSQL docs, https://www.postgresql.org/docs/current/ddl-rowsecurity.html (report [R25]) |
| `uuidv7()` | Built-in `uuidv7()` function in PostgreSQL 18+ (RFC 9562 time-ordered UUIDs), plus `uuidv4()` as an alias for `gen_random_uuid()` and `uuid_extract_timestamp()` / `uuid_extract_version()` | **CONFIRMED 7 November 2026 (iteration-02 review):** PostgreSQL 18 release announcement ("the database-friendly uuidv7() function"), https://www.postgresql.org/about/news/postgresql-18-released-3142/ ; multiple independent write-ups agreeing `uuidv7()` is native in PG 18 with no extension. The earlier secondary-only qualifier is retired; keep the build-time `SELECT uuidv7()` as a cheap sanity check, not as an open question |

**Pin for Phase 1:** PostgreSQL **18** (major); re-verify the latest minor
at build time. Do NOT target PostgreSQL 19 (release candidate only as of
this check).

## 7. tracing, utoipa, tower-http

| Item | Finding | Source |
|---|---|---|
| tracing | **0.1.44**; tracing-subscriber **0.3.23** | Secondary compilation (rustoffrails stack table, verified 12 Oct 2026; zenpass ADR-0013, 6 Sep 2026, both listing tracing 0.1.44 / tracing-subscriber 0.3.23) — **re-verify against docs.rs at build time** |
| utoipa | **5.5.0** (4 May 2026); OpenAPI 3.1; framework-agnostic | lib.rs, https://lib.rs/crates/utoipa (58 releases; 5.5.0, 4 May 2026) |
| utoipa-gen | **6.0.1** (separate version line from utoipa itself — do not assume matching versions) | lib.rs, https://lib.rs/crates/utoipa-gen |
| utoipa-axum | **0.3.0** (bindings crate; a secondary comparison notes the satellites lag the core) | docs.rs, https://docs.rs/crate/utoipa-axum/latest/ |
| tower / tower-http | tower **0.5.3**, tower-http **0.7.0** (hyper 1.x stack) | Secondary comparison (hartym wiki, checked 4 Aug 2026) — **re-verify at build time** |
| OpenTelemetry (Rust) | **0.32.0**; logs and metrics stable, distributed tracing still Beta; breaks minors every 3–4 months | Secondary comparison (hartym wiki, checked 4 Aug 2026) — **re-verify at build time; pin exactly and put the exporter behind a recorded decision (P1.20)** |

## 8. Open verification tasks (Phase 1)

1. At build time, re-check every pinned version above against crates.io /
   docs.rs / the official release page and record any change in the build log.
2. ~~Confirm the NATS CVE-2026-33222 fix version against the NATS GitHub
   releases before choosing the server patch.~~ **RESOLVED 7 November 2026
   (iteration-02 review):** CVE-2026-33222 was fixed in 2.11.15 / 2.12.6
   (see §5); the task as written (implying the fix landed in 2.15.2) was
   based on the wrong attribution. The remaining build-time action is the
   routine latest-2.15-patch re-check in task 1.
3. ~~Confirm the NATS Apache-2.0/CNCF license position against a primary
   announcement before the D4 review (P1.28).~~ **RESOLVED 7 November 2026
   (iteration-02 review):** confirmed from the Synadia/CNCF joint
   announcement of 1 May 2025 and closed decisively by CNCF graduation on
   2 November 2026 (see §5). The D4 review may cite §5 directly.
4. Confirm PostgreSQL 18.6 (latest minor) on the pinned server image at
   build time. ~~`uuidv7()` availability~~ — **RESOLVED 7 November 2026
   (iteration-02 review):** `uuidv7()` is native in PostgreSQL 18
   (see §6); keep the build-time `SELECT uuidv7()` only as a sanity check.
5. Confirm tracing 0.1.44 / tracing-subscriber 0.3.23 / tower 0.5.3 /
   tower-http 0.7.0 / OpenTelemetry 0.32.0 against docs.rs at build time.
6. Confirm Rust 1.100 stable status after 12 November 2026 before any
   toolchain bump (do not bump on the planned date alone). **Added
   7 November 2026 (iteration-02 review):** 1.100 carries above-normal
   breakage risk — never-type stabilization changing type inference and the
   Cargo target-directory layout v2 (Crater reports) — so a bump requires a
   `cargo +beta test` pass on the full workspace first, not just a version
   read.

## 9. Sources NOT used

- No blog summary was used as the version source for any crate; registry
  pages (lib.rs/docs.rs) and official release notes were used. Where only a
  secondary compilation was available (tracing, tower-http, OpenTelemetry),
  the entry says so and requires a build-time re-check.
- The "Tokio 2.0" claim (dev.to, March 2026) was found and rejected as
  unverified — no Tokio 2.0 exists in the registry record.
