use near_sdk::{env, AccountId};
use serde_json::json;

use crate::errors::ContractError;
use crate::events::emit_event;
use crate::storage::now_seconds;
use crate::types::{
    Lock, PendingExit, PendingLockOffer, PendingSale, Recovery, RecoveryResolution,
    Sale, Status, WeightSource, DATA_VERSION, MAX_REF_LEN,
};

impl crate::ReceiptContract {
    pub fn internal_lock_offer(
        &mut self,
        receipt_id: u64,
        amount_kg: u64,
        loan_ref: String,
        due_date: u64,
        grace_period_seconds: u64,
        expires_at: u64,
    ) {
        let lender = env::predecessor_account_id();
        let receipt = self
            .receipts
            .get(&receipt_id)
            .cloned()
            .unwrap_or_else(|| ContractError::ReceiptNotFound.panic());

        if receipt.status != Status::Active {
            ContractError::InvalidStatus.panic();
        }
        if receipt.weight_source != WeightSource::Scale {
            ContractError::OnlyScaleEligibleForLock.panic();
        }
        if lender == receipt.owner {
            ContractError::LenderIsOwner.panic();
        }
        if amount_kg == 0 || amount_kg > receipt.weight_kg {
            ContractError::InvalidSplitAmount.panic();
        }
        let now = now_seconds();
        if due_date <= now || due_date.checked_add(grace_period_seconds).is_none() {
            ContractError::InvalidDueDate.panic();
        }
        if loan_ref.len() > MAX_REF_LEN {
            ContractError::RefTooLong.panic();
        }
        if expires_at <= now {
            ContractError::PendingExpired.panic();
        }

        let pending = PendingLockOffer {
            receipt_id,
            lender: lender.clone(),
            amount_kg,
            loan_ref,
            due_date,
            grace_period_seconds,
            expires_at,
        };
        self.pending_locks.insert((receipt_id, lender.clone()), pending);

        emit_event(
            "lock_offered",
            &json!({
                "receipt_id": receipt_id.to_string(),
                "lender": lender.to_string(),
                "amount_kg": amount_kg.to_string(),
                "expires_at": expires_at,
            }),
        );
    }

    pub fn internal_lock_accept(
        &mut self,
        receipt_id: u64,
        lender: AccountId,
        expected_amount_kg: u64,
        expected_loan_ref: String,
        expected_due_date: u64,
        expected_grace_period: u64,
    ) -> String {
        let caller = env::predecessor_account_id();
        let receipt = self
            .receipts
            .get(&receipt_id)
            .cloned()
            .unwrap_or_else(|| ContractError::ReceiptNotFound.panic());

        if receipt.owner != caller {
            ContractError::Unauthorized.panic();
        }
        if receipt.status != Status::Active {
            ContractError::InvalidStatus.panic();
        }

        let key = (receipt_id, lender.clone());
        let offer = self
            .pending_locks
            .get(&key)
            .cloned()
            .unwrap_or_else(|| ContractError::PendingNotFound.panic());

        let now = now_seconds();
        if now > offer.expires_at {
            ContractError::PendingExpired.panic();
        }

        if offer.amount_kg != expected_amount_kg
            || offer.loan_ref != expected_loan_ref
            || offer.due_date != expected_due_date
            || offer.grace_period_seconds != expected_grace_period
        {
            ContractError::TermsMismatch.panic();
        }

        self.pending_locks.remove(&key);

        let locked_id = if expected_amount_kg < receipt.weight_kg {
            let parts = self
                .internal_split_receipt(&receipt, expected_amount_kg)
                .unwrap_or_else(|e| e.panic());
            let mut taken = parts.taken;
            taken.status = Status::Locked;
            taken.lock = Some(Lock {
                lender: lender.clone(),
                loan_ref: expected_loan_ref.clone(),
                due_date: expected_due_date,
                grace_period: expected_grace_period,
            });
            self.receipts.insert(taken.receipt_id, taken.clone());
            taken.receipt_id
        } else {
            let mut locking = receipt;
            locking.status = Status::Locked;
            locking.lock = Some(Lock {
                lender: lender.clone(),
                loan_ref: expected_loan_ref.clone(),
                due_date: expected_due_date,
                grace_period: expected_grace_period,
            });
            self.receipts.insert(locking.receipt_id, locking.clone());
            locking.receipt_id
        };

        emit_event(
            "locked",
            &json!({
                "receipt_id": locked_id.to_string(),
                "lender": lender.to_string(),
                "weight_kg": expected_amount_kg.to_string(),
                "loan_ref": expected_loan_ref,
                "due_date": expected_due_date.to_string(),
                "grace_period": expected_grace_period.to_string(),
                "locked_at": now.to_string(),
            }),
        );

        locked_id.to_string()
    }

