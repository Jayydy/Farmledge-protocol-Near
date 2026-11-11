

#!/usr/bin/env bash
# Deploy and initialize farmledge-receipt smart contract on NEAR Testnet.
#
# Usage:
#   CONTRACT_ACCOUNT_ID=my-receipt.testnet ADMIN_ACCOUNT_ID=my-admin.testnet ./scripts/deploy-testnet.sh

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
CONTRACT_ACCOUNT_ID=${CONTRACT_ACCOUNT_ID:-}
ADMIN_ACCOUNT_ID=${ADMIN_ACCOUNT_ID:-$CONTRACT_ACCOUNT_ID}
NETWORK=${NEAR_NETWORK:-testnet}

if [ -z "$CONTRACT_ACCOUNT_ID" ]; then
  echo "Error: CONTRACT_ACCOUNT_ID is required" >&2
  echo "Usage: CONTRACT_ACCOUNT_ID=<account>.testnet [ADMIN_ACCOUNT_ID=<admin>.testnet] ./scripts/deploy-testnet.sh" >&2
  exit 1
fi

echo "==> Building farmledge_receipt WASM..."
cd "$ROOT"
cargo build --target wasm32-unknown-unknown --release -p farmledge-receipt

WASM_PATH="$ROOT/target/wasm32-unknown-unknown/release/farmledge_receipt.wasm"
if [ ! -f "$WASM_PATH" ]; then
  echo "Error: WASM not found at $WASM_PATH" >&2
  exit 1
fi

echo "==> Deploying to $CONTRACT_ACCOUNT_ID on $NETWORK..."
near contract deploy "$CONTRACT_ACCOUNT_ID" \
  use-file "$WASM_PATH" \
  with-init-call new \
  json-args "{\"admin\": \"$ADMIN_ACCOUNT_ID\"}" \
  prepaid-gas '30.0 Tgas' \
  attached-deposit '0 NEAR' \
  network-config "$NETWORK" \
  sign-with-keychain send

echo "==> Contract deployed and initialized successfully!"
echo "Contract ID: $CONTRACT_ACCOUNT_ID"
echo "Admin ID:    $ADMIN_ACCOUNT_ID"
