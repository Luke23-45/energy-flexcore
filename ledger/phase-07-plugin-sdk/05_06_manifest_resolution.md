# P7.05 + P7.06 — Plugin manifest schema and dependency resolution: IMPLEMENTED (awaiting human build/test)

Date: 2026-11-13. Subtasks: P7.05, P7.06, Phase 7 ledger
(`ledger_phase_7.md`). Owner role: Rust engineer (SDK) (per ledger).
Depends on: P7.05 ← P7.02 scope (concurrent — see §7); P7.06 ← P7.05
(satisfied in-wave: the resolver consumes the validator's output type).
Report trace: §5.5; §5.6; §5.7; §9.3; §8.1/§8.2; §22.1 (ADR-005, ADR-006,
ADR-010).

**Run-status honesty (AGENTS.md rule 7; the loop bans carried over from
Phase 6):** the code and tests in this wave are WRITTEN, NOT BUILT and NOT
RUN. No `cargo` command of any kind was executed (no build, no check, no
test, no clippy, no fmt). Every test below is UNRUN until the human
build/test pass executes it. Nothing in this document claims a test passed,
a build succeeded, or a compile completed. The only tools run were the three
permitted verification scripts (§8, output quoted verbatim).

## 1. Files (complete list; nothing outside it was touched)

| File | Status |
|---|---|
| `crates/flexcore-kernel/Cargo.toml` | NEW (P7.05/P7.06) |
| `crates/flexcore-kernel/src/lib.rs` | NEW (module map, crate-level design-for-later note) |
| `crates/flexcore-kernel/src/manifest.rs` | NEW (P7.05 validator + schema types) |
| `crates/flexcore-kernel/src/resolve.rs` | NEW (P7.06 resolver + protected-kernel guard) |
| `crates/flexcore-kernel/tests/manifest_p7_05.rs` | NEW (P7.05 tests) |
| `crates/flexcore-kernel/tests/resolve_p7_06.rs` | NEW (P7.06 tests) |
| `plugins/manifests/manifest-schema-v1.md` | NEW (published schema document, P7.05 step 2) |
| `plugins/manifests/builtin-ess-provider.manifest.json` | NEW (valid Class A example) |
| `plugins/manifests/forecast-worker.manifest.json` | NEW (valid Class C example, with a secret reference) |
| `plugins/manifests/optimizer-wasm.manifest.json` | NEW (valid Class B example) |
| `Cargo.toml` (root) | EDITED (ADDITIVE ONLY: one member entry + comment after `crates/edge-agent`; no existing line reordered or changed) |
| `tools/check_dependency_rule.py` | EDITED (ADDITIVE ONLY: one `COMPOSITION_CRATES` entry + comment, one `MEMBER_DIRS` entry; no existing line reordered or changed) |
| `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | EDITED (P7.05/P7.06 ticks ONLY — P7.01–P7.04 belong to the concurrent agent and were not touched) |
| this file | NEW (decision record) |

## 2. P7.05 — what was implemented

1. **The versioned manifest** (`flexcore-plugin-manifest-v1`): the field
   table lives in the published schema document
   (`plugins/manifests/manifest-schema-v1.md` §2); the validator is
   `crates/flexcore-kernel/src/manifest.rs`. It covers everything the ledger
   step 1 names: plugin id (namespaced), name, version (SemVer core),
   `api-version` comparator range, class (§5.4 Class A/B/C under the §5.5
   runtime-kind names `builtin-rust`/`wasm-component`/`grpc-worker`),
   `provider-of`/`consumes` contract ids, `permissions` (closed §5.7 id
   vocabulary — never free text), embedded `configuration-schema` (§4
   below), secret REFERENCES only, per-class `entry-point`, declared
   `resources`, and `metadata` (license REQUIRED, description REQUIRED,
   source optional).
2. **Schema versioned and published**: the schema id is a required envelope
   field validated against the constant `MANIFEST_SCHEMA_ID`; the document
   `manifest-schema-v1.md` is the published form, SemVer'd 1.0.0, with a
   differences-from-§5.5 table (§8 of the document — the ledger's recorded
   requirement).
3. **The validator** (`parse_manifest` / `parse_manifest_value`):
   unknown fields REJECTED at every level (recorded choice — reject, not
   warn; the rejection carries the field name, which is the log — no log
   dependency was added); missing required fields rejected; SemVer ranges
   PARSED as ranges (comparators `{=, >=, <=, >, <, ^, ~}`), never
   string-compared; strict fail-closed field order; a 64 KiB source cap.
4. **Differences from the §5.5 example** are recorded in the schema document
   §8 (YAML→JSON, envelope field, metadata shape, class/entry-point naming,
   embedded config schema, health.readiness ABSENT until P7.07/P7.20 define
   the semantics, added resource bounds). Each carries a reason.

### 2.1 The enforced JSON-subset (configuration-schema)

NOT full JSON Schema — a versioned structural subset, documented in the
schema document §5 and validated by `parse_configuration_schema`:

- root: `"type": "object"` (literal) and `"additionalProperties": false`
  (literal) both REQUIRED; allowed keys exactly
  `{type, properties, required, additionalProperties}`.
- `properties`: map of name → `{type: string|number|integer|boolean|array|object,
  description?}`; property names `[A-Za-z0-9_.-]`, ≤ 64 chars.
- `required`: optional array of names, each a key of `properties`, no
  duplicates.
- Anything else (other JSON-Schema keywords, `default`, `items`, `pattern`,
  conditionals) is a named rejection. P7.19 (config validation at
  `Configured`) must consume THIS subset so the manifest schema and the
  config validator cannot drift apart.

### 2.2 Secrets: references only, structurally incapable of values

Secret entries are objects with EXACTLY the keys `{name, reference}`;
`reference` must use `env:NAME`, `file:PATH`, or `secret-store:KEY`. A field
named `secret_value` anywhere, an inline credential string as a reference,
or any non-scheme-prefixed value is a named rejection
(`SECRET_REFERENCE_INVALID` / `UNKNOWN_FIELD`). Error text never echoes the
rejected reference value (it may be the credential itself) — pinned by a
test. There is no manifest field anywhere that can carry a secret value.

### 2.3 Parser dependency decision (recorded per the task's check-first rule)

The task allowed serde_json ONLY if already available to this crate. **It
is**: `serde_json = "1.0"` is pinned in the root `[workspace.dependencies]`
and already used by `flexcore-contracts` and `flexcore-simulator`. The
kernel therefore takes `serde_json = { workspace = true }` — **no new
external dependency is introduced by P7.05** (the crate's Cargo.toml header
records this). Parsing uses `serde_json::Value` with strict manual field
extraction and closed key sets — never derive-on-untrusted-input — so
unknown fields are rejections and no proc-macro surface faces plugin input.
A hand-rolled reader (the `edge-agent` openems style) was considered and
rejected: it would duplicate a parser the workspace already pins and trusts.

## 3. P7.06 — what was implemented

`crates/flexcore-kernel/src/resolve.rs`: a PURE resolver over validated
manifests. Same inputs → identical plan (no HashMap iteration anywhere in
outputs; `BTreeMap`/`BTreeSet` and explicit sorts only). No activation code
exists in the crate — a failing resolution leaves nothing behind (the
ledger's "confirm no activation occurs" is satisfied structurally, and the
P7.08 registry is the future consumer of the plan, not this module).

1. **Resolution**: consumers' `consumes` are matched against providers'
   `provider-of` (§5.2 three-role vocabulary; the §5.5 example's
   `provides`/`requires` under different names — recorded in the schema doc
   §8). Provider maps, per-contract resolution, dependency edges, and the
   plan are all emitted sorted.
2. **Failure modes, each a distinct named error** (table in §5 below):
   `MISSING_PROVIDER`, `UNSUPPORTED_API_VERSION` (consumer/provider
   `api-version` ranges must INTERSECT — the §9.3 compatibility-matrix rule,
   with boundary inclusivity handled), `CYCLIC_DEPENDENCY` (DFS cycle
   detection; the cycle is NAMED as the ordered plugin-id path, first node
   repeated at the end; a manifest consuming its own provided contract is a
   named self-cycle), `CONFLICTING_PROVIDERS` (two or more providers for one
   contract without an explicit selection — never silent first-wins).
3. **The conflict-resolution rule, in writing (ledger step 3)**: a
   multi-provider contract resolves ONLY via an explicit selection in the
   composition input (`ExplicitSelections`: contract id → plugin id). There
   is no priority field, no first-wins, no default. `SELECTION_EXPLICIT`
   (the ledger's fifth mode) is the SUCCESS marker:
   `SelectionBasis::ExplicitSelection` (machine name `SELECTION_EXPLICIT`)
   on the resolved contract, deterministic across runs and input orders.
   Selections referencing a contract nobody provides, or a plugin that does
   not provide the named contract, are `SELECTION_UNRESOLVED` — never
   silently ignored.
4. **Deterministic ordering (ledger risk: P7.42 depends on it)**: the plan's
   registration order is the lexicographically-smallest topological order
   (Kahn's algorithm over a sorted ready set, ties by plugin id); contracts
   are sorted by id; consumer lists are sorted. Tests pin plan equality
   across runs and across shuffled input orders.
5. **Plan type designed for P7.08/P7.16**: `CompositionPlan {
   registrations: Vec<PlanRegistration>, contracts: Vec<ResolvedContract> }`
   — the ordered registration list (plugin id, version, class, sorted
   provides/consumes) and the resolved contract table (contract, provider,
   selection basis, sorted consumers). P7.08 registers in `registrations`
   order at the composition root; P7.16 re-composition re-runs the resolver
   and diffs plans. Only resolution is implemented; the registry, lifecycle,
   and re-composition are deliberately NOT stubbed (P1.03 rule: no empty
   scaffolding).
6. **Protected-kernel guard**: every manifest's `provider-of` is checked
   against the reserved-id list (§6 below); a claim is
   `PROTECTED_KERNEL_REPLACE` — the §5.7/ADR-010 non-replaceability rule
   made structural. The guard runs before provider resolution, so a hostile
   manifest fails on the boundary violation even if it would also fail for
   other reasons (fail-closed ordering, pinned by a test).

## 4. Named-error tables

### 4.1 Manifest validator (P7.05) — 24 names (`ManifestError::name`)

`MANIFEST_JSON_INVALID`, `MANIFEST_NOT_AN_OBJECT`,
`SCHEMA_VERSION_UNSUPPORTED`, `UNKNOWN_FIELD`, `MISSING_FIELD`, `WRONG_TYPE`,
`PLUGIN_ID_MALFORMED`, `INVALID_SEMVER`, `INVALID_API_VERSION_RANGE`,
`UNKNOWN_CLASS`, `MISSING_ENTRY_POINT`, `ENTRY_POINT_AMBIGUOUS`,
`ENTRY_POINT_MISMATCH`, `ENTRY_POINT_INVALID`, `CONTRACT_ID_MALFORMED`,
`DUPLICATE_PROVIDER_DECLARATION`, `DUPLICATE_CONSUMES_DECLARATION`,
`PERMISSION_ID_UNKNOWN`, `PERMISSION_NEVER_GRANTED`,
`DUPLICATE_PERMISSION_DECLARATION`, `CONFIGURATION_SCHEMA_INVALID`,
`SECRET_REFERENCE_INVALID`, `DUPLICATE_SECRET_NAME`,
`RESOURCE_BOUND_INVALID`.

### 4.2 Resolver (P7.06) — 10 names (`ResolveError::name`)

`EMPTY_MANIFEST_SET`, `MANIFEST_SET_TOO_LARGE`,
`DUPLICATE_PLUGIN_ID`, `PROTECTED_KERNEL_REPLACE`, `MISSING_PROVIDER`,
`UNSUPPORTED_API_VERSION`, `PLATFORM_API_UNSUPPORTED`, `CYCLIC_DEPENDENCY`,
`CONFLICTING_PROVIDERS`, `SELECTION_UNRESOLVED`. Plus the success marker
`SELECTION_EXPLICIT` (`SelectionBasis::ExplicitSelection.name()`; the
unique-provider case is `SELECTION_UNIQUE_PROVIDER`).

Both tables are printed in the published schema document; the tests assert
every name appears in that document (schema-doc ↔ implementation consistency
spot check) and that the names are pairwise distinct.

## 5. Validator↔resolver contract

The resolver consumes `manifest::Manifest` — the validator's output type —
so a manifest that reached the resolver has already passed the P7.05 gate.
The resolver adds: identity uniqueness, the protected-kernel guard, the
platform API gate (`CURRENT_PLATFORM_API_VERSION = 0.5.0`, pinned constant —
the §2.1 v0.5.0 SDK baseline), provider/compatibility/cycle/conflict
resolution, and the plan.

## 6. Reserved contract-id list (protected kernel) and citations

Hardcoded in `resolve::PROTECTED_KERNEL_CONTRACT_IDS` (the P7.04 seam record
is being written CONCURRENTLY by another agent and was not citable at write
time; when it lands, the ids must be reconciled against the seam map — the
citations below are from the report directly, per the task):

| Reserved id | Protected invariant | Report citation |
|---|---|---|
| `kernel.command-authorizer` | authorization | §9.1 (`CommandAuthorizer` — protected core); §5.1 |
| `kernel.dispatch-gateway` | dispatch invocation boundary | §9.1 (`DispatchGateway` — protected core); §5.7 |
| `kernel.dispatch-validation` | final dispatch validation | §5.1; §5.7 |
| `kernel.command-lifecycle` | command lifecycle | §5.1; §14.1 |
| `kernel.audit-integrity` | audit integrity | §5.1 |
| `kernel.tenant-isolation` | tenant isolation | §5.1; §16.2 |

Cross-references: ADR-005 (protected kernel non-replaceable), §5.7 (a plugin
can never select the control-authority mode), the P6.29 handover §3 ("the
hard boundary": no plugin capability may map to enabling/re-scoping the
command gate), and the never-granted permission set of §5.7
(`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`) which
the P7.05 validator refuses one layer earlier. These are contract-id
RESERVATIONS — no contract by these names exists (and none ever should as a
plugin-implementable seam); the reservation makes claiming them a structural
refusal.

## 7. DEFAULT-labelled decisions (recommendations awaiting human sign-off)

| # | Decision | Label |
|---|---|---|
| D-P7.05-1 | JSON manifest format (the §5.5 example is YAML); no YAML dependency added | DEFAULT — a YAML front-end can be added later without schema change |
| D-P7.05-2 | Single `manifest-schema` envelope field instead of `apiVersion` + `kind` | DEFAULT |
| D-P7.05-3 | `license` REQUIRED in metadata (a plugin with no declared license cannot be distributed; D4 discipline) | DEFAULT |
| D-P7.05-4 | Prerelease/build metadata rejected in manifest `version` for schema v1 | DEFAULT — revisit together with P7.14 signing, where pre-releases need provenance semantics |
| D-P7.05-5 | `health.readiness` ABSENT from schema v1 | DEFAULT — P7.07/P7.20 own health semantics; a field ahead of any health-check implementation would validate but mean nothing (the recorded P7.05 risk) |
| D-P7.05-6 | Permission scope applied at grant time (P7.11 broker/profile), not a manifest property | DEFAULT — consistent with §5.7's scoping column |
| D-P7.06-1 | Consumer↔provider compatibility = api-version range INTERSECTION (plus the platform-version gate) | DEFAULT — the §9.3 matrix's mechanical reading; a per-contract version matrix (contract ids pinned to specific contract versions) is P7.04/P7.09 material |
| D-P7.06-2 | Pinning a UNIQUE provider via explicit selection is allowed (basis `SELECTION_EXPLICIT`) | DEFAULT |
| D-P7.06-3 | Composition size cap 1024 manifests (`MAX_COMPOSITION_MANIFESTS`, bounds resolver work and DFS depth) | DEFAULT |
| D-P7.06-4 | Resolver check order: identity → protected-kernel guard → platform gate → selection validation → conflict resolution → missing/version checks → cycle detection → plan | DEFAULT — deterministic and fail-closed; pinned by tests |

## 8. Honest limitations (UNRUN and otherwise)

1. **UNRUN**: everything above is written, not built and not run. In
   particular the hand-checked compile reasoning (lifetimes in the resolver
   borrow graph, trait bounds, JSON map borrow chains) has not been checked
   by a compiler. The human build/test pass must run `cargo fmt`, `cargo
   clippy --workspace --all-targets -- -D warnings`, and `cargo test
   -p flexcore-kernel` and record results.
2. **No full JSON Schema.** The configuration schema is the §2.1 subset.
   Anything richer must wait for a schema-engine decision (and a license
   audit) — recorded, not half-built.
3. **serde_json duplicate-key collapse.** `serde_json::Value` maps silently
   keep the LAST occurrence of a duplicated JSON key; rejecting duplicate
   keys needs a custom parser (or the `raw-input` escalation) and is NOT
   implemented. Recorded because P7.41 names duplicate keys as fuzz input —
   the fuzz task must treat duplicate keys as a known accepted-collapse case
   or add the parser.
4. **No YAML.** Manifests are JSON only (D-P7.05-1); the §5.5 example's YAML
   shape is documented as a deliberate difference.
5. **No fuzz/property tests yet.** P7.41 owns them; this wave ships the
   fixed example set the ledger asks for. The validator is written to be
   fuzzable (total: every input → Ok or a named error; no `unwrap`/`expect`
   on untrusted input in `src/`; bounded input size).
6. **Reserved ids are hardcoded from the report, not from P7.04** (which is
   being written concurrently). Reconciliation is a follow-up the moment
   P7.04 lands; if P7.04 names additional protected seams, this list and the
   guard must be extended.
7. **Cycle detection is recursive DFS** with depth bounded by
   `MAX_COMPOSITION_MANIFESTS` (1024) — deep chains beyond that are refused
   by the cap, not crashed into; the cap is a DEFAULT (D-P7.06-3).
8. **`metadata.publisher` is deferred to P7.14** (signing/provenance), where
   it belongs with signature verification; the §5.5 example's `publisher`
   field is recorded as a deliberate difference.
9. **No `verify_docs.py`-visible prose changed**: the report and foundations
   were not touched; this wave's documentation is the schema document and
   this record.

## 9. Verification actually run (the three permitted tools)

Run 2026-11-13, from the repository root (Windows, Git Bash, `python -I`):

```text
$ python -I tools/check_dependency_rule.py
OK — dependency rule holds for 18 workspace members.
[exit 0]

$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
[exit 0]

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
[exit 0]
```

All three tools pass after the ledger tick. 18 workspace members = the
previous 17 + `crates/flexcore-kernel`. No cargo command was executed.

## 10. Ledger ticks

Only P7.05 and P7.06 were ticked, in the exact ledger wording, with the
required suffix. P7.01–P7.04 belong to the concurrent agent and were not
touched.
