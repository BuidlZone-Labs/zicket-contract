use crate::errors::EventError;
use crate::types::{
    AnonClaimSettings, AnonWindowState, ClaimSettings, Event, PostponementInfo, PrivacyLevel,
    ZkClaimType, ZkVerificationConfig,
};
use soroban_sdk::{contracttype, Address, BytesN, Env, Symbol, Vec};

const CURRENT_VERSION: u32 = 1;
const TTL_THRESHOLD: u32 = 60 * 60 * 24 * 30;
const TTL_BUMP: u32 = 60 * 60 * 24 * 30 * 2;

#[contracttype]
pub enum DataKey {
    Event(Symbol),
    Registration(Symbol, Address),
    EventAttendees(Symbol),
    Reservation(Symbol, Address),
    Admin,
    TicketContract,
    PaymentsContract,
    EventPrivacy(Symbol),
    ContractVersion,
    /// Number of free tickets claimed by a wallet for a specific event.
    FreeClaimCount(Symbol, Address),
    /// Timestamp of the wallet's most recent free claim for a specific event.
    LastFreeClaim(Symbol, Address),
    /// Organizer-configured sybil-protection settings for a specific event.
    EventClaimSettings(Symbol),
    /// Active postponement window (new date, choice deadline) for a specific event.
    /// Present only while the event is `Postponed`.
    Postponement(Symbol),
    /// Cumulative number of times an event has been postponed (anti-abuse counter).
    /// Persists across postponements, independent of the active window record.
    PostponeCount(Symbol),
    /// Marks a commitment as used for the anonymous free-claim path.
    AnonCommitment(Symbol, BytesN<32>),
    /// Rolling window state (index + count) for the anonymous rate limiter.
    EventAnonWindow(Symbol),
    /// Organizer-configured rate-limit settings for the anonymous free-claim path.
    EventAnonSettings(Symbol),
    /// Marks a zkPassport nullifier as spent for a specific event.
    /// The nullifier is stored; the proof bytes are NEVER stored.
    ZkNullifier(Symbol, BytesN<32>),
    /// Organizer-configured zkPassport verification settings for an event.
    ZkVerificationConfig(Symbol),
}

/// Check if an event exists in storage.
pub fn event_exists(env: &Env, event_id: &Symbol) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Event(event_id.clone()))
}

/// Retrieve an event from storage, returning an error if not found.
pub fn get_event(env: &Env, event_id: &Symbol) -> Result<Event, EventError> {
    env.storage()
        .persistent()
        .get(&DataKey::Event(event_id.clone()))
        .ok_or(EventError::EventNotFound)
}

/// Save a new event to persistent storage with TTL extension.
pub fn save_event(env: &Env, event_id: &Symbol, event: &Event) {
    let key = DataKey::Event(event_id.clone());
    env.storage().persistent().set(&key, event);
    env.storage().persistent().extend_ttl(
        &key,
        60 * 60 * 24 * 30,     // ~30 days threshold
        60 * 60 * 24 * 30 * 2, // ~60 days max
    );
}

/// Update an existing event in storage. Returns error if event doesn't exist.
pub fn update_event(env: &Env, event_id: &Symbol, event: &Event) -> Result<(), EventError> {
    if !event_exists(env, event_id) {
        return Err(EventError::EventNotFound);
    }
    save_event(env, event_id, event);
    Ok(())
}

pub fn is_registered(env: &Env, event_id: &Symbol, attendee: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Registration(event_id.clone(), attendee.clone()))
}

pub fn save_registration(env: &Env, event_id: &Symbol, attendee: &Address) {
    let key = DataKey::Registration(event_id.clone(), attendee.clone());
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);

    let attendees_key = DataKey::EventAttendees(event_id.clone());
    let mut attendees: Vec<Address> = env
        .storage()
        .persistent()
        .get(&attendees_key)
        .unwrap_or(Vec::new(env));
    attendees.push_back(attendee.clone());
    env.storage().persistent().set(&attendees_key, &attendees);
    env.storage()
        .persistent()
        .extend_ttl(&attendees_key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
}

pub fn get_attendees(env: &Env, event_id: &Symbol) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::EventAttendees(event_id.clone()))
        .unwrap_or(Vec::new(env))
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().persistent().set(&DataKey::Admin, admin);
    env.storage().persistent().extend_ttl(
        &DataKey::Admin,
        60 * 60 * 24 * 30,
        60 * 60 * 24 * 30 * 2,
    );
}

pub fn get_admin(env: &Env) -> Result<Address, EventError> {
    env.storage()
        .persistent()
        .get(&DataKey::Admin)
        .ok_or(EventError::ContractLinksNotConfigured)
}

pub fn set_ticket_contract(env: &Env, ticket_contract: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::TicketContract, ticket_contract);
}

pub fn set_payments_contract(env: &Env, payments_contract: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::PaymentsContract, payments_contract);
}

pub fn get_ticket_contract(env: &Env) -> Result<Address, EventError> {
    env.storage()
        .persistent()
        .get(&DataKey::TicketContract)
        .ok_or(EventError::ContractLinksNotConfigured)
}

