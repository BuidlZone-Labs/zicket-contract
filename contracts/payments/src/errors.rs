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
    /// Token transfer via the Soroban token interface failed unexpectedly.
    TransferFailed = 32,
    PostponementWindowClosed = 33,
    EventNotPostponed = 34,
    /// Revenue split configuration is invalid (bad sum, too many recipients,
    /// duplicate or empty recipient, or an attempt to mutate an existing config).
    InvalidSplitConfig = 35,
    /// No revenue split has been configured for this event.
    SplitsNotConfigured = 36,
    /// The caller is not one of the configured split recipients.
    NotASplitRecipient = 37,
    /// This recipient has already withdrawn (or had reassigned) its split share.
    SplitAlreadyWithdrawn = 38,
    /// The recipient's share is frozen because the wallet has been flagged.
    RecipientFlagged = 39,
    /// The recipient is not currently flagged.
    RecipientNotFlagged = 40,
    /// A zkEmail commitment is already bound to this payment; commitments are
    /// write-once and cannot be overwritten.
    CommitmentAlreadySet = 41,
    /// The payment is in a state that no longer accepts a commitment
    /// (e.g. it has been refunded).
    CommitmentNotAllowed = 42,
}
