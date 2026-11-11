use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{near, AccountId};

pub const DATA_VERSION: u32 = 3;
pub const MAX_REF_LEN: usize = 64;
pub const MAX_TREE_DEPTH: usize = 20;
pub const MAX_LEAVES: u32 = 1 << MAX_TREE_DEPTH; // 1,048,576
pub const MAX_PRECLAIM_BATCH: usize = 25;
pub const MINT_WINDOW_SECONDS: u64 = 300;

/// Flexible u64 wrapper accepting both JSON string (`"100"`) and JSON number (`100`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
pub struct NumU64(pub u64);

impl<'de> Deserialize<'de> for NumU64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: near_sdk::serde::Deserializer<'de>,
    {
        struct V;
        impl<'de> near_sdk::serde::de::Visitor<'de> for V {
            type Value = NumU64;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a u64 integer or decimal string")
            }
            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
                Ok(NumU64(v))
            }
            fn visit_i64<E: near_sdk::serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                u64::try_from(v).map(NumU64).map_err(near_sdk::serde::de::Error::custom)
            }
            fn visit_str<E: near_sdk::serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                v.parse::<u64>().map(NumU64).map_err(near_sdk::serde::de::Error::custom)
            }
        }
        deserializer.deserialize_any(V)
    }
}

impl Serialize for NumU64 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: near_sdk::serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

#[cfg(feature = "abi")]
impl near_sdk::schemars::JsonSchema for NumU64 {
    fn schema_name() -> String {
        "NumU64".to_string()
    }
    fn json_schema(gen: &mut near_sdk::schemars::gen::SchemaGenerator) -> near_sdk::schemars::schema::Schema {
        <String>::json_schema(gen)
    }
}

impl From<u64> for NumU64 {
    fn from(v: u64) -> Self {
        NumU64(v)
    }
}

impl From<NumU64> for u64 {
    fn from(v: NumU64) -> Self {
        v.0
    }
}

/// How a receipt's weight was determined.
/// Discriminants match Borsh layout: BagEstimate = 0, Scale = 1.
#[near(serializers = [json, borsh])]
#[borsh(use_discriminant = true)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum WeightSource {
    BagEstimate = 0,
    Scale = 1,
}

/// Lifecycle status of a digital warehouse receipt.
#[near(serializers = [json, borsh])]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Active,
    Locked,
    Recovery,
    Exited,
    Revoked,
}

/// Collateral pledge on a receipt.
#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lock {
    pub lender: AccountId,
    pub loan_ref: String,
    pub due_date: u64,
    pub grace_period: u64,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryResolution {
    Open,
    Repaid,
    Completed,
}

/// One default recovery record.
#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recovery {
    pub lender: AccountId,
    pub loan_ref: String,
    pub owner: AccountId,
    pub started_at: u64,
    pub resolution: RecoveryResolution,
    pub resolved_at: Option<u64>,
    pub sale_id: Option<u64>,
    pub buyer: Option<AccountId>,
    pub sale_ref: Option<String>,
}

/// Canonical record of a completed recovery sale.
#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sale {
    pub data_version: u32,
    pub sale_id: u64,
    pub receipt_id: u64,
    pub warehouse_id: String,
    pub weight_kg: u64,
    pub lender: AccountId,
    pub loan_ref: String,
    pub operator: AccountId,
    pub previous_owner: AccountId,
    pub buyer: AccountId,
    pub sale_ref: String,
    pub started_at: u64,
    pub completed_at: u64,
    pub settlement_ref: Option<String>,
    pub settled_at: Option<u64>,
}

/// Stored Digital Warehouse Receipt.
#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    pub data_version: u32,
    pub receipt_id: u64,
    pub deposit_id: String,
    pub deposit_no: u64,
    pub parent_id: Option<u64>,
    pub commodity: String,
    pub grade: String,
    pub weight_kg: u64,
    pub weight_source: WeightSource,
    pub bag_count: u32,
    pub moisture_pct: u32,
    pub warehouse_id: String,
    pub custodian: AccountId,
    pub verifier: AccountId,
    pub owner: AccountId,
    pub status: Status,
    pub lock: Option<Lock>,
    pub recovery: Option<Recovery>,
    pub recovery_history: Vec<Recovery>,
    pub deposit_date: u64,
    pub exit_date: Option<u64>,
    pub exit_weight_kg: Option<u64>,
}

/// Anchored daily root and head state.
#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DayRoot {
    pub day: u32,
    pub root: String,
    pub leaves: u32,
    pub prev_head: String,
    pub head: String,
    pub custodian: AccountId,
    pub anchored_at: u64,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnchorHead {
    pub day: u32,
    pub head: String,
}

