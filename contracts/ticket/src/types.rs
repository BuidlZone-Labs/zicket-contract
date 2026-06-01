use soroban_sdk::{contracttype, Address, Symbol};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum TicketStatus {
    Valid,
    Used,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Ticket {
    pub ticket_id: u64,
    pub event_id: Symbol,
    pub organizer: Address,
    pub owner: Address,
    pub issued_at: u64,
    pub status: TicketStatus,
    pub is_transferable: bool,
    pub is_used: bool,
}
