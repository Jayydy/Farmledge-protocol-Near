use near_sdk::store::{LookupMap, LookupSet};
use near_sdk::{env, near, AccountId, PanicOnDefault};

pub mod admin;
pub mod anchor;
pub mod errors;
pub mod events;
pub mod lending;
pub mod merkle;
pub mod split;
pub mod storage;
pub mod types;

#[cfg(test)]
mod test;

pub use errors::ContractError;
pub use types::*;

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct ReceiptContract {
    pub admin: AccountId,
    pub proposed_admin: Option<AccountId>,
    pub last_receipt_id: u64,
    pub last_sale_id: u64,
    pub receipts: LookupMap<u64, Receipt>,
    pub sales: LookupMap<u64, Sale>,
    pub anchors: LookupMap<String, AnchorHead>,
    pub day_roots: LookupMap<(String, u32), DayRoot>,
    pub claimed: LookupMap<(String, u64), ()>,
    pub pending_mints: LookupMap<(String, u64), PendingMint>,
    pub pending_locks: LookupMap<(u64, AccountId), PendingLockOffer>,
    pub pending_exits: LookupMap<u64, PendingExit>,
    pub pending_sales: LookupMap<u64, PendingSale>,
    pub warehouses: LookupSet<String>,
    pub custodians: LookupMap<(String, AccountId), ()>,
    pub verifiers: LookupSet<AccountId>,
    pub recovery_operators: LookupSet<AccountId>,
}

#[near]
impl ReceiptContract {
    #[init]
    pub fn new(admin: AccountId) -> Self {
        Self {
            admin,
            proposed_admin: None,
            last_receipt_id: 0,
            last_sale_id: 0,
            receipts: LookupMap::new(storage::StorageKey::Receipts),
            sales: LookupMap::new(storage::StorageKey::Sales),
            anchors: LookupMap::new(storage::StorageKey::Anchors),
            day_roots: LookupMap::new(storage::StorageKey::DayRoots),
            claimed: LookupMap::new(storage::StorageKey::Claimed),
            pending_mints: LookupMap::new(storage::StorageKey::PendingMints),
            pending_locks: LookupMap::new(storage::StorageKey::PendingLocks),
            pending_exits: LookupMap::new(storage::StorageKey::PendingExits),
            pending_sales: LookupMap::new(storage::StorageKey::PendingSales),
            warehouses: LookupSet::new(storage::StorageKey::Warehouses),
            custodians: LookupMap::new(storage::StorageKey::Custodians),
            verifiers: LookupSet::new(storage::StorageKey::Verifiers),
            recovery_operators: LookupSet::new(storage::StorageKey::RecoveryOperators),
        }
    }

    // --- Admin & Registries (requires 1 yoctoNEAR) ---

    #[payable]
    pub fn propose_admin(&mut self, new_admin: AccountId) {
        self.assert_admin_with_deposit();
        self.proposed_admin = Some(new_admin);
    }

    #[payable]
    pub fn accept_admin(&mut self) {
        let caller = env::predecessor_account_id();
        if Some(&caller) != self.proposed_admin.as_ref() {
            ContractError::Unauthorized.panic();
        }
        if env::attached_deposit().as_yoctonear() < 1 {
            ContractError::Unauthorized.panic();
        }
        self.admin = caller;
        self.proposed_admin = None;
    }

    #[payable]
    pub fn add_warehouse(&mut self, warehouse_id: String) {
        self.internal_add_warehouse(warehouse_id);
    }

    #[payable]
    pub fn remove_warehouse(&mut self, warehouse_id: String) {
        self.internal_remove_warehouse(warehouse_id);
    }

    #[payable]
    pub fn add_custodian(&mut self, warehouse_id: String, custodian: AccountId) {
        self.internal_add_custodian(warehouse_id, custodian);
    }

    #[payable]
    pub fn remove_custodian(&mut self, warehouse_id: String, custodian: AccountId) {
        self.internal_remove_custodian(warehouse_id, custodian);
    }

    #[payable]
    pub fn add_verifier(&mut self, verifier: AccountId) {
        self.internal_add_verifier(verifier);
    }

    #[payable]
    pub fn remove_verifier(&mut self, verifier: AccountId) {
        self.internal_remove_verifier(verifier);
    }

    #[payable]
    pub fn add_recovery_operator(&mut self, operator: AccountId) {
        self.internal_add_recovery_operator(operator);
    }

    #[payable]
    pub fn remove_recovery_operator(&mut self, operator: AccountId) {
        self.internal_remove_recovery_operator(operator);
    }

    #[payable]
    pub fn revoke(&mut self, receipt_id: NumU64) {
        self.internal_revoke(receipt_id.0);
    }

