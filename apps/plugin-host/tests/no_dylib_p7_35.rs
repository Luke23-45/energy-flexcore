// P7.35 — ADR-006 automated enforcement: the no-dylib lexical-scan test
//         (report §22.1 ADR-006, §5.4; ledger `ledger_phase_7.md` §P7.35).
//         Written, NEVER RUN — the loop bans hold; the first human
//         `cargo test -p flexcore-plugin-host --test no_dylib_p7_35` run is
//         the verification gate.
//
// WHAT THIS IS: the test-side twin of the CI `no-dylib-gate` job — the SAME
// calibrated token set over the SAME trees, so the ADR-006 prohibition ("no
// arbitrary Rust dynamic-library loading, including via a C ABI wrapper as
// the default mechanism"; ENTRY_NOTE boundary 2) is enforced BOTH in CI and
// in the workspace test suite. It follows the P6.07 structural-scan pattern
// (`crates/edge-agent/tests/no_inbound_listener.rs`): a lexical gate that
// fails on the FIRST introduction of the banned shape, with nothing
// "disabled" or "guarded".
//
// TOKEN CALIBRATION (recorded against the current tree — the gate is
// green-by-construction; the recorded baseline greps live in
// `ledger/phase-07-plugin-sdk/26_27_35_ci_performance.md` §3):
//
// - `.rs` tokens: `libloading`, `dlopen`, `#[link(name`, `extern "C"`,
//   `LoadLibrary`, `GetProcAddress` — ZERO hits on the current tree.
//   The bare word `dylib` is deliberately NOT a `.rs` token: it legitimately
//   occurs in prose ("not dylibs ...") in
//   `crates/edge-agent/src/adapters/registry.rs` and in the kernel manifest
//   validator's NEGATIVE test (`crates/flexcore-kernel/tests/manifest_p7_05.rs`
//   asserts a `class: "dylib"` manifest is REJECTED — the P7.05 gate is
//   itself part of the ADR-006 defence). A bare-word grep would false-fail
//   on the defences.
// - `Cargo.toml` tokens: `crate-type` carrying a `dylib`/`cdylib` value,
//   plus the loader-crate names as dependencies — ZERO hits on the current
//   tree (the workspace builds lib/bin targets only).
//
// SCOPE (stated honestly, mirrored in the CI job comment):
//
// - Scanned: every `crates/*/src/**` and `apps/*/src/**` `.rs` file, every
//   workspace `Cargo.toml`, and everything under `plugins/` (examples are
//   the ledger's named hiding place: "a check that scans only crates/ while
//   a loader hides in an example").
// - NOT scanned: `tests/**` — test files never load libraries at runtime,
//   and negative tests legitimately NAME the banned tokens (including this
//   file, which defines them). The exclusion is documented here and in the
//   CI job — never silent.
//
// DOCUMENTED LIMITS (the honesty rule; mirrored in the CI job and the
// decision record): this is a LEXICAL gate. It catches direct
// dynamic-library loading patterns; it cannot catch every possible FFI
// route (e.g. an indirect build-script-generated binding, or a
// hand-rolled `extern "C"` table spelled without any listed token). The
// structural guarantee rests on THREE legs together: this gate, the
// dependency rule (`tools/check_dependency_rule.py` — the composition-crate
// graph), and review. ADR-006 remains a design decision that review
// enforces; the gate makes the obvious routes mechanically fail.

use std::path::{Path, PathBuf};

/// The `.rs` loader tokens (calibrated — see the header). Any occurrence in
/// a scanned source file is an ADR-006 violation.
const FORBIDDEN_RS_TOKENS: [&str; 6] = [
    "libloading",
    "dlopen",
    "#[link(name",
    "extern \"C\"",
    "LoadLibrary",
    "GetProcAddress",
];

/// The `Cargo.toml` loader tokens: a crate-type that produces a dynamic
/// library, or a loader crate as a dependency. (`crate-type` is checked by
/// line so the `dylib`/`cdylib` value test applies to the declaration, not
/// to unrelated prose.)
const FORBIDDEN_MANIFEST_TOKENS: [&str; 4] = ["libloading", "dlopen", "LoadLibrary", "GetProcAddress"];
const DYNAMIC_CRATE_TYPE_VALUES: [&str; 2] = ["dylib", "cdylib"];

fn collect_files(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return; // an absent tree (e.g. no plugins/ in a shallow checkout) scans as empty
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, extension, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some(extension) {
            out.push(path);
        }
    }
}

fn collect_cargo_manifests(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // `target/` holds build artifacts, never sources — skipped.
            if path.file_name().and_then(|n| n.to_str()) == Some("target") {
                continue;
            }
            collect_cargo_manifests(&path, out);
        } else if path.file_name().and_then(|n| n.to_str()) == Some("Cargo.toml") {
            out.push(path);
        }
    }
}

fn scan_rs_tokens(files: &[PathBuf], violations: &mut Vec<String>) {
    for file in files {
        let content = std::fs::read_to_string(file).expect("source file is readable");
        for token in FORBIDDEN_RS_TOKENS {
            if content.contains(token) {
                violations.push(format!("{file:?} contains forbidden token '{token}'"));
            }
        }
    }
}

