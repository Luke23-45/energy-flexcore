# Manifest authoring guide — `flexcore-plugin-manifest-v1`

Every FlexCore plugin — Class A, B, and C — is declared by a JSON manifest
validated by `crates/flexcore-kernel/src/manifest.rs`. The authoritative
field table is `plugins/manifests/manifest-schema-v1.md`; this guide is the
author's condensed walkthrough. The validator **rejects, never warns**: the
first violation in the documented field order aborts the parse, and every
violation class has a DISTINCT stable machine name.

## 1. The envelope

```json
{
  "manifest-schema": "flexcore-plugin-manifest-v1",
  "...": "everything else below"
}
```

- `manifest-schema` MUST be exactly `flexcore-plugin-manifest-v1`; anything
  else (including a future-looking id) is `SCHEMA_VERSION_UNSUPPORTED`.
- The whole manifest is capped at 65 536 bytes (`MAX_MANIFEST_BYTES`);
  oversize input is refused before parsing as `MANIFEST_JSON_INVALID`.
- Unknown fields at ANY level (manifest, `entry-point`, `resources`,
  `metadata`, `configuration-schema`, secret objects) are rejected with
  `UNKNOWN_FIELD` — there is no level where an undeclared field is
  silently accepted.

## 2. Field table (condensed)

| Field | Req | Rules | Failure name |
|---|---|---|---|
| `plugin-id` | yes | `<namespace>.<name>`; lowercase ASCII `[a-z0-9.-]`; ≥ 2 dot segments, each starting with a letter, each ≤ 63 chars, total ≤ 128 | `PLUGIN_ID_MALFORMED` |
| `name` | yes | non-empty, ≤ 256 chars | `WRONG_TYPE` |
| `version` | yes | strict SemVer core `MAJOR.MINOR.PATCH`; no prerelease/build metadata, no leading zeros, two-component cores rejected | `INVALID_SEMVER` |
| `api-version` | yes | comparator range over `{=, >=, <=, >, <, ^, ~}` of three-component SemVer cores (e.g. `">=0.5.0, <1.0.0"`, `"^0.5.0"`); parsed as a RANGE, never string-compared; empty range invalid | `INVALID_API_VERSION_RANGE` |
| `class` | yes | exactly `builtin-rust` \| `wasm-component` \| `grpc-worker` | `UNKNOWN_CLASS` |
| `entry-point` | yes | exactly ONE key matching the class: `registered-type` (A), `symbol` — the WIT-exported symbol id (B), `target` — the gRPC target URI (C); non-empty ≤ 256 | `MISSING_ENTRY_POINT` / `ENTRY_POINT_AMBIGUOUS` / `ENTRY_POINT_MISMATCH` / `ENTRY_POINT_INVALID` |
| `provider-of` | yes (may be empty) | contract ids `<domain>.<capability>`; duplicates rejected | `CONTRACT_ID_MALFORMED` / `DUPLICATE_PROVIDER_DECLARATION` |
| `consumes` | yes (may be empty) | same id rules; duplicates rejected | `CONTRACT_ID_MALFORMED` / `DUPLICATE_CONSUMES_DECLARATION` |
| `permissions` | yes (may be empty) | ids from the closed §5.7 vocabulary only — never free text | `PERMISSION_ID_UNKNOWN` / `PERMISSION_NEVER_GRANTED` / `DUPLICATE_PERMISSION_DECLARATION` |
| `configuration-schema` | yes | the enforced v1 subset (see §5 below) | `CONFIGURATION_SCHEMA_INVALID` / `MISSING_FIELD` / `UNKNOWN_FIELD` |
| `secrets` | no (default `[]`) | references ONLY (see §6 below) | `SECRET_REFERENCE_INVALID` / `DUPLICATE_SECRET_NAME` / `UNKNOWN_FIELD` |
| `resources` | yes | closed keys: `max-memory-mb` (1..=65536) and `timeout-ms` (1..=3 600 000) REQUIRED; `max-output-mb` (1..=4096) and `max-calls-per-minute` (1..=1 000 000) optional | `MISSING_FIELD` / `RESOURCE_BOUND_INVALID` / `WRONG_TYPE` |
| `metadata` | yes | closed keys: `license` (REQUIRED — a plugin with no declared license cannot be distributed), `description` (REQUIRED), `source` optional | `MISSING_FIELD` / `WRONG_TYPE` / `UNKNOWN_FIELD` |

The complete named-error table (all 24 validator names) is
`plugins/manifests/manifest-schema-v1.md` §7.

## 3. A valid example (Class C, annotated)

From the shipped set (`plugins/manifests/forecast-worker.manifest.json`):

```json
{
  "manifest-schema": "flexcore-plugin-manifest-v1",
  "plugin-id": "acme.forecast-worker",
  "name": "Acme Day-Ahead Forecast Worker",
  "version": "0.3.1",
  "api-version": "^0.5.0",
  "class": "grpc-worker",
  "entry-point": { "target": "https://forecast-worker.internal:9443" },
  "provider-of": ["forecast.day-ahead"],
  "consumes": ["telemetry.history"],
  "permissions": ["telemetry.read", "forecast.publish"],
  "configuration-schema": { "...": "the §5 subset with required [\"horizon-hours\"]" },
  "secrets": [ { "name": "worker-credentials", "reference": "secret-store:forecast/worker" } ],
  "resources": {
    "max-memory-mb": 512, "timeout-ms": 30000,
    "max-output-mb": 16, "max-calls-per-minute": 600
  },
  "metadata": { "license": "Apache-2.0", "description": "Out-of-process Class C forecast worker." }
}
```

