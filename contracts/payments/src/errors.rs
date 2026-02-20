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
}
