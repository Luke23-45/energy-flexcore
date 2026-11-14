# The deliberately bad plugin — an error-catalogue walkthrough

| Field | Value |
|---|---|
| Subtask | P7.24 step 3 (`ledger/phase-07-plugin-sdk/ledger_phase_7.md` §P7.24) |
| Files | `bad-manifest.json` (a real document the real validator rejects), `bad-plugin-source.py` (annotated pseudo-source — not runnable, not presented as code) |
| Companion good example | [../echo-worker/README.md](../echo-worker/README.md) |
| Verification | every error name below is verified against the cited source file (read 2026-11-13) |

## 0. Honest status — the recorded rejection output

The ledger's step 3 asks for the rejection output "recorded". **It cannot
be recorded yet and is not pretended here**: the validator and host have
never been built or run in this loop (the loop bans). What IS recorded is
the exact named error each violation produces, per the validator source —
and every one of these names is pinned as an assertion in the kernel's
written-unrun test suite. The human build pass records the actual output
by running:

```sh
# PREPARED-UNRUN — the manifest-gate walkthrough (kernel suite):
cargo test -p flexcore-kernel --test manifest_p7_05
# PREPARED-UNRUN — the resolver, profile, and config gates:
cargo test -p flexcore-kernel
# PREPARED-UNRUN — the host door (broker + enforcement):
cargo test -p flexcore-plugin-host
```

The bad example is rejected by construction, not by the suite's good
graces: each violation below dies at a DIFFERENT gate, which is the point
— the SDK's rejections are layered, and the bad example walks you through
every layer.

## 1. The violation walk, in the order the gates actually fire

The pipeline order is: manifest validation (P7.05) → profile plan
validation (P7.15) → resolver (P7.06) → host door (P7.11/P7.12) →
lifecycle gates (P7.07/P7.19). The validator aborts at the FIRST
violation, so the walkthrough fixes one violation at a time and names the
next one that appears.

### (d) First gate: the manifest validator — `PERMISSION_NEVER_GRANTED`

`"permissions": ["telemetry.read", "forecast.publish", "dispatch.authorize"]`

`dispatch.authorize` is in the §5.7 never-granted set; the manifest
validator refuses (`crates/flexcore-kernel/src/manifest.rs`,
`NEVER_GRANTED_PERMISSIONS`). This is the earliest point the rule can
bite. If the id survived to a later seam anyway, the broker and the
enforcement gate re-refuse it as `PERMISSION_NEVER_GRANTABLE`
(`apps/plugin-host/src/broker.rs`, `enforcement.rs`). A plugin author
whose ambition is dispatch authority stops here — the ambition itself is
the rejected field (ADR-010).

### (b, first half) Second: `SECRET_REFERENCE_INVALID`

`"secrets": [{ "name": "worker-credentials", "reference": "hunter2-is-not-a-scheme-prefix" }]`

A reference without an `env:` / `file:` / `secret-store:` prefix is
refused as a likely inline credential
(`crates/flexcore-kernel/src/manifest.rs::SECRET_REFERENCE_SCHEMES`). The
rejected VALUE is never echoed back in the error message — the message
may be shown to someone, and the value may be the credential itself.

### (f) Third: `MISSING_FIELD` — the omitted resource bounds

`"resources": {}`

Resource bounds are NOT optional defaults: `max-memory-mb` (1..=65536)
and `timeout-ms` (1..=3 600 000) are REQUIRED keys of the `resources`
object. Their absence is `MISSING_FIELD` at location `resources`. There
is no "unbounded" spelling: a plugin whose bounds the host cannot enforce
is a plugin that does not run.

### (c) Fourth gate: profile plan validation — `PROFILE_PERMISSION_CEILING_EXCEEDED`

