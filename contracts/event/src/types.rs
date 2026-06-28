pub use privacy_utils::{mask_address, MaskedAddress, PrivacyLevel};
use soroban_sdk::{contracttype, Address, Bytes, BytesN, String, Symbol, Vec};
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ZkClaimType {
    Any = 0,
    Age = 1,
    Location = 2,
    Citizenship = 3,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ZkPassportClaim {
    pub claim_type: ZkClaimType,
    pub proof: Bytes,
    pub nullifier: BytesN<32>,
    pub expiry_ledger: u32,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ZkVerificationConfig {
    pub required_claim_type: ZkClaimType,
    pub enabled: bool,
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
pub struct TicketTier {
    pub tier_id: u32,
    pub name: String,
    pub price: i128,
    pub capacity: u32,
    pub sold: u32,
    pub reserved: u32,
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
    pub payout_token: Address,
    pub name: String,
    pub description: String,
    pub venue: String,
    pub event_date: u64,
    pub allow_anonymous: bool,
    pub requires_verification: bool,
    pub tiers: Vec<TicketTier>,
    pub status: EventStatus,
    pub created_at: u64,
    pub privacy_level: PrivacyLevel,
    pub max_tickets_per_user: u32,
    pub max_supply: u32,
    pub sold_count: u32,
    pub event_start_ledger: u32,
    pub event_end_ledger: u32,
    pub withdrawal_delay_ledgers: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateEventParams {
    pub organizer: Address,
    pub payout_token: Address,
    pub event_id: Symbol,
    pub name: String,
    pub description: String,
    pub venue: String,
    pub event_date: u64,
    pub initial_tiers: Vec<TicketTierParams>,
    pub allow_anonymous: bool,
    pub requires_verification: bool,
    pub privacy_level: PrivacyLevel,
    pub max_tickets_per_user: u32,
    pub event_start_ledger: u32,
    pub event_end_ledger: u32,
    pub withdrawal_delay_ledgers: u32,
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
    pub allow_anonymous: Option<bool>,
    pub requires_verification: Option<bool>,
    pub max_tickets_per_user: Option<u32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reservation {
    pub tier_id: u32,
    pub expires_at: u64,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostponementInfo {
    pub new_date_ledger: u64,
    pub choice_deadline_ledger: u64,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimSettings {
    pub max_free_claims: u32,
    pub cooldown_secs: u64,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnonClaimSettings {
    pub max_anon_claims_per_window: u32,
    pub anon_window_size: u32,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnonWindowState {
    pub window_index: u32,
    pub count: u32,
}
