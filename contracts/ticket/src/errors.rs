use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TicketError {
    TicketNotFound = 1,
    TicketAlreadyExists = 2,
    InvalidStatusTransition = 3,
    Unauthorized = 4,
    InvalidInput = 5,
    TicketNotActive = 6,
    InvalidTicketDate = 7,
    InvalidTicketCount = 8,
    InvalidPrice = 9,
    TicketNotUpdatable = 10,
    TicketNotTransferable = 11,
    TransferToSelf = 12,
    TicketAlreadyUsed = 13,
    EventNotActive = 14,
}
