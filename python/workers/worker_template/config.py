# P7.17 — Worker configuration from the environment: fail-closed defaults.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. This module is STDLIB-ONLY and genuinely importable/runnable today —
#    no grpcio anywhere in it. The pure parsing functions take a mapping
#    (an env-like dict), so tests exercise them without any process env
#    mutation.
# 2. Fail-closed rule: an INVALID value is a fatal startup error
#    (`ConfigError`), never a silent fallback to a default. A MISSING
#    value falls back to the recorded DEFAULT for that key — defaults are
#    the conservative ones (loopback bind, bounded concurrency,
#    deterministic mode ON).
# 3. No secret ever lands here: these keys describe transport and limits
#    only. A worker needing credentials receives them out-of-band from
#    the deployment, resolved by reference (P7.19 discipline) — never as
#    an env var this module would log.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

from dataclasses import dataclass

# The environment keys the template reads (prefix: FLEXCORE_WORKER_).
ENV_WORKER_ID = "FLEXCORE_WORKER_ID"
ENV_BIND_ADDR = "FLEXCORE_WORKER_BIND_ADDR"
ENV_MAX_INFLIGHT = "FLEXCORE_WORKER_MAX_INFLIGHT"
ENV_DETERMINISTIC = "FLEXCORE_WORKER_DETERMINISTIC"
ENV_DRAIN_GRACE_MS = "FLEXCORE_WORKER_DRAIN_GRACE_MS"
ENV_LOG_LEVEL = "FLEXCORE_WORKER_LOG_LEVEL"

# Recorded DEFAULTs (each DEFAULT is a decision recorded in
# `ledger/phase-07-plugin-sdk/17_18_workers_supervision.md`):
DEFAULT_BIND_ADDR = "127.0.0.1:50051"  # DEFAULT: loopback only, opt-in exposure.
DEFAULT_MAX_INFLIGHT = 4  # DEFAULT: bounded in-flight cap (backpressure, P7.18).
DEFAULT_DETERMINISTIC = True  # DEFAULT: deterministic mode ON (P7.42 posture).
DEFAULT_DRAIN_GRACE_MS = 30_000  # DEFAULT: 30 s drain grace (mirrors the kernel's DEFAULT_DRAIN_DEADLINE_MS).
DEFAULT_LOG_LEVEL = "info"  # DEFAULT: info; never debug in a deployed worker.

_LOG_LEVELS: frozenset[str] = frozenset({"debug", "info", "warning", "error"})

# The contract version this template serves (single source with
# errors.py, which re-exports it; pinned here).
CONTRACT_VERSION = "flexcore.worker.v1"


class ConfigError(Exception):
    """A fatal configuration error — the worker refuses to start.

    Fail-closed: an invalid value never degrades into a default. The
    message names the offending env key and is safe for logs (no value
    is echoed for keys that could carry secrets — none are defined, and
    none may be added without revisiting this rule).
    """


@dataclass(frozen=True)
class WorkerConfig:
    """The template's effective configuration.

    Constructed via `from_env` (or `from_mapping` in tests); immutable
    once built. `deterministic` is the P7.42 flag: when True the worker
    must not place wall-clock values in result payloads and must honour
    the request seed.
    """

    worker_id: str
    bind_addr: str
    max_inflight: int
    deterministic: bool
    drain_grace_ms: int
    log_level: str
    contract_version: str = CONTRACT_VERSION


def _parse_bool(value: str, key: str) -> bool:
    lowered = value.strip().lower()
    if lowered in {"1", "true", "yes", "on"}:
        return True
    if lowered in {"0", "false", "no", "off"}:
        return False
    raise ConfigError(f"{key}: expected a boolean, got {lowered!r}")


def _parse_positive_int(value: str, key: str) -> int:
    try:
        parsed = int(value.strip(), 10)
    except ValueError as exc:
        raise ConfigError(f"{key}: expected a positive integer, got {value!r}") from exc
    if parsed <= 0:
        raise ConfigError(f"{key}: expected a positive integer, got {value!r}")
    return parsed


def _parse_bind_addr(value: str, key: str) -> str:
    text = value.strip()
    host, sep, port_text = text.rpartition(":")
    if not sep or not host:
        raise ConfigError(f"{key}: expected 'host:port', got {value!r}")
    try:
        port = int(port_text, 10)
    except ValueError as exc:
        raise ConfigError(f"{key}: expected 'host:port', got {value!r}") from exc
    if not 1 <= port <= 65535:
        raise ConfigError(f"{key}: port out of range 1..65535 in {value!r}")
    if host not in {"127.0.0.1", "localhost", "0.0.0.0", "::", "[::1]"}:
        # Fail-closed on bind host: the template serves the supervisor
        # over loopback or an explicitly opted-in wildcard. Any other
        # host string must be added here deliberately, with a recorded
        # reason — never passed through silently.
        raise ConfigError(
            f"{key}: host {host!r} is not an allowed bind host "
            f"(loopback or explicit wildcard only)"
        )
    return text


def from_mapping(env: dict[str, str]) -> WorkerConfig:
    """Build the config from an env-like mapping (pure, testable).

    Raises `ConfigError` on any invalid value (fail-closed). Missing
    keys take the recorded DEFAULTs.
    """
    worker_id = env.get(ENV_WORKER_ID, "").strip()
    if not worker_id:
        # No default: an unidentified worker cannot be supervised or
        # audited coherently (P7.18 keys everything on the worker id).
        raise ConfigError(
            f"{ENV_WORKER_ID}: required (no default) — name the worker"
        )
    if len(worker_id) > 128:
        raise ConfigError(f"{ENV_WORKER_ID}: longer than 128 characters")

    bind_addr_raw = env.get(ENV_BIND_ADDR, DEFAULT_BIND_ADDR)
    bind_addr = _parse_bind_addr(bind_addr_raw, ENV_BIND_ADDR)

    max_inflight_raw = env.get(ENV_MAX_INFLIGHT, str(DEFAULT_MAX_INFLIGHT))
    max_inflight = _parse_positive_int(max_inflight_raw, ENV_MAX_INFLIGHT)

    deterministic_raw = env.get(ENV_DETERMINISTIC, str(DEFAULT_DETERMINISTIC))
    deterministic = _parse_bool(deterministic_raw, ENV_DETERMINISTIC)

    drain_grace_raw = env.get(ENV_DRAIN_GRACE_MS, str(DEFAULT_DRAIN_GRACE_MS))
    drain_grace_ms = _parse_positive_int(drain_grace_raw, ENV_DRAIN_GRACE_MS)

    log_level = env.get(ENV_LOG_LEVEL, DEFAULT_LOG_LEVEL).strip().lower()
    if log_level not in _LOG_LEVELS:
        raise ConfigError(
            f"{ENV_LOG_LEVEL}: expected one of {sorted(_LOG_LEVELS)}, "
            f"got {log_level!r}"
        )

    return WorkerConfig(
        worker_id=worker_id,
        bind_addr=bind_addr,
        max_inflight=max_inflight,
        deterministic=deterministic,
        drain_grace_ms=drain_grace_ms,
        log_level=log_level,
    )


def from_env() -> WorkerConfig:
    """Build the config from the real process environment.

    The ONLY non-stdlib dependency is `os` — the process env is read
    here, at the composition edge, so the parsing itself stays a pure
    function (`from_mapping`) that tests exercise directly.
    """
    import os  # noqa: PLC0415 — deliberate: the env read is the composition edge.

    return from_mapping(dict(os.environ))
