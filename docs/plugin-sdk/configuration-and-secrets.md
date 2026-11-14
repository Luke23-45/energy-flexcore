# Configuration and secrets (P7.19)

Sources of record: `crates/flexcore-kernel/src/plugin_config.rs`,
`crates/flexcore-kernel/src/manifest.rs` (the embedded schema), report
§5.7 (secret references resolved by host/adapter).

## 1. Where configuration comes from

A plugin's manifest declares its `configuration-schema` (the enforced v1
subset — see [manifest-guide.md](manifest-guide.md) §5). The OPERATOR
supplies actual values in a configuration document, which the kernel
resolves against the manifest's declared schema at the
`Compatible → Configured` gate. The same embedded declaration is used by
both the manifest validator and the config resolver, so the two cannot
drift.

## 2. The rules, each with its named error

| Rule | Named error (verified against `plugin_config.rs::ConfigError`) |
|---|---|
| Config source capped at 65 536 bytes | `CONFIG_TOO_LARGE` |
| Config must be a JSON object | `CONFIG_NOT_AN_OBJECT` |
| No property outside the manifest's declared schema (re-refused over the resolved surface) | `UNKNOWN_PROPERTY` |
| Every REQUIRED property present (unless an EXPLICIT default exists) | `MISSING_REQUIRED` |
| Values typed against the declared property type (`string|number|integer|boolean|array|object`) | `CONFIG_WRONG_TYPE` |
| Enum-constrained properties hold only allowed values | `ENUM_VALUE_NOT_ALLOWED` |
| Bounded properties hold only in-range values | `BOUND_VIOLATION` |
| A plain (non-reference) value in a secret-shaped property | `SECRET_VALUE_IN_CONFIG` |
| A config property named exactly like a manifest secret (a smuggling attempt) | `SECRET_FIELD_COLLISION` |
| A malformed reference string (scheme present, empty location) | `SECRET_REF_INVALID` |
| Constraints document (see §4) violations | `CONSTRAINTS_DOCUMENT_INVALID`, `CONSTRAINTS_MISSING_FIELD`, `CONSTRAINTS_UNKNOWN_FIELD`, `CONSTRAINT_UNDECLARED`, `CONSTRAINT_TYPE_MISMATCH`, `CONSTRAINTS_TOO_LARGE` |
| Configuration bound to a different manifest version than the one instantiated | `CONFIG_DRIFT` (re-instantiation required) |

## 3. The defaults-recorded rule

Absent optional properties are filled from the layered constraints
document's `default` entries — and this is EXPLICIT, never silent: the
resolved view records which properties came from a default
(`ResolvedConfig::defaults_applied`, a sorted list the operator can read).
An absent REQUIRED property with no default is `MISSING_REQUIRED`.

## 4. The layered constraints document (`flexcore-plugin-config-constraints-v1`)

The documented P7.19 extension: a per-deployment constraints document
narrowing the manifest's declared properties with closed enum sets
(`Enum`), inclusive numeric bounds (`Bounds`), and pre-validated `default`
values. Fail-closed: closed key sets at every level, a constraint on an
undeclared property is `CONSTRAINT_UNDECLARED`, a constraint of a
mismatched type is `CONSTRAINT_TYPE_MISMATCH`, and a default that violates
its own property's type or constraints is refused at document-parse time —
never a deferred surprise.

## 5. Secrets — references in, handles out

The rule has two halves, both structural:

1. **In the manifest** ([manifest-guide.md](manifest-guide.md) §6): secret
   entries are `{name, reference}` objects only; `reference` must use
   `env:NAME`, `file:PATH`, or `secret-store:KEY`. There is no manifest
   field anywhere that can carry a secret value, and error messages never
   echo a rejected reference value (it may be the credential itself).
2. **In the config document** (this page): a STRING value in a property
   whose normalised name matches the closed secret-field vocabulary —
   `password`, `secret`, `token`, `credential`, `api-key`, `private-key`,
   `access-key` (`SECRET_FIELD_VOCABULARY`, DEFAULT decision; `api_key`,
   `APIKey`, and `api-key` all normalise to the match) — is refused as an
   inline-credential attempt (`SECRET_VALUE_IN_CONFIG`) BEFORE any further
   processing. A config property named exactly like a manifest secret is
   `SECRET_FIELD_COLLISION`. A string carrying one of the closed scheme
   prefixes with a non-empty location is resolved to a HANDLE.

## 6. The resolution-to-handle seam

The resolved configuration carries a `SecretRef` handle INSTEAD of any
secret value: `{property, scheme, location}` — scheme (`ENV` / `FILE` /
`SECRET_STORE`) and the LOCATION string only, the same non-secret
location the manifest already carries in the clear. The `ResolvedConfig`
type has no value accessor for secrets: the only secret-shaped output is
`secret_refs()` (handles, sorted). Reading the actual secret is a HOST-side
capability — the broker's scoped grants (P7.11) — and the plugin receives
a handle or nothing (`secret.read` is in the §5.7 never-granted set). The
kernel itself has no secrets-manager port, by design; the seam is
RECORDED, not implemented, in the kernel.

The deterministic JSON rendering of the resolved view renders reference
values as their `{reference, scheme, location}` handle objects — never a
value — so logs and audit records can carry the resolved view verbatim.

## 7. What the operator sees

The console (P7.25) shows the configuration SUMMARY only: property names,
types, and whether a value came from a default — never a secret value,
never a secret location that could look like one. There is no endpoint
anywhere on the control-plane surface that returns plugin configuration
values.
