# P13.10 — API versioning and deprecation policy

| Field | Value |
|---|---|
| Subtask | P13.10 — `/api/v1/` confirmed as the current version; `Deprecation` + `Sunset` headers design; 6–12 month deprecation windows; OpenAPI lint and breaking-change detection plan over the generated spec |
| Wave / agent | Phase 13 wave 1, Agent B |
| Status | WRITTEN-UNRUN — the policy is registered and its mechanics are implemented code that has never been compiled or executed; no version is deprecated, no header has ever been attached to a response, no lint has ever run in CI. Every evidence cell is EMPTY/UNRUN. |
| Sources | [S-84] (RFC 8594, the Sunset HTTP response header — fetched basis: rfc-editor.org/rfc/rfc8594; §2 defines the field value as an HTTP-date); [S-85] (Speakeasy API-versioning best-practice guidance — the 6–12 month window this policy registers; speakeasy.com/blog/api-versioning-best-practices); the generated-spec seam `apps/control-plane/src/openapi.rs` (utoipa generation serving `/api/v1/openapi.json`; the spec is GENERATED from the route annotations, never hand-written) |
| Files touched | `apps/control-plane/src/api_versioning.rs` (new), `apps/control-plane/src/lib.rs` (module declaration, marked P13.10) |

## 1. Version confirmation — `/api/v1/` is the current version

The entire existing control-plane surface is **v1**: every implemented route
lives under `/api/v1/` (`routes.rs` and the per-phase route modules), and the
generated OpenAPI document is served at `/api/v1/openapi.json`
(`openapi.rs`). This policy confirms v1 as CURRENT and re-version nothing:
no route moves, no path is renamed, no numbering changes (AGENTS.md §11).
The policy constants live in code (`api_versioning.rs`):
`CURRENT_API_VERSION = "v1"`, `CURRENT_API_PREFIX = "/api/v1/"`.

| State | Meaning | Current inventory |
|---|---|---|
| `CURRENT` | the default surface new integrations build against | `/api/v1/**` (all implemented routes) |
| `DEPRECATED` | announced for retirement; Sunset header attached; still served | none — nothing has ever been deprecated |
| `RETIRED` | past the Sunset date; requests answered with the version's retirement contract | none |

The policy declares one supported major version at a time. A future v2
would be introduced ADDITIVELY alongside v1 (new paths under `/api/v2/`), never
by in-place mutation of v1's contract — the same expand-only discipline the
migration policy applies to the database (P11.18), applied to the wire.

## 2. The deprecation headers design

Two response headers, both advisory in transport and binding in policy:

| Header | Basis | Value shape | Semantics in this policy |
|---|---|---|---|
| `Sunset` | **RFC 8594** [S-84] — an RFC | IMF-fixdate HTTP-date (`Sunset: Wed, 11 Nov 2026 00:00:00 GMT`) | The MACHINE-authoritative retirement date. Once the Sunset date passes, the version MAY stop serving (the retirement act itself is an operator deployment decision, recorded, not automatic middleware behavior this batch installs) |
| `Deprecation` | an IETF HTTP API working-group DRAFT (draft-ietf-httpapi-deprecation-header) — **not an RFC**. Status: UNVERIFIED-AT-A-DISTANCE (recorded honestly: the draft's exact grammar may evolve and this repo does not track the draft) | HTTP-date (the draft's timestamp form) | The ANNOUNCEMENT marker, rendered as advisory text. Consumers MUST key on `Sunset` (RFC 8594) and treat `Deprecation` as informational — the module renders both, relies on neither's draft grammar |

Design rules:

1. Both headers are attached to responses of a version from its deprecation
   announcement until retirement. The attachment point is a middleware layer
   over the deprecated prefix — a composition decision (recorded here, not
   installed: no middleware is wired in this batch, and no route is
   currently deprecated so there is nothing to attach).
2. The header values are computed by `api_versioning.rs`
   (`sunset_header_value` — the `time` crate already in the crate's
   dependency set renders the RFC 8594 HTTP-date; `deprecation_header_value`
   delegates). The functions are pure; tests pin known-date renderings
   (epoch Thursday; a Wednesday; a leap-day Thursday).
3. Documentation duty: a deprecation announcement MUST also update the
   generated-spec description (a spec note naming the Sunset date) and the
   operator release notes — the headers are for machines, the announcement
   is for humans. Neither act has ever occurred (EMPTY).

## 3. The 6–12 month window (registered policy)

| Bound | Value | Source | Enforcement |
|---|---|---|---|
| Minimum window | 180 days between the deprecation announcement and the Sunset date | the 6-month floor of the 6–12 month guidance [S-85] (ASSUMPTION: the guidance's exact recommendation is the blog/source text; the numeric floor is this policy's registration) | `validate_deprecation_window` refuses an announcement below the floor, naming the violated bound |
| Maximum window | 365 days | the 12-month ceiling of the same guidance — a distant Sunset stops being information | same function refuses above the ceiling |

