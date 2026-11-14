// P7.41 — Manifest-validator fuzz and property tests (ledger P7.41; report
// §19.2(2) property tests, §5.5, §5.7). WRITTEN, NEVER RUN — the loop bans
// hold; the human build/test pass executes these
// (`cargo test -p flexcore-kernel --test fuzz_manifest_p7_41`).
//
// WHAT THIS SUITE IS (and is not):
//
// It is a DEPENDENCY-FREE property-based fuzz harness: a small deterministic
// seeded LCG generator (no proptest/quickcheck — no new dependency is added;
// the kernel's `Cargo.toml` is untouched) producing mutated manifests from
// the valid published-fixture shape, plus a fixed seed corpus of adversarial
// JSON strings embedded below. It runs the manifest validator over every
// generated and corpus input under four properties:
//
// - **P1 — the validator NEVER panics on any input** (`catch_unwind`
//   harness). "Accepted, rejected, or error cleanly — never crash" is the
//   ledger's fuzz acceptance; "never hang" is bounded here by construction:
//   every generated input is size-capped, the validator's loops are all
//   bounded by the (capped) input length, and the iteration counts below
//   are fixed constants.
// - **P2 — every rejection is a NAMED error**: `ManifestError::name()` is
//   non-empty and is one of the recorded name-table entries (the table is
//   re-declared below so a silent name change fails this suite).
// - **P3 — validity is STABLE**: the same input always yields the same
//   verdict (every generated case is parsed twice; `Ok` payloads compare
//   equal and error names compare equal).
// - **P4 — the valid fixtures REMAIN VALID across generator runs**: the
//   generator mutates a COPY, never the fixture; the fixture is re-parsed
//   after every full generator pass so a mutation colliding back into a
//   false-acceptance of a named mutated form cannot hide.
//
// Plus the ledger's targeted property (step 1): manifests never validate
// when a required field is missing, a SemVer version is malformed, the
// api-version range is malformed, a runtime kind is unknown, or an
// undeclared / never-granted permission is requested — asserted on
// deterministic targeted mutants.
//
// Plus the ledger step-3 property: api-version ranges are parsed as RANGES,
// never string-compared — asserted on generated comparator expressions AND
// on crafted cases where lexicographic string comparison and numeric SemVer
// comparison DISAGREE (the string-compare answer must never appear).
//
// THE RECORDED serde_json DUPLICATE-KEY FINDING PATH (decision record
// `05_06_manifest_resolution.md` §3 item 3): `serde_json::Value` maps
// silently keep the LAST occurrence of a duplicated JSON key; rejecting
// duplicate keys needs a custom parser and is NOT implemented. The
// `duplicate_keys_*` tests PROVE the recorded limitation with constructed
// duplicate-key inputs and pin it HONESTLY: the validator cannot detect
// duplicates, so the pinned claim is **deterministic-last-wins** (the
// verdict on a duplicate-key input equals the verdict on its collapsed
// form, stable across runs) — NOT "duplicates are refused". The recorded
// acceptance class is "REFUSED-or-DETERMINISTIC"; this build pins the
// DETERMINISTIC half. If a future kernel adds duplicate detection, the
// `duplicate_keys_verdict_is_refused_or_deterministic` test fails loudly
// and the pin is re-recorded instead of silently flipping.
//
// THE REAL `cargo-fuzz` TARGET IS RECORDED, NOT ADDED: a libFuzzer target
// (`fuzz_target!(|data: &[u8]| { ... })` over `parse_manifest`) is the
// deployment-stage follow-up — it needs the `cargo-fuzz` tooling and a
// `fuzz/` crate surface (new `Cargo.toml` entries), which this loop must
// not add. Recorded as a human/deployment follow-up in the decision record
// `ledger/phase-07-plugin-sdk/41_42_43_hardening_determinism_regression.md`
// §2. The non-UTF-8 corpus class of a byte-level fuzzer is structurally
// unreachable through this entry point: `parse_manifest` takes `&str`,
// which Rust guarantees is valid UTF-8 — the corpus covers replacement,
// combining, RTL, zero-width, and control characters instead.

use std::panic::{catch_unwind, AssertUnwindSafe};

use flexcore_kernel::manifest::{
    parse_manifest, ApiVersionRange, Manifest, SemVer, ALLOWED_PERMISSIONS, MAX_MANIFEST_BYTES,
    NEVER_GRANTED_PERMISSIONS,
};

// --- The recorded name table (re-declared so a silent name change fails here) --

const KNOWN_ERROR_NAMES: [&str; 24] = [
    "MANIFEST_JSON_INVALID",
    "MANIFEST_NOT_AN_OBJECT",
    "SCHEMA_VERSION_UNSUPPORTED",
    "UNKNOWN_FIELD",
    "MISSING_FIELD",
    "WRONG_TYPE",
    "PLUGIN_ID_MALFORMED",
    "INVALID_SEMVER",
    "INVALID_API_VERSION_RANGE",
    "UNKNOWN_CLASS",
    "MISSING_ENTRY_POINT",
    "ENTRY_POINT_AMBIGUOUS",
    "ENTRY_POINT_MISMATCH",
    "ENTRY_POINT_INVALID",
    "CONTRACT_ID_MALFORMED",
    "DUPLICATE_PROVIDER_DECLARATION",
    "DUPLICATE_CONSUMES_DECLARATION",
    "PERMISSION_ID_UNKNOWN",
    "PERMISSION_NEVER_GRANTED",
    "DUPLICATE_PERMISSION_DECLARATION",
    "CONFIGURATION_SCHEMA_INVALID",
    "SECRET_REFERENCE_INVALID",
    "DUPLICATE_SECRET_NAME",
    "RESOURCE_BOUND_INVALID",
];

