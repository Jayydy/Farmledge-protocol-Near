use near_sdk::{env, AccountId};
use serde_json::json;

use crate::errors::ContractError;
use crate::events::emit_event;
use crate::merkle::{
    compute_head, compute_leaf_hash, depth_for, fold_proof, ZERO_HEAD,
};
use crate::storage::{current_utc_day, now_seconds};
use crate::types::{
    AnchorHead, DayRoot, LeafInput, PendingMint, Receipt, Status, WeightSource,
    DATA_VERSION, MAX_LEAVES, MAX_PRECLAIM_BATCH, MAX_REF_LEN, MINT_WINDOW_SECONDS,
};

impl crate::ReceiptContract {
    pub fn internal_anchor(
        &mut self,
        warehouse_id: String,
        day: u32,
        root: String,
        leaves: u32,
    ) -> String {
        let caller = env::predecessor_account_id();
        if !self.warehouses.contains(&warehouse_id) {
            ContractError::WarehouseNotRegistered.panic();
        }
        if !self.custodians.contains_key(&(warehouse_id.clone(), caller.clone())) {
            ContractError::CustodianNotRegistered.panic();
        }
        if leaves == 0 || leaves > MAX_LEAVES {
            ContractError::InvalidLeafCount.panic();
        }

        let prev = self.anchors.get(&warehouse_id);
        if let Some(p) = prev {
            if day <= p.day {
                ContractError::AnchorOutOfOrder.panic();
            }
        }
        if day > current_utc_day() {
            ContractError::AnchorInFuture.panic();
        }

        let prev_head_bytes = match prev {
            Some(p) => hex::decode(&p.head)
                .map(|b| {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&b);
                    arr
                })
                .unwrap_or(ZERO_HEAD),
            None => ZERO_HEAD,
        };

        let root_bytes = hex::decode(&root)
            .map(|b| {
                if b.len() != 32 {
                    ContractError::InvalidProof.panic();
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&b);
                arr
            })
            .unwrap_or_else(|_| ContractError::InvalidProof.panic());

        let head_bytes = compute_head(&prev_head_bytes, day, leaves, &root_bytes);
        let head = hex::encode(head_bytes);
        let prev_head_hex = hex::encode(prev_head_bytes);

        let now = now_seconds();
        let day_root = DayRoot {
            day,
            root: root.clone(),
            leaves,
            prev_head: prev_head_hex.clone(),
            head: head.clone(),
            custodian: caller.clone(),
            anchored_at: now,
        };

        self.day_roots.insert((warehouse_id.clone(), day), day_root);
        self.anchors.insert(
            warehouse_id.clone(),
            AnchorHead {
                day,
                head: head.clone(),
            },
        );

        emit_event(
            "anchored",
            &json!({
                "warehouse_id": warehouse_id,
                "day": day,
                "leaves": leaves,
                "root": root,
                "prev_head": prev_head_hex,
                "head": head,
                "custodian": caller.to_string(),
                "anchored_at": now.to_string(),
            }),
        );

