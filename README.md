
# Farmledge-protocol-Near #

The NEAR smart contract behind Farmledge (`data_version` 3): a digital
warehouse receipt for commodities deposited at a certified partner warehouse,
minted from anchored daily Merkle books, used as loan collateral and, on
default, sold through bank escrow.

See the Farmledge System Design Document v2.2 (§4, §7), `events.md` and
[BUILD_EXECUTION.md](./BUILD_EXECUTION.md) for the full specification. This
repo implements a single contract, `receipt`, with commodity as a field rather
than a separate contract per commodity.

## Current state

The execution plan ([BUILD_EXECUTION.md](./BUILD_EXECUTION.md)) and the cross-repo
Merkle leaf test vectors ([test-vectors/leaf_hash.json](./test-vectors/leaf_hash.json),
shared with `Farmledge-app-Near`) are in place; the `contracts/receipt` crate
is ready to be scaffolded and implemented against them.

## Prerequisites

| Tool | Version used here | Install |
|---|---|---|
| Rust | ≥ 1.95.0 (pinned in `rust-toolchain.toml`) | [rustup.rs](https://rustup.rs) |
| `wasm32-unknown-unknown` target | — | `rustup target add wasm32-unknown-unknown` |
| `cargo-near` | latest stable | `cargo install --locked cargo-near` |
| `near-cli-rs` (`near`) | latest stable | `cargo install --locked near-cli-rs` |

## Setup

```bash
git clone <repo-url> farmledge-protocol-near
cd farmledge-protocol-near
rustup target add wasm32-unknown-unknown   # if not already installed
cargo test -p farmledge-receipt --lib      # fast unit + property tests
cargo near build non-reproducible-wasm     # produces target/near/farmledge_receipt/farmledge_receipt.wasm + ABI
cargo test -p farmledge-receipt --test '*' # near-workspaces sandbox integration tests
```

For a reproducible build (NEP-330, requires Docker):

```bash
cargo near build reproducible-wasm
```

## Deploying to Testnet

```bash
# Deploy and initialize in one step
near contract deploy <CONTRACT_ACCOUNT_ID>.testnet \
  use-file target/near/farmledge_receipt/farmledge_receipt.wasm \
  with-init-call new \
  json-args '{"admin": "<ADMIN_ACCOUNT_ID>.testnet"}' \
  prepaid-gas '30.0 Tgas' \
  attached-deposit '0 NEAR' \
  network-config testnet \
  sign-with-keychain send
```

The `admin` argument is passed to `new(admin)`. In production, `admin` must be
a multisig or Sputnik DAO account (design doc §6) — the contract checks
`env::predecessor_account_id() == admin` and requires `1 yoctoNEAR` attached to
every admin call so a function-call access key can never invoke admin methods.

## Repository layout

```
├── BUILD_EXECUTION.md          full NEAR port spec, test matrix & schedule
├── test-vectors/
│   └── leaf_hash.json          canonical Borsh leaf preimage, hashes, proofs & heads
├── scripts/
│   ├── deploy-testnet.sh       create sub-account, deploy WASM, call `new`
│   └── seed-testnet.sh         register warehouse, custodian, verifier & operator
└── contracts/receipt/
    ├── src/
    │   ├── lib.rs              contract state + #[near] entry points
    │   ├── types.rs            Receipt, Lock, Recovery, Sale, Leaf, Status, WeightSource, Pending*
    │   ├── merkle.rs           Borsh leaf preimage, leaf_hash, proof folding, head chaining
    │   ├── anchor.rs           daily Merkle root anchoring (DayRoot + Head chain)
    │   ├── split.rs            in-place receipt splitting (conservation-checked)
    │   ├── lending.rs          lock_offer/lock_accept, unlock, extend_due, recovery & sales
    │   ├── admin.rs            warehouse, custodian, verifier & recovery-operator registries
    │   ├── storage.rs          BorshStorageKey prefixes, ID counters, timestamp helpers
    │   ├── errors.rs           ContractError enum (v3 error codes + NEAR 70–77 codes)
    │   ├── events.rs           NEP-297 `farmledge_receipt` (v3.0.0) structured events
    │   └── test.rs             unit & proptest suites per entry point
    └── tests/                  near-workspaces sandbox integration tests
```

## Contract overview

| Area | Entry points |
|---|---|
| **Init & admin** (`admin`, 1 yoctoNEAR) | `new(admin)`, `propose_admin`, `accept_admin`, `add_warehouse`/`remove_warehouse`, `add_custodian`/`remove_custodian`, `add_verifier`/`remove_verifier`, `add_recovery_operator`/`remove_recovery_operator`, `revoke` |
| **Anchoring & minting** | `anchor(warehouse_id, day, root, leaves)` (custodian) → `mint_propose(leaf, proof, not_after)` (custodian) → `mint_approve(warehouse_id, deposit_no, expected_leaf_hash)` (verifier) |
| **Lending** | `lock_offer(receipt_id, amount_kg, loan_ref, due_date, grace_period_seconds, expires_at)` (lender) → `lock_accept(receipt_id, lender, expected_amount_kg, expected_loan_ref, expected_due_date, expected_grace_period)` (owner), `unlock(receipt_id, kg)` (lender), `extend_due(receipt_id, new_due)` (lender) |
| **Default recovery** | `start_recovery(receipt_id)` (lender) → `approve_sale(receipt_id, buyer, sale_ref, expires_at)` (lender) → `complete_recovery(receipt_id, buyer, sale_ref)` (recovery operator) → `record_settlement(sale_id, settlement_ref)` (recovery operator) |
| **Exit** | `request_exit(receipt_id, kg, expires_at)` (owner) → `exit(receipt_id, kg, exit_weight_kg)` (warehouse custodian) |
| **Views** | `get_receipt`, `last_receipt_id`, `get_anchor`, `get_day_root`, `is_claimed`, `leaf_hash`, `verify_leaf`, `get_lock`, `get_recovery`, `get_sale`, `get_pending_*`, `is_custodian`, `is_verifier`, `is_recovery_operator`, `get_admin`, `data_version` |

## Design notes worth knowing before extending this

- **Two-step intents replace multi-signer Soroban transactions.** NEAR has a
  single signer (`env::predecessor_account_id()`) per transaction. Operations
  that require two parties (minting, locking, exiting, recovery sale) record an
  expiring `Pending*` entry in Step 1 and execute in Step 2 after verifying that
  the second signer restated the exact expected terms or `leaf_hash`.
- **Anchor-first minting (`data_version` 3).** Receipts are never minted from
  arbitrary caller arguments. A custodian first commits the warehouse's daily
  Merkle root via `anchor`; `mint_propose`/`mint_approve` then verify a Merkle
  proof for an end-of-day balance leaf (`kind == 2`) from the warehouse's
  latest anchored day and copy all receipt fields from that proven leaf.
- **Mint deadline (`not_after`, INV-1).** Because NEAR transactions do not
  carry a ledger `max_time`, `mint_propose` requires `now <= not_after <= now + 300`
  and `mint_approve` rejects any approval after `not_after`.
- **Borsh leaf hashing.** On NEAR, a leaf hash is
  `sha256(0x00 || borsh(LeafHashPreimage))` prefixed with the 20-byte domain
  tag `FARMLEDGE_LEAF_V3\0\0\0` (`WeightSource`: `BagEstimate = 0`,
  `Scale = 1`). All implementations must match
  [test-vectors/leaf_hash.json](./test-vectors/leaf_hash.json).
- **In-place splits.** A partial `lock_accept`, `unlock`, or `exit` reduces the
  parent receipt in place (`remainder_id == parent_id`) and mints a new child
  receipt (`taken_id`, with `parent_id` set) for the acted-on portion, emitting
  `split` before the operation's own event.
- **Storage staking instead of TTL.** NEAR uses storage staking rather than
  Soroban's ledger TTL archival, so `extend_ttl` / `extend_ttl_batch` are not
  needed and `DayRoot` entries are retained permanently on-chain.
- **Structured errors and NEP-297 events.** Errors panic with `E{code}:{Name}`
  (keeping the v3 error codes `10–57` and adding `70–77` for NEAR two-step
  intents) so `Farmledge-app-Near` can parse codes deterministically. State
  changes emit NEP-297 `EVENT_JSON:` logs under `standard = "farmledge_receipt"`,
  `version = "3.0.0"`.
- **Timestamps and fixed-point numbers.** `env::block_timestamp()` returns
  nanoseconds on NEAR; the contract converts all timestamps to Unix seconds
  (`block_timestamp() / 1_000_000_000`). Moisture is stored in basis points of
  a percent (`moisture_pct: u32`, e.g. `1250` = 12.50%), and `u64`/`i128`
  quantities cross the JSON boundary as decimal strings.

## Licence

Apache 2.0 — see `LICENSE`.
