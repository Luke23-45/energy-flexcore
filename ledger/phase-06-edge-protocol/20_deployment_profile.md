# P6.20 — Edge deployment profile: packaging, service, and signed updates

Date: 2026-11-12. Status: **DESIGN RECORDED — nothing built, nothing
installed, nothing executed.** Wave 1b of Phase 6
(`ledger/phase-06-edge-protocol/ENTRY_NOTE.md`); loop bans in force. Every
 artefact below is a documented design or a plain config file; the install,
 upgrade, rollback, and signature-tamper tests the ledger P6.20 exit check
 requires are **NOT RUN** and are listed in §8 as the open verification work.

Report trace: §18.3 (edge deployment profile), §21.4 (build form, control
authority modes), §21.2 (availability/recovery), §22.2–§22.3 (CI/release
gates), §9.3 (supply chain: SBOM, signatures, no self-upgrade), §2.1 (v0.4.0
read-only/test-lab default).

---

## 1. Scope and honest status

| Ledger P6.20 step | Status in this wave | Evidence |
|---|---|---|
| 1. Define the deployment profile (Linux x86-64/ARM64, systemd or approved container runtime, explicit network allowlist, no shell access for plugins) | **RECORDED as design** (§§2–4 here + `deploy/edge/flexcore-edge-agent.service`) | the unit file and the network allowlist below are written artifacts, never installed |
| 2. Build the versioned package/OCI image with checksums and SBOM | **NOT BUILT** — the build procedure is recorded (§5); `cargo build` is banned this wave | no package artifact exists in the tree |
| 3. Signed update bundles: verification before install, staged rollout, rollback on failure, audit per update | **DESIGN only** (§6) — signature verification is specified as a deployment requirement; **no PKI, no signing keys, no verification code exists** | honest gap recorded in §7 |
| 4. Test install / upgrade / failed upgrade (rollback) / downgrade on both architectures | **NOT RUN** | §8 open verification tasks |

Nothing in this document implies a pilot site is configured, connected, or
approved — the D6 cyber/OT boundary review (P6.03) is OPEN and gates any
real deployment (CONTEXT.md §8).

## 2. Deployment shape (§18.3, §3.3)

| Item | Decision (DEFAULT, to be confirmed per site at D6) | Reason |
|---|---|---|
| Runtime form | A single static Rust binary (`edge-agent`) under **systemd** on Linux x86-64 and ARM64; an OCI image is an ALTERNATIVE for sites that mandate containers — not built in this wave | §18.3 names "systemd service or an approved container runtime"; systemd is the simpler first profile and avoids a container-toolchain dependency in the 0.x line |
| Architectures | x86-64 and ARM64, built by the same procedure (§5); **neither is exercised yet** — the §23/§21.4 requirement that both ship is a release-gate item (P6.27 CI, P6.30) | the report requires both architectures but names no hardware (CONTEXT.md open question 3 — the target gateway hardware profile is UNDECIDED) |
| Process identity | Dedicated non-login system user `flexcore-edge` (no shell, `nologin`); the agent runs with an empty capability bounding set | §18.3 hardening; least privilege |
| Network posture | **Outbound-only** (§3.3): the agent dials the control-plane endpoint (mTLS from P6.07) and the site EMS API (from the composed adapter). No listening socket exists in the binary, and the unit file ships no `Listen*` directive. The explicit network allowlist is: (a) the control-plane endpoint from `FLEXCORE_EDGE_CONTROL_PLANE_ENDPOINT`, (b) the site EMS endpoint from the adapter configuration. Anything else is absent by construction | the §3.3 topology rule; inbound internet access into OT is to be avoided |
| Filesystem | `ProtectSystem=strict`; writable state confined to the buffer/state directory (`/var/lib/flexcore-edge`, 0700) and, once P6.09 lands, nothing else. Configuration read from `/etc/flexcore/` | §18.3; the buffer is the agent's only legitimate write surface |
| Plugins | No plugin runtime exists at the edge in this wave; when the v0.5.0 plugin SDK/WASM host reaches the edge profile, it inherits this profile's no-shell rule (§18.3) — plugins get no shell, no arbitrary process execution | §5.4/ADR-006 (no `dylib` loading) carried to the edge |
| Time | `Wants=time-sync.target`; start only after sync. Clock quality is a recorded field (P0.09 §4), never assumed | §17.4 clock drift; P6.31 owns edge time-sync details |

## 3. Package / directory layout (recorded, not built)