    #[payable]
    pub fn preclaim(&mut self, warehouse_id: String, deposit_nos: Vec<NumU64>) {
        self.internal_preclaim(
            warehouse_id,
            deposit_nos.into_iter().map(|n| n.0).collect(),
        );
    }

    // --- Anchoring & Minting ---

    pub fn anchor(&mut self, warehouse_id: String, day: u32, root: String, leaves: u32) -> String {
        self.internal_anchor(warehouse_id, day, root, leaves)
    }

    pub fn mint_propose(
        &mut self,
        leaf: LeafInput,
        proof: Vec<String>,
        not_after: NumU64,
    ) -> String {
        self.internal_mint_propose(leaf, proof, not_after.0)
    }

    pub fn mint_approve(
        &mut self,
        warehouse_id: String,
        deposit_no: NumU64,
        expected_leaf_hash: String,
    ) -> String {
        self.internal_mint_approve(warehouse_id, deposit_no.0, expected_leaf_hash)
    }

    // --- Lending ---

    #[payable]
    pub fn lock_offer(
        &mut self,
        receipt_id: NumU64,
        amount_kg: NumU64,
        loan_ref: String,
        due_date: NumU64,
        grace_period_seconds: NumU64,
        expires_at: NumU64,
    ) {
        self.internal_lock_offer(
            receipt_id.0,
            amount_kg.0,
            loan_ref,
            due_date.0,
            grace_period_seconds.0,
            expires_at.0,
        );
    }

    #[payable]
    pub fn offer_lock(
        &mut self,
        receipt_id: NumU64,
        amount_kg: NumU64,
        loan_ref: String,
        due_date: NumU64,
        grace_period_seconds: NumU64,
        expires_at: NumU64,
    ) {
        self.lock_offer(
            receipt_id,
            amount_kg,
            loan_ref,
            due_date,
            grace_period_seconds,
            expires_at,
        );
    }

    pub fn lock_accept(
        &mut self,
        receipt_id: NumU64,
        lender: AccountId,
        expected_amount_kg: NumU64,
        expected_loan_ref: String,
        expected_due_date: NumU64,
        expected_grace_period: NumU64,
    ) -> String {
        self.internal_lock_accept(
            receipt_id.0,
            lender,
            expected_amount_kg.0,
            expected_loan_ref,
            expected_due_date.0,
            expected_grace_period.0,
        )
    }

    pub fn accept_lock(
        &mut self,
        receipt_id: NumU64,
        lender: AccountId,
        expected_amount_kg: NumU64,
        expected_loan_ref: String,
        expected_due_date: NumU64,
        expected_grace_period: NumU64,
    ) -> String {
        self.lock_accept(
            receipt_id,
            lender,
            expected_amount_kg,
            expected_loan_ref,
            expected_due_date,
            expected_grace_period,
        )
    }

    pub fn unlock(&mut self, receipt_id: NumU64, kg: NumU64) -> String {
        self.internal_unlock(receipt_id.0, kg.0)
    }

    pub fn extend_due(&mut self, receipt_id: NumU64, new_due: NumU64) {
        self.internal_extend_due(receipt_id.0, new_due.0);
    }

    // --- Recovery ---

    pub fn start_recovery(&mut self, receipt_id: NumU64) {
        self.internal_start_recovery(receipt_id.0);
    }

    pub fn approve_sale(
        &mut self,
        receipt_id: NumU64,
        buyer: AccountId,
        sale_ref: String,
        expires_at: NumU64,
    ) {
        self.internal_approve_sale(receipt_id.0, buyer, sale_ref, expires_at.0);
    }

    pub fn complete_recovery(
        &mut self,
        receipt_id: NumU64,
        buyer: AccountId,
        sale_ref: String,
    ) -> String {
        self.internal_complete_recovery(receipt_id.0, buyer, sale_ref)
    }

    pub fn record_settlement(&mut self, sale_id: NumU64, settlement_ref: String) {
        self.internal_record_settlement(sale_id.0, settlement_ref);
    }

    // --- Exit ---

    pub fn request_exit(&mut self, receipt_id: NumU64, kg: NumU64, expires_at: NumU64) {
        self.internal_request_exit(receipt_id.0, kg.0, expires_at.0);
    }

    pub fn exit(&mut self, receipt_id: NumU64, kg: NumU64, exit_weight_kg: NumU64) -> String {
        self.internal_exit(receipt_id.0, kg.0, exit_weight_kg.0)
    }

    // --- Views ---

    pub fn get_receipt(&self, receipt_id: NumU64) -> Option<ReceiptView> {
        self.receipts.get(&receipt_id.0).map(ReceiptView::from)
    }

    pub fn last_receipt_id(&self) -> String {
        self.last_receipt_id.to_string()
    }

