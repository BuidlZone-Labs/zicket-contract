pub use privacy_utils::{mask_address, MaskedAddress, PrivacyLevel};
use soroban_sdk::{contracttype, Address, Bytes, BytesN, String, Symbol, Vec};

// ── zkPassport types ──────────────────────────────────────────────────────────

/// The category of identity claim being asserted by a zkPassport proof.
///
/// - `Age`         – Proves the attendee is above a minimum age threshold.
/// - `Location`    – Proves the attendee's issuing country / region.
/// - `Citizenship` – Proves the attendee holds citizenship in an accepted nation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ZkClaimType {
    Age = 0,
    Location = 1,
    Citizenship = 2,
}

/// A zero-knowledge passport claim submitted by an attendee for gated event
/// registration.
///
/// # Privacy guarantees
/// - `proof` bytes are **never persisted on-chain**; they are consumed transiently
///   during the `verify_and_attend` call and immediately discarded.
/// - Only the `nullifier` (a cryptographic commitment derived from the proof) is
///   stored, which prevents reuse without revealing the underlying identity.
///
/// # Fields
/// - `claim_type`    – Which identity property is being asserted.
/// - `proof`         – Raw ZK proof bytes (validated off-chain by a relayer).
/// - `nullifier`     – A 32-byte commitment that uniquely identifies this proof
///                     without revealing the attendee's identity.
/// - `expiry_ledger` – The ledger sequence number after which this proof is
///                     considered stale and must be rejected.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ZkPassportClaim {
    pub claim_type: ZkClaimType,
    /// Raw proof bytes — accepted as input but NEVER written to storage.
    pub proof: Bytes,
    /// Unique per-proof commitment stored on-chain to prevent reuse.
    pub nullifier: BytesN<32>,
    /// Ledger sequence at or before which this proof is valid.
    pub expiry_ledger: u32,
}

/// Organizer-level configuration for zkPassport-gated attendance.
///
/// - `required_claim_type`: which proof category attendees must present.
///   `None` means the event accepts any valid ZK claim type.
/// - `enabled`: master switch; when `false` the `verify_and_attend` path is
///   disabled even if `requires_verification` is set on the event.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ZkVerificationConfig {
    /// If `Some`, only claims of this type are accepted.
    pub required_claim_type: Option<ZkClaimType>,
    /// When `false`, `verify_and_attend` will reject all calls.
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

/// Active window data backing the [`EventStatus::Postponed`] state, matching the
/// issue's `Postponed { new_date_ledger, choice_deadline_ledger }` specification.
///
/// Stored under its own persistent key per event (see `storage::set_postponement`)
/// and **removed when the event resumes to `Active`**, so getters never expose
/// stale window data. The cumulative anti-abuse counter that bounds how many times
/// an event may be postponed (see `MAX_POSTPONEMENTS`) is tracked separately under
/// `DataKey::PostponeCount` so it survives across successive postponements.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostponementInfo {
    /// Ledger sequence at which the rescheduled event is set to start.
    pub new_date_ledger: u64,
    /// Ledger sequence after which the refund-choice window closes and the event
    /// can be finalized back to `Active`.
    pub choice_deadline_ledger: u64,
}

/// Per-event configuration controlling free-ticket claim abuse prevention.
///
/// - `max_free_claims`: max number of free tickets a single wallet may claim for this event.
///   0 means unlimited (default).
/// - `cooldown_secs`: minimum seconds between consecutive free claims from the same wallet.
///   0 means no cooldown (default).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimSettings {
    pub max_free_claims: u32,
    pub cooldown_secs: u64,
}

/// Per-event rate-limit configuration for the truly anonymous (no-wallet) free-claim path.
///
/// - `max_anon_claims_per_window`: max anonymous claims allowed within one ledger window.
///   0 means no window-based limit.
/// - `anon_window_size`: size of the rate-limiting window in ledgers.
///   0 means no window-based limit applies.
///
/// Both fields must be > 0 for the window rate limit to be enforced.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnonClaimSettings {
    pub max_anon_claims_per_window: u32,
    pub anon_window_size: u32,
}

/// Tracks anonymous claim counts within the current ledger window for a single event.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnonWindowState {
    pub window_index: u32,
    pub count: u32,
}
