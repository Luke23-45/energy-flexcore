// P6.07 — STRUCTURAL no-inbound-listener test (report §3.3, §18.2; ledger
//         P6.07 acceptance criterion: "no listener socket is opened on the
//         edge for inbound central connections").
//
// The guarantee is structural: the edge agent crate must not contain ANY
// listen/bind/accept surface — the agent dials OUT (§3.3) or fails. This
// test lexically scans EVERY `.rs` file under `crates/edge-agent/src/`
// (including `main.rs` and every future module) for listener-type tokens
// and FAILS if one is introduced. Nothing may be "disabled" or guarded —
// an inbound path must not exist in the code shape at all.
//
// Scope note (honesty): the outbound DIAL is not forbidden —
// `TcpStream::connect*` is the edge's only legitimate socket operation
// (`uplink_mtls.rs`), and this test deliberately does NOT flag connect.
// No network operation is performed by this test (it reads source files;
// UNRUN like every other test in this loop — the ENTRY_NOTE bans hold).
//
// UNRUN — written, never executed this wave; the human build pass runs it.

use std::path::{Path, PathBuf};

/// Listener-type tokens whose appearance anywhere in the edge-agent crate
/// source is a structural violation of the outbound-only principle.
const FORBIDDEN_TOKENS: [&str; 8] = [
    "TcpListener",
    "UdpSocket",
    "TcpListener::bind",
    "::bind(",
    ".bind(",
    ".listen(",
    "listen(",
    ".accept(",
];

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).expect("the edge-agent src directory is readable");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[test]
fn the_edge_agent_crate_contains_no_inbound_listener_surface() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = manifest_dir.join("src");
    let mut files = Vec::new();
    collect_rs_files(&src, &mut files);
    assert!(
        files.len() >= 10,
        "the scan must actually cover the crate sources (found {} files under {:?})",
        files.len(),
        src
    );

    let mut violations: Vec<String> = Vec::new();
    for file in &files {
        let content = std::fs::read_to_string(file).expect("source file is readable");
        for token in FORBIDDEN_TOKENS {
            if content.contains(token) {
                violations.push(format!("{file:?} contains forbidden token '{token}'"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "OUTBOUND-ONLY VIOLATION — the edge agent must never open an inbound listener \
         (report §3.3, §18.2; ledger P6.07):\n{}",
        violations.join("\n")
    );
}

#[test]
fn the_binary_entrypoint_is_covered_by_the_scan() {
    // main.rs is part of src/ and therefore scanned; assert it exists so
    // the scan can never silently skip the binary target.
    let main = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs");
    assert!(main.exists(), "src/main.rs must exist and be scanned");
}