    pub fn get_anchor(&self, warehouse_id: String) -> Option<AnchorView> {
        self.anchors.get(&warehouse_id).map(|a| AnchorView {
            day: a.day,
            head: a.head.clone(),
        })
    }

    pub fn get_day_root(&self, warehouse_id: String, day: u32) -> Option<DayRootView> {
        self.day_roots.get(&(warehouse_id, day)).map(|r| DayRootView {
            day: r.day,
            root: r.root.clone(),
            leaves: r.leaves,
            prev_head: r.prev_head.clone(),
            head: r.head.clone(),
            custodian: r.custodian.to_string(),
            anchored_at: r.anchored_at,
        })
    }

    pub fn is_claimed(&self, warehouse_id: String, deposit_no: NumU64) -> bool {
        self.claimed.contains_key(&(warehouse_id, deposit_no.0))
    }

    pub fn leaf_hash(&self, leaf: LeafInput) -> String {
        merkle::compute_leaf_hash(&leaf)
            .map(hex::encode)
            .unwrap_or_else(|e| e.panic())
    }

    pub fn verify_leaf(&self, leaf: LeafInput, proof: Vec<String>) -> bool {
        let leaf_hash_bytes = match merkle::compute_leaf_hash(&leaf) {
            Ok(h) => h,
            Err(_) => return false,
        };
        let day_root = match self.day_roots.get(&(leaf.warehouse_id.clone(), leaf.day)) {
            Some(r) => r,
            None => return false,
        };
        if proof.len() != merkle::depth_for(day_root.leaves) || leaf.index >= day_root.leaves {
            return false;
        }
        let mut proof_bytes = Vec::with_capacity(proof.len());
        for p in &proof {
            match hex::decode(p) {
                Ok(b) if b.len() == 32 => {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&b);
                    proof_bytes.push(arr);
                }
                _ => return false,
            }
        }
        let root_bytes = match hex::decode(&day_root.root) {
            Ok(b) if b.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&b);
                arr
            }
            _ => return false,
        };
        merkle::fold_proof(&leaf_hash_bytes, leaf.index, &proof_bytes) == root_bytes
    }

    pub fn get_lock(&self, receipt_id: NumU64) -> Option<LockView> {
        self.receipts.get(&receipt_id.0).and_then(|r| {
            r.lock.as_ref().map(|l| LockView {
                lender: l.lender.to_string(),
                loan_ref: l.loan_ref.clone(),
                due_date: l.due_date,
                grace_period: l.grace_period,
            })
        })
    }

    pub fn get_recovery(&self, receipt_id: NumU64) -> Option<RecoveryView> {
        self.receipts.get(&receipt_id.0).and_then(|r| {
            r.recovery.as_ref().map(|rec| RecoveryView {
                lender: rec.lender.to_string(),
                loan_ref: rec.loan_ref.clone(),
                owner: rec.owner.to_string(),
                started_at: rec.started_at,
                resolution: rec.resolution,
                resolved_at: rec.resolved_at,
                sale_id: rec.sale_id.map(|id| id.to_string()),
                buyer: rec.buyer.as_ref().map(|b| b.to_string()),
                sale_ref: rec.sale_ref.clone(),
            })
        })
    }

    pub fn get_sale(&self, sale_id: NumU64) -> Option<SaleView> {
        self.sales.get(&sale_id.0).map(|s| SaleView {
            data_version: s.data_version,
            sale_id: s.sale_id.to_string(),
            receipt_id: s.receipt_id.to_string(),
            warehouse_id: s.warehouse_id.clone(),
            weight_kg: s.weight_kg.to_string(),
            lender: s.lender.to_string(),
            loan_ref: s.loan_ref.clone(),
            operator: s.operator.to_string(),
            previous_owner: s.previous_owner.to_string(),
            buyer: s.buyer.to_string(),
            sale_ref: s.sale_ref.clone(),
            started_at: s.started_at,
            completed_at: s.completed_at,
            settlement_ref: s.settlement_ref.clone(),
            settled_at: s.settled_at,
        })
    }

    pub fn is_warehouse(&self, warehouse_id: String) -> bool {
        self.warehouses.contains(&warehouse_id)
    }

    pub fn is_custodian(&self, warehouse_id: String, custodian: AccountId) -> bool {
        self.custodians.contains_key(&(warehouse_id, custodian))
    }

    pub fn is_verifier(&self, verifier: AccountId) -> bool {
        self.verifiers.contains(&verifier)
    }

    pub fn is_recovery_operator(&self, operator: AccountId) -> bool {
        self.recovery_operators.contains(&operator)
    }

    pub fn get_admin(&self) -> AccountId {
        self.admin.clone()
    }

    pub fn get_proposed_admin(&self) -> Option<AccountId> {
        self.proposed_admin.clone()
    }

    pub fn data_version(&self) -> u32 {
        DATA_VERSION
    }
}