    pub fn internal_unlock(&mut self, receipt_id: u64, kg: u64) -> String {
        let caller = env::predecessor_account_id();
        let receipt = self
            .receipts
            .get(&receipt_id)
            .cloned()
            .unwrap_or_else(|| ContractError::ReceiptNotFound.panic());

        if receipt.status != Status::Locked && receipt.status != Status::Recovery {
            ContractError::InvalidStatus.panic();
        }

        let lock = receipt.lock.as_ref().unwrap_or_else(|| ContractError::NotLocked.panic());
        if lock.lender != caller {
            ContractError::Unauthorized.panic();
        }

        if kg == 0 || kg > receipt.weight_kg {
            ContractError::InvalidSplitAmount.panic();
        }

        let now = now_seconds();
        let (mut released, remaining_id) = if kg == receipt.weight_kg {
            (receipt, None)
        } else {
            let parts = self
                .internal_split_receipt(&receipt, kg)
                .unwrap_or_else(|e| e.panic());
            (parts.taken, Some(parts.remainder.receipt_id))
        };

        released.status = Status::Active;
        released.lock = None;
        if let Some(mut rec) = released.recovery.take() {
            rec.resolution = RecoveryResolution::Repaid;
            rec.resolved_at = Some(now);
            released.recovery_history.push(rec);
        }

        self.receipts.insert(released.receipt_id, released.clone());

        emit_event(
            "unlocked",
            &json!({
                "receipt_id": released.receipt_id.to_string(),
                "lender": caller.to_string(),
                "owner": released.owner.to_string(),
                "warehouse_id": released.warehouse_id,
                "weight_kg": released.weight_kg.to_string(),
                "partial": remaining_id.is_some(),
                "remaining_receipt_id": remaining_id.map(|id| id.to_string()),
                "unlocked_at": now.to_string(),
            }),
        );

        released.receipt_id.to_string()
    }

    pub fn internal_extend_due(&mut self, receipt_id: u64, new_due: u64) {
        let caller = env::predecessor_account_id();
        let mut receipt = self
            .receipts
            .get(&receipt_id)
            .cloned()
            .unwrap_or_else(|| ContractError::ReceiptNotFound.panic());

        if receipt.status != Status::Locked {
            ContractError::InvalidStatus.panic();
        }

        let lock = receipt.lock.as_mut().unwrap_or_else(|| ContractError::NotLocked.panic());
        if lock.lender != caller {
            ContractError::Unauthorized.panic();
        }

        if new_due <= lock.due_date {
            ContractError::DueDateNotLater.panic();
        }
        if new_due.checked_add(lock.grace_period).is_none() {
            ContractError::InvalidDueDate.panic();
        }

        let old_due = lock.due_date;
        lock.due_date = new_due;
        let lender = lock.lender.clone();
        let grace = lock.grace_period;
        let weight = receipt.weight_kg;

        self.receipts.insert(receipt_id, receipt);

        emit_event(
            "due_extended",
            &json!({
                "receipt_id": receipt_id.to_string(),
                "lender": lender.to_string(),
                "weight_kg": weight.to_string(),
                "old_due_date": old_due.to_string(),
                "new_due_date": new_due.to_string(),
                "grace_period": grace.to_string(),
                "extended_at": now_seconds().to_string(),
            }),
        );
    }

    pub fn internal_start_recovery(&mut self, receipt_id: u64) {
        let caller = env::predecessor_account_id();
        let mut receipt = self
            .receipts
            .get(&receipt_id)
            .cloned()
            .unwrap_or_else(|| ContractError::ReceiptNotFound.panic());

        if receipt.status != Status::Locked {
            ContractError::InvalidStatus.panic();
        }

        let lock = receipt.lock.as_ref().unwrap_or_else(|| ContractError::NotLocked.panic());
        if lock.lender != caller {
            ContractError::Unauthorized.panic();
        }

        let now = now_seconds();
        let deadline = lock.due_date.saturating_add(lock.grace_period);
        if now <= deadline {
            ContractError::RecoveryNotYetDue.panic();
        }

        receipt.status = Status::Recovery;
        receipt.recovery = Some(Recovery {
            lender: lock.lender.clone(),
            loan_ref: lock.loan_ref.clone(),
            owner: receipt.owner.clone(),
            started_at: now,
            resolution: RecoveryResolution::Open,
            resolved_at: None,
            sale_id: None,
            buyer: None,
            sale_ref: None,
        });

        let lender = lock.lender.clone();
        let warehouse = receipt.warehouse_id.clone();
        self.receipts.insert(receipt_id, receipt);

        emit_event(
            "recovery_started",
            &json!({
                "receipt_id": receipt_id.to_string(),
                "lender": lender.to_string(),
                "warehouse_id": warehouse,
                "started_at": now.to_string(),
            }),
        );
    }

