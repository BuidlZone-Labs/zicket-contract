use soroban_sdk::{contracttype, Address, String, Symbol, Vec};

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
pub struct TicketTier {
    pub tier_id: u32,
    pub name: String,
    pub price: i128,
    pub capacity: u32,
    pub sold: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketTierParams {
    pub name: String,
    pub price: i128,
    pub capacity: u32,
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
    pub tiers: Vec<TicketTier>,
    pub status: EventStatus,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateEventParams {
    pub organizer: Address,
    pub event_id: Symbol,
    pub name: String,
    pub description: String,
    pub venue: String,
    pub event_date: u64,
    pub initial_tiers: Vec<TicketTierParams>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateEventParams {
    pub organizer: Address,
    pub event_id: Symbol,
    pub name: Option<String>,
    pub description: Option<String>,
    pub venue: Option<String>,
    pub event_date: Option<u64>,
}
