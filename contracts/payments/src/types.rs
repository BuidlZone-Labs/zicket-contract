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
    pub payer: Address,
    pub amount: i128,
    pub token: Address,
    pub status: PaymentStatus,
    pub paid_at: u64,
    pub privacy_level: PaymentPrivacy,
    pub refunded_amount: i128,
    /
    /
    /
    /
    pub zk_email_commitment: Option<BytesN<32>>,
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

/// A single revenue-split recipient and its allocation in basis points.
///
/// The public configuration is expressed as `Vec<(Address, u32)>` (per the
/// feature spec), but it is normalised into this struct for storage and event
/// emission so the fields are self-describing.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevenueSplit {
    pub recipient: Address,
    pub basis_points: u32,
}

/// Snapshot taken the first time any recipient settles a split event.
///
/// `net_distributable` is the amount left for the recipients *after* the
/// platform fee has been deducted and the (cancellation) withdrawable ratio has
/// been applied. It is frozen so every recipient's share is computed against the
/// same base regardless of the order in which they withdraw.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitSettlement {
    pub token: Address,
    pub net_distributable: i128,
}

/// How a primary organizer's dispute over a flagged co-host wallet is resolved.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlagResolution {
    /// Clear the flag and let the recipient withdraw its share normally.
    ReleaseToRecipient = 0,
    /// Send the escrowed share to the primary organizer instead.
    ReassignToPrimary = 1,
}
