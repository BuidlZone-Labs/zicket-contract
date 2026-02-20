use soroban_sdk::contracterror;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracterror]
pub enum TicketError {
    TicketNotFound = 1,
    TicketAlreadyUsed = 2,
    Unauthorized = 3,
    InvalidEvent = 4,
}