fn scan_manifest_tokens(files: &[PathBuf], violations: &mut Vec<String>) {
    for file in files {
        let content = std::fs::read_to_string(file).expect("manifest file is readable");
        for token in FORBIDDEN_MANIFEST_TOKENS {
            if content.contains(token) {
                violations.push(format!("{file:?} contains forbidden token '{token}'"));
            }
        }
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue; // a commented-out line is not a declaration
            }
            if trimmed.contains("crate-type")
                && DYNAMIC_CRATE_TYPE_VALUES.iter().any(|v| trimmed.contains(v))
            {
                violations.push(format!(
                    "{file:?} declares a dynamic-library crate-type: {trimmed:?}"
                ));
            }
        }
    }
}

// --- 1. The host crate itself (the crate that runs untrusted code) ------------

#[test]
fn the_plugin_host_crate_contains_no_dynamic_library_loading_surface() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = manifest_dir.join("src");
    let mut files = Vec::new();
    collect_files(&src, "rs", &mut files);
    assert!(
        files.len() >= 8,
        "the scan must actually cover the plugin-host sources (found {} files under {:?})",
        files.len(),
        src
    );

    let mut violations: Vec<String> = Vec::new();
    scan_rs_tokens(&files, &mut violations);
    assert!(
        violations.is_empty(),
        "ADR-006 VIOLATION — the plugin host must never load a dynamic library \
         (report §22.1 ADR-006, §5.4; ledger P7.35; extensions are WASM/WIT or \
         out-of-process gRPC workers only):\n{}",
        violations.join("\n")
    );
}

// --- 2. The workspace-wide twin of the CI gate ----------------------------------

#[test]
fn the_workspace_source_and_manifest_trees_contain_no_dylib_loading_surface() {
    // `apps/plugin-host` -> repo root is two levels up.
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let repo_root = repo_root
        .canonicalize()
        .expect("the repository root resolves from the plugin-host crate");

    let mut rs_files: Vec<PathBuf> = Vec::new();
    for member_dir in ["crates", "apps"] {
        let dir = repo_root.join(member_dir);
        assert!(
            dir.is_dir(),
            "the {member_dir}/ tree must exist for the workspace scan"
        );
        collect_files(&dir, "rs", &mut rs_files);
    }
    // Every member's src/** only: the collected set above includes tests/,
    // which are deliberately OUT of scope (see the header). Filter here so
    // the workspace scan and the CI gate share ONE definition of scope.
    let src_files: Vec<PathBuf> = rs_files
        .into_iter()
        .filter(|p| {
            p.components()
                .any(|c| c.as_os_str().to_str() == Some("src"))
        })
        .collect();
    assert!(
        src_files.len() >= 40,
        "the workspace scan must actually cover the member sources (found {} src .rs files)",
        src_files.len()
    );

    let mut manifests: Vec<PathBuf> = Vec::new();
    collect_cargo_manifests(&repo_root, &mut manifests);
    assert!(
        manifests.len() >= 15,
        "the workspace scan must cover the member Cargo.toml files (found {})",
        manifests.len()
    );

    // `plugins/` — the ledger's named hiding place (examples, manifests).
    let mut plugin_files: Vec<PathBuf> = Vec::new();
    let plugins = repo_root.join("plugins");
    collect_files(&plugins, "rs", &mut plugin_files);
    collect_files(&plugins, "json", &mut plugin_files);
    collect_files(&plugins, "toml", &mut plugin_files);

    let mut violations: Vec<String> = Vec::new();
    scan_rs_tokens(&src_files, &mut violations);
    scan_rs_tokens(&plugin_files, &mut violations);
    scan_manifest_tokens(&manifests, &mut violations);
    assert!(
        violations.is_empty(),
        "ADR-006 VIOLATION — no arbitrary Rust dylib loading may exist anywhere in \
         the workspace (report §22.1 ADR-006; ledger P7.35; this is the same scan \
         the CI `no-dylib-gate` job runs):\n{}",
        violations.join("\n")
    );
}

// --- 3. The manifest validator remains part of the defence ----------------------

#[test]
fn the_kernel_manifest_validator_still_rejects_the_dylib_runtime_class() {
    // The P7.05 manifest gate is one of the structural legs ADR-006 stands
    // on (a `runtime.class: "dylib"` manifest is refused before any host
    // sees it). This test does not re-run that validator (the kernel crate
    // is a separate wave's ownership); it pins the DEFENCE-IN-DEPTH claim:
    // the negative case exists in the kernel's own suite and must stay.
    let kernel_test = repo_root_from_here()
        .join("crates")
        .join("flexcore-kernel")
        .join("tests")
        .join("manifest_p7_05.rs");
    let content =
        std::fs::read_to_string(&kernel_test).expect("the kernel manifest test is readable");
    assert!(
        content.contains("\"dylib\""),
        "the kernel manifest validator's negative test must keep its dylib-class \
         rejection case ({kernel_test:?}) — it is part of the ADR-006 defence"
    );
}

fn repo_root_from_here() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("the repository root resolves")
}