    pub fn internal_approve_sale(
        &mut self,
        receipt_id: u64,
        buyer: AccountId,
        sale_ref: String,
        expires_at: u64,
    ) {
        let caller = env::predecessor_account_id();
        let receipt = self
            .receipts
            .get(&receipt_id)
            .cloned()
            .unwrap_or_else(|| ContractError::ReceiptNotFound.panic());

        if receipt.status != Status::Recovery {
            ContractError::InvalidStatus.panic();
        }

        let lock = receipt.lock.as_ref().unwrap_or_else(|| ContractError::NotLocked.panic());
        if lock.lender != caller {
            ContractError::Unauthorized.panic();
        }

        if sale_ref.len() > MAX_REF_LEN {
            ContractError::RefTooLong.panic();
        }
        let now = now_seconds();
        if expires_at <= now {
            ContractError::PendingExpired.panic();
        }

        let pending = PendingSale {
            receipt_id,
            lender: caller,
            buyer,
            sale_ref,
            expires_at,
        };
        self.pending_sales.insert(receipt_id, pending);
    }

    pub fn internal_complete_recovery(
        &mut self,
        receipt_id: u64,
        buyer: AccountId,
        sale_ref: String,
    ) -> String {
        let caller = env::predecessor_account_id();
        if !self.recovery_operators.contains(&caller) {
            ContractError::RecoveryOperatorNotRegistered.panic();
        }

        let mut receipt = self
            .receipts
            .get(&receipt_id)
            .cloned()
            .unwrap_or_else(|| ContractError::ReceiptNotFound.panic());

        if receipt.status != Status::Recovery {
            ContractError::InvalidStatus.panic();
        }

        let pending = self
            .pending_sales
            .get(&receipt_id)
            .cloned()
            .unwrap_or_else(|| ContractError::PendingNotFound.panic());

        let now = now_seconds();
        if now > pending.expires_at {
            ContractError::PendingExpired.panic();
        }
        if pending.buyer != buyer || pending.sale_ref != sale_ref {
            ContractError::TermsMismatch.panic();
        }

        self.pending_sales.remove(&receipt_id);

        let mut recovery = receipt.recovery.take().unwrap_or_else(|| ContractError::InvalidStatus.panic());
        let lock = receipt.lock.take().unwrap_or_else(|| ContractError::NotLocked.panic());

        self.last_sale_id += 1;
        let sale_id = self.last_sale_id;

        let sale = Sale {
            data_version: DATA_VERSION,
            sale_id,
            receipt_id,
            warehouse_id: receipt.warehouse_id.clone(),
            weight_kg: receipt.weight_kg,
            lender: lock.lender.clone(),
            loan_ref: lock.loan_ref.clone(),
            operator: caller.clone(),
            previous_owner: receipt.owner.clone(),
            buyer: buyer.clone(),
            sale_ref: sale_ref.clone(),
            started_at: recovery.started_at,
            completed_at: now,
            settlement_ref: None,
            settled_at: None,
        };

        recovery.resolution = RecoveryResolution::Completed;
        recovery.resolved_at = Some(now);
        recovery.sale_id = Some(sale_id);
        recovery.buyer = Some(buyer.clone());
        recovery.sale_ref = Some(sale_ref.clone());

        receipt.recovery_history.push(recovery);
        receipt.owner = buyer.clone();
        receipt.status = Status::Active;
        receipt.lock = None;
        receipt.recovery = None;

        self.receipts.insert(receipt_id, receipt);
        self.sales.insert(sale_id, sale);

        emit_event(
            "recovery_completed",
            &json!({
                "receipt_id": receipt_id.to_string(),
                "sale_id": sale_id.to_string(),
                "lender": lock.lender.to_string(),
                "buyer": buyer.to_string(),
                "operator": caller.to_string(),
                "sale_ref": sale_ref,
                "completed_at": now.to_string(),
            }),
        );

        sale_id.to_string()
    }

