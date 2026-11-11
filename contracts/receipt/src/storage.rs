use borsh::BorshSerialize;
use near_sdk::env;
use near_sdk::BorshStorageKey;

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
pub enum StorageKey {
    Receipts,
    Sales,
    Anchors,
    DayRoots,
    Claimed,
    PendingMints,
    PendingLocks,
    PendingExits,
    PendingSales,
    Warehouses,
    Custodians,
    Verifiers,
    RecoveryOperators,
}

/// Returns current block timestamp converted to Unix seconds.
pub fn now_seconds() -> u64 {
    env::block_timestamp() / 1_000_000_000
}

/// Returns current UTC day (days since Unix epoch).
pub fn current_utc_day() -> u32 {
    (now_seconds() / 86400) as u32
}
