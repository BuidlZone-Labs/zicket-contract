pub use privacy_utils::PrivacyLevel;
use soroban_sdk::{contracttype, Address, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisputeReason {
    EventNoShow = 0,
    TicketNotDelivered = 1,
    DescriptionMismatch = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisputeStatus {
    Open = 0,
    Reviewing = 1,
    ApprovedRefund = 2,
    Rejected = 3,
    TimedOut = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketDispute {
    pub ticket_id: u64,
    pub reason: DisputeReason,
    pub opened_at: u64,
    pub status: DisputeStatus,
    pub escrow_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventStatus {
    Upcoming = 0,
    Active = 1,
    Completed = 2,
    Cancelled = 3,
    Postponed = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaymentStatus {
    Held = 0,
    Released = 1,
    Refunded = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowMetadata {
    pub organizer: Address,
    pub event_end_time: u64,
    pub auto_released: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaymentPrivacy {
    Anonymous = 0,
    Private = 1,
    Standard = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentRecord {
    pub payment_id: u64,
    pub event_id: Symbol,
    pub payer: Address,
    pub amount: i128,
    pub token: Address,
    pub status: PaymentStatus,
    pub paid_at: u64,
    pub privacy_level: PaymentPrivacy,
    pub refunded_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ticket {
    pub ticket_id: u64,
    pub event_id: Symbol,
    pub owner: Address,
    pub payment_id: u64,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawalRecord {
    pub amount: i128,
    pub timestamp: u64,
    pub organizer: Address,
}