## 4. The permission vocabulary (§5.7 — ids, never free text)

| Allowed to request | §5.7 initial policy |
|---|---|
| `telemetry.read` | scoped to tenant/site/asset |
| `asset.read` | scoped |
| `forecast.publish` | allowed after schema validation |
| `schedule.propose` | candidate only |
| `dispatch.propose` | candidate only |
| `dispatch.validate.request` | request to protected validator |

| Never granted (requesting one is `PERMISSION_NEVER_GRANTED` at the manifest gate) | §5.7 policy |
|---|---|
| `dispatch.authorize` | never granted to general plugins |
| `dispatch.issue` | owned by the protected dispatch service |
| `secret.read` | never granted directly; references resolved by host/adapter |
| `network.any` | prohibited by default |

Scope is applied at GRANT time by the profile ceiling and the capability
broker (P7.11/P7.15) — it is not a manifest property. A manifest permission
above its profile's ceiling is refused (`PROFILE_PERMISSION_CEILING_EXCEEDED`
at plan validation, `PERMISSION_ABOVE_PROFILE` at the host door), never
trimmed to fit.

## 5. `configuration-schema` — the enforced v1 subset

```json
{
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "<property-name>": { "type": "string|number|integer|boolean|array|object", "description": "..." }
  },
  "required": ["<property-name>"]
}
```

- `type` (literal `"object"`) and `additionalProperties` (literal `false`)
  are REQUIRED.
- Property names: non-empty, ≤ 64 chars, `[A-Za-z0-9_.-]`.
- Other keywords (`default`, `items`, regex patterns, conditional schemas)
  are rejected: `CONFIGURATION_SCHEMA_INVALID` or `UNKNOWN_FIELD`.
- Full JSON Schema is deliberately NOT implemented in v1 (recorded
  limitation); the same embedded subset is what the operator-configuration
  resolver (P7.19) validates values against, so the two cannot drift.

## 6. Secrets — references only, structurally incapable of holding values

- Each entry is an object with EXACTLY the keys `name` and `reference`.
  A key named `value`, `credential`, or anything else is `UNKNOWN_FIELD`.
- `reference` must use `env:NAME`, `file:PATH`, or `secret-store:KEY` with a
  non-empty location; a scheme-less string is refused as a likely inline
  credential (`SECRET_REFERENCE_INVALID`), and the rejected value is never
  echoed back in the error.
- There is no manifest field anywhere that can carry a secret value. The
  runtime half of this rule (config values, handles, the
  `SECRET_VALUE_IN_CONFIG` refusal) is
  [configuration-and-secrets.md](configuration-and-secrets.md).

## 7. Version negotiation (what `api-version` commits you to)

- Your range must cover the running platform plugin-API version (`0.5.0`,
  pinned as `CURRENT_PLATFORM_API_VERSION`), else the resolver refuses with
  `PLATFORM_API_UNSUPPORTED`.
- Every consumer↔provider pair your `consumes` entries link you to must
  have intersecting ranges, else `UNSUPPORTED_API_VERSION` (report §9.3
  compatibility matrix rule).

## 8. Common named errors and what they actually mean

| Error name | What it means | Typical author mistake |
|---|---|---|
| `UNKNOWN_FIELD` | a key outside the closed set at that level | copying the report's illustrative §5.5 example fields (`apiVersion`, `kind`, `runtime.kind`) verbatim — they are NOT this schema |
| `INVALID_SEMVER` | version is not a three-component numeric core | `"1.0"` or `"1.0.0-beta.1"` (prerelease is rejected in v1: it needs the P7.14 provenance semantics) |
| `INVALID_API_VERSION_RANGE` | the range is prose or a two-component core | `"0.5 or later"`, `">=0.5,<1.0"` |
| `ENTRY_POINT_MISMATCH` | the entry-point key does not match the class | a `grpc-worker` declaring `registered-type` |
| `PERMISSION_NEVER_GRANTED` | a never-granted id was requested | requesting `dispatch.authorize` |
| `PERMISSION_ID_UNKNOWN` | an id outside the §5.7 vocabulary | free-text permissions |
| `SECRET_REFERENCE_INVALID` | a scheme-less secret reference | pasting the credential itself into `reference` |
| `MISSING_FIELD` | a required field absent (including `resources.max-memory-mb` / `timeout-ms`) | omitting resource bounds — the bounds are what the host enforces; a manifest without them is refused, not defaulted |
| `RESOURCE_BOUND_INVALID` | a bound out of its closed range | `max-memory-mb: 0` |
| `CONFIGURATION_SCHEMA_INVALID` | the embedded schema violates the §5 subset | `additionalProperties: true` |

A manifest that fails is fixed by editing the manifest — nothing is
half-accepted, and no warning log exists to ignore.

## 9. Validation walkthrough (PREPARED-UNRUN)

The validator runs inside the kernel crate's test suite; there is no CLI in
v1. On a built workspace the human pass runs:

```sh
cargo test -p flexcore-kernel --test manifest_p7_05
```

Written, NEVER RUN in this loop (the loop bans; the ledger tick discipline).
The tests pin every named error above, including the deliberately-invalid
example in `plugins/manifests/manifest-schema-v1.md` §9 and the bad-plugin
example in [examples/bad-plugin/](examples/bad-plugin/README.md).
