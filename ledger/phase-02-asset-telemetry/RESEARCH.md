# RESEARCH — Phase 2: Asset and Telemetry Foundation

Research cut-off for this ledger: **7 November 2026** (same as report §27).
Checked: **7 November 2026**. Sources are primary only (the project's own
documentation or release pages), except where explicitly labelled secondary.
Anything unverified is labelled **ASSUMPTION** or written as an open
verification task. Re-verify at build time.

## 1. PostgreSQL declarative partitioning (telemetry storage, P2.05–P2.06)

| Item | Finding | Source |
|---|---|---|
| Feature | Built-in declarative partitioning since PostgreSQL 10: `PARTITION BY RANGE / LIST / HASH`. The partitioned table is virtual (no storage of its own); rows route to partitions by the partition key; updating the partition key moves the row | PostgreSQL docs (current = 18), https://www.postgresql.org/docs/current/ddl-partitioning.html — **opened and read 6 November 2026** |
| Range bounds | Inclusive at the lower end, exclusive at the upper end | Same docs page |
| When it pays | Benefits normally worthwhile only when a table would otherwise be very large; rule of thumb: table size exceeds the physical memory of the database server | Same docs page |
| Bulk load/delete | Dropping/detaching a partition is far faster than bulk `DELETE` and avoids `VACUUM` overhead; `DETACH PARTITION ... CONCURRENTLY` (PG 14+) needs only a `SHARE UPDATE EXCLUSIVE` lock | Same docs page |
| No auto-creation | Inserting a row that maps to no existing partition errors; an appropriate partition must be added manually (pre-create ahead of the frontier) | Same docs page |
| PK/unique constraint rule | Any primary key or unique constraint on a partitioned table must include all partition-key columns | Same docs page (and secondary partitioning guides consistent with it) |
| Pruning | `enable_partition_pruning` (on by default) lets the planner skip partitions that cannot match the `WHERE` clause | Same docs page |
| Sub-partitioning | Partitions may themselves be partitioned tables | Same docs page |

**Implication for Phase 2:** P2.05's decisions (partition key in the PK,
pre-creation job, detach/drop retention, `EXPLAIN` pruning tests) follow
directly from this page. The "table must exceed server memory" rule of thumb
means partitioning in Phase 2 is justified as the *designed* shape for the
telemetry volume trajectory (§16.1), not by a measured Phase 2 size — record
that honestly in P2.05.

## 2. PostgreSQL 18 (telemetry store version)

| Item | Finding | Source |
|---|---|---|
| Current major | **PostgreSQL 18**, released 25 September 2025 | PostgreSQL press release, https://www.postgresql.org/about/news/postgresql-18-released-3142/ |
| Latest minor | **18.6, released 13 August 2026** (18.5 never released) | **CONFIRMED 7 November 2026 (final review), primary source:** PostgreSQL announcement, https://www.postgresql.org/about/news/postgresql-186-1711-1615-1519-1424-and-19-beta-3-released-3365/ — the earlier secondary-only qualifier is retired; keep the build-time `SELECT version()` as a cheap sanity check |
| `uuidv7()` | Built-in in PostgreSQL 18+ (RFC 9562 time-ordered UUIDs) | **CONFIRMED 7 November 2026 (iteration-02 review):** PostgreSQL 18 release announcement, https://www.postgresql.org/about/news/postgresql-18-released-3142/ ; multiple independent write-ups agreeing `uuidv7()` is native in PG 18 with no extension. The earlier secondary-only qualifier is retired; keep the build-time `SELECT uuidv7()` as a cheap sanity check, not as an open question |
| Support | PostgreSQL 18 supported until 14 November 2030 | postgresql.org versioning/FAQ |

**Pin for Phase 2:** PostgreSQL **18** (same as Phase 1; re-verify the minor
at build time).

## 3. NATS JetStream (telemetry event transport, P2.16)