After fixing the manifest-gate violations, composing this plugin into the
shipped `sim-lab` profile fails: `forecast.publish` is a legal §5.7 id
but the profile's ceiling is `["telemetry.read", "asset.read",
"schedule.propose"]` (`plugins/profiles/sim-lab.profile.json`). The
ceiling refuses it (`crates/flexcore-kernel/src/profile.rs`) — and the
host door would refuse the same over-broad request as
`PERMISSION_ABOVE_PROFILE` (never trimmed to fit). Over-broad requests
are refused, not silently narrowed.

### (a) Fifth gate: the resolver — `PROTECTED_KERNEL_REPLACE`

`"provider-of": ["kernel.dispatch-validation", "forecast.day-ahead"]`

`kernel.dispatch-validation` is a protected-kernel contract id (report
§5.1/§5.7/§9.1; ADR-005). The claim passes the manifest grammar (it is a
well-formed contract id) and dies at the resolver, before any composition
plan exists (`crates/flexcore-kernel/src/resolve.rs::PROTECTED_KERNEL_CONTRACT_IDS`).
The same claim is re-refused at three more layers: `PROFILE_PROTECTED_CONTRACT_REFUSED`
(profile), `PROTECTED_KERNEL_CONTRACT_REFUSED` (provider registry),
`PROTECTED_CONTRACT_CLAIM` (plugin-path verifier). The protected kernel is
not replaceable — structurally.

### (e) Sixth gate: the resolver again — `PLATFORM_API_UNSUPPORTED` (the api-version lie)

`"api-version": "^0.4.0"`

This is a WELL-FORMED range — it passes the manifest validator — and it
is a lie: it does not cover the running platform plugin-API version
`0.5.0` (`CURRENT_PLATFORM_API_VERSION`). The resolver refuses
(`PLATFORM_API_UNSUPPORTED`). The sibling lie — declaring a range that
does not intersect a consumed provider's range — is
`UNSUPPORTED_API_VERSION` (report §9.3 compatibility matrix rule). The
lesson: the range is parsed as a RANGE and checked against pinned
platform facts; a manifest cannot claim compatibility into existence.

### (b, second half) Seventh gate: configuration resolution — `SECRET_VALUE_IN_CONFIG`

At the `Compatible → Configured` lifecycle gate, the operator's
configuration document for this plugin is resolved against the manifest's
declared schema. The declared property `api-key` arrives as a plain
string (the cleartext credential): the property name normalises into the
closed secret-field vocabulary (`SECRET_FIELD_VOCABULARY` — `password`,
`secret`, `token`, `credential`, `api-key`, `private-key`, `access-key`)
and a plain value there is refused BEFORE any further processing
(`crates/flexcore-kernel/src/plugin_config.rs`). The honest alternative
is the manifest's secrets object with a scheme reference — resolved to a
HANDLE (scheme + location), never a value; reading is a host capability
([../configuration-and-secrets.md](../configuration-and-secrets.md)).

## 2. What the bad plugin can never do at runtime (the pseudo-source half)

`bad-plugin-source.py` annotates the runtime attempts — ambient file,
network, environment access, dispatch-authority calls — with where each
dies. Most have NO error name because there is nothing to refuse: the WIT
worlds carry zero `wasi:` imports, so the import needed to attempt the
access does not exist. Where an id is checked, the broker answers
`CAPABILITY_NOT_GRANTED` (defaults deny) or `SCOPE_OUTSIDE_GRANT`
(`apps/plugin-host/src/broker.rs`).

## 3. The error-catalogue table (every name verified against source)

| # | Violation in this example | Gate | Named error | Source verified against |
|---|---|---|---|---|
| 1 | requests `dispatch.authorize` (d) | manifest validator | `PERMISSION_NEVER_GRANTED` | `crates/flexcore-kernel/src/manifest.rs` |
| 2 | scheme-less secret reference (b) | manifest validator | `SECRET_REFERENCE_INVALID` | `crates/flexcore-kernel/src/manifest.rs` |
| 3 | `resources: {}` — bounds omitted (f) | manifest validator | `MISSING_FIELD` | `crates/flexcore-kernel/src/manifest.rs` |
| 4 | `forecast.publish` above the `sim-lab` ceiling (c) | profile plan validation | `PROFILE_PERMISSION_CEILING_EXCEEDED` | `crates/flexcore-kernel/src/profile.rs` |
| 5 | claims `kernel.dispatch-validation` (a) | resolver | `PROTECTED_KERNEL_REPLACE` | `crates/flexcore-kernel/src/resolve.rs` |
| 6 | `api-version: "^0.4.0"` does not cover platform `0.5.0` (e) | resolver | `PLATFORM_API_UNSUPPORTED` | `crates/flexcore-kernel/src/resolve.rs` |
| 7 | `"api-key": "<cleartext>"` in config (b) | config resolution (`Compatible → Configured`) | `SECRET_VALUE_IN_CONFIG` | `crates/flexcore-kernel/src/plugin_config.rs` |
| 8 | (if it ever got this far) never-granted id at the host door | broker + enforcement | `PERMISSION_NEVER_GRANTABLE` | `apps/plugin-host/src/broker.rs`, `apps/plugin-host/src/enforcement.rs` |
| 9 | (if it ever got this far) over-broad request at the host door | enforcement gate | `PERMISSION_ABOVE_PROFILE` | `apps/plugin-host/src/enforcement.rs` |

Each of names 1–7 is also pinned as an individual written-unrun test
assertion in `crates/flexcore-kernel/tests/` (the P7.05/P7.06/P7.15/P7.19
suites); the host-door names in the `apps/plugin-host/tests/` suites. The
assertions are written, NEVER RUN — the human build pass is the
verification gate.

## 4. Why this example cannot "accidentally pass"

The ledger's risk row for P7.24 is "the bad example accidentally passing".
It is countered structurally: the bad example is not one plugin that one
suite might miss — it is a walkthrough of SIX independent gates, each of
which refuses the document before the next gate is reached, and each
refusal is a distinct pinned test assertion. For the example to pass, six
independent validators would have to simultaneously regress.