```text
/usr/local/bin/edge-agent              the binary (release build; §5 procedure)
/etc/flexcore/edge/env                 environment file: FLEXCORE_EDGE_* required vars
/etc/flexcore/edge/adapter-allowlist   P6.16 enforcement point (future; adapter allowlist config)
/var/lib/flexcore-edge/                buffer + state root (0700, StateDirectory)
  buffer/                              P6.09 durable store-and-forward buffer
  audit/                               P6.18 offline audit buffer (future)
/usr/lib/systemd/system/flexcore-edge-agent.service   the unit (deploy/edge copy)
```

- The service file is versioned in-tree at `deploy/edge/flexcore-edge-agent.service`
  as a **documented artifact** — it is NOT installed anywhere and its
  directives are systemd-standard hardening options; site adaptation happens
  at deployment time under D6.
- The shipped environment file **deliberately omits**
  `FLEXCORE_EDGE_COMMAND_PATH_ENABLED`: the flag defaults to `false` in
  code (`crates/edge-agent/src/config.rs`), so the shipped profile cannot
  accidentally ship the flag on — the P6.14 risk ("a config example or test
  fixture that ships the flag on") is countered by absence. Enabling it is
  a signed, per-site, test-env-only change under the P6.14 gate, never an
  edit to the shipped file.
- Config precedence: environment file → unit environment → binary defaults.
  There are no default credentials anywhere; the endpoint may embed
  credentials and is redacted from all log renderings (`EdgeConfig: Debug`).

## 4. Resource profile (configuration bounds, NOT measured)

All bounds are **configuration** — the honest status is that no edge
hardware profile has been measured (P6.22 owns the performance work; CONTEXT
open question 3 means the target hardware is itself undecided).

| Resource | Recorded bound | Where enforced |
|---|---|---|
| Buffer capacity | `FLEXCORE_EDGE_BUFFER_CAPACITY`, default 10 000 samples, hard-bounded 1..=1 000 000 | `config.rs` parse bounds; runner backpressure (pause polling at capacity — audited, never a silent drop) |
| Forward batch | `FLEXCORE_EDGE_FORWARD_BATCH_SIZE`, default 100, bounded 1..=10 000 | `config.rs` |
| Poll cadence | `FLEXCORE_EDGE_POLL_INTERVAL_MS`, default 5 000 ms (supervisory minutes-scale, §3.5 — never sub-second) | `config.rs` |
| Memory ceiling | `MemoryMax=256M`, `MemoryHigh=192M` in the unit file — a STARTING BOUND chosen conservatively, not a measured requirement | systemd cgroup limits |
| Task ceiling | `TasksMax=64` | systemd |
| Restart | `Restart=on-failure`, `RestartSec=5s` — with the recorded caveat that the P6.09 durable buffer does not exist yet: until it lands, an in-memory restart discards un-forwarded samples, and the startup log says so | unit + `main.rs` startup log |

## 5. Build and package procedure (recorded; NEVER executed)

The procedure below is the reproducible-build NOTE the ledger criteria ask
for — written as the steps a maintainer runs; **none of it was executed in
this wave** (the loop bans forbid building). Toolchain pin: Rust **1.99.1**
stable, exact-pinned in `rust-toolchain.toml` since P1.02 (research cut-off
7 November 2026; the 1.100 bump policy in that file's header applies — it
was NOT adopted).

1. `cargo build --release -p flexcore-edge-agent` (workspace; exact toolchain
   pin honoured automatically). x86-64 and ARM64 are two separate release
   builds of the same source (cross-compile via the pinned toolchain's
   targets or native builders — the CI design in P6.27 decides; recorded as
   OPEN, not assumed).
2. Record the version triple in the release notes: package version, git
   commit, toolchain (`rustc --version` under the pin).
3. Generate checksums (`sha256sum`) for each produced binary and the
   service file copy.
4. Generate an SBOM for the release (§9.3, §22.2). **Tool UNDECIDED** — the
   report requires an SBOM per release but names no tool; candidate
   approaches (cargo-auditable, cargo-cyclonedx, Syft on the OCI image) are
   recorded in `.agent/PROVENANCE.md` terms as a to-be-audited dependency
   decision (P6.02/D4 discipline), NOT chosen here. No hallucinated tool
   pin.
5. Publish binary + checksums + SBOM together (the §21.4 release-gate
   requirement; P6.30 verifies the set at the phase close).

Reproducibility status: HONEST — the build is pinned by toolchain and
source commit but **bit-for-bit reproducibility is not claimed and not
tested**; making it reproducible (locked build environment, deterministic
paths) is a recorded candidate item for P6.27, not an assertion.

## 6. Signed updates: design (no PKI implemented)

**The requirement (§9.3, §17.5, §18.3): updates are SIGNED, verified before
install, staged, and reversible. There is no self-upgrade.** The design
below records how the edge meets it; what does NOT exist yet is stated in
§7.

```text
operator/release CI ──signs──► update bundle (binary + manifest + signature)
        │                                        │
        │ staged rollout (per site, per cohort)  │ downloaded over the
        ▼                                        ▼ outbound channel (P6.07)
  edge: download → VERIFY SIGNATURE (before ANY swap)
        → stage to disk (never overwrite the running binary)
        → health-gated activation:
              restart into the staged binary
              → health check passes (P6.17 local health) → keep, audit "applied"
              → health check fails  → automatic rollback to the previous
                version (kept on disk), audit "rolled_back", operator visible
```

| Design rule | Content | Trace |
|---|---|---|
| Signature verification BEFORE swap | The bundle manifest (version, checksums, expiry) is signature-verified against the platform release key BEFORE any file is touched; a bad or missing signature refuses the bundle entirely — nothing partial is ever installed | §9.3; ledger risk "unsigned updates as an attack path into OT" |
| No self-upgrade | The AGENT never installs its own update: activation is performed by the package layer (systemd/`dpkg`-equivalent or a dedicated out-of-process updater owned by the OS), so a compromised agent cannot silently replace itself with a malicious build and the update path stays operator-auditable | §9.3 "no self-upgrade" (CONTEXT.md §2) |
| Staged rollout | Updates roll out per site and per cohort, with a hold window between cohorts; a failing cohort freezes the rollout | §21.4 staged deployment; §22.3 release gates |
| Rollback on failed health | The previous version's binary is retained on disk until the new version passes its health check (P6.17 defines "healthy": loop alive, buffer sane, adapter health visible); a failed check triggers automatic rollback + audit | ledger P6.20 step 3; risk "a failed upgrade bricking a remote edge" |
| Audit per update | Applied / rolled back / refused (bad signature) are each audit events on the edge (P6.18 offline audit buffer) and surfaced centrally | ledger P6.20 step 3; P4.28/P4.30 audit discipline |
| Version visibility | The running version is visible to the operator (health surface, P6.17/P6.25) | ledger P6.20 exit check |
| Downgrade | An explicit operator-signed action only (never automatic); recorded with the same audit events | ledger P6.20 step 4 |

**Key management (recorded honestly):** the signing keys, the trust anchor
distribution to edges, rotation, and revocation are **Stage-6+
deployment requirements, not implemented**. No real PKI exists in this
wave: no release key pair was generated, no signature format was chosen,
no verification code was written, and nothing was signed. This is a
deployment-time requirement that must exist before ANY edge receives an
update — the design above is the contract its implementation must satisfy.
Enrollment key material (P6.06) is a separate, likewise-unimplemented
lifecycle.

## 7. What does NOT exist yet (the honest gap list)

| Absent artifact | Owning wave / gate |
|---|---|
| Built binary, package image, checksums, SBOM | §5 procedure above; P6.27 CI / P6.30 release gate |
| Signature verification code + release keys + trust anchor distribution | §6 design; Stage-6+ deployment requirement, before any pilot update |
| The durable disk buffer the restart policy assumes | P6.09 |
| Local health definition driving rollback activation | P6.17 |
| Offline audit buffer backing the per-update audit events | P6.18 |
| Install/upgrade/rollback/downgrade tests on x86-64 AND ARM64 | §8; ledger P6.20 verification method |
| Target gateway hardware profile (CPU/RAM/disk per architecture) | CONTEXT.md open question 3 — human decision |

## 8. Open verification tasks (the P6.20 exit check, for the human run)

1. Build the release binary per §5 on both architectures; quote
   `rustc --version` under the pin and the artifact checksums.
2. Install the unit on a lab host; confirm the agent starts fail-fast with
   a valid environment file and refuses to start without one (exit 2, named
   error), and that NO listening socket opens (`ss -tlnp` shows nothing for
   the agent).
3. Upgrade path: stage a signed update, confirm verification refusal on a
   tampered bundle (bad signature), confirm health-gated activation and
   automatic rollback on a simulated failed health check, confirm each
   outcome produced an audit event and the running version is visible.
4. Downgrade path: explicit signed downgrade, audited.
5. Repeat 2–4 on the second architecture.

Each of these is UNRUN. The P6.20 ledger checkbox is ticked
"implemented, awaiting human build/test" in that exact sense: the design
record, the unit file, and the configuration artifacts exist; nothing was
built, installed, signed, verified, or executed.