pub fn get_payments_contract(env: &Env) -> Result<Address, EventError> {
    env.storage()
        .persistent()
        .get(&DataKey::PaymentsContract)
        .ok_or(EventError::ContractLinksNotConfigured)
}

pub fn has_linked_contracts(env: &Env) -> bool {
    env.storage().persistent().has(&DataKey::TicketContract)
        && env.storage().persistent().has(&DataKey::PaymentsContract)
}

pub fn save_reservation(
    env: &Env,
    event_id: &Symbol,
    attendee: &Address,
    reservation: &crate::types::Reservation,
) {
    let key = DataKey::Reservation(event_id.clone(), attendee.clone());
    env.storage().persistent().set(&key, reservation);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60, 60 * 60 * 2);
}

pub fn get_reservation(
    env: &Env,
    event_id: &Symbol,
    attendee: &Address,
) -> Result<crate::types::Reservation, EventError> {
    let key = DataKey::Reservation(event_id.clone(), attendee.clone());
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(EventError::ReservationNotFound)
}

pub fn remove_reservation(env: &Env, event_id: &Symbol, attendee: &Address) {
    let key = DataKey::Reservation(event_id.clone(), attendee.clone());
    env.storage().persistent().remove(&key);
}

/// Get the current contract version from storage.
pub fn get_contract_version(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::ContractVersion)
        .unwrap_or(1)
}

/// Set the contract version in storage.
pub fn set_contract_version(env: &Env, version: u32) {
    env.storage()
        .persistent()
        .set(&DataKey::ContractVersion, &version);
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::ContractVersion, TTL_THRESHOLD, TTL_BUMP);
}

/// Verify that the contract version is supported. Returns error if version is not compatible.
pub fn verify_version(env: &Env) -> Result<(), EventError> {
    let version = get_contract_version(env);
    if version > CURRENT_VERSION {
        return Err(EventError::UnsupportedVersion);
    }
    Ok(())
}

pub fn set_event_privacy(env: &Env, event_id: &Symbol, level: &PrivacyLevel) {
    let key = DataKey::EventPrivacy(event_id.clone());
    env.storage().persistent().set(&key, level);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
}

pub fn get_event_privacy(env: &Env, event_id: &Symbol) -> PrivacyLevel {
    env.storage()
        .persistent()
        .get(&DataKey::EventPrivacy(event_id.clone()))
        .unwrap_or(PrivacyLevel::Standard)
}

pub fn has_reservation(env: &Env, event_id: &Symbol, attendee: &Address) -> bool {
    let key = DataKey::Reservation(event_id.clone(), attendee.clone());
    env.storage().persistent().has(&key)
}

// ── Postponement helpers ──────────────────────────────────────────────────────

/// Persist the active postponement window for an event. Cleared on resume via
/// [`remove_postponement`].
pub fn set_postponement(env: &Env, event_id: &Symbol, info: &PostponementInfo) {
    let key = DataKey::Postponement(event_id.clone());
    env.storage().persistent().set(&key, info);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

/// Read the active postponement window, or `None` if the event is not postponed.
pub fn get_postponement(env: &Env, event_id: &Symbol) -> Option<PostponementInfo> {
    env.storage()
        .persistent()
        .get(&DataKey::Postponement(event_id.clone()))
}

/// Remove the active postponement window when an event resumes to `Active`.
pub fn remove_postponement(env: &Env, event_id: &Symbol) {
    env.storage()
        .persistent()
        .remove(&DataKey::Postponement(event_id.clone()));
}

/// Cumulative number of times an event has been postponed (0 if never).
pub fn get_postpone_count(env: &Env, event_id: &Symbol) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::PostponeCount(event_id.clone()))
        .unwrap_or(0u32)
}

/// Persist the cumulative postponement counter. Survives across postponements and
/// across the active-window record being cleared on resume.
pub fn set_postpone_count(env: &Env, event_id: &Symbol, count: u32) {
    let key = DataKey::PostponeCount(event_id.clone());
    env.storage().persistent().set(&key, &count);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

/// Remove an attendee's registration for an event (used when a postponement refund
/// revokes participation). Clears the registration flag and drops the attendee from
/// the public attendee list.
pub fn remove_registration(env: &Env, event_id: &Symbol, attendee: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::Registration(event_id.clone(), attendee.clone()));

    let attendees_key = DataKey::EventAttendees(event_id.clone());
    let attendees: Vec<Address> = env
        .storage()
        .persistent()
        .get(&attendees_key)
        .unwrap_or(Vec::new(env));
    let mut remaining = Vec::new(env);
    for a in attendees.iter() {
        if a != *attendee {
            remaining.push_back(a);
        }
    }
    env.storage().persistent().set(&attendees_key, &remaining);
    env.storage()
        .persistent()
        .extend_ttl(&attendees_key, TTL_THRESHOLD, TTL_BUMP);
}

// ── Sybil-resistance helpers ──────────────────────────────────────────────────