| Item | Finding | Source |
|---|---|---|
| Server version | **2.15** (17 September 2026, "Latest"); latest patch **v2.15.2** (published 26 October 2026; "5 November 2026" in the earlier draft was a secondary-source crawl date, corrected 7 November 2026 in the final assembly pass). **CORRECTED 7 November 2026 (iteration-02 review):** v2.15.2 does NOT fix CVE-2026-33222. CVE-2026-33222 (JetStream stream-restore endpoint auth bypass, GHSA-9983-vrx2-fg9c, published 24 March 2026, CVSS 4.9) was fixed in **2.11.15 and 2.12.6**; every 2.15.x release merely includes that fix. The v2.15.2 changelog item is CVE-2026-111943 (eMudhra certificate distrust, Go 1.27.3). Sibling **CVE-2026-33223** (Nats-Request-Info header spoof, CVSS 6.4, same fix versions 2.11.15 / 2.12.6) — added 7 November 2026 (iteration-09 review). | NATS docs, https://docs.nats.io/release-notes/ ; CVE records for CVE-2026-33222 (Wiz, Vulert, Feedly — all agreeing the fixed releases are 2.11.15 / 2.12.6) |
| Semantics | At-least-once delivery; consumers must be idempotent; do not promise exactly-once from broker acknowledgements alone | NATS docs, https://docs.nats.io/concepts/jetstream (report [R6]) |
| Durability caution | Jepsen (NATS 2.12.1, Dec 2025) found write loss under file truncation/corruption on a minority of nodes and documented the default fsync policy (flush every two minutes, not before acknowledgement); NATS has since documented the default fsync policy | Jepsen, https://jepsen.io/analyses/nats-2.12.1.pdf |

**Implication for Phase 2:** telemetry events go through the outbox (P1.15)
and idempotent consumers (P1.16); the broker is never the durability
guarantee for telemetry. The fsync policy is a deployment decision.

## 4. TimescaleDB — evaluated, not adopted (P2.05)

| Item | Finding | Source |
|---|---|---|
| Report position | TimescaleDB is an *optional* time-series extension; evaluate as an optional deployment/profile; do not make proprietary/optional features mandatory for the open baseline (§16.1) | Report §16.1 |
| Phase 2 decision | Not adopted. Revisit trigger: measured telemetry volume requiring a dedicated time-series store (§25.1 revisit trigger for PostgreSQL) | Report §16.1, §25.1 |

**Open verification task:** no version was checked for TimescaleDB in this
ledger, because it is not adopted. If a later phase proposes it, check the
current version and license from the TimescaleDB official documentation
before proposing adoption. Do not cite a version from memory.

## 5. Telemetry / time-series design references

No separate time-series database or telemetry protocol was adopted in Phase 2,
so no further primary-source version checks apply. The following were
considered and deliberately not used as sources for design decisions:

- Vendor time-series marketing pages (not primary sources for FlexCore's
  schema decisions; the schema follows report §10/§16 and the PostgreSQL
  partitioning docs above).
- **ASSUMPTION:** none. The partition-interval choice (P2.05) is a design
  decision recorded with its reasoning, not a sourced fact.

## 6. Simulator and benchmark cases (P2.13, P2.25)

| Item | Finding | Source |
|---|---|---|
| Benchmark feeders | IEEE 13-bus, 34-bus, and 123-bus distribution test feeders "where licensing and model availability are confirmed" (§19.3) — Phase 2 uses synthetic portfolios, not IEEE feeders; feeder cases belong to later (grid) phases | Report §19.3 |
| Synthetic portfolios | BESS, PV, flexible load, EVSE synthetic portfolios are the Phase 2 vehicle (§19.3) | Report §19.3 |
| Scenario location | `scenarios/` tree: `ieee-feeders/`, `synthetic-portfolio/`, `failure-injection/` (§8.1) | Report §8.1 |

No IEEE feeder model was downloaded or verified in this ledger — Phase 2
needs none. If a later phase adopts an IEEE feeder, confirm licensing and
model availability first (§19.3) and record the source.

## 7. Open verification tasks (Phase 2)

1. Re-verify PostgreSQL 18.6 (latest minor) and `uuidv7()` on the pinned
   server at build time (P2.06). **Update 7 November 2026 (iteration-02
   review):** `uuidv7()` is confirmed native in PostgreSQL 18 (see §2);
   the build-time `SELECT uuidv7()` is a sanity check only.
2. ~~Confirm the NATS 2.15 patch level and the CVE-2026-33222 fix against the
   NATS GitHub releases before the exit-gate run (P2.26).~~ **RESOLVED
   7 November 2026 (iteration-02 review):** CVE-2026-33222 was fixed in
   2.11.15 / 2.12.6 (see §3); the task as written (implying the fix landed
   in 2.15.2) was based on the wrong attribution. The remaining build-time
   action is the routine latest-2.15-patch re-check before the exit-gate run.
3. If TimescaleDB is ever proposed, check its current version and license
   from official TimescaleDB documentation first (P2.05).
4. Before any IEEE feeder is adopted (later phase), confirm licensing and
   model availability and record the source (§19.3).
