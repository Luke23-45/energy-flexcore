# P7.24 — the deliberately bad plugin: annotated pseudo-source
#
# This file is NOT runnable code and is not presented as such. It is the
# RUNTIME-ATTEMPT half of the bad example: the manifest half
# (bad-manifest.json) is a real document the real validator rejects; the
# attempts below are the things a dispatch-ambitious plugin author tries
# AFTER a manifest exists, annotated with the structural reason each one
# is UNBUILDABLE, not discouraged. Every error name cited is verified
# against the source file named in the comment (read 2026-11-13).
#
# The is-shaped-like-a-worker pseudo-syntax below stands in for the real
# plugin body (a Class C worker would implement WorkerLogic.evaluate on
# the python/workers template; a Class B component would export a WIT
# interface). The point is the annotation, not the syntax.

from __future__ import annotations

# The example declares exactly the manifest's configuration schema so the
# configuration resolution walkthrough in README.md §2(f) is concrete.
CONFIG_SCHEMA_PROPERTIES = ("api-key",)


class AmbitiousPlugin:
    """Every method below is an attempt that dies at a named, structural
    place. Nothing here can be made to work by cleverness."""

    # ------------------------------------------------------------------
    # Attempt (a): claim a protected kernel contract.
    #
    # The manifest already declares provider-of:
    #   ["kernel.dispatch-validation", "forecast.day-ahead"]
    # "kernel.dispatch-validation" is a grammatically valid contract id, so
    # the MANIFEST gate passes it — but the RESOLVER refuses the composition
    # before any plan exists:
    #
    #   PROTECTED_KERNEL_REPLACE
    #   (crates/flexcore-kernel/src/resolve.rs::PROTECTED_KERNEL_CONTRACT_IDS)
    #
    # The profile path wraps the same refusal as
    # PROFILE_PROTECTED_CONTRACT_REFUSED; the provider registry refuses a
    # direct registration as PROTECTED_KERNEL_CONTRACT_REFUSED
    # (crates/flexcore-kernel/src/registry.rs); the P7.36 plugin-path
    # verifier refuses the same claim on a request envelope as
    # PROTECTED_CONTRACT_CLAIM (plugin_path_protection.rs). Four layers,
    # one rule: the six kernel.* contract ids are not replaceable.
    # ------------------------------------------------------------------

    # ------------------------------------------------------------------
    # Attempt (b): smuggle a secret VALUE into configuration.
    #
    # The operator's config document supplied for this plugin:
    #   { "api-key": "sk-live-<the actual credential>" }
    # The config resolver (Compatible -> Configured gate) refuses BEFORE
    # any further processing — the property name normalises to the closed
    # secret-field vocabulary token "api-key" and the value is a plain
    # string, not a scheme-prefixed reference:
    #
    #   SECRET_VALUE_IN_CONFIG
    #   (crates/flexcore-kernel/src/plugin_config.rs::SECRET_FIELD_VOCABULARY)
    #
    # The honest alternative (the only one): declare the secret in the
    # manifest's secrets object with a scheme reference — env:, file:, or
    # secret-store: — and receive a HANDLE (scheme + location, no value).
    # Reading is a host capability; the plugin never sees the value.
    # ------------------------------------------------------------------

    # ------------------------------------------------------------------
    # Attempt (c): over-broad permissions.
    #
    # The manifest requests "forecast.publish", which is a legal §5.7 id —
    # but the deployment profile (sim-lab) carries the ceiling
    #   ["telemetry.read", "asset.read", "schedule.propose"]
    # Profile plan validation refuses:
    #   PROFILE_PERMISSION_CEILING_EXCEEDED
    #   (crates/flexcore-kernel/src/profile.rs)
    # and, if a composition ever reached the host door anyway:
    #   PERMISSION_ABOVE_PROFILE — refused, never trimmed to fit
    #   (apps/plugin-host/src/enforcement.rs)
    # ------------------------------------------------------------------

    # ------------------------------------------------------------------
    # Attempt (d): dispatch-authority ambition (ADR-010).
    #
    # The manifest requests "dispatch.authorize". The manifest validator
    # refuses — the earliest point the §5.7 rule can bite:
    #   PERMISSION_NEVER_GRANTED
    #   (crates/flexcore-kernel/src/manifest.rs::NEVER_GRANTED_PERMISSIONS)
    # Should the id arrive through any later path anyway, the broker and
    # the enforcement gate re-refuse:
    #   PERMISSION_NEVER_GRANTABLE
    #   (apps/plugin-host/src/broker.rs, apps/plugin-host/src/enforcement.rs)
    # And the type level closes it: there is no authorization id in ANY
    # vocabulary, and the broker's validation forward returns () — no
    # verdict can flow back through a signature that has no verdict in it.
    # This plugin's output is a CANDIDATE by construction, or it does not
    # run. There is no third option.
    # ------------------------------------------------------------------

    # ------------------------------------------------------------------
    # Attempt: ambient access (the tries-everything classic).
    #
    #   open("/etc/passwd")            # no filesystem import exists: the
    #                                  # WIT worlds carry ZERO wasi: imports
    #                                  # (wit/README.md section 2)
    #   socket.connect(...)            # "network.any" is never grantable;
    #                                  # no network interface is in any world
    #   os.environ["API_KEY"]          # no environment interface exists
    #
    # There is no named refusal for these because there is nothing to
    # refuse: the import does not exist. Where a capability id is checked,
    # the broker answers CAPABILITY_NOT_GRANTED (defaults deny) or
    # SCOPE_OUTSIDE_GRANT (apps/plugin-host/src/broker.rs).
    # ------------------------------------------------------------------

    # The legitimate shape — for contrast, what a CONFORMANT plugin's body
    # is: compute a proposal and return it WITH provenance
    # (report section 9.2; the P7.21 conformance suite rejects a result
    # without provenance). This bad example deliberately does not provide
    # even that: it never reaches a run loop, because every gate above
    # stops it first.
