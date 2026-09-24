# Deployed Contract Addresses

This document records all deployed Trellis contracts across networks, including contract IDs, versions, and deployment dates.

For upgrade procedures and version history, see [UPGRADEABILITY.md](./UPGRADEABILITY.md).

---

## Testnet Deployments

> **Status**: Not yet deployed to testnet.
> 
> To deploy, run:
> ```bash
> ./scripts/deploy.sh testnet
> ```
> This will generate contract addresses and update this file.

| Contract | Address | Version | Deployed | Notes |
|----------|---------|---------|----------|-------|
| — | — | — | — | No active testnet deployments |

---

## Mainnet Deployments

> **Status**: Not yet deployed to mainnet.
> 
> Mainnet deployments require formal security review and governance approval.
> See [UPGRADEABILITY.md](./UPGRADEABILITY.md) for the upgrade process.

| Contract | Address | Version | Deployed | Notes |
|----------|---------|---------|----------|-------|
| — | — | — | — | Awaiting mainnet readiness |

---

## Deployment Information

### What Each Column Means

- **Contract**: Smart contract name (e.g., `aid-contract`, `treasury-contract`)
- **Address**: Deployed Soroban contract ID (format: `CDXXXXXXXX...`)
- **Version**: Current contract version deployed (tracked in upgradeability registry)
- **Deployed**: ISO 8601 timestamp of most recent deployment
- **Notes**: Additional context (e.g., "security patch", "upgrade from v1", "testnet-only")

### Finding Addresses

After deployment, contract addresses can be:
1. Viewed in this file (automatically updated by `scripts/deploy.sh`)
2. Looked up in the on-chain upgradeability registry
3. Queried via `scripts/verify.sh` against a live network

### Upgrading a Deployed Contract

See [UPGRADEABILITY.md](./UPGRADEABILITY.md) for:
- Creating an upgrade proposal via the governance contract
- Executing the upgrade through the registry
- Tracking version history on-chain

---

## Deployment Process

### Prerequisites

1. Build all contracts:
   ```bash
   cargo build --release --target wasm32-unknown-unknown
   ```

2. Ensure you have Soroban CLI installed:
   ```bash
   cargo install soroban-cli
   ```

3. Have testnet/mainnet access configured:
   ```bash
   soroban config set --help
   ```

### Deploying to Testnet

```bash
./scripts/deploy.sh testnet
```

This script:
1. Builds all WASM artifacts
2. Deploys each contract to testnet
3. Outputs contract addresses
4. Updates DEPLOYMENTS.md with new addresses and timestamps

### Deploying to Mainnet

```bash
./scripts/deploy.sh mainnet
```

Mainnet deployments require:
- Formal security audit sign-off
- Community governance approval
- Multi-signature authorization from admins
- Careful coordination with all integrators

### Post-Deployment Verification

After deployment, verify all contracts:

```bash
./scripts/verify.sh testnet
```

This checks:
- All contracts are initialized
- Cross-contract references are wired correctly
- Treasury and registry are functioning

---

## Contract Descriptions

### Core Contracts

| Contract | Purpose | Status |
|----------|---------|--------|
| `aid-contract` | Direct aid settlement and escrow | Core |
| `treasury-contract` | Protocol treasury and reward distribution | Core |
| `referral-contract` | Multi-tier commission system | Core |
| `governance-contract` | Multi-signature governance and role management | Core |
| `oracle-contract` | Off-chain data integration | Core |
| `registry-contract` | Contract discovery and version tracking | Core |

### Supporting Contracts

| Contract | Purpose | Status |
|----------|---------|--------|
| `payments-contract` | Token transfer and settlement | Supporting |
| `rebalancer-contract` | Liquidity management | Supporting |
| `nft-marketplace` | NFT trading and minting | Supporting |
| `access-control` | Role-based access primitives | Utility |
| `upgradeability` | Upgrade registry and coordinator | Utility |

---

## Emergency Contacts

For urgent issues with deployed contracts:

1. Check [GitHub Issues](https://github.com/TRELLIS-STELLAR/Trellis-contracts/issues)
2. Review [UPGRADEABILITY.md](./UPGRADEABILITY.md) for emergency upgrade procedures
3. Contact maintainers via [security@trellis.foundation](mailto:security@trellis.foundation)

---

## Changelog

For changes between versions, see [CHANGELOG.md](./CHANGELOG.md).

For on-chain upgrade history, query the upgradeability registry:
```bash
soroban contract invoke \
  --id <UPGRADEABILITY_CONTRACT_ID> \
  -- get_upgrade_history \
  --contract-id <CONTRACT_ID>
```
