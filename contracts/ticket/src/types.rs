use soroban_sdk::{contracttype, Address, Symbol};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum TicketStatus {
    Valid,
    Used,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct Ticket {
    pub ticket_id: u64,
    pub event_id: Symbol,
    pub owner: Address,
    pub issued_at: u64,
    pub status: TicketStatus,
}
