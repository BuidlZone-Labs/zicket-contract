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
    InvalidEventDate = 7,
    InvalidTicketCount = 8,
    InvalidPrice = 9,
    EventNotUpdatable = 10,
    EventSoldOut = 11,
    AlreadyRegistered = 12,
}
