use serde_json::json;

use crate::errors::ContractError;
use crate::events::emit_event;
use crate::storage::now_seconds;
use crate::types::{Receipt, DATA_VERSION};

pub struct SplitParts {
    pub taken: Receipt,
    pub remainder: Receipt,
}

impl crate::ReceiptContract {
    /// Divides `parent` into `kg` (taken child) and `parent.weight_kg - kg` (remainder parent).
    pub fn internal_split_receipt(
        &mut self,
        parent: &Receipt,
        kg: u64,
    ) -> Result<SplitParts, ContractError> {
        if kg == 0 || kg >= parent.weight_kg {
            return Err(ContractError::InvalidSplitAmount);
        }

        let taken_weight = kg;
        let remainder_weight = parent.weight_kg - kg;

        let taken_bags = ((parent.bag_count as u128 * kg as u128) / parent.weight_kg as u128) as u32;
        let remainder_bags = parent.bag_count - taken_bags;

        self.last_receipt_id += 1;
        let taken_id = self.last_receipt_id;

        let taken = Receipt {
            data_version: DATA_VERSION,
            receipt_id: taken_id,
            deposit_id: parent.deposit_id.clone(),
            deposit_no: parent.deposit_no,
            parent_id: Some(parent.receipt_id),
            commodity: parent.commodity.clone(),
            grade: parent.grade.clone(),
            weight_kg: taken_weight,
            weight_source: parent.weight_source,
            bag_count: taken_bags,
            moisture_pct: parent.moisture_pct,
            warehouse_id: parent.warehouse_id.clone(),
            custodian: parent.custodian.clone(),
            verifier: parent.verifier.clone(),
            owner: parent.owner.clone(),
            status: parent.status,
            lock: parent.lock.clone(),
            recovery: parent.recovery.clone(),
            recovery_history: parent.recovery_history.clone(),
            deposit_date: parent.deposit_date,
            exit_date: None,
            exit_weight_kg: None,
        };

        let mut remainder = parent.clone();
        remainder.weight_kg = remainder_weight;
        remainder.bag_count = remainder_bags;

        self.receipts.insert(remainder.receipt_id, remainder.clone());
        self.receipts.insert(taken.receipt_id, taken.clone());

        emit_event(
            "split",
            &json!({
                "parent_id": parent.receipt_id.to_string(),
                "taken_id": taken.receipt_id.to_string(),
                "remainder_id": remainder.receipt_id.to_string(),
                "warehouse_id": parent.warehouse_id,
                "owner": parent.owner.to_string(),
                "parent_weight_kg": parent.weight_kg.to_string(),
                "taken_weight_kg": taken_weight.to_string(),
                "taken_kg": taken_weight.to_string(),
                "remainder_weight_kg": remainder_weight.to_string(),
                "remainder_kg": remainder_weight.to_string(),
                "taken_bag_count": taken_bags,
                "remainder_bag_count": remainder_bags,
                "split_at": now_seconds().to_string(),
            }),
        );

        Ok(SplitParts { taken, remainder })
    }
}
