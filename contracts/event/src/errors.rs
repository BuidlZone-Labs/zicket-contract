use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EventError {
    EventNotFound = 1,
    EventAlreadyExists = 2,
    InvalidStatusTransition = 3,
    Unauthorized = 4,
    InvalidInput = 5,
    EventNotActive = 6,
}
