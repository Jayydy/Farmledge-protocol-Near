use near_sdk::{env, AccountId};
use crate::errors::ContractError;
use crate::events::emit_event;
use serde_json::json;

use crate::storage::now_seconds;

impl crate::ReceiptContract {
    pub fn assert_admin(&self) {
        if env::predecessor_account_id() != self.admin {
            ContractError::Unauthorized.panic();
        }
    }

    pub fn assert_admin_with_deposit(&self) {
        self.assert_admin();
        if env::attached_deposit().as_yoctonear() < 1 {
            ContractError::Unauthorized.panic();
        }
    }

    pub fn internal_add_warehouse(&mut self, warehouse_id: String) {
        self.assert_admin_with_deposit();
        if self.warehouses.contains(&warehouse_id) {
            ContractError::AlreadyRegistered.panic();
        }
        self.warehouses.insert(warehouse_id.clone());
        emit_event(
            "warehouse_added",
            &json!({
                "warehouse_id": warehouse_id,
                "added_at": now_seconds().to_string(),
            }),
        );
    }

    pub fn internal_remove_warehouse(&mut self, warehouse_id: String) {
        self.assert_admin_with_deposit();
        if !self.warehouses.contains(&warehouse_id) {
            ContractError::NotRegistered.panic();
        }
        self.warehouses.remove(&warehouse_id);
        emit_event(
            "warehouse_removed",
            &json!({
                "warehouse_id": warehouse_id,
                "removed_at": now_seconds().to_string(),
            }),
        );
    }

    pub fn internal_add_custodian(&mut self, warehouse_id: String, custodian: AccountId) {
        self.assert_admin_with_deposit();
        if !self.warehouses.contains(&warehouse_id) {
            ContractError::WarehouseNotRegistered.panic();
        }
        let key = (warehouse_id.clone(), custodian.clone());
        if self.custodians.contains_key(&key) {
            ContractError::AlreadyRegistered.panic();
        }
        self.custodians.insert(key, ());
        emit_event(
            "custodian_added",
            &json!({
                "warehouse_id": warehouse_id,
                "custodian": custodian.to_string(),
                "added_at": now_seconds().to_string(),
            }),
        );
    }

    pub fn internal_remove_custodian(&mut self, warehouse_id: String, custodian: AccountId) {
        self.assert_admin_with_deposit();
        let key = (warehouse_id.clone(), custodian.clone());
        if !self.custodians.contains_key(&key) {
            ContractError::NotRegistered.panic();
        }
        self.custodians.remove(&key);
        emit_event(
            "custodian_removed",
            &json!({
                "warehouse_id": warehouse_id,
                "custodian": custodian.to_string(),
                "removed_at": now_seconds().to_string(),
            }),
        );
    }

    pub fn internal_add_verifier(&mut self, verifier: AccountId) {
        self.assert_admin_with_deposit();
        if self.verifiers.contains(&verifier) {
            ContractError::AlreadyRegistered.panic();
        }
        self.verifiers.insert(verifier.clone());
        emit_event(
            "verifier_added",
            &json!({
                "verifier": verifier.to_string(),
                "added_at": now_seconds().to_string(),
            }),
        );
    }

    pub fn internal_remove_verifier(&mut self, verifier: AccountId) {
        self.assert_admin_with_deposit();
        if !self.verifiers.contains(&verifier) {
            ContractError::NotRegistered.panic();
        }
        self.verifiers.remove(&verifier);
        emit_event(
            "verifier_removed",
            &json!({
                "verifier": verifier.to_string(),
                "removed_at": now_seconds().to_string(),
            }),
        );
    }

    pub fn internal_add_recovery_operator(&mut self, operator: AccountId) {
        self.assert_admin_with_deposit();
        if self.recovery_operators.contains(&operator) {
            ContractError::AlreadyRegistered.panic();
        }
        self.recovery_operators.insert(operator.clone());
        emit_event(
            "recovery_operator_added",
            &json!({
                "operator": operator.to_string(),
                "added_at": now_seconds().to_string(),
            }),
        );
    }

    pub fn internal_remove_recovery_operator(&mut self, operator: AccountId) {
        self.assert_admin_with_deposit();
        if !self.recovery_operators.contains(&operator) {
            ContractError::NotRegistered.panic();
        }
        self.recovery_operators.remove(&operator);
        emit_event(
            "recovery_operator_removed",
            &json!({
                "operator": operator.to_string(),
                "removed_at": now_seconds().to_string(),
            }),
        );
    }
}
