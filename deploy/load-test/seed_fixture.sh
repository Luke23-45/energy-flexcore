#!/bin/sh
# P20.02 — Synthetic fixture seeder (generalizes deploy/compose/smoke.sh).
#
# WRITTEN-UNRUN: this script has NEVER been executed. It creates the
# synthetic fixture chain the load legs reference — tenant -> portfolio ->
# site -> asset — through the real REST API with the dev-auth token
# (requires the dev-auth build per deploy/compose/smoke.sh), then prints
# the env contract the k6/Locust legs consume.
#
# Data policy: deploy/load-test/data/synthetic-data-policy.md (SYN- prefix
# convention; synthetic only). NO THRESHOLD LOGIC: the script asserts only
# creation success codes (a failed seed is a failed seed, not a load
# measurement).
set -euo pipefail

API_BASE="${FLEXCORE_LOAD_BASE_URL:-http://localhost:8080}"
TENANTS="${FLEXCORE_SEED_TENANTS:-8}"
RUN_TAG="${FLEXCORE_SEED_RUN_TAG:-SYN}"

log() { printf '[seed] %s\n' "$1"; }

log "waiting for /ready"
i=0
until curl -fsS "$API_BASE/ready" >/dev/null 2>&1; do
  i=$((i + 1))
  if [ "$i" -gt 60 ]; then
    log "control plane never became ready"
    exit 1
  fi
  sleep 2
done

post() { # path body -> body with HTTP 201 asserted
  code_file=$(mktemp)
  resp=$(curl -sS -w '\n%{http_code}' -o "$code_file" -X POST \
    -H 'Content-Type: application/json' -d "$2" "$API_BASE$1")
  code=$(tail -n 1 "$resp" 2>/dev/null || true)
  body=$(head -n -1 "$resp" 2>/dev/null || cat "$code_file")
  rm -f "$code_file"
  # shellcheck disable=SC2181
  if [ "${code:-}" != "201" ]; then
    log "POST $1 returned ${code:-<none>}: $body"
    exit 1
  fi
  printf '%s' "$body"
}

json_field() { # json field -> extracted string value (portable sed extraction
              # of "field":"value"; the DTO id fields are flat strings)
  printf '%s' "$1" | sed -n 's/.*"'"$2"'":"\([^"]*\)".*/\1/p'
}

uuid_ts() { # a deterministic-ish synthetic tenant uuid per index (the
            # dev-auth resolver accepts any well-formed tenant uuid — the
            # smoke.sh recorded pattern)
  printf '%08x-0000-7e2f-9a1b-%012x' "$1" "$1"
}

n=1
first_tenant=""
first_portfolio=""
first_site=""
first_asset=""

while [ "$n" -le "$TENANTS" ]; do
  tuuid=$(uuid_ts "$n")
  TOKEN="Bearer dev tenant=${tuuid} subject=syn-seeder-${n} role=platform-admin"

  log "tenant $n/$TENANTS ($tuuid)"
  tenant_json=$(post /api/v1/tenants "{\"name\":\"${RUN_TAG} Load Tenant ${n}\",\"identity_provider_reference\":\"${RUN_TAG}:idp:${n}\",\"policy_profile\":\"default\"}")

  log "portfolio $n"
  portfolio_json=$(post /api/v1/portfolios "{\"name\":\"${RUN_TAG} Load Portfolio ${n}\",\"operator\":\"synthetic\",\"market_region\":\"simulation-only\",\"aggregation_rules\":\"default\"}")

  portfolio_id=$(json_field "$portfolio_json" "id")

  log "site $n"
  site_json=$(post /api/v1/sites "{\"portfolio_id\":\"${portfolio_id}\",\"name\":\"${RUN_TAG} Load Site ${n}\",\"location\":\"synthetic\",\"timezone\":\"UTC\",\"point_of_common_coupling\":\"SYN-PCC-${n}\",\"grid_model_reference\":\"SYN-grid-${n}\",\"edge_identity\":\"SYN-edge-${n}\"}")

  site_id=$(json_field "$site_json" "id")

  log "asset $n (BESS — the §2.2 row the flexibility surfaces target)"
  asset_json=$(post /api/v1/assets "{\"site_id\":\"${site_id}\",\"name\":\"${RUN_TAG} Load Asset ${n}\",\"asset_type\":\"BESS\",\"vendor_model\":\"SYN-bess-1\",\"rated_power_mw\":1.0,\"rated_energy_mwh\":2.0,\"phase\":\"three\",\"bus_reference\":\"SYN-bus-${n}\",\"capability_profile\":\"default\"}")

  asset_id=$(json_field "$asset_json" "id")

  if [ "$n" = "1" ]; then
    first_tenant="$tuuid"
    first_portfolio="$portfolio_id"
    first_site="$site_id"
    first_asset="$asset_id"
  fi

  n=$((n + 1))
done

log "fixture seeded; export the contract below before running the legs"
cat <<EOF

export FLEXCORE_LOAD_BASE_URL="$API_BASE"
export FLEXCORE_LOAD_TENANT_UUID="$first_tenant"
export FLEXCORE_LOAD_PORTFOLIO_ID="$first_portfolio"
export FLEXCORE_LOAD_SITE_ID="$first_site"
export FLEXCORE_LOAD_ASSET_ID="$first_asset"
# The dev-auth token for the first fixture tenant (the legs' identity):
export FLEXCORE_LOAD_TOKEN="Bearer dev tenant=${first_tenant} subject=syn-loader role=platform-admin"

EOF
log "SEED OK"