/// One leaf of a warehouse's daily Merkle tree as provided by backend / farmer.
#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeafInput {
    pub version: u32,
    pub kind: u8,
    pub warehouse_id: String,
    pub day: u32,
    pub index: u32,
    pub deposit_no: u64,
    pub deposit_id: String,
    pub receipt_id: u64,
    pub receipt_version: u32,
    pub owner: String,
    pub commodity: String,
    pub grade: String,
    pub kg: i128,
    pub weight_source: WeightSource,
    pub bags: u32,
    pub moisture_bps: u32,
    pub custodian: String,
    pub verifier: String,
    pub deposited_at: u64,
    pub at: u64,
    pub salt: String, // 64 hex chars
}

/// Two-step intent: Pending mint proposal.
#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingMint {
    pub leaf: LeafInput,
    pub leaf_hash: String,
    pub custodian: AccountId,
    pub not_after: u64,
}

/// Two-step intent: Pending lock offer.
#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingLockOffer {
    pub receipt_id: u64,
    pub lender: AccountId,
    pub amount_kg: u64,
    pub loan_ref: String,
    pub due_date: u64,
    pub grace_period_seconds: u64,
    pub expires_at: u64,
}

/// Two-step intent: Pending exit request.
#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingExit {
    pub receipt_id: u64,
    pub owner: AccountId,
    pub kg: u64,
    pub expires_at: u64,
}

/// Two-step intent: Pending recovery sale approval.
#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingSale {
    pub receipt_id: u64,
    pub lender: AccountId,
    pub buyer: AccountId,
    pub sale_ref: String,
    pub expires_at: u64,
}

// ============================================================================
// Public View Structs (compatible with NearChainClient JSON serialization)
// ============================================================================

#[near(serializers = [json])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptView {
    pub receipt_id: String,
    pub parent_id: Option<String>,
    pub status: Status,
    pub weight_kg: String,
    pub bag_count: u32,
    pub owner: String,
    pub warehouse_id: String,
    pub deposit_no: String,
    pub deposit_id: String,
    pub commodity: String,
    pub grade: String,
    pub moisture_pct: u32,
    pub weight_source: WeightSource,
    pub custodian: String,
    pub verifier: String,
    pub deposit_date: u64,
    pub exit_date: Option<u64>,
    pub exit_weight_kg: Option<String>,
    pub lender: Option<String>,
}

impl From<&Receipt> for ReceiptView {
    fn from(r: &Receipt) -> Self {
        Self {
            receipt_id: r.receipt_id.to_string(),
            parent_id: r.parent_id.map(|id| id.to_string()),
            status: r.status,
            weight_kg: r.weight_kg.to_string(),
            bag_count: r.bag_count,
            owner: r.owner.to_string(),
            warehouse_id: r.warehouse_id.clone(),
            deposit_no: r.deposit_no.to_string(),
            deposit_id: r.deposit_id.clone(),
            commodity: r.commodity.clone(),
            grade: r.grade.clone(),
            moisture_pct: r.moisture_pct,
            weight_source: r.weight_source,
            custodian: r.custodian.to_string(),
            verifier: r.verifier.to_string(),
            deposit_date: r.deposit_date,
            exit_date: r.exit_date,
            exit_weight_kg: r.exit_weight_kg.map(|w| w.to_string()),
            lender: r.lock.as_ref().map(|l| l.lender.to_string()),
        }
    }
}

#[near(serializers = [json])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnchorView {
    pub day: u32,
    pub head: String,
}

#[near(serializers = [json])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DayRootView {
    pub day: u32,
    pub root: String,
    pub leaves: u32,
    pub prev_head: String,
    pub head: String,
    pub custodian: String,
    pub anchored_at: u64,
}

#[near(serializers = [json])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockView {
    pub lender: String,
    pub loan_ref: String,
    pub due_date: u64,
    pub grace_period: u64,
}

#[near(serializers = [json])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaleView {
    pub data_version: u32,
    pub sale_id: String,
    pub receipt_id: String,
    pub warehouse_id: String,
    pub weight_kg: String,
    pub lender: String,
    pub loan_ref: String,
    pub operator: String,
    pub previous_owner: String,
    pub buyer: String,
    pub sale_ref: String,
    pub started_at: u64,
    pub completed_at: u64,
    pub settlement_ref: Option<String>,
    pub settled_at: Option<u64>,
}

#[near(serializers = [json])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryView {
    pub lender: String,
    pub loan_ref: String,
    pub owner: String,
    pub started_at: u64,
    pub resolution: RecoveryResolution,
    pub resolved_at: Option<u64>,
    pub sale_id: Option<String>,
    pub buyer: Option<String>,
    pub sale_ref: Option<String>,
}
