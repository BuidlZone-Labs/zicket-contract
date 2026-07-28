use soroban_sdk::contracterror;

/// Payment contract error codes.
///
/// These errors follow common patterns defined in `common_utils::errors::CommonErrorCode`
/// where applicable, but are numbered to avoid conflicts and maintain backward compatibility.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PaymentError {
    PaymentNotFound = 1,            // CommonErrorCode::NotFound
    TicketNotFound = 2,             // CommonErrorCode::NotFound
    InsufficientFunds = 3,          // CommonErrorCode::InsufficientFunds
    Unauthorized = 4,               // CommonErrorCode::Unauthorized
    PaymentAlreadyProcessed = 5,    // CommonErrorCode::AlreadyProcessed
    InvalidAmount = 6,              // CommonErrorCode::InvalidAmount
    RefundFailed = 7,
    NotInitialized = 8,             // CommonErrorCode::NotInitialized
    PaymentAlreadyRefunded = 9,     // CommonErrorCode::AlreadyProcessed
    NoRevenue = 10,
    AnonymousPaymentsDisabled = 11,
    VerificationRequired = 12,
    UnauthorizedWithdrawal = 13,    // CommonErrorCode::Unauthorized
    InvalidOrganizer = 14,
    InvalidPayoutToken = 15,
    EventNotActive = 16,            // CommonErrorCode::NotActive
    EventNotCompleted = 17,         // CommonErrorCode::NotCompleted
    RefundNotAllowed = 18,
    EscrowNotExpired = 19,
    EscrowAlreadyReleased = 20,     // CommonErrorCode::AlreadyProcessed
    EscrowNotConfigured = 21,       // CommonErrorCode::NotConfigured
    AccountingMismatch = 22,        // CommonErrorCode::AccountingMismatch
    InvalidFeeBps = 23,             // CommonErrorCode::InvalidFeeBps
    NoPlatformRevenue = 24,
    DuplicateRequest = 25,
    MigrationFailed = 26,           // CommonErrorCode::MigrationFailed
    UnsupportedVersion = 27,        // CommonErrorCode::UnsupportedVersion
    MaxTicketsReached = 28,         // CommonErrorCode::MaxLimitReached
    EventSoldOut = 29,              // CommonErrorCode::SoldOut
    NonceRequired = 30,
    ContractPaused = 31,            // CommonErrorCode::ContractPaused
    /// Token transfer via the Soroban token interface failed unexpectedly.
    TransferFailed = 32,            // CommonErrorCode::TransferFailed
    PostponementWindowClosed = 33,
    EventNotPostponed = 34,
    /// Revenue split configuration is invalid (bad sum, too many recipients,
    /// duplicate or empty recipient, or an attempt to mutate an existing config).
    InvalidSplitConfig = 35,        // CommonErrorCode::InvalidInput
    /// No revenue split has been configured for this event.
    SplitsNotConfigured = 36,       // CommonErrorCode::NotConfigured
    /// The caller is not one of the configured split recipients.
    NotASplitRecipient = 37,
    /// This recipient has already withdrawn (or had reassigned) its split share.
    SplitAlreadyWithdrawn = 38,     // CommonErrorCode::AlreadyProcessed
    /// The recipient's share is frozen because the wallet has been flagged.
    RecipientFlagged = 39,
    /// The recipient is not currently flagged.
    RecipientNotFlagged = 40,
    /// A zkEmail commitment is already bound to this payment; commitments are
    /// write-once and cannot be overwritten.
    CommitmentAlreadySet = 41,      // CommonErrorCode::AlreadyProcessed
    /// The payment is in a state that no longer accepts a commitment
    /// (e.g. it has been refunded).
    CommitmentNotAllowed = 42,
    /// Anonymous payment is missing its required nullifier commitment
    MissingNullifierCommitment = 43,
    /// Private payment is missing its required stealth delivery key
    MissingStealthDeliveryKey = 44,
    /// The supplied privacy data does not match the declared privacy level
    PrivacyLevelMismatch = 45,
}