Both constants (`MIN_DEPRECATION_WINDOW_DAYS`, `MAX_DEPRECATION_WINDOW_DAYS`)
are REGISTERED policy bounds written before any version was ever deprecated
(X-8 discipline; conventions §5). Changing a bound is a recorded amendment
to this record. The validation is announced-time, not transport-time: no
runtime middleware computes windows.

## 4. The OpenAPI lint and breaking-change detection plan

Target artifact: the GENERATED spec — `/api/v1/openapi.json` from
`openapi.rs` (utoipa, generated from the route annotations). The lint runs
over the generator's OUTPUT, never a hand-maintained copy: a spec edited by
hand would drift from the served surface and the diff would miss it.

### 4.1 The lint mechanics (implemented, written-unrun)

`api_versioning.rs::detect_breaking_changes(old, new)` is a pure std-only
(serde_json) comparator over two generated documents. The difference
vocabulary is CLOSED, and the deny-by-default extends to contracts:

| Diff class | Breaking? | Rationale |
|---|---|---|
| `PathRemoved` | yes | an existing caller's URL stops existing |
| `OperationRemoved` | yes | an existing caller's method stops existing |
| `ResponseRemoved` | yes | a declared response status disappears (error contracts are part of the surface) |
| `RequiredRequestFieldAdded` | yes | callers that omit the field start failing |
| `ResponseFieldRemoved` | yes | consumers reading the field break |
| `FieldTypeChanged` | yes | consumers parsing the field break |
| `Unclassified` (any difference the checker does not recognize) | yes | nothing is silently opted in — an unrecognized diff is treated as breaking until it is classified |

Additive change (a new path, a new optional request property, a new response
field) is NOT in any diff class — the comparator reports only the classes
above, so additive evolution passes. The lint's CI wiring (fetch the
previous release's generated spec, diff against the current one, fail the
gate on any breaking class) is the recorded composition step; nothing ran
here, and no lint has ever executed against any spec.

### 4.2 The lint plan (the recorded procedure)

| Step | Content | Status |
|---|---|---|
| L1 | At each release gate, serve/extract the generated spec of the release candidate | NOT-STARTED (no release exists) |
| L2 | Diff against the previous release's generated spec with `detect_breaking_changes` | NOT-STARTED |
| L3 | Any breaking diff blocks the release under the SAME version; the change either reverts or the version lineage moves per §1 | policy text — never yet applied |
| L4 | Any `Unclassified` finding is a lint BUG to classify (extend the closed vocabulary in a recorded amendment), never a pass | policy text |
| L5 | Additive diffs are recorded in the release notes as the version's evolution log | NOT-STARTED |

### 4.3 The lint scope (registered limitation)

The comparator diffs the success-response (`200`)
`application/json` schema properties, the declared response statuses, the
required request-body properties, and the path/operation inventory. It does
NOT diff: schema-level `$ref` component evolution, header/cookie parameters,
non-JSON content types, or semantic units (a `number` that changed meaning
without changing type is invisible to any structural lint — the review
checklist remains the human backstop for semantic breaks; the lint is a
floor, never a ceiling). These gaps are recorded, not hidden.

## 5. Cross-phase seam (consumed downstream, not copied)

Phase 16 (market integration) consumes THIS record for the market API
surface's versioning/deprecation discipline (plan §5: "Phase 16 consumes
the records, not new copies"). The market surface, when it exists, inherits
the same version confirmation, header design, window bounds, and lint
mechanics — the `api_versioning.rs` module is the single implementation.

## 6. ASSUMPTION / UNRUN register

1. **UNVERIFIED-AT-A-DISTANCE** — the `Deprecation` header's draft status
   and grammar (§2): the IETF httpapi draft was not fetched this batch; the
   policy deliberately does not depend on it. RFC 8594's Sunset semantics
   are the load-bearing citation [S-84].
2. **ASSUMPTION** — the numeric 180/365-day bounds are this policy's
   registration of the [S-85] guidance; the source's exact numbers were
   not re-fetched this batch (the guidance is cited, the bounds are policy).
3. **UNRUN** — the module, its tests, the lint, and the header rendering:
   never compiled, never executed. No header has ever been attached to any
   HTTP response anywhere.
4. **EMPTY** — no deprecated version, no Sunset date, no lint finding, no
   breaking-change report exists.

## 7. Handoff notes

1. **Orchestrator**: tick P13.10 with this record's citation; the policy is
   implemented (written-unrun), never tested or verified.
2. **Wave 2 / human build**: the middleware attachment (§2 rule 1) and the
   CI lint wiring (§4.2) are the two composition steps; neither is urgent
   while nothing is deprecated.
3. **Phase 16**: consume §1–§4 for the market API surface; do not re-derive
   the policy or fork the module.