// --- The valid fixtures (the published manifest shapes, embedded so the
// --- suite is self-contained and the bytes are pinned) -------------------------

const VALID_BUILTIN_MANIFEST: &str = r#"{
  "manifest-schema": "flexcore-plugin-manifest-v1",
  "plugin-id": "neryva.sim-builtin-ess",
  "name": "Built-in ESS Simulator Provider",
  "version": "0.1.0",
  "api-version": ">=0.5,<1.0",
  "class": "builtin-rust",
  "entry-point": { "registered-type": "neryva.builtin/sim-ess" },
  "provider-of": ["ess.simulation"],
  "consumes": ["telemetry.edge-sample"],
  "permissions": ["telemetry.read"],
  "configuration-schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "rated-power-mw": { "type": "number", "description": "Rated power in MW." }
    },
    "required": ["rated-power-mw"]
  },
  "secrets": [{ "name": "auth-token", "reference": "env:FLEXCORE_ESS_TOKEN" }],
  "resources": { "max-memory-mb": 64, "timeout-ms": 30000 },
  "metadata": {
    "license": "Apache-2.0",
    "description": "Deterministic built-in ESS simulator provider.",
    "source": "crates/flexcore-simulator"
  }
}"#;

const VALID_WASM_MANIFEST: &str = r#"{
  "manifest-schema": "flexcore-plugin-manifest-v1",
  "plugin-id": "neryva.optimizer-wasm-lp",
  "name": "WASM LP Optimizer Component",
  "version": "0.2.0",
  "api-version": ">=0.5.0, <0.6.0",
  "class": "wasm-component",
  "entry-point": { "symbol": "neryva:optimizer/lp-solve@0.5.0" },
  "provider-of": ["optimization.lp"],
  "consumes": ["portfolio.read"],
  "permissions": ["schedule.propose"],
  "configuration-schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "gap-tolerance": { "type": "number", "description": "Relative gap tolerance." }
    },
    "required": []
  },
  "secrets": [],
  "resources": { "max-memory-mb": 256, "timeout-ms": 60000, "max-output-mb": 8 },
  "metadata": {
    "license": "Apache-2.0",
    "description": "Class B WASM component providing LP optimization (candidate schedules only)."
  }
}"#;

// --- The fixed adversarial seed corpus (embedded; each entry is class-labelled) ---

