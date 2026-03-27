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
}
