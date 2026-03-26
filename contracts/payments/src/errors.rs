use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PaymentError {
    PaymentNotFound = 1,
    InsufficientFunds = 2,
    Unauthorized = 3,
    PaymentAlreadyProcessed = 4,
    InvalidAmount = 5,
    RefundFailed = 6,
    NotInitialized = 7,
    PaymentAlreadyRefunded = 8,
    TicketNotFound = 9,
    NoRevenue = 10,
    /// Token transfer failed; no state has been modified
    TransferFailed = 11,
}
