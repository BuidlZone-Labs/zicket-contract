use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PaymentError {
    PaymentNotFound = 1,
    TicketNotFound = 2,
    InsufficientFunds = 3,
    Unauthorized = 4,
    PaymentAlreadyProcessed = 5,
    InvalidAmount = 6,
    RefundFailed = 7,
    NotInitialized = 8,
    PaymentAlreadyRefunded = 9,
    NoRevenue = 10,
    AnonymousPaymentsDisabled = 11,
    VerificationRequired = 12,
    UnauthorizedWithdrawal = 13,
    InvalidOrganizer = 14,
    InvalidPayoutToken = 15,
    EventNotActive = 16,
    EventNotCompleted = 17,
    RefundNotAllowed = 18,
    EscrowNotExpired = 19,
    EscrowAlreadyReleased = 20,
    EscrowNotConfigured = 21,
    AccountingMismatch = 22,
    InvalidFeeBps = 23,
    NoPlatformRevenue = 24,
    DuplicateRequest = 25,
    MigrationFailed = 26,
    UnsupportedVersion = 27,
    MaxTicketsReached = 28,
    EventSoldOut = 29,
    NonceRequired = 30,
    ContractPaused = 31,
    /// Token transfer failed; no state has been modified
    TransferFailed = 32,
    PostponementWindowClosed = 33,
    EventNotPostponed = 34,
    /// Anonymous payment is missing its required nullifier commitment
    MissingNullifierCommitment = 35,
    /// Private payment is missing its required stealth delivery key
    MissingStealthDeliveryKey = 36,
    /// The supplied privacy data does not match the declared privacy level
    PrivacyLevelMismatch = 37,
}