        head
    }

    pub fn internal_mint_propose(
        &mut self,
        leaf: LeafInput,
        proof: Vec<String>,
        not_after: u64,
    ) -> String {
        let caller = env::predecessor_account_id();
        let warehouse_id = leaf.warehouse_id.clone();

        if !self.warehouses.contains(&warehouse_id) {
            ContractError::WarehouseNotRegistered.panic();
        }
        if !self.custodians.contains_key(&(warehouse_id.clone(), caller.clone())) {
            ContractError::CustodianNotRegistered.panic();
        }

        if leaf.version != 1 {
            ContractError::UnsupportedLeafVersion.panic();
        }
        if leaf.kind != 2 {
            // Balance leaf only
            ContractError::WrongLeafKind.panic();
        }
        if leaf.receipt_id != 0 || leaf.receipt_version != 0 {
            ContractError::LeafHasReceipt.panic();
        }

        let claim_key = (warehouse_id.clone(), leaf.deposit_no);
        if self.claimed.contains_key(&claim_key) {
            ContractError::DuplicateDepositId.panic();
        }

        let anchor = self
            .anchors
            .get(&warehouse_id)
            .unwrap_or_else(|| ContractError::NoAnchor.panic());
        if leaf.day != anchor.day {
            ContractError::StaleLeaf.panic();
        }

        let day_root = self
            .day_roots
            .get(&(warehouse_id.clone(), leaf.day))
            .unwrap_or_else(|| ContractError::NoAnchor.panic());

        let now = now_seconds();
        if now > not_after {
            ContractError::PendingExpired.panic();
        }
        if not_after > now + MINT_WINDOW_SECONDS {
            ContractError::MintWindowTooLong.panic();
        }

        if leaf.kg <= 0 {
            ContractError::InvalidWeight.panic();
        }
        if leaf.weight_source == WeightSource::BagEstimate && leaf.bags == 0 {
            ContractError::InvalidWeight.panic();
        }
        if leaf.moisture_bps > 10_000 {
            ContractError::InvalidMoisture.panic();
        }
        if leaf.deposit_id.len() > MAX_REF_LEN {
            ContractError::RefTooLong.panic();
        }

        let leaf_hash_bytes = compute_leaf_hash(&leaf)
            .unwrap_or_else(|e| e.panic());
        let leaf_hash = hex::encode(leaf_hash_bytes);

        let required_depth = depth_for(day_root.leaves);
        if proof.len() != required_depth || leaf.index >= day_root.leaves {
            ContractError::InvalidProof.panic();
        }

        let mut proof_bytes = Vec::with_capacity(proof.len());
        for p in &proof {
            let b = hex::decode(p).map_err(|_| ContractError::InvalidProof).unwrap_or_else(|e| e.panic());
            if b.len() != 32 {
                ContractError::InvalidProof.panic();
            }
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            proof_bytes.push(arr);
        }

        let root_bytes = hex::decode(&day_root.root)
            .map(|b| {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&b);
                arr
            })
            .unwrap_or_else(|_| ContractError::InvalidProof.panic());

        let folded = fold_proof(&leaf_hash_bytes, leaf.index, &proof_bytes);
        if folded != root_bytes {
            ContractError::InvalidProof.panic();
        }

        let pending_key = (warehouse_id.clone(), leaf.deposit_no);
        self.pending_mints.insert(
            pending_key,
            PendingMint {
                leaf: leaf.clone(),
                leaf_hash: leaf_hash.clone(),
                custodian: caller.clone(),
                not_after,
            },
        );

        emit_event(
            "mint_proposed",
            &json!({
                "warehouse_id": warehouse_id,
                "deposit_no": leaf.deposit_no.to_string(),
                "custodian": caller.to_string(),
                "not_after": not_after,
            }),
        );

        leaf_hash
    }

    pub fn internal_mint_approve(
        &mut self,
        warehouse_id: String,
        deposit_no: u64,
        expected_leaf_hash: String,
    ) -> String {
        let caller = env::predecessor_account_id();
        if !self.verifiers.contains(&caller) {
            ContractError::VerifierNotRegistered.panic();
        }

        let key = (warehouse_id.clone(), deposit_no);
        let pending = self
            .pending_mints
            .get(&key)
            .cloned()
            .unwrap_or_else(|| ContractError::PendingNotFound.panic());

        let now = now_seconds();
        if now > pending.not_after {
            ContractError::PendingExpired.panic();
        }
        if pending.leaf_hash != expected_leaf_hash {
            ContractError::TermsMismatch.panic();
        }

        if self.claimed.contains_key(&key) {
            ContractError::DuplicateDepositId.panic();
        }
        self.claimed.insert(key.clone(), ());
        self.pending_mints.remove(&key);

        self.last_receipt_id += 1;
        let receipt_id = self.last_receipt_id;
        let leaf = pending.leaf;

        let owner_account: AccountId = leaf.owner.parse().unwrap_or_else(|_| {
            ContractError::CorruptRecord.panic()
        });
        let custodian_account: AccountId = leaf.custodian.parse().unwrap_or_else(|_| {
            ContractError::CorruptRecord.panic()
        });

        let receipt = Receipt {
            data_version: DATA_VERSION,
            receipt_id,
            deposit_id: leaf.deposit_id.clone(),
            deposit_no: leaf.deposit_no,
            parent_id: None,
            commodity: leaf.commodity.clone(),
            grade: leaf.grade.clone(),
            weight_kg: leaf.kg as u64,
            weight_source: leaf.weight_source,
            bag_count: leaf.bags,
            moisture_pct: leaf.moisture_bps,
            warehouse_id: leaf.warehouse_id.clone(),
            custodian: custodian_account,
            verifier: caller,
            owner: owner_account,
            status: Status::Active,
            lock: None,
            recovery: None,
            recovery_history: Vec::new(),
            deposit_date: leaf.deposited_at,
            exit_date: None,
            exit_weight_kg: None,
        };

        self.receipts.insert(receipt_id, receipt);

        emit_event(
            "minted",
            &json!({
                "receipt_id": receipt_id.to_string(),
                "warehouse_id": leaf.warehouse_id,
                "deposit_no": leaf.deposit_no.to_string(),
                "deposit_id": leaf.deposit_id,
                "owner": leaf.owner,
                "commodity": leaf.commodity,
                "grade": leaf.grade,
                "weight_kg": leaf.kg.to_string(),
                "bag_count": leaf.bags,
                "moisture_bps": leaf.moisture_bps,
                "weight_source": leaf.weight_source,
                "anchor_day": leaf.day,
                "leaf_hash": pending.leaf_hash,
                "minted_at": now.to_string(),
            }),
        );

        receipt_id.to_string()
    }

    pub fn internal_preclaim(&mut self, warehouse_id: String, deposit_nos: Vec<u64>) {
        self.assert_admin_with_deposit();
        if !self.warehouses.contains(&warehouse_id) {
            ContractError::WarehouseNotRegistered.panic();
        }
        if deposit_nos.len() > MAX_PRECLAIM_BATCH {
            ContractError::BatchTooLarge.panic();
        }

        // Verify none already claimed
        for no in &deposit_nos {
            let key = (warehouse_id.clone(), *no);
            if self.claimed.contains_key(&key) {
                ContractError::DuplicateDepositId.panic();
            }
        }

        // Claim all
        for no in &deposit_nos {
            let key = (warehouse_id.clone(), *no);
            self.claimed.insert(key, ());
        }

        emit_event(
            "preclaimed",
            &json!({
                "warehouse_id": warehouse_id,
                "deposit_nos": deposit_nos.iter().map(|n| n.to_string()).collect::<Vec<_>>(),
                "preclaimed_at": now_seconds().to_string(),
            }),
        );
    }
}
