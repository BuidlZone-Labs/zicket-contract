pub use privacy_utils::PrivacyLevel;
use soroban_sdk::{contracttype, Address, BytesN, Symbol};

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
    // Identity fields — only one set is populated, based on privacy_level:
    pub payer: Option<Address>,                   // Standard only
    pub hashed_wallet: Option<BytesN<32>>,        // Private only
    pub stealth_delivery_key: Option<BytesN<32>>, // Private only
    pub nullifier_commitment: Option<BytesN<32>>, // Anonymous only
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
    pub owner: Option<Address>,                   // Standard only
    pub hashed_owner: Option<BytesN<32>>,         // Private only
    pub nullifier_commitment: Option<BytesN<32>>, // Anonymous only
    pub payment_id: u64,
    pub privacy_level: PaymentPrivacy,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawalRecord {
    pub amount: i128,
    pub timestamp: u64,
    pub organizer: Address,
}
