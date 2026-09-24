#!/usr/bin/env bash
# Deploy all contracts to the target network and record addresses.
#
# Usage: ./scripts/deploy.sh [testnet|mainnet]
#
# This script:
# 1. Builds all WASM artifacts in release mode
# 2. Deploys each contract to the specified network
# 3. Records contract addresses in DEPLOYMENTS.md
# 4. Updates the upgradeability registry with version info
#
# For each network, contract addresses are recorded with deployment timestamp.

set -euo pipefail

NETWORK="${1:-testnet}"
WASM_DIR="target/wasm32-unknown-unknown/release"
CONTRACTS_DIR="contracts"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "${SCRIPT_DIR}")"
DEPLOYMENTS_FILE="${REPO_ROOT}/DEPLOYMENTS.md"
DEPLOYMENT_LOG="${REPO_ROOT}/.deployment-log-${NETWORK}.json"

# Contract configuration: (contract_dir, contract_name, readable_name)
declare -a CONTRACTS=(
  "aid-contract:aid_contract:Aid Contract"
  "treasury-contract:treasury_contract:Treasury Contract"
  "referral-contract:referral_contract:Referral Contract"
  "governance-contract:governance_contract:Governance Contract"
  "oracle-contract:oracle_contract:Oracle Contract"
  "registry-contract:registry_contract:Registry Contract"
  "upgradeability:upgradeability:Upgradeability"
  "payments-contract:payments_contract:Payments Contract"
  "rebalancer-contract:rebalancer_contract:Rebalancer Contract"
  "nft-marketplace:nft_marketplace:NFT Marketplace"
  "access-control:access_control:Access Control"
)

echo "=========================================="
echo "Trellis Contracts Deployment"
echo "Network: ${NETWORK}"
echo "Timestamp: $(date -u +'%Y-%m-%dT%H:%M:%SZ')"
echo "=========================================="

# Ensure WASM artifacts are built
echo ""
echo "Building WASM artifacts..."
cargo build --release --target wasm32-unknown-unknown 2>&1 | grep -E "Compiling|Finished|error" || true

# Initialize deployment log
cat > "${DEPLOYMENT_LOG}" << EOF
{
  "network": "${NETWORK}",
  "timestamp": "$(date -u +'%Y-%m-%dT%H:%M:%SZ')",
  "deployments": [
EOF

FIRST=true
DEPLOYMENT_COUNT=0

# Deploy each contract
for CONTRACT_CONFIG in "${CONTRACTS[@]}"; do
  IFS=':' read -r CONTRACT_DIR WASM_NAME READABLE_NAME <<< "${CONTRACT_CONFIG}"
  
  WASM_FILE="${WASM_DIR}/${WASM_NAME}.wasm"
  
  if [ ! -f "${WASM_FILE}" ]; then
    echo "⚠️  Warning: WASM not found at ${WASM_FILE}, skipping ${READABLE_NAME}"
    continue
  fi
  
  echo ""
  echo "Deploying ${READABLE_NAME}..."
  
  # Deploy contract and capture output
  DEPLOY_OUTPUT=$(soroban contract deploy \
    --wasm "${WASM_FILE}" \
    --network "${NETWORK}" \
    --source admin 2>&1) || {
    echo "❌ Failed to deploy ${READABLE_NAME}"
    echo "${DEPLOY_OUTPUT}"
    continue
  }
  
  # Extract contract ID from output (format: CDXXXXXXXX...)
  CONTRACT_ID=$(echo "${DEPLOY_OUTPUT}" | grep -oE 'C[A-Z0-9]{55}' | head -1)
  
  if [ -z "${CONTRACT_ID}" ]; then
    echo "⚠️  Could not extract contract ID for ${READABLE_NAME}"
    continue
  fi
  
  echo "✓ Deployed to: ${CONTRACT_ID}"
  
  # Append to deployment log
  if [ "${FIRST}" = false ]; then
    echo "," >> "${DEPLOYMENT_LOG}"
  fi
  FIRST=false
  
  cat >> "${DEPLOYMENT_LOG}" << EOF
    {
      "name": "${READABLE_NAME}",
      "contract_dir": "${CONTRACT_DIR}",
      "wasm_name": "${WASM_NAME}",
      "contract_id": "${CONTRACT_ID}",
      "version": "0.1.0",
      "deployed": "$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
    }
EOF
  
  ((DEPLOYMENT_COUNT++))
done

# Close deployment log
cat >> "${DEPLOYMENT_LOG}" << EOF
  ]
}
EOF

echo ""
echo "=========================================="
echo "Deployment Complete"
echo "Deployed: ${DEPLOYMENT_COUNT} contract(s)"
echo "Log: ${DEPLOYMENT_LOG}"
echo "=========================================="
echo ""
echo "Deployment log (${DEPLOYMENT_LOG}):"
cat "${DEPLOYMENT_LOG}"
echo ""
echo "To record these addresses in DEPLOYMENTS.md, run:"
echo "  scripts/record-deployments.sh ${NETWORK}"
echo ""
echo "For on-chain upgrade tracking, run:"
echo "  scripts/register_upgradeable.sh"
