# Plugin manifest schema — `flexcore-plugin-manifest-v1`

| Field | Value |
|---|---|
| Schema id | `flexcore-plugin-manifest-v1` |
| Version | 1.0.0 (the schema itself is SemVer'd; a breaking field change gets a new schema id) |
| Subtask | P7.05 (Phase 7, `ledger/phase-07-plugin-sdk/ledger_phase_7.md`) |
| Report trace | §5.5 (the illustrative example this schema formalises), §9.3 (validate before activation), §5.7 (permission vocabulary), §10.5 |
| Validator | `crates/flexcore-kernel/src/manifest.rs` (`flexcore-kernel` crate) |
| Status | Implemented, awaiting human build/test (loop bans; see the decision record `ledger/phase-07-plugin-sdk/05_06_manifest_resolution.md`) |

The report's §5.5 manifest example is **illustrative, not a final schema**
(§5.5 says so verbatim). This document is the published, versioned schema it
points at. Every deliberate difference from the §5.5 example is listed in
§8 below. The validator implements exactly this document; the tests
(`crates/flexcore-kernel/tests/manifest_p7_05.rs`) spot-check
schema-document ↔ implementation consistency, including this file.

## 1. Envelope

A manifest is a single JSON object. The first byte sequence the validator
commits to is the envelope:

- `manifest-schema` (REQUIRED, string): exactly `flexcore-plugin-manifest-v1`.
  Any other value — including a future-looking id — is rejected
  (`SCHEMA_VERSION_UNSUPPORTED`). New schema versions ship new ids and new
  validator support; nothing is parsed permissively against a future schema.
- Manifest source text is capped at 65 536 bytes (`MAX_MANIFEST_BYTES`).
  A manifest is metadata, not a payload.
- Unknown top-level fields are **REJECTED, not warned** (`UNKNOWN_FIELD`,
  recorded choice, P7.05 step 3). The same closed-key rule applies at every
  nested level (entry-point, resources, metadata, configuration-schema,
  secret objects) — there is no level at which an undeclared field is
  silently accepted.

## 2. Field table

| Field | Type | Req | Validation (enforced subset) |
|---|---|---|---|
| `manifest-schema` | string | yes | exactly `flexcore-plugin-manifest-v1` |
| `plugin-id` | string | yes | namespaced `<namespace>.<name>`: lowercase ASCII `[a-z0-9.-]`, ≥ 2 dot-separated segments, each starting with a letter, each ≤ 63 chars, total ≤ 128 (e.g. `neryva.sim-builtin-ess`) |
| `name` | string | yes | non-empty, ≤ 256 chars (display name) |
| `version` | string | yes | strict SemVer core `MAJOR.MINOR.PATCH`; numeric components, no leading zeros; prerelease/build metadata rejected in v1 |
| `api-version` | string | yes | comparator range: comma-separated list over `{=, >=, <=, >, <, ^, ~}` of SemVer cores (e.g. `">=0.5.0, <1.0.0"`, `"^0.5.0"` — three-component SemVer cores; two-component cores are rejected). Parsed as a RANGE, never string-compared; an empty range is invalid |
| `class` | string | yes | exactly one of `builtin-rust` (Class A), `wasm-component` (Class B), `grpc-worker` (Class C) |
| `entry-point` | object | yes | exactly ONE key, matching the class: `registered-type` (A), `symbol` — the WIT-exported symbol id (B), `target` — the gRPC target URI (C); non-empty string ≤ 256 |
| `provider-of` | array of strings | yes (may be empty) | contract ids `<domain>.<capability>`: ≥ 2 lowercase dot segments, each starting with a letter; duplicates rejected (`DUPLICATE_PROVIDER_DECLARATION`) |
| `consumes` | array of strings | yes (may be empty) | same contract-id rules; duplicates rejected (`DUPLICATE_CONSUMES_DECLARATION`) |
| `permissions` | array of strings | yes (may be empty) | ids from the closed §5.7 vocabulary only (§3 below) — never free text; duplicates rejected |
| `configuration-schema` | object | yes | the embedded declaration, structurally validated against the §5 subset — NOT full JSON Schema |
| `secrets` | array of objects | no (default `[]`) | references ONLY (§4 below) |
| `resources` | object | yes | closed keys: `max-memory-mb` (1..=65536) and `timeout-ms` (1..=3 600 000) REQUIRED; `max-output-mb` (1..=4096) and `max-calls-per-minute` (1..=1 000 000) optional; non-negative integers |
| `metadata` | object | yes | closed keys: `license` (REQUIRED, non-empty ≤ 128) and `description` (REQUIRED, non-empty ≤ 2000); `source` optional (non-empty ≤ 256) |

## 3. Permissions — the §5.7 vocabulary (ids, never free text)

Allowed to request (exactly these ids; scope is applied at grant time by the
profile/capability broker, P7.11 — it is not a manifest property):

| Id | §5.7 initial policy |
|---|---|
| `telemetry.read` | scoped to tenant/site/asset |
| `asset.read` | scoped |
| `forecast.publish` | allowed after schema validation |
| `schedule.propose` | candidate only |
| `dispatch.propose` | candidate only |
| `dispatch.validate.request` | request to protected validator |

Never granted — a manifest requesting one is rejected at the manifest gate
(`PERMISSION_NEVER_GRANTED`), the earliest point the §5.7 rule can bite
(P7.11/P7.12 re-enforce at grant and enforcement time):

| Id | §5.7 policy |
|---|---|
| `dispatch.authorize` | never granted to general plugins |
| `dispatch.issue` | owned by the protected dispatch service |
| `secret.read` | never granted directly; references resolved by host/adapter |
| `network.any` | prohibited by default |

Any other id: `PERMISSION_ID_UNKNOWN`.

## 4. Secrets — references only, structurally incapable of holding values

Each entry is an object with EXACTLY the keys `name` and `reference`
(anything else — a field named `value`, `secret_value`, `credential` — is an
`UNKNOWN_FIELD` rejection). `reference` must use one of the schemes
`env:NAME`, `file:PATH`, `secret-store:KEY` with a non-empty location. A
reference without a scheme prefix is rejected
(`SECRET_REFERENCE_INVALID`) as a likely inline credential. There is no
manifest field anywhere that can carry a secret value. Error messages never
echo the rejected reference value. Duplicate secret names:
`DUPLICATE_SECRET_NAME`.

## 5. `configuration-schema` — the enforced v1 subset (NOT full JSON Schema)

The declaration is validated structurally against exactly this subset:

```json
{
  "type": "object",                     // REQUIRED, literal "object"
  "additionalProperties": false,        // REQUIRED, literal false (fail-closed subset)
  "properties": {                       // optional object of property name → property schema
    "<property-name>": {
      "type": "string|number|integer|boolean|array|object",  // REQUIRED
      "description": "..."              // optional string
    }
  },
  "required": ["<property-name>"]       // optional; every name must be a key of properties
}
```

- Property names: non-empty, ≤ 64 chars, `[A-Za-z0-9_.-]`.
- Anything else (other keywords, `default`, nested `items`, regex patterns,
  conditional schemas) is rejected: `CONFIGURATION_SCHEMA_INVALID` or
  `UNKNOWN_FIELD` with the property-level location.
- Full JSON Schema is deliberately NOT implemented in v1 — recorded
  limitation; P7.19 (config validation at `Configured`) consumes this same
  subset so manifest schema and config validator cannot drift apart.

## 6. Version negotiation

- `Manifest::covers_api_version(v)` — range membership, parsed never string-
  compared (`^0.5.0` covers `0.5.3`, not `0.6.0`; `">=0.5,<1.0"` covers
  `0.9.9`, not `1.0.0`).
- The resolver (P7.06) additionally requires every manifest to cover the
  pinned platform plugin-API version (`0.5.0`,
  `resolve::CURRENT_PLATFORM_API_VERSION`) and every consumer–provider pair
  to have intersecting ranges (§9.3 compatibility matrix rule).

## 7. Named-error table (validator)

| Error name | Violation |
|---|---|
| `MANIFEST_JSON_INVALID` | source is not valid JSON (or exceeds the size cap) |
| `MANIFEST_NOT_AN_OBJECT` | parsed JSON is not an object |
| `SCHEMA_VERSION_UNSUPPORTED` | `manifest-schema` ≠ `flexcore-plugin-manifest-v1` |
| `UNKNOWN_FIELD` | a field outside the closed key set at any level |
| `MISSING_FIELD` | a required field is absent |
| `WRONG_TYPE` | a field has the wrong JSON type |
| `PLUGIN_ID_MALFORMED` | plugin id violates the §2 id rules |
| `INVALID_SEMVER` | `version` is not a valid SemVer core |
| `INVALID_API_VERSION_RANGE` | `api-version` is not a parseable comparator range |
| `UNKNOWN_CLASS` | `class` is not a §5.5 runtime kind |
| `MISSING_ENTRY_POINT` | `entry-point` is empty |
| `ENTRY_POINT_AMBIGUOUS` | `entry-point` declares more than one entry |
| `ENTRY_POINT_MISMATCH` | entry-point key does not match the declared class |
| `ENTRY_POINT_INVALID` | entry-point value empty or overlong |
| `CONTRACT_ID_MALFORMED` | a provider-of/consumes entry violates the contract-id rules |
| `DUPLICATE_PROVIDER_DECLARATION` | same contract id twice in `provider-of` |
| `DUPLICATE_CONSUMES_DECLARATION` | same contract id twice in `consumes` |
| `PERMISSION_ID_UNKNOWN` | permission id outside the §5.7 vocabulary |
| `PERMISSION_NEVER_GRANTED` | permission id in the §5.7 never-granted set |
| `DUPLICATE_PERMISSION_DECLARATION` | same permission requested twice |
| `CONFIGURATION_SCHEMA_INVALID` | embedded schema violates the §5 subset |
| `SECRET_REFERENCE_INVALID` | secret reference is not a scheme-prefixed location (likely an inline credential) |
| `DUPLICATE_SECRET_NAME` | two secret references share a name |
| `RESOURCE_BOUND_INVALID` | a resource bound is out of its closed range |

Resolver error names (P7.06) live in `crates/flexcore-kernel/src/resolve.rs`
and the decision record.

## 8. Deliberate differences from the §5.5 example (recorded, P7.05 step 4)

| §5.5 example | This schema | Reason |
|---|---|---|
| YAML source format | JSON objects | The validator (and this crate) has exactly one serialization dependency (`serde_json`, already in the workspace tree); no YAML parser exists in the workspace and none is introduced by P7.05. JSON manifest files are what the tooling loads; a YAML front-end can be added later without schema change. |
| `apiVersion: flexcore.neryva.io/v1` + `kind: Plugin` | single `manifest-schema: flexcore-plugin-manifest-v1` envelope field | One field carries both the schema identity and its version; `kind: Plugin` carried no information when only plugin manifests validate through this path. |
| `metadata.{id, version, displayName, publisher}` | top-level `plugin-id`, `version`, `name`; `metadata.{license, description, source}` | Keeps the identity fields the resolver and registry key on at the top level; `publisher` is deferred to P7.14 (signing/provenance), where it belongs with signature verification. `license` is REQUIRED (D4 discipline: a plugin with no declared license cannot be distributed). |
| `runtime.kind` / `runtime.entrypoint` | top-level `class` / `entry-point` | Same information; flatter, and `class` is the §5.4 term the whole SDK uses. The per-class entry-point KEY is enforced (`registered-type`/`symbol`/`target`), which the free-form example field did not constrain. |
| `compatibility.flexcoreApi` | `api-version` | Shorter; same semantics (SemVer range over the plugin API). |
| `capabilities.provides` / `capabilities.requires` | `provider-of` / `consumes` | Same semantics under the §5.2 three-role names (provider/consumer); the resolver's graph is built directly from these names. |
| `configuration.schema: ./config.schema.json` (external file ref) | embedded `configuration-schema` object (validated subset) | An external reference cannot be validated at manifest-validation time; the embedded subset can. P7.19 consumes the same embedded declaration. |
| `health.readiness` | ABSENT in v1 | Health semantics are a lifecycle/runtime concern (P7.07/P7.20 define the per-class health contract); a manifest field ahead of any health-check implementation would validate but mean nothing — exactly the recorded P7.05 risk. |
| no resource bounds beyond memory/time | `resources` adds `max-output-mb`, `max-calls-per-minute` | §5.4 Class B names fuel/time, concurrency, output-size, and call-rate limits; the two bounds a host can pin from the manifest are declared here. |

## 9. Examples

| File | Kind |
|---|---|
| `builtin-ess-provider.manifest.json` | VALID, Class A built-in provider |
| `forecast-worker.manifest.json` | VALID, Class C gRPC worker (with a secret reference) |
| `optimizer-wasm.manifest.json` | VALID, Class B WASM component |

### The deliberately-invalid example (each violation named)

```json
{
  "manifest-schema": "flexcore-plugin-manifest-v1",
  "plugin-id": "Bad Plugin",
  "name": "Hostile Example",
  "version": "1.0",
  "api-version": "0.5 or later",
  "class": "grpc-worker",
  "entry-point": { "registered-type": "wrong.key.for.class" },
  "provider-of": ["ess.simulation", "ess.simulation"],
  "consumes": ["portfolio.read"],
  "permissions": ["telemetry.read", "dispatch.authorize"],
  "configuration-schema": { "type": "object", "additionalProperties": true },
  "secrets": [ { "name": "db-password", "reference": "hunter2" } ],
  "resources": { "max-memory-mb": 0, "timeout-ms": 30000 },
  "metadata": { "license": "Apache-2.0", "description": "violates everything" },
  "extra-field": true
}
```

Violations, in the order the validator would report them if each earlier one
were fixed (each is a distinct named error):

1. `UNKNOWN_FIELD` — `extra-field` is not in the closed top-level key set (checked immediately after the envelope, before any field is interpreted).
2. `PLUGIN_ID_MALFORMED` — `plugin-id` has an uppercase character and a space, not a namespaced lowercase id.
3. `INVALID_SEMVER` — `version: "1.0"` is two components.
4. `INVALID_API_VERSION_RANGE` — `api-version` is prose, not a comparator range.
5. `ENTRY_POINT_MISMATCH` — a `grpc-worker` must declare `target`, not `registered-type`.
6. `DUPLICATE_PROVIDER_DECLARATION` — `ess.simulation` twice in `provider-of`.
7. `PERMISSION_NEVER_GRANTED` — `dispatch.authorize` is in the never-granted set.
8. `CONFIGURATION_SCHEMA_INVALID` — `additionalProperties` is `true`; the subset requires the literal `false` (fail-closed).
9. `SECRET_REFERENCE_INVALID` — `secrets[0].reference` `"hunter2"` is an inline credential value, not a scheme-prefixed reference (the value is never echoed back).
10. `RESOURCE_BOUND_INVALID` — `max-memory-mb: 0` is below the 1 MB floor.
11. `MISSING_FIELD` — removing `api-version` entirely yields `MISSING_FIELD` (each required field's absence is named; absence is checked before values are interpreted).

Every invalid case above is pinned as an individual test in
`crates/flexcore-kernel/tests/manifest_p7_05.rs` with its own error variant
asserted; the tests also assert the error names are pairwise distinct.
P7.41 (fuzz/property tests) targets this same validator.