const ADVERSARIAL_CORPUS: [(&str, &str); 26] = [
    ("truncated", r#"{"manifest-schema": "flexcore-plugin-manifest-v1", "plugin-id": "ner"#),
    ("truncated-2", r#"{"manifest-schema""#),
    ("empty", ""),
    ("whitespace", "   \n\t  "),
    ("not-json", "this is not json at all {{{"),
    ("not-an-object", "[1, 2, 3]"),
    ("null", "null"),
    ("string-doc", "\"just a string\""),
    ("wrong-envelope", r#"{"manifest-schema": "flexcore-plugin-manifest-v0", "plugin-id": "neryva.x"}"#),
    ("no-envelope", r#"{"plugin-id": "neryva.x"}"#),
    ("unknown-field", r#"{"manifest-schema": "flexcore-plugin-manifest-v1", "plugin-id": "neryva.x", "evil-extra": true}"#),
    ("unicode-key", r#"{"manifest-schema": "flexcore-plugin-manifest-v1", "plugin-id": "neryva.x", "ключ-злодей": 1}"#),
    ("emoji-key", "{\"manifest-schema\": \"flexcore-plugin-manifest-v1\", \"plugin-id\": \"neryva.x\", \"\u{1F512}-key\": 1}"),
    ("zero-width-key", "{\"manifest-schema\": \"flexcore-plugin-manifest-v1\", \"plugin-id\": \"neryva.x\", \"hidden\u{200B}key\": 1}"),
    ("duplicate-plugin-id", r#"{"manifest-schema": "flexcore-plugin-manifest-v1", "plugin-id": "neryva.first-one", "plugin-id": "neryva.second-one", "name": "n", "version": "0.1.0", "api-version": ">=0.5.0", "class": "builtin-rust", "entry-point": {"registered-type": "t"}, "provider-of": [], "consumes": [], "permissions": [], "configuration-schema": {"type": "object", "additionalProperties": false}, "resources": {"max-memory-mb": 1, "timeout-ms": 1}, "metadata": {"license": "Apache-2.0", "description": "d"}}"#),
    ("duplicate-class", r#"{"manifest-schema": "flexcore-plugin-manifest-v1", "plugin-id": "neryva.x-y", "class": "wasm-component", "class": "builtin-rust", "entry-point": {"registered-type": "t"}, "name": "n", "version": "0.1.0", "api-version": ">=0.5.0", "provider-of": [], "consumes": [], "permissions": [], "configuration-schema": {"type": "object", "additionalProperties": false}, "resources": {"max-memory-mb": 1, "timeout-ms": 1}, "metadata": {"license": "Apache-2.0", "description": "d"}}"#),
    ("never-granted-permission", r#"{"manifest-schema": "flexcore-plugin-manifest-v1", "plugin-id": "neryva.x-y", "permissions": ["dispatch.authorize"]}"#),
    ("undeclared-permission", r#"{"manifest-schema": "flexcore-plugin-manifest-v1", "plugin-id": "neryva.x-y", "permissions": ["kernel.root-everything"]}"#),
    ("unknown-class", r#"{"manifest-schema": "flexcore-plugin-manifest-v1", "plugin-id": "neryva.x-y", "class": "kernel-process"}"#),
    ("bad-semver", r#"{"manifest-schema": "flexcore-plugin-manifest-v1", "plugin-id": "neryva.x-y", "version": "1.0.0-beta+build"}"#),
    ("bad-range", r#"{"manifest-schema": "flexcore-plugin-manifest-v1", "plugin-id": "neryva.x-y", "api-version": "0.5.x"}"#),
    ("secret-value", r#"{"manifest-schema": "flexcore-plugin-manifest-v1", "plugin-id": "neryva.x-y", "secrets": [{"name": "k", "reference": "super-secret-password-value"}]}"#),
    ("schema-not-false", r#"{"manifest-schema": "flexcore-plugin-manifest-v1", "plugin-id": "neryva.x-y", "configuration-schema": {"type": "object", "additionalProperties": true}}"#),
    ("resource-bound-lie", r#"{"manifest-schema": "flexcore-plugin-manifest-v1", "plugin-id": "neryva.x-y", "resources": {"max-memory-mb": -1, "timeout-ms": 1}}"#),
    ("deep-nesting", "[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[1]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]"),
    ("control-char-in-string", "{\"manifest-schema\": \"flexcore-plugin-manifest-v1\", \"plugin-id\": \"neryva.x-y\", \"name\": \"a\u{0000}b\"}"),
];

// --- The deterministic generator (seeded LCG; no external dependency) -----------

/// A 64-bit LCG (Knuth's MMIX constants). Deterministic: the same seed
/// produces the same input sequence on every run, on every platform — the
/// re-run-from-recorded-seed verification method of ledger P7.41.
struct Lcg {
    state: u64,
}

impl Lcg {
    const MULTIPLIER: u64 = 6_364_136_223_846_793_005;
    const INCREMENT: u64 = 1_442_695_040_888_963_407;

    fn new(seed: u64) -> Lcg {
        Lcg { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(Lcg::MULTIPLIER)
            .wrapping_add(Lcg::INCREMENT);
        self.state
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound.max(1)
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }
}

/// The suite's fixed generator seed (hex for "FLXP741", the P7.18
/// `DEFAULT_SUPERVISION_SEED` naming pattern — a stable, recorded seed so
/// the re-run-from-recorded-seed comparison is exact).
const GENERATOR_SEED: u64 = 0x464C_5843_5037_3431;

/// Generator passes per test (each pass = the full deterministic mutation
/// sequence for its seed).
const GENERATOR_PASSES: u64 = 3;

const GENERATOR_ITERATIONS: usize = 512;

/// The valid fixtures the generator mutates (copies only — P4).
const GENERATOR_FIXTURES: [&str; 2] = [VALID_BUILTIN_MANIFEST, VALID_WASM_MANIFEST];

const INJECTED_KEYS: [&str; 8] = [
    "evil-key",
    "ключ",
    "\u{1F512}",
    "hidden\u{200B}key",
    "password",
    "api-key",
    "k",
    "very-long-injected-key-name-that-keeps-going-and-going-0123456789",
];

/// One generator-produced input: the mutated source text plus a label of
/// what was done (for failure messages and the recorded run log).
struct GeneratedInput {
    label: String,
    source: String,
}

/// Produce the deterministic mutated-manifest sequence for one seed.
fn generated_inputs_seeded(seed: u64) -> Vec<GeneratedInput> {
    let mut rng = Lcg::new(seed);
    let mut out = Vec::with_capacity(GENERATOR_ITERATIONS);
    for _ in 0..GENERATOR_ITERATIONS {
        let fixture = rng.pick(&GENERATOR_FIXTURES);
        let mut mutated: serde_json::Value =
            serde_json::from_str(fixture).expect("the embedded fixtures are valid JSON");
        let op = rng.below(8);
        let label;
        match op {
            0 => {
                // Truncation at a char-boundary offset of the serialized text.
                let text =
                    serde_json::to_string(&mutated).expect("serialization cannot fail");
                let mut cut = rng.below(text.len() as u64) as usize;
                while cut > 0 && !text.is_char_boundary(cut) {
                    cut -= 1;
                }
                label = format!("truncate@{cut}");
                out.push(GeneratedInput { label, source: text[..cut].to_string() });
                continue;
            }
            1 => {
                // Key injection at the top level (the closed key set must refuse).
                let key = (*rng.pick(&INJECTED_KEYS)).to_string();
                label = format!("inject-key:{key}");
                if let serde_json::Value::Object(map) = &mut mutated {
                    map.insert(key, serde_json::json!({"nested": {"deeper": [1, 2, 3]}}));
                }
            }
            2 => {
                // Type flip: replace a declared top-level field with a value of
                // a randomly different JSON type.
                let fields = [
                    "plugin-id", "name", "version", "api-version", "class", "entry-point",
                    "provider-of", "consumes", "permissions", "configuration-schema",
                    "secrets", "resources", "metadata",
                ];
                let field = (*rng.pick(&fields)).to_string();
                let wrong = rng
                    .pick(&[
                        serde_json::json!(null),
                        serde_json::json!(17),
                        serde_json::json!(true),
                        serde_json::json!([{"a": 1}]),
                        serde_json::json!("text"),
                    ])
                    .clone();
                label = format!("type-flip:{field}");
                if let serde_json::Value::Object(map) = &mut mutated {
                    map.insert(field, wrong);
                }
            }
            3 => {
                // Deep nesting under configuration-schema (bounded at 200 —
                // beyond serde_json's documented 128-level recursion limit, so
                // the STRING path surfaces a clean recursion error).
                let depth = 100 + rng.below(100) as usize;
                let mut nested = serde_json::json!({"type": "string"});
                for _ in 0..depth {
                    nested = serde_json::json!({ "wrapped": nested });
                }
                label = format!("deep-nest:{depth}");
                if let serde_json::Value::Object(map) = &mut mutated {
                    if let Some(schema) = map.get_mut("configuration-schema") {
                        if let serde_json::Value::Object(props) = schema {
                            props.insert("wrapped-prop".to_string(), nested);
                        }
                    }
                }
            }
            4 => {
                // Unicode / control characters inside a declared string field.
                let payloads = [
                    "neryva.\u{0301}combining",
                    "tab\tinside",
                    "quote\"inside",
                    "\u{FFFD}replacement",
                    "rtl\u{202E}override",
                ];
                let payload = (*rng.pick(&payloads)).to_string();
                label = "unicode-string".to_string();
                if let serde_json::Value::Object(map) = &mut mutated {
                    map.insert("name".to_string(), serde_json::Value::String(payload));
                }
            }
            5 => {
                // Huge strings: one unknown top-level field, one inside the
                // declared metadata.description (its 2000-char cap).
                let size = 300 + rng.below(2_000) as usize;
                label = format!("huge-string:{size}");
                if let serde_json::Value::Object(map) = &mut mutated {
                    map.insert(
                        "description-huge".to_string(),
                        serde_json::Value::String("x".repeat(size)),
                    );
                    if let Some(metadata) = map.get_mut("metadata") {
                        if let serde_json::Value::Object(meta) = metadata {
                            meta.insert(
                                "description".to_string(),
                                serde_json::Value::String("y".repeat(size)),
                            );
                        }
                    }
                }
            }
            6 => {
                // Oversized document (above MAX_MANIFEST_BYTES — the fail-closed
                // size gate must reject before parsing; generation stays bounded
                // so the test itself never allocates unboundedly).
                let pad = 4_096 + rng.below(4_096) as usize;
                label = format!("pad:{pad}");
                if let serde_json::Value::Object(map) = &mut mutated {
                    map.insert(
                        "pad".to_string(),
                        serde_json::Value::String("p".repeat(pad)),
                    );
                }
            }
            _ => {
                // Targeted semantic mutant class (the ledger step-1 property):
                // drop a required field / corrupt a version / malformed range /
                // unknown class / undeclared or never-granted permission.
                let target = rng.below(6);
                match target {
                    0 => {
                        if let serde_json::Value::Object(map) = &mut mutated {
                            map.remove("plugin-id");
                        }
                        label = "drop-required:plugin-id".to_string();
                    }
                    1 => {
                        if let serde_json::Value::Object(map) = &mut mutated {
                            map.insert("version".to_string(), serde_json::json!("1.0"));
                        }
                        label = "malformed-semver".to_string();
                    }
                    2 => {
                        if let serde_json::Value::Object(map) = &mut mutated {
                            map.insert("api-version".to_string(), serde_json::json!(">=x.y.z"));
                        }
                        label = "malformed-range".to_string();
                    }
                    3 => {
                        if let serde_json::Value::Object(map) = &mut mutated {
                            map.insert("class".to_string(), serde_json::json!("kernel-mode"));
                        }
                        label = "unknown-class".to_string();
                    }
                    4 => {
                        let permission = if rng.below(2) == 0 {
                            "not.a.permission"
                        } else {
                            "dispatch.issue"
                        };
                        if let serde_json::Value::Object(map) = &mut mutated {
                            map.insert(
                                "permissions".to_string(),
                                serde_json::json!([permission]),
                            );
                        }
                        label = format!("bad-permission:{permission}");
                    }
                    _ => {
                        if let serde_json::Value::Object(map) = &mut mutated {
                            map.remove("metadata");
                        }
                        label = "drop-required:metadata".to_string();
                    }
                }
            }
        }
        let source = serde_json::to_string(&mutated).expect("serialization cannot fail");
        out.push(GeneratedInput { label, source });
    }
    out
}

fn generated_inputs() -> Vec<GeneratedInput> {
    generated_inputs_seeded(GENERATOR_SEED)
}

// --- The verdict type (P3's stable verdict, and P2's named rejection) -----------

/// The stable verdict of one input: `Valid(manifest)` or
/// `Refused(named-error)`. Two runs over the same input must yield equal
/// verdicts (P3), and every refused verdict's name must be in the recorded
/// table (P2).
#[derive(Debug, PartialEq)]
enum Verdict {
    Valid(Manifest),
    Refused(String),
}

impl Verdict {
    fn of(source: &str) -> Verdict {
        match parse_manifest(source) {
            Ok(manifest) => Verdict::Valid(manifest),
            Err(err) => Verdict::Refused(err.name().to_string()),
        }
    }
}

/// P1 harness: the validator never panics. A panic fails the test through
/// the explicit branch.
fn verdict_without_panic(source: &str, label: &str) -> Verdict {
    match catch_unwind(AssertUnwindSafe(|| Verdict::of(source))) {
        Ok(verdict) => verdict,
        Err(payload) => panic!(
            "P1 violated: the manifest validator PANICKED on input '{label}' (payload: {payload:?})"
        ),
    }
}

/// P2 harness: every rejection is a named error from the recorded table.
fn assert_named(verdict: &Verdict, label: &str) {
    if let Verdict::Refused(name) = verdict {
        assert!(
            KNOWN_ERROR_NAMES.contains(&name.as_str()),
            "P2 violated: input '{label}' was refused with the UNNAMED error '{name}'"
        );
        assert!(!name.is_empty(), "P2 violated: empty error name for '{label}'");
    }
}

// --- P1–P4 over the generator + corpus -------------------------------------------

#[test]
fn p1_the_validator_never_panics_on_any_generated_or_corpus_input() {
    let mut cases = Vec::new();
    for pass in 0..GENERATOR_PASSES {
        for input in generated_inputs_seeded(GENERATOR_SEED.wrapping_add(pass)) {
            cases.push((format!("pass{pass}:{}", input.label), input.source));
        }
    }
    for (label, source) in &ADVERSARIAL_CORPUS {
        cases.push((format!("corpus:{label}"), source.to_string()));
    }
    for (label, source) in &cases {
        let _ = verdict_without_panic(source, label);
    }
    println!(
        "P1 no-panic run: {} inputs ({} generated passes + {} corpus entries); \
         zero panics — the assertion IS the absence of a panic failure",
        cases.len(),
        GENERATOR_PASSES,
        ADVERSARIAL_CORPUS.len()
    );
}

#[test]
fn p2_every_rejection_is_a_named_error() {
    for pass in 0..GENERATOR_PASSES {
        for input in generated_inputs_seeded(GENERATOR_SEED.wrapping_add(pass)) {
            let verdict = verdict_without_panic(&input.source, &input.label);
            assert_named(&verdict, &input.label);
        }
    }
    for (label, source) in &ADVERSARIAL_CORPUS {
        let verdict = verdict_without_panic(source, &format!("corpus:{label}"));
        assert_named(&verdict, &format!("corpus:{label}"));
    }
}

#[test]
fn p3_validity_is_stable_the_same_input_always_yields_the_same_verdict() {
    for input in generated_inputs() {
        let first = verdict_without_panic(&input.source, &input.label);
        let second = verdict_without_panic(&input.source, &input.label);
        assert_eq!(
            first, second,
            "P3 violated: input '{}' yielded different verdicts across runs ({first:?} vs {second:?})",
            input.label
        );
    }
    for (label, source) in &ADVERSARIAL_CORPUS {
        let first = verdict_without_panic(source, &format!("corpus:{label}"));
        let second = verdict_without_panic(source, &format!("corpus:{label}"));
        assert_eq!(first, second, "P3 violated on corpus input '{label}'");
    }
}

#[test]
fn p4_the_valid_fixtures_remain_valid_across_every_generator_run() {
    for pass in 0..GENERATOR_PASSES {
        // Run a full mutation pass (the generator mutates COPIES, never the
        // fixture constants)...
        let _inputs = generated_inputs_seeded(GENERATOR_SEED.wrapping_add(pass));
        // ...then re-assert fixture validity: a mutation colliding back into
        // a false-acceptance of a named mutated form cannot hide behind a
        // one-off fixture corruption.
        for fixture in GENERATOR_FIXTURES {
            match verdict_without_panic(fixture, "fixture") {
                Verdict::Valid(_) => {}
                Verdict::Refused(name) => {
                    panic!("P4 violated: the valid fixture was refused with '{name}'")
                }
            }
        }
    }
}

// --- The ledger step-1 targeted property ------------------------------------------

#[test]
fn targeted_mutants_never_validate() {
    // Ledger P7.41 step 1: manifests never validate when a required field is
    // missing, a SemVer version is malformed, the api-version range is
    // malformed, a runtime kind is unknown, or an undeclared / never-granted
    // permission is requested. The mutants are DETERMINISTIC (built from the
    // valid fixture by name) so the property does not depend on generator
    // draw coverage.
    let mutants: [(&str, &str, &str); 8] = [
        (
            "missing-required:plugin-id",
            "\"plugin-id\": \"neryva.sim-builtin-ess\",",
            "",
        ),
        (
            "missing-required:api-version",
            "\"api-version\": \">=0.5,<1.0\",",
            "",
        ),
        ("malformed-semver", "\"version\": \"0.1.0\",", "\"version\": \"1.0\","),
        (
            "prerelease-semver",
            "\"version\": \"0.1.0\",",
            "\"version\": \"1.0.0-beta+build\",",
        ),
        ("malformed-range", "\"api-version\": \">=0.5,<1.0\",", "\"api-version\": \">=x.y.z\","),
        ("unknown-class", "\"class\": \"builtin-rust\",", "\"class\": \"kernel-mode\","),
        (
            "undeclared-permission",
            "\"permissions\": [\"telemetry.read\"],",
            "\"permissions\": [\"not.a.permission\"],",
        ),
        (
            "never-granted-permission",
            "\"permissions\": [\"telemetry.read\"],",
            "\"permissions\": [\"dispatch.issue\"],",
        ),
    ];
    for (label, from, to) in mutants {
        let source = VALID_BUILTIN_MANIFEST.replace(from, to);
        assert_ne!(source, VALID_BUILTIN_MANIFEST, "mutant '{label}' did not apply");
        match verdict_without_panic(&source, label) {
            Verdict::Valid(_) => panic!("targeted mutant '{label}' VALIDATED (it must never)"),
            Verdict::Refused(name) => assert!(!name.is_empty(), "unnamed refusal for '{label}'"),
        }
    }
}

// --- Duplicate keys (the recorded serde_json limitation, pinned honestly) ---------

#[test]
fn duplicate_keys_the_recorded_serde_json_last_wins_collapse_is_proven() {
    // Proof 1 — the collapse exists at the parser layer (the recorded
    // limitation, `05_06_manifest_resolution.md` §3 item 3): serde_json
    // silently keeps the LAST occurrence of a duplicated key.
    let raw = r#"{"a": 1, "a": 2}"#;
    let value: serde_json::Value =
        serde_json::from_str(raw).expect("duplicate keys still parse");
    let map = value.as_object().expect("an object");
    assert_eq!(
        map.get("a"),
        Some(&serde_json::json!(2)),
        "the recorded last-wins collapse did not hold — serde_json behaviour changed; \
         the P7.41 duplicate-key pin must be re-recorded"
    );
    assert_eq!(map.len(), 1, "the duplicate key was not collapsed to one entry");
}

#[test]
fn duplicate_keys_verdict_is_refused_or_deterministic_here_deterministic_last_wins() {
    // A duplicate `plugin-id` over an otherwise valid body: the validator
    // CANNOT detect the duplicate (no custom parser — the recorded
    // limitation), so the honest pinned claim is that the verdict is
    // DETERMINISTIC: exactly the collapsed (last-wins) form's verdict,
    // stable across runs. NOT a claim of detection.
    let duplicate = r#"{
        "manifest-schema": "flexcore-plugin-manifest-v1",
        "plugin-id": "neryva.duplicate-a",
        "plugin-id": "neryva.duplicate-b",
        "name": "Duplicate Key Probe",
        "version": "0.1.0",
        "api-version": ">=0.5,<1.0",
        "class": "builtin-rust",
        "entry-point": { "registered-type": "neryva.builtin/probe" },
        "provider-of": ["probe.duplicate"],
        "consumes": [],
        "permissions": ["telemetry.read"],
        "configuration-schema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {},
            "required": []
        },
        "secrets": [],
        "resources": { "max-memory-mb": 8, "timeout-ms": 1000 },
        "metadata": { "license": "Apache-2.0", "description": "duplicate-key probe" }
    }"#;

    // The collapsed form (what serde_json actually hands the validator).
    let collapsed_value: serde_json::Value =
        serde_json::from_str(duplicate).expect("the duplicate-key document parses");
    let collapsed =
        serde_json::to_string(&collapsed_value).expect("re-serialization cannot fail");

    // Verdict equality: the duplicate-key input and its collapsed form yield
    // IDENTICAL verdicts (deterministic last-wins), stable across runs.
    let v_dup_first = verdict_without_panic(duplicate, "duplicate-plugin-id");
    let v_dup_second = verdict_without_panic(duplicate, "duplicate-plugin-id");
    let v_collapsed_first = verdict_without_panic(&collapsed, "collapsed");
    let v_collapsed_second = verdict_without_panic(&collapsed, "collapsed");
    assert_eq!(v_dup_first, v_dup_second, "the duplicate-key verdict is not stable");
    assert_eq!(
        v_collapsed_first, v_collapsed_second,
        "the collapsed verdict is not stable"
    );
    assert_eq!(
        v_dup_first, v_collapsed_first,
        "the duplicate-key input does not behave as its last-wins collapsed form — \
         duplicate handling changed; re-record the limitation"
    );

    // And the last-wins CONTENT is pinned: the parsed manifest carries the
    // LAST declared plugin id.
    match &v_dup_first {
        Verdict::Valid(manifest) => {
            assert_eq!(
                manifest.plugin_id, "neryva.duplicate-b",
                "last-wins content changed: the parsed plugin id is not the last occurrence"
            );
        }
        Verdict::Refused(name) => {
            // A future kernel that REFUSES duplicates lands here — which the
            // recorded acceptance ("REFUSED-or-DETERMINISTIC") also permits,
            // but it contradicts today's recorded limitation, so the pin
            // requires a re-record, not a silent pass.
            panic!(
                "the duplicate-key input is now REFUSED with '{name}': duplicate detection \
                 exists; re-record `05_06_manifest_resolution.md` §3 item 3 and this test"
            );
        }
    }
}

// --- api-version ranges are parsed as RANGES, never string-compared ---------------

/// The independent reference matcher for the enforced comparator subset
/// (the recorded semantics: `{=, >=, <=, >, <, ^, ~}` over the SemVer core,
/// caret/tilde upper bounds per the module's documented definitions).
fn reference_satisfies(expr: &str, v: SemVer) -> bool {
    let text = expr.trim();
    let (op, rest) = if let Some(rest) = text.strip_prefix(">=") {
        (">=", rest)
    } else if let Some(rest) = text.strip_prefix("<=") {
        ("<=", rest)
    } else if let Some(rest) = text.strip_prefix('>') {
        (">", rest)
    } else if let Some(rest) = text.strip_prefix('<') {
        ("<", rest)
    } else if let Some(rest) = text.strip_prefix('=') {
        ("=", rest)
    } else if let Some(rest) = text.strip_prefix('^') {
        ("^", rest)
    } else if let Some(rest) = text.strip_prefix('~') {
        ("~", rest)
    } else {
        ("=", text)
    };
    let bound = SemVer::parse(rest).expect("reference cases carry valid cores");
    match op {
        "=" => v == bound,
        ">" => v > bound,
        ">=" => v >= bound,
        "<" => v < bound,
        "<=" => v <= bound,
        "^" => {
            let upper = if bound.major > 0 {
                SemVer::new(bound.major + 1, 0, 0)
            } else if bound.minor > 0 {
                SemVer::new(0, bound.minor + 1, 0)
            } else {
                SemVer::new(0, 0, bound.patch + 1)
            };
            v >= bound && v < upper
        }
        "~" => {
            let upper = SemVer::new(bound.major, bound.minor + 1, 0);
            v >= bound && v < upper
        }
        _ => unreachable!("the operator set above is closed"),
    }
}

#[test]
fn semver_ranges_are_decided_as_ranges_never_string_comparisons() {
    // Generated valid comparator expressions, checked against the reference
    // matcher over a dense version grid.
    let mut rng = Lcg::new(GENERATOR_SEED ^ 0x5EED_0741);
    let ops = ["=", ">=", "<=", ">", "<", "^", "~", ""];
    let mut checked = 0usize;
    for _ in 0..128 {
        let major = rng.below(3);
        let minor = rng.below(20);
        let patch = rng.below(20);
        let op = *rng.pick(&ops);
        let core = format!("{major}.{minor}.{patch}");
        let expr = format!("{op}{core}");
        let range = ApiVersionRange::parse(&expr)
            .unwrap_or_else(|e| panic!("generated range '{expr}' refused: {}", e.reason));
        for dm in 0..3u64 {
            for dn in 0..22u64 {
                for dp in 0..4u64 {
                    let v = SemVer::new(dm, dn, dp);
                    assert_eq!(
                        range.satisfied_by(v),
                        reference_satisfies(&expr, v),
                        "range '{expr}' decided differently from the recorded semantics for {v}; \
                         a string-compare drift would surface here"
                    );
                    checked += 1;
                }
            }
        }
    }
    println!("range-vs-reference grid: {checked} decisions compared (generated expressions)");
}

#[test]
fn semver_ranges_disagree_with_lexicographic_comparison_and_numeric_wins() {
    // The non-circular half: cases where lexicographic string ordering and
    // numeric SemVer ordering DISAGREE. The parsed-range answer must be the
    // NUMERIC one — a string comparison can never produce these verdicts.
    let numeric_wins: [(&str, &str, bool); 6] = [
        // "0.9.0" is lexicographically GREATER than "0.10.0" ('9' > '1').
        (">=0.10.0", "0.9.0", false),
        ("<0.10.0", "0.9.0", false),
        (">0.9.0", "0.10.0", true),
        ("<=0.9.0", "0.10.0", false),
        // Multi-digit patch: "0.5.11" vs "0.5.2" — lexicographic flips it.
        ("~0.5.2", "0.5.11", false),
        ("^0.5.2", "0.5.2", true),
    ];
    for (expr, version, expected) in numeric_wins {
        let range = ApiVersionRange::parse(expr)
            .unwrap_or_else(|e| panic!("recorded range '{expr}' refused: {}", e.reason));
        let v = SemVer::parse(version).expect("recorded version parses");
        assert_eq!(
            range.satisfied_by(v),
            expected,
            "range '{expr}' vs version '{version}': the numeric (range) answer must be \
             {expected}; a lexicographic string comparison produces the other answer"
        );
    }
}

#[test]
fn semver_range_mutants_are_refused_not_string_accepted() {
    // Generated malformed range fragments must be REFUSED (a parse error,
    // never a string-comparison acceptance). For every fragment: the
    // fragment alone is refused, AND the fragment appended after a valid
    // prefix is refused, AND the prefix alone is accepted — so the refusal
    // is attributable to the fragment, not the prefix.
    let fragments = [
        "0.5.x", ">=1.0", "1.0.0.0", "^", "~", ",", ">=,0.5.0", "0.5.0,", "  ",
        ">=1.0.0-", "1..0", "01.2.3", "1.2.3+meta", "1.2.3-beta", ">=<<1.0.0",
    ];
    let prefix = ">=0.4.0";
    let good_prefix_range = ApiVersionRange::parse(prefix)
        .unwrap_or_else(|e| panic!("the control prefix '{prefix}' refused: {}", e.reason));
    assert!(good_prefix_range.satisfied_by(SemVer::new(0, 5, 0)));
    for fragment in fragments {
        let alone = ApiVersionRange::parse(fragment);
        assert!(alone.is_err(), "malformed fragment '{fragment}' alone was ACCEPTED");
        let with_prefix = ApiVersionRange::parse(&format!("{prefix},{fragment}"));
        assert!(
            with_prefix.is_err(),
            "malformed fragment '{fragment}' after a valid prefix was ACCEPTED (string-compare?)"
        );
    }
}

// --- The config-surface twin (P7.19 validator: P1/P2/P3 over its inputs) ----------

#[test]
fn config_validator_never_panics_and_stays_named_on_generated_configs() {
    let fixture = parse_manifest(VALID_BUILTIN_MANIFEST)
        .expect("the builtin fixture parses (needed for the config surface)");
    let configs: [serde_json::Value; 10] = [
        serde_json::json!({}),
        serde_json::json!({"rated-power-mw": 2.5}),
        serde_json::json!({"rated-power-mw": "2.5"}),
        serde_json::json!({"rated-power-mw": null}),
        serde_json::json!({"rated-power-mw": -1e300}),
        serde_json::json!({"rated-power-mw": 2.5, "unknown-prop": true}),
        serde_json::json!({"rated-power-mw": 2.5, "password": "hunter2"}),
        serde_json::json!({"rated-power-mw": 2.5, "auth-token": "smuggled"}),
        serde_json::json!({"rated-power-mw": 2.5, "api-key": "env:REAL_ONE"}),
        serde_json::json!({"nested": {"deep": [{"x": 1}]}}),
    ];
    const CONFIG_NAMES: [&str; 18] = [
        "CONFIG_TOO_LARGE", "CONFIG_NOT_AN_OBJECT", "UNKNOWN_PROPERTY",
        "MISSING_REQUIRED", "CONFIG_WRONG_TYPE", "ENUM_VALUE_NOT_ALLOWED",
        "BOUND_VIOLATION", "SECRET_VALUE_IN_CONFIG", "SECRET_FIELD_COLLISION",
        "SECRET_REF_INVALID", "CONSTRAINTS_DOCUMENT_INVALID",
        "CONSTRAINTS_MISSING_FIELD", "CONSTRAINTS_UNKNOWN_FIELD",
        "CONSTRAINT_UNDECLARED", "CONSTRAINT_TYPE_MISMATCH", "CONSTRAINTS_TOO_LARGE",
        "CONFIG_DRIFT", "PLUGIN_ID_MISSING",
    ];
    for config in &configs {
        let verdict = catch_unwind(AssertUnwindSafe(|| {
            flexcore_kernel::plugin_config::resolve_for_instance(&fixture, "0.1.0", None, config)
        }))
        .unwrap_or_else(|payload| {
            panic!("P1 violated: the config validator panicked on {config} (payload: {payload:?})")
        });
        match verdict {
            Ok(resolved) => {
                // P3: the same inputs resolve to the identical view.
                let again = flexcore_kernel::plugin_config::resolve_for_instance(
                    &fixture, "0.1.0", None, config,
                )
                .expect("the resolution was stable across runs a moment ago");
                assert_eq!(
                    resolved.resolved_json(),
                    again.resolved_json(),
                    "the config resolution is not stable for {config}"
                );
            }
            Err(err) => {
                // P2: every config rejection is one of the recorded names.
                let name = err.name();
                assert!(
                    CONFIG_NAMES.contains(&name),
                    "config rejection '{name}' is not in the recorded table"
                );
            }
        }
    }
}

// --- Oversized input: the fail-closed size gate (bounded generation) --------------

#[test]
fn oversized_input_is_rejected_cleanly_never_parsed_never_panicking() {
    // One full limit-size padding chunk on top of the fixture (~1 KiB) puts
    // the document above the fail-closed 64 KiB gate — bounded generation,
    // no unbounded allocation.
    let pad = MAX_MANIFEST_BYTES;
    let mut value: serde_json::Value =
        serde_json::from_str(VALID_BUILTIN_MANIFEST).expect("the fixture is valid JSON");
    if let serde_json::Value::Object(map) = &mut value {
        map.insert("pad".to_string(), serde_json::Value::String("p".repeat(pad)));
    }
    let oversized = serde_json::to_string(&value).expect("serialization cannot fail");
    assert!(oversized.len() > MAX_MANIFEST_BYTES);
    let verdict = verdict_without_panic(&oversized, "oversize");
    assert_eq!(
        verdict,
        Verdict::Refused("MANIFEST_JSON_INVALID".to_string()),
        "oversized input must be rejected at the size gate, before parsing"
    );
}

// --- The vocabulary stays closed on generated permission combinations -------------

#[test]
fn generated_permission_combinations_stay_inside_the_closed_vocabulary() {
    let mut rng = Lcg::new(GENERATOR_SEED ^ 0xC0DF_F16A);
    let base = r#"{
        "manifest-schema": "flexcore-plugin-manifest-v1",
        "plugin-id": "neryva.perm-probe",
        "name": "p",
        "version": "0.1.0",
        "api-version": ">=0.5,<1.0",
        "class": "builtin-rust",
        "entry-point": { "registered-type": "t" },
        "provider-of": [],
        "consumes": [],
        "permissions": ["PERMISSION_SLOT"],
        "configuration-schema": { "type": "object", "additionalProperties": false },
        "resources": { "max-memory-mb": 1, "timeout-ms": 1 },
        "metadata": { "license": "Apache-2.0", "description": "d" }
    }"#;
    for _ in 0..128 {
        // A random id from the allowed set is accepted; a random never-granted
        // id is refused with PERMISSION_NEVER_GRANTED; a random junk id is
        // refused with PERMISSION_ID_UNKNOWN. No combination leaks through.
        let allowed = *rng.pick(&ALLOWED_PERMISSIONS);
        let never = *rng.pick(&NEVER_GRANTED_PERMISSIONS);
        let junk = format!("perm-{}", rng.below(1_000_000));
        let build = |permission: &str| base.replace("PERMISSION_SLOT", permission);
        match verdict_without_panic(&build(allowed), "allowed-permission") {
            Verdict::Valid(_) => {}
            other => panic!("an allowed permission '{allowed}' was refused: {other:?}"),
        }
        assert_eq!(
            verdict_without_panic(&build(never), "never-granted-permission"),
            Verdict::Refused("PERMISSION_NEVER_GRANTED".to_string()),
            "the never-granted permission '{never}' was not refused with its named error"
        );
        assert_eq!(
            verdict_without_panic(&build(&junk), "junk-permission"),
            Verdict::Refused("PERMISSION_ID_UNKNOWN".to_string()),
            "the junk permission '{junk}' was not refused with its named error"
        );
    }
}
