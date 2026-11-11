use near_sdk::env;

/// Contract error codes matching Farmledge v3 specification (codes 10–58)
/// and NEAR-specific two-step intent codes (codes 70–77).
///
/// Panic strings follow the format: `E{code}:{Name}`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    // Receipts & lifecycle
    ReceiptNotFound = 10,
    DuplicateDepositId = 11,
    InvalidStatus = 12,
    InvalidWeight = 13,
    InvalidSplitAmount = 14,
    InvalidMoisture = 15,
    InvalidExitWeight = 16,
    CorruptRecord = 17,

    // Registries
    WarehouseNotRegistered = 20,
    CustodianNotRegistered = 21,
    VerifierNotRegistered = 22,
    RecoveryOperatorNotRegistered = 23,
    AlreadyRegistered = 24,
    NotRegistered = 25,

    // Lending
    OnlyScaleEligibleForLock = 30,
    DueDateNotLater = 31,
    RecoveryNotYetDue = 32,
    RecoveryAlreadySettled = 33,
    NotLocked = 34,
    InvalidDueDate = 35,
    LenderIsOwner = 36,
    SaleNotFound = 37,
    RefTooLong = 38,

    // Batch operations
    BatchTooLarge = 40,

    // Anchoring & minting
    WrongLeafKind = 50,
    StaleLeaf = 51,
    InvalidProof = 52,
    NoAnchor = 53,
    AnchorOutOfOrder = 54,
    AnchorInFuture = 55,
    InvalidLeafCount = 56,
    LeafHasReceipt = 57,
    UnsupportedLeafVersion = 58,

    // NEAR Two-Step Intents & Authorization (70–77)
    Unauthorized = 70,
    PendingNotFound = 71,
    PendingExpired = 72,
    PendingExists = 73,
    TermsMismatch = 74,
    MintWindowTooLong = 75,
    InvalidSymbol = 76,
    Overflow = 77,
}

impl ContractError {
    pub fn panic(&self) -> ! {
        env::panic_str(&format!("E{}:{:?}", *self as u32, self))
    }
}