    pub fn internal_record_settlement(&mut self, sale_id: u64, settlement_ref: String) {
        let caller = env::predecessor_account_id();
        if !self.recovery_operators.contains(&caller) {
            ContractError::RecoveryOperatorNotRegistered.panic();
        }

        let mut sale = self
            .sales
            .get(&sale_id)
            .cloned()
            .unwrap_or_else(|| ContractError::SaleNotFound.panic());

        if settlement_ref.len() > MAX_REF_LEN {
            ContractError::RefTooLong.panic();
        }
        if sale.settlement_ref.is_some() {
            ContractError::RecoveryAlreadySettled.panic();
        }

        let now = now_seconds();
        sale.settlement_ref = Some(settlement_ref.clone());
        sale.settled_at = Some(now);

        let lender = sale.lender.clone();
        self.sales.insert(sale_id, sale);

        emit_event(
            "settlement_recorded",
            &json!({
                "sale_id": sale_id.to_string(),
                "lender": lender.to_string(),
                "operator": caller.to_string(),
                "settlement_ref": settlement_ref,
                "settled_at": now.to_string(),
            }),
        );
    }

    pub fn internal_request_exit(&mut self, receipt_id: u64, kg: u64, expires_at: u64) {
        let caller = env::predecessor_account_id();
        let receipt = self
            .receipts
            .get(&receipt_id)
            .cloned()
            .unwrap_or_else(|| ContractError::ReceiptNotFound.panic());

        if receipt.owner != caller {
            ContractError::Unauthorized.panic();
        }
        if receipt.status != Status::Active {
            ContractError::InvalidStatus.panic();
        }
        if kg == 0 || kg > receipt.weight_kg {
            ContractError::InvalidSplitAmount.panic();
        }
        let now = now_seconds();
        if expires_at <= now {
            ContractError::PendingExpired.panic();
        }

        self.pending_exits.insert(
            receipt_id,
            PendingExit {
                receipt_id,
                owner: caller,
                kg,
                expires_at,
            },
        );
    }

    pub fn internal_exit(&mut self, receipt_id: u64, kg: u64, exit_weight_kg: u64) -> String {
        let caller = env::predecessor_account_id();
        let receipt = self
            .receipts
            .get(&receipt_id)
            .cloned()
            .unwrap_or_else(|| ContractError::ReceiptNotFound.panic());

        if !self.custodians.contains_key(&(receipt.warehouse_id.clone(), caller.clone())) {
            ContractError::CustodianNotRegistered.panic();
        }
        if receipt.status != Status::Active {
            ContractError::InvalidStatus.panic();
        }

        let pending = self
            .pending_exits
            .get(&receipt_id)
            .cloned()
            .unwrap_or_else(|| ContractError::PendingNotFound.panic());

        let now = now_seconds();
        if now > pending.expires_at {
            ContractError::PendingExpired.panic();
        }
        if pending.kg != kg {
            ContractError::TermsMismatch.panic();
        }
        if exit_weight_kg == 0 {
            ContractError::InvalidExitWeight.panic();
        }

        self.pending_exits.remove(&receipt_id);

        let exited_receipt = if kg < receipt.weight_kg {
            let parts = self
                .internal_split_receipt(&receipt, kg)
                .unwrap_or_else(|e| e.panic());
            let mut taken = parts.taken;
            taken.status = Status::Exited;
            taken.exit_date = Some(now);
            taken.exit_weight_kg = Some(exit_weight_kg);
            self.receipts.insert(taken.receipt_id, taken.clone());
            taken
        } else {
            let mut exiting = receipt;
            exiting.status = Status::Exited;
            exiting.exit_date = Some(now);
            exiting.exit_weight_kg = Some(exit_weight_kg);
            self.receipts.insert(exiting.receipt_id, exiting.clone());
            exiting
        };

        emit_event(
            "exited",
            &json!({
                "receipt_id": exited_receipt.receipt_id.to_string(),
                "warehouse_id": exited_receipt.warehouse_id,
                "weight_kg": exited_receipt.weight_kg.to_string(),
                "exit_weight_kg": exit_weight_kg.to_string(),
                "exited_at": now.to_string(),
            }),
        );

        exited_receipt.receipt_id.to_string()
    }

    pub fn internal_revoke(&mut self, receipt_id: u64) {
        self.assert_admin_with_deposit();
        let mut receipt = self
            .receipts
            .get(&receipt_id)
            .cloned()
            .unwrap_or_else(|| ContractError::ReceiptNotFound.panic());

        if receipt.status != Status::Active {
            ContractError::InvalidStatus.panic();
        }

        receipt.status = Status::Revoked;
        let warehouse = receipt.warehouse_id.clone();
        self.receipts.insert(receipt_id, receipt);

        emit_event(
            "revoked",
            &json!({
                "receipt_id": receipt_id.to_string(),
                "warehouse_id": warehouse,
                "revoked_at": now_seconds().to_string(),
            }),
        );
    }
}
