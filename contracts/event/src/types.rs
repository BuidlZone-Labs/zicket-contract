use soroban_sdk::{contracttype, Address, String, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventStatus {
    Upcoming = 0,
    Active = 1,
    Completed = 2,
    Cancelled = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub event_id: Symbol,
    pub organizer: Address,
    pub name: String,
    pub description: String,
    pub venue: String,
    pub event_date: u64,
    pub total_tickets: u32,
    pub tickets_sold: u32,
    pub ticket_price: i128,
    pub status: EventStatus,
    pub created_at: u64,
}