pub fn get_claim_settings(env: &Env, event_id: &Symbol) -> ClaimSettings {
    env.storage()
        .persistent()
        .get(&DataKey::EventClaimSettings(event_id.clone()))
        .unwrap_or(ClaimSettings {
            max_free_claims: 0,
            cooldown_secs: 0,
        })
}

pub fn set_claim_settings(env: &Env, event_id: &Symbol, settings: &ClaimSettings) {
    let key = DataKey::EventClaimSettings(event_id.clone());
    env.storage().persistent().set(&key, settings);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_free_claim_count(env: &Env, event_id: &Symbol, attendee: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::FreeClaimCount(event_id.clone(), attendee.clone()))
        .unwrap_or(0u32)
}

pub fn increment_free_claim_count(env: &Env, event_id: &Symbol, attendee: &Address) {
    let key = DataKey::FreeClaimCount(event_id.clone(), attendee.clone());
    let count: u32 = env.storage().persistent().get(&key).unwrap_or(0u32);
    env.storage().persistent().set(&key, &(count + 1));
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_last_free_claim(env: &Env, event_id: &Symbol, attendee: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::LastFreeClaim(event_id.clone(), attendee.clone()))
        .unwrap_or(0u64)
}

pub fn set_last_free_claim(env: &Env, event_id: &Symbol, attendee: &Address, timestamp: u64) {
    let key = DataKey::LastFreeClaim(event_id.clone(), attendee.clone());
    env.storage().persistent().set(&key, &timestamp);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

// ── Anonymous free-claim helpers ──────────────────────────────────────────────

pub fn has_anon_commitment(env: &Env, event_id: &Symbol, commitment: &BytesN<32>) -> bool {
    let key = DataKey::AnonCommitment(event_id.clone(), commitment.clone());
    let exists = env.storage().persistent().has(&key);
    if exists {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    }
    exists
}

pub fn save_anon_commitment(env: &Env, event_id: &Symbol, commitment: &BytesN<32>) {
    let key = DataKey::AnonCommitment(event_id.clone(), commitment.clone());
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_anon_claim_settings(env: &Env, event_id: &Symbol) -> AnonClaimSettings {
    let key = DataKey::EventAnonSettings(event_id.clone());
    let settings: Option<AnonClaimSettings> = env.storage().persistent().get(&key);
    match settings {
        Some(s) => {
            env.storage()
                .persistent()
                .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
            s
        }
        None => AnonClaimSettings {
            max_anon_claims_per_window: 0,
            anon_window_size: 0,
        },
    }
}

pub fn set_anon_claim_settings(env: &Env, event_id: &Symbol, settings: &AnonClaimSettings) {
    let key = DataKey::EventAnonSettings(event_id.clone());
    env.storage().persistent().set(&key, settings);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_anon_window_state(env: &Env, event_id: &Symbol) -> AnonWindowState {
    let key = DataKey::EventAnonWindow(event_id.clone());
    let state: Option<AnonWindowState> = env.storage().persistent().get(&key);
    match state {
        Some(s) => {
            env.storage()
                .persistent()
                .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
            s
        }
        None => AnonWindowState {
            window_index: 0,
            count: 0,
        },
    }
}

pub fn set_anon_window_state(env: &Env, event_id: &Symbol, state: &AnonWindowState) {
    let key = DataKey::EventAnonWindow(event_id.clone());
    env.storage().persistent().set(&key, state);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

// ── zkPassport helpers ────────────────────────────────────────────────────────

/// Returns `true` if the given nullifier has already been recorded for this
/// event, meaning the associated proof has been consumed and must not be
/// accepted again.
pub fn has_zk_nullifier(env: &Env, event_id: &Symbol, nullifier: &BytesN<32>) -> bool {
    let key = DataKey::ZkNullifier(event_id.clone(), nullifier.clone());
    let exists = env.storage().persistent().has(&key);
    if exists {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    }
    exists
}

/// Record a nullifier as spent for this event. Only the nullifier is stored;
/// the raw proof bytes that produced it are deliberately not passed here and
/// are never written to the ledger.
pub fn save_zk_nullifier(env: &Env, event_id: &Symbol, nullifier: &BytesN<32>) {
    let key = DataKey::ZkNullifier(event_id.clone(), nullifier.clone());
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

/// Retrieve the organizer-configured zkPassport verification settings for an
/// event. Defaults to `enabled: false` (no ZK gating) if never explicitly set.
pub fn get_zk_verification_config(env: &Env, event_id: &Symbol) -> ZkVerificationConfig {
    let key = DataKey::ZkVerificationConfig(event_id.clone());
    let cfg: Option<ZkVerificationConfig> = env.storage().persistent().get(&key);
    match cfg {
        Some(c) => {
            env.storage()
                .persistent()
                .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
            c
        }
        None => ZkVerificationConfig {
            required_claim_type: ZkClaimType::Any,
            enabled: false,
        },
    }
}

/// Persist the organizer-configured zkPassport verification settings for an
/// event.
pub fn set_zk_verification_config(env: &Env, event_id: &Symbol, config: &ZkVerificationConfig) {
    let key = DataKey::ZkVerificationConfig(event_id.clone());
    env.storage().persistent().set(&key, config);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}
