#!/usr/bin/env bash
# Bootstrap / Seed roles (warehouse, custodian, verifier, operator) on NEAR Testnet.
#
# Usage:
#   CONTRACT_ACCOUNT_ID=my-receipt.testnet ADMIN_ACCOUNT_ID=my-admin.testnet ./scripts/seed-testnet.sh

set -euo pipefail

CONTRACT_ACCOUNT_ID=${CONTRACT_ACCOUNT_ID:-}
ADMIN_ACCOUNT_ID=${ADMIN_ACCOUNT_ID:-}
NETWORK=${NEAR_NETWORK:-testnet}

WAREHOUSE_ID=${WAREHOUSE_ID:-"KN_DAWANAU"}
CUSTODIAN_ACCOUNT_ID=${CUSTODIAN_ACCOUNT_ID:-}
VERIFIER_ACCOUNT_ID=${VERIFIER_ACCOUNT_ID:-}
OPERATOR_ACCOUNT_ID=${OPERATOR_ACCOUNT_ID:-}

if [ -z "$CONTRACT_ACCOUNT_ID" ] || [ -z "$ADMIN_ACCOUNT_ID" ]; then
  echo "Error: CONTRACT_ACCOUNT_ID and ADMIN_ACCOUNT_ID are required." >&2
  exit 1
fi

echo "==> Registering warehouse: $WAREHOUSE_ID"
near contract call-type "$CONTRACT_ACCOUNT_ID" add_warehouse \
  json-args "{\"warehouse_id\": \"$WAREHOUSE_ID\"}" \
  prepaid-gas '30.0 Tgas' \
  attached-deposit '1 yoctoNEAR' \
  network-config "$NETWORK" \
  sign-with-keychain send

if [ -n "$CUSTODIAN_ACCOUNT_ID" ]; then
  echo "==> Registering custodian: $CUSTODIAN_ACCOUNT_ID for $WAREHOUSE_ID"
  near contract call-type "$CONTRACT_ACCOUNT_ID" add_custodian \
    json-args "{\"warehouse_id\": \"$WAREHOUSE_ID\", \"custodian\": \"$CUSTODIAN_ACCOUNT_ID\"}" \
    prepaid-gas '30.0 Tgas' \
    attached-deposit '1 yoctoNEAR' \
    network-config "$NETWORK" \
    sign-with-keychain send
fi

if [ -n "$VERIFIER_ACCOUNT_ID" ]; then
  echo "==> Registering verifier: $VERIFIER_ACCOUNT_ID"
  near contract call-type "$CONTRACT_ACCOUNT_ID" add_verifier \
    json-args "{\"verifier\": \"$VERIFIER_ACCOUNT_ID\"}" \
    prepaid-gas '30.0 Tgas' \
    attached-deposit '1 yoctoNEAR' \
    network-config "$NETWORK" \
    sign-with-keychain send
fi

if [ -n "$OPERATOR_ACCOUNT_ID" ]; then
  echo "==> Registering recovery operator: $OPERATOR_ACCOUNT_ID"
  near contract call-type "$CONTRACT_ACCOUNT_ID" add_recovery_operator \
    json-args "{\"operator\": \"$OPERATOR_ACCOUNT_ID\"}" \
    prepaid-gas '30.0 Tgas' \
    attached-deposit '1 yoctoNEAR' \
    network-config "$NETWORK" \
    sign-with-keychain send
fi

echo "==> Roles seeded successfully!"
