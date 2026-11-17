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
    # P14.01 — the common protocol-adapter port layer (scope declaration,
    # capability manifest, connection lifecycle) that every
    # crates/adapters/protocol-* crate implements. Core: adapters depend on
    # it; no core crate depends on it. serde-only dependency line — no
    # transport, no runtime (decision record
    # docs/phases-11-20/phase-14-protocol-device-connectivity/records/
    # 01_p14_01_adapter_architecture.md).
    "flexcore-protocol-ports",
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
    # P14.01 — the protocol-ports layer depends on NO other workspace crate
    # (the scope declaration mirrors flexcore-grid-api's self-contained
    # scope.rs; serde sits outside the internal-edge check).
    "flexcore-protocol-ports": set(),
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
    # P6.05 — edge agent (composition root for the edge deployment; allowed
    # to use infrastructure like apps/*; nothing depends ON it except future
    # crates/adapters/<protocol> crates implementing its EmsAdapter port).
    "flexcore-edge-agent",
    # P7.05/P7.06 — plugin kernel (manifest validation + dependency
    # resolution; report §8.1/§8.2: flexcore-kernel manages plugin
    # composition and lifecycle). Composition side, like apps/worker: the
    # core-crate edge policy does not apply to it, and nothing depends ON it
    # except future composition roots (P7.08 registry, apps/*).
    "flexcore-kernel",
    # P7.10/P7.11 — WASM plugin host (apps/plugin-host): the constrained
    # component-model host prototype as a SEPARATE process (report §8.3:
    # extracted for security isolation) plus the P7.11 capability broker.
    # Composition side (apps/* rules): it may use infrastructure — wasmtime
    # is exactly the infrastructure identifier this classification exists to
    # allow — and nothing depends ON it except future composition (P7.20
    # gRPC template, P7.34 mTLS trust boundary).
    "flexcore-plugin-host",
    # P8.04–P8.06 — flexcore-research (the Phase 8 independent validation
    # harness + the frozen deterministic baseline registry). Composition
    # side, like flexcore-kernel/plugin-host: the crate composes the
    # platform's EXISTING seams (the §12.5 feasibility gate surface of
    # flexcore-application as an INJECTED port, the §12.6 result
    # vocabulary, the §13.4 proposal vocabulary) rather than defining a new
    # core contract; the core-crate edge policy does not apply to it, and
    # no core crate may depend on it — nothing depends ON it except future
    # composition roots (see
    # ledger/phase-08-advanced-coordination/05_06_harness_baselines.md §3
    # for the recorded placement rationale).
    "flexcore-research",
    # P14.02 — the IEEE 2030.5 AggregatorServer adapter (composition side,
    # like crates/adapters/*: it may use infrastructure, but deliberately
    # adds none — the mTLS/HTTP transport is a designed UNRUN seam; decision
    # record 02_p14_02_aggregator_server.md). Nothing depends ON it except
    # future composition roots.
    "flexcore-protocol-20305",
    # P14.05 — the OpenADR 3.0 VTN adapter (composition side, like
    # crates/adapters/*: no HTTP framework, no transport dependency — the
    # endpoint serving is a designed seam; decision record
    # 21_p14_05_openadr_vtn.md). It depends on the CORE protocol-ports
    # layer and flexcore-domain; nothing depends ON it except future
    # composition roots.
    "flexcore-openadr-adapter",
    # P14.07 — the SunSpec Modbus client adapter (composition side: no
    # Modbus stack, no serial/TCP runtime — the TCP/RTU transport is a
    # designed UNWIRED seam; decision record 23_p14_07_sunspec_modbus_
    # client.md). Nothing depends ON it except future composition roots.
    "flexcore-modbus-adapter",
    # P14.08 — the DNP3 IEEE 1815.2-2025 DER-profile adapter (composition
    # side: no DNP3 stack — the wire transport is a designed UNWIRED seam;
    # decision record 24_p14_08_dnp3_der_profile.md). Nothing depends ON it
    # except future composition roots.
    "flexcore-dnp3-adapter",
    # P14.11 — the MQTT 5 + Sparkplug B edge-transport adapter (composition
    # side: no MQTT client, no broker runtime, no TLS stack — the wire
    # transport is a designed UNWIRED seam; decision record
    # 43_p14_11_mqtt_sparkplug_b_uplink.md). Nothing depends ON it except
    # future composition roots.
    "flexcore-mqtt-adapter",
    # P14.12 — the CIM/CGMES network-model import adapter (composition
    # side: no XML/RDF parser dependency — the profile parsing is a
    # designed UNWIRED seam; decision record 44_p14_12_cim_cgmes_import.md).
    # It depends on the CORE protocol-ports layer, flexcore-contracts, and
    # flexcore-grid-api (the network IR the mapping targets); nothing
    # depends ON it except future composition roots.
    "flexcore-cim-adapter",
    # P15.04 — the ENTSO-E Transparency Platform price-signal ingestion
    # adapter (composition side, like crates/adapters/*: zero external
    # dependencies — the XML navigation and the REST transport are
    # designed UNWIRED seams; decision record
    # docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
    # 04_p15_04_price_ingestion_entsoe.md). Nothing depends ON it except
    # future composition roots.
    "flexcore-entsoe-adapter",
    # P17.01-P17.05 — the OCPP CSMS adapter (composition side, like
    # crates/adapters/*: no WebSocket library, no TLS stack, no HTTP
    # framework — the WebSocket/OCPP-J transport is a designed UNRUN seam;
    # decision record
    # docs/phases-11-20/phase-17-ev-fleet-charging/records/
    # 01_p17_01_ocpp_csms_adapter_architecture.md). It depends on the CORE
    # protocol-ports layer and flexcore-domain; nothing depends ON it
    # except future composition roots.
    "flexcore-ocpp-adapter",
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
    "crates/edge-agent",
    "crates/flexcore-kernel",
    "crates/flexcore-research",
    # P14.01/P14.02 — protocol adapter crates (member notes in the root
    # Cargo.toml; decision records under
    # docs/phases-11-20/phase-14-protocol-device-connectivity/records/).
    "crates/flexcore-protocol-ports",
    "crates/adapters/protocol-20305",
    # P14.05/P14.07/P14.08 (wave 1, Agent B) — protocol adapter crates
    # (member notes in the root Cargo.toml; decision records under
    # docs/phases-11-20/phase-14-protocol-device-connectivity/records/).
    "crates/adapters/protocol-openadr",
    "crates/adapters/protocol-modbus",
    "crates/adapters/protocol-dnp3",
    # P14.11/P14.12 (wave 2, Agent A) — protocol adapter crates
    # (member notes in the root Cargo.toml; decision records under
    # docs/phases-11-20/phase-14-protocol-device-connectivity/records/).
    "crates/adapters/protocol-mqtt",
    "crates/adapters/protocol-cim",
    # P15.04 — the ENTSO-E price-signal ingestion adapter crate (member
    # note in the root Cargo.toml; decision record
    # docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
    # 04_p15_04_price_ingestion_entsoe.md).
    "crates/adapters/protocol-entsoe",
    # P17.01-P17.05 — the OCPP CSMS adapter crate (member note in the root
    # Cargo.toml; decision records under
    # docs/phases-11-20/phase-17-ev-fleet-charging/records/).
    "crates/adapters/protocol-ocpp",
    "apps/control-plane",
    "apps/sim-runner",
    "apps/worker",
    "apps/plugin-host",
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
