#!/usr/bin/env python3
"""P1.04 — Enforce the hexagonal dependency rule (report §6.2, §8.2).

Parses every workspace member's `Cargo.toml` and fails if a forbidden edge
exists. Run in CI (P1.27) as a required gate:

    python tools/check_dependency_rule.py

Policy (the allowed/forbidden edge classes):

- Infrastructure identifiers (crate names): axum, sqlx, async-nats, tokio,
  hyper, tower, tower-http, utoipa, tonic, prost, wasmtime, redis — plus the
  adapter/app crates themselves. A core crate must not depend on any of them.
- flexcore-contracts: std + serde/serde_json/time only (value types).
- flexcore-domain: flexcore-contracts only (+ std). Domain depends on
  nothing infrastructure.
- flexcore-ports: domain + contracts only. Ports define interfaces.
- flexcore-events: contracts + domain only.
- flexcore-application: domain + ports + contracts + events (application
  orchestration; the outbox record type lives in flexcore-events).
- flexcore-storage / flexcore-audit / flexcore-authz: domain + contracts
  (+ events where justified and recorded).
- crates/adapters/* and apps/*: no restriction on dependencies (they are the
  only places allowed to reference infrastructure), and nothing may depend
  ON them except apps.

An exception requires an ADR and a review, not a comment (P1.04 step 4) —
add it to ALLOWED_EXTRA_EDGES with the ADR reference.

Exit codes: 0 = tree conforms; 1 = violation found; 2 = usage/IO error.
Requires Python 3.11+ (stdlib only).
"""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

# Crate name fragments that mean "infrastructure" for core crates.
INFRA_IDENTIFIERS = {
    "axum", "sqlx", "async-nats", "tokio", "hyper", "tower", "tower-http",
    "utoipa", "tonic", "prost", "wasmtime", "redis", "tracing",
    "tracing-subscriber",
}

# Core crates: the hexagonal inside of the monolith.
CORE_CRATES = {
    "flexcore-contracts",
    "flexcore-domain",
    "flexcore-ports",
    "flexcore-application",
    "flexcore-events",
    "flexcore-storage",
    "flexcore-audit",
    "flexcore-authz",
    "flexcore-optimization-api",
    "flexcore-grid-api",
}

# Allowed internal edges (from -> set of to). Anything else between core
# crates is a violation. Adapters/apps are not listed: core -> adapter/app is
# always forbidden; adapter/app -> anything is always allowed.
ALLOWED_INTERNAL_EDGES: dict[str, set[str]] = {
    "flexcore-contracts": set(),
    "flexcore-domain": {"flexcore-contracts"},
    "flexcore-ports": {"flexcore-contracts", "flexcore-domain"},
    "flexcore-events": {"flexcore-contracts", "flexcore-domain"},
    "flexcore-application": {
        "flexcore-contracts", "flexcore-domain", "flexcore-ports",
        "flexcore-events", "flexcore-optimization-api",
    },
    "flexcore-storage": {"flexcore-contracts", "flexcore-domain", "flexcore-events"},
    "flexcore-audit": {"flexcore-contracts", "flexcore-domain", "flexcore-events"},
    "flexcore-authz": {"flexcore-contracts", "flexcore-domain"},
    "flexcore-optimization-api": {"flexcore-contracts"},
    "flexcore-grid-api": {"flexcore-contracts"},
}

# Exceptions granted by ADR: (from_crate, to_identifier) -> ADR reference.
# Keep this list empty unless a reviewed ADR exists.
ALLOWED_EXTRA_EDGES: dict[tuple[str, str], str] = {}

# Workspace-member package names that are adapters/apps (composition side).
COMPOSITION_CRATES = {
    "flexcore-postgres-adapter",
    "flexcore-nats-adapter",
    "flexcore-simulator",
    "flexcore-sim-runner",
    "flexcore-control-plane",
    "flexcore-worker",
}

MEMBER_DIRS = [
    "crates/flexcore-contracts",
    "crates/flexcore-domain",
    "crates/flexcore-ports",
    "crates/flexcore-application",
    "crates/flexcore-events",
    "crates/flexcore-storage",
    "crates/flexcore-audit",
    "crates/flexcore-authz",
    "crates/flexcore-optimization-api",
    "crates/flexcore-grid-api",
    "crates/adapters/postgres",
    "crates/adapters/nats",
    "crates/flexcore-simulator",
    "apps/control-plane",
    "apps/sim-runner",
    "apps/worker",
]


def dep_names(manifest: dict) -> list[str]:
    names: list[str] = []
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        deps = manifest.get(section, {})
        names.extend(deps.keys())
    return names


def main() -> int:
    violations: list[str] = []
    checked = 0
    for rel in MEMBER_DIRS:
        cargo = REPO_ROOT / rel / "Cargo.toml"
        if not cargo.exists():
            violations.append(f"{rel}: missing Cargo.toml (workspace member incomplete)")
            continue
        try:
            manifest = tomllib.loads(cargo.read_text(encoding="utf-8"))
        except tomllib.TOMLDecodeError as exc:
            violations.append(f"{rel}: unparseable Cargo.toml: {exc}")
            continue
        package = manifest.get("package", {})
        name = package.get("name", rel)
        if name not in CORE_CRATES and name not in COMPOSITION_CRATES:
            violations.append(
                f"{rel}: package '{name}' is not a declared workspace member — "
                "update MEMBER_DIRS in tools/check_dependency_rule.py"
            )
            continue
        checked += 1
        if name not in CORE_CRATES:
            continue  # adapters/apps may use infrastructure
        for dep in dep_names(manifest):
            if (name, dep) in ALLOWED_EXTRA_EDGES:
                continue  # ADR-granted exception
            base = dep.split()[-1] if dep else dep
            if dep in COMPOSITION_CRATES or base in COMPOSITION_CRATES:
                violations.append(
                    f"{name}: forbidden edge to composition crate '{dep}' (§6.2: "
                    "core crates must not depend on adapters/apps)"
                )
            elif dep in INFRA_IDENTIFIERS or base in INFRA_IDENTIFIERS:
                violations.append(
                    f"{name}: forbidden infrastructure dependency '{dep}' (§6.2 "
                    "dependency rule; an exception needs an ADR, recorded in "
                    "ALLOWED_EXTRA_EDGES)"
                )
            elif dep in CORE_CRATES and dep not in ALLOWED_INTERNAL_EDGES.get(name, set()):
                violations.append(
                    f"{name}: internal edge to '{dep}' is not in the allowed set "
                    f"{sorted(ALLOWED_INTERNAL_EDGES.get(name, set()))} (§8.2)"
                )
    if violations:
        print(f"FAIL — dependency-rule violations ({len(violations)}):")
        for v in violations:
            print(f"  - {v}")
        return 1
    print(f"OK — dependency rule holds for {checked} workspace members.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except OSError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        sys.exit(2)
