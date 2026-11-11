# Farmledge Protocol NEAR — Build Execution Plan

Execution specification for the NEAR port of the Farmledge Digital Warehouse Receipt contract (`farmledge-protocol-near`). Companion to `Farmledge-app-Near`.

---

## 1. Architectural Differences: Soroban vs. NEAR

| Dimension | Soroban (`farmledge-protocol`) | NEAR (`farmledge-protocol-near`) |
|---|---|---|
| **Signatures** | Multi-auth (`require_auth` on multiple explicit addresses per transaction) | Single predecessor (`env::predecessor_account_id()`). Two-step intents for multi-party operations |
| **State Storage** | Ledger rent & TTL archival (`extend_ttl`, `extend_ttl_batch`) | Permanent storage staking via contract balance; no TTL keep-alive required |
| **Hashing & Serialization** | Soroban XDR + SHA-256 | Borsh + SHA-256 (`FARMLEDGE_LEAF_V3\0\0\0` 20-byte domain tag) |
| **Events** | Soroban contract events (`#[contractevent]`) | NEP-297 `EVENT_JSON:` logs (`standard = "farmledge_receipt"`, `version = "3.0.0"`) |
| **Time & Numbers** | Ledger seconds, i128 | Nanoseconds (`env::block_timestamp() / 1_000_000_000` for seconds), string decimal u64/i128 over JSON RPC |
| **Error Handling** | Contract error codes (`#11`, `#12`, etc.) | Panics with `E{code}:{Name}` (retaining codes 10–58, adding 70–77) |

---

## 2. Contract Surface & Two-Step Intent Flow

### 2.1 Two-Step Intent Invariants
1. **Minting:**
   - Step 1: Custodian calls `mint_propose(leaf, proof, not_after)`. Validates leaf inclusion against latest anchored root for warehouse. Checks `now <= not_after <= now + 300`. Emits `mint_proposed`.
   - Step 2: Verifier calls `mint_approve(warehouse_id, deposit_no, expected_leaf_hash)`. Verifies deadline `now <= not_after` and hash match. Emits `minted` and returns `receipt_id`.
2. **Lending:**
   - Step 1: Lender calls `lock_offer(receipt_id, amount_kg, loan_ref, due_date, grace_period_seconds, expires_at)`. Emits `lock_offered`.
   - Step 2: Farmer/Owner calls `lock_accept(receipt_id, lender, expected_amount_kg, expected_loan_ref, expected_due_date, expected_grace_period)`. If partial, splits receipt in-place, then locks child. Emits `locked`.
3. **Exit:**
   - Step 1: Owner calls `request_exit(receipt_id, kg, expires_at)`.
   - Step 2: Custodian calls `exit(receipt_id, kg, exit_weight_kg)`. Emits `exited`.
4. **Recovery & Sale:**
   - Step 1: Lender calls `approve_sale(receipt_id, buyer, sale_ref, expires_at)`.
   - Step 2: Recovery Operator calls `complete_recovery(receipt_id, buyer, sale_ref)`. Emits `recovery_completed`.

---

## 3. Test Matrix & Verification

- **Merkle Leaf Vectors:** Verified byte-for-byte against `test-vectors/leaf_hash.json`.
- **Unit Tests:** State transitions, invalid states, unauthorized callers, expired deadlines, split conservation.
- **NEP-297 Format:** Exact JSON schema matching `farmledge-app-near` event parser.
