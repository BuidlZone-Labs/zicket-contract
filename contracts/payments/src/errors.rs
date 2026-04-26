use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PaymentError {
    PaymentNotFound = 1,
    TicketNotFound = 9,
    InsufficientFunds = 2,
    Unauthorized = 3,
    PaymentAlreadyProcessed = 4,
    InvalidAmount = 5,
    RefundFailed = 6,
    NotInitialized = 7,
    PaymentAlreadyRefunded = 8,
    NoRevenue = 10,
    InvalidFeeBps = 11,
    NoPlatformRevenue = 12,
    AnonymousPaymentsDisabled = 13,
    VerificationRequired = 14,
    UnauthorizedWithdrawal = 15,
    InvalidOrganizer = 16,
    InvalidPayoutToken = 17,
    EventNotActive = 18,
    EventNotCompleted = 19,
    RefundNotAllowed = 20,
    EscrowNotExpired = 21,
    EscrowAlreadyReleased = 22,
    EscrowNotConfigured = 23,
    DuplicateRequest = 24,
    MigrationFailed = 25,
    UnsupportedVersion = 26,
    MaxTicketsReached = 27,
    EventSoldOut = 28,
    NonceRequired = 29,
    ContractPaused = 30,
}
