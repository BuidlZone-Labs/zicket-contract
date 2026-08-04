use crate::errors::EventError;
use crate::types::{
    AnonClaimSettings, AnonWindowState, ClaimSettings, Event, PostponementInfo, PrivacyLevel,
    ZkClaimType, ZkVerificationConfig,
};
use soroban_sdk::{contracttype, Address, BytesN, Env, Symbol, Vec};

const CURRENT_VERSION: u32 = 1;
/// TTL refresh threshold in ledgers (~30 days at 5s/ledger).
const TTL_THRESHOLD: u32 = 518_400;
/// TTL extension target in ledgers (~60 days at 5s/ledger), well within the
/// network maximum of 3,110,400 ledgers.
const TTL_BUMP: u32 = 1_036_800;
/// Reservations expire after 15 minutes; keep their entries on a ~1h/2h
/// ledger-based schedule so they outlive the reservation window.
const RESERVATION_TTL_THRESHOLD: u32 = 720;
const RESERVATION_TTL_BUMP: u32 = 1_440;

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
    FreeClaimCount(Symbol, Address),
    LastFreeClaim(Symbol, Address),
    EventClaimSettings(Symbol),
    Postponement(Symbol),
    PostponeCount(Symbol),
    AnonCommitment(Symbol, Address, BytesN<32>),
    /// Pre-per-claimant-scoping commitment key, kept read-only for replay
    /// protection against entries written before this key shape changed.
    LegacyAnonCommitment(Symbol, BytesN<32>),
    EventAnonWindow(Symbol),
    EventAnonSettings(Symbol),
    AnonymousClaimVerifier,
    AnonymousNullifier(Symbol, BytesN<32>),
    ZkNullifier(Symbol, BytesN<32>),
    ZkVerificationConfig(Symbol),
    EventAttendeeIndex(Symbol, u64),
    EventAttendeesCount(Symbol),
}
pub fn event_exists(env: &Env, event_id: &Symbol) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Event(event_id.clone()))
}
pub fn get_event(env: &Env, event_id: &Symbol) -> Result<Event, EventError> {
    let key = DataKey::Event(event_id.clone());
    let event = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(EventError::EventNotFound)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    Ok(event)
}
pub fn save_event(env: &Env, event_id: &Symbol, event: &Event) {
    let key = DataKey::Event(event_id.clone());
    env.storage().persistent().set(&key, event);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}
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

pub fn get_attendees_count(env: &Env, event_id: &Symbol) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::EventAttendeesCount(event_id.clone()))
        .unwrap_or(0)
}

pub fn save_registration(env: &Env, event_id: &Symbol, attendee: &Address) {
    let key = DataKey::Registration(event_id.clone(), attendee.clone());
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);

    let count = get_attendees_count(env, event_id);
    let idx_key = DataKey::EventAttendeeIndex(event_id.clone(), count);
    env.storage().persistent().set(&idx_key, attendee);
    env.storage()
        .persistent()
        .extend_ttl(&idx_key, TTL_THRESHOLD, TTL_BUMP);

    let count_key = DataKey::EventAttendeesCount(event_id.clone());
    env.storage().persistent().set(&count_key, &(count + 1));
    env.storage()
        .persistent()
        .extend_ttl(&count_key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_attendees(env: &Env, event_id: &Symbol) -> Vec<Address> {
    let count = get_attendees_count(env, event_id);
    let mut attendees = Vec::new(env);
    for i in 0..count {
        if let Some(attendee) = env
            .storage()
            .persistent()
            .get(&DataKey::EventAttendeeIndex(event_id.clone(), i))
        {
            attendees.push_back(attendee);
        }
    }
    attendees
}

pub fn get_attendees_paginated(
    env: &Env,
    event_id: &Symbol,
    start: u64,
    limit: u64,
) -> Vec<Address> {
    let count = get_attendees_count(env, event_id);
    let mut attendees = Vec::new(env);
    let actual_limit = limit.min(100);
    let end = count.min(start.saturating_add(actual_limit));
    for i in start..end {
        if let Some(attendee) = env
            .storage()
            .persistent()
            .get(&DataKey::EventAttendeeIndex(event_id.clone(), i))
        {
            attendees.push_back(attendee);
        }
    }
    attendees
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().persistent().set(&DataKey::Admin, admin);
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::Admin, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_admin(env: &Env) -> Result<Address, EventError> {
    let key = DataKey::Admin;
    let admin = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(EventError::ContractLinksNotConfigured)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    Ok(admin)
}

pub fn set_ticket_contract(env: &Env, ticket_contract: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::TicketContract, ticket_contract);
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::TicketContract, TTL_THRESHOLD, TTL_BUMP);
}

pub fn set_payments_contract(env: &Env, payments_contract: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::PaymentsContract, payments_contract);
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::PaymentsContract, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_ticket_contract(env: &Env) -> Result<Address, EventError> {
    let key = DataKey::TicketContract;
    let address = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(EventError::ContractLinksNotConfigured)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    Ok(address)
}

pub fn get_payments_contract(env: &Env) -> Result<Address, EventError> {
    let key = DataKey::PaymentsContract;
    let address = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(EventError::ContractLinksNotConfigured)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    Ok(address)
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
        .extend_ttl(&key, RESERVATION_TTL_THRESHOLD, RESERVATION_TTL_BUMP);
}

pub fn get_reservation(
    env: &Env,
    event_id: &Symbol,
    attendee: &Address,
) -> Result<crate::types::Reservation, EventError> {
    let key = DataKey::Reservation(event_id.clone(), attendee.clone());
    let reservation = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(EventError::ReservationNotFound)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, RESERVATION_TTL_THRESHOLD, RESERVATION_TTL_BUMP);
    Ok(reservation)
}

pub fn remove_reservation(env: &Env, event_id: &Symbol, attendee: &Address) {
    let key = DataKey::Reservation(event_id.clone(), attendee.clone());
    env.storage().persistent().remove(&key);
}
pub fn get_contract_version(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::ContractVersion)
        .unwrap_or(1)
}
pub fn set_contract_version(env: &Env, version: u32) {
    env.storage()
        .persistent()
        .set(&DataKey::ContractVersion, &version);
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::ContractVersion, TTL_THRESHOLD, TTL_BUMP);
}
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
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_event_privacy(env: &Env, event_id: &Symbol) -> PrivacyLevel {
    let key = DataKey::EventPrivacy(event_id.clone());
    let privacy: Option<PrivacyLevel> = env.storage().persistent().get(&key);
    // Only extend the TTL when the key already exists: extend_ttl on a missing
    // key panics with Error(Storage, MissingValue).
    if privacy.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    }
    privacy.unwrap_or(PrivacyLevel::Standard)
}

pub fn has_reservation(env: &Env, event_id: &Symbol, attendee: &Address) -> bool {
    let key = DataKey::Reservation(event_id.clone(), attendee.clone());
    env.storage().persistent().has(&key)
}
pub fn set_postponement(env: &Env, event_id: &Symbol, info: &PostponementInfo) {
    let key = DataKey::Postponement(event_id.clone());
    env.storage().persistent().set(&key, info);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}
pub fn get_postponement(env: &Env, event_id: &Symbol) -> Option<PostponementInfo> {
    let key = DataKey::Postponement(event_id.clone());
    let info = env.storage().persistent().get(&key);
    if info.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    }
    info
}
pub fn remove_postponement(env: &Env, event_id: &Symbol) {
    env.storage()
        .persistent()
        .remove(&DataKey::Postponement(event_id.clone()));
}
pub fn get_postpone_count(env: &Env, event_id: &Symbol) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::PostponeCount(event_id.clone()))
        .unwrap_or(0u32)
}
pub fn set_postpone_count(env: &Env, event_id: &Symbol, count: u32) {
    let key = DataKey::PostponeCount(event_id.clone());
    env.storage().persistent().set(&key, &count);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}
pub fn remove_registration(env: &Env, event_id: &Symbol, attendee: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::Registration(event_id.clone(), attendee.clone()));

    let count = get_attendees_count(env, event_id);
    if count == 0 {
        return;
    }

    let mut target_index = None;
    for i in 0..count {
        if let Some(a) = env
            .storage()
            .persistent()
            .get::<_, Address>(&DataKey::EventAttendeeIndex(event_id.clone(), i))
        {
            if a == *attendee {
                target_index = Some(i);
                break;
            }
        }
    }

    if let Some(idx) = target_index {
        let last_idx = count - 1;
        if idx != last_idx {
            if let Some(last_attendee) = env
                .storage()
                .persistent()
                .get::<_, Address>(&DataKey::EventAttendeeIndex(event_id.clone(), last_idx))
            {
                let target_key = DataKey::EventAttendeeIndex(event_id.clone(), idx);
                env.storage().persistent().set(&target_key, &last_attendee);
                env.storage()
                    .persistent()
                    .extend_ttl(&target_key, TTL_THRESHOLD, TTL_BUMP);
            }
        }
        env.storage()
            .persistent()
            .remove(&DataKey::EventAttendeeIndex(event_id.clone(), last_idx));

        let count_key = DataKey::EventAttendeesCount(event_id.clone());
        env.storage().persistent().set(&count_key, &last_idx);
        env.storage()
            .persistent()
            .extend_ttl(&count_key, TTL_THRESHOLD, TTL_BUMP);
    }
}

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

pub fn has_anon_commitment(
    env: &Env,
    event_id: &Symbol,
    claimant: &Address,
    commitment: &BytesN<32>,
) -> bool {
    let key = DataKey::AnonCommitment(event_id.clone(), claimant.clone(), commitment.clone());
    let exists = env.storage().persistent().has(&key);
    if exists {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
        return true;
    }

    // Fall back to the pre-per-claimant-scoping key so commitments saved
    // before this schema change still count as reused.
    let legacy_key = DataKey::LegacyAnonCommitment(event_id.clone(), commitment.clone());
    let legacy_exists = env.storage().persistent().has(&legacy_key);
    if legacy_exists {
        env.storage()
            .persistent()
            .extend_ttl(&legacy_key, TTL_THRESHOLD, TTL_BUMP);
    }
    legacy_exists
}

pub fn save_anon_commitment(
    env: &Env,
    event_id: &Symbol,
    claimant: &Address,
    commitment: &BytesN<32>,
) {
    let key = DataKey::AnonCommitment(event_id.clone(), claimant.clone(), commitment.clone());
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn set_anonymous_claim_verifier(env: &Env, verifier: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::AnonymousClaimVerifier, verifier);
    env.storage().persistent().extend_ttl(
        &DataKey::AnonymousClaimVerifier,
        TTL_THRESHOLD,
        TTL_BUMP,
    );
}

pub fn get_anonymous_claim_verifier(env: &Env) -> Result<Address, EventError> {
    let key = DataKey::AnonymousClaimVerifier;
    let verifier = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(EventError::AnonymousClaimVerifierNotConfigured)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    Ok(verifier)
}

pub fn get_anonymous_ticket_commitment(
    env: &Env,
    event_id: &Symbol,
    nullifier: &BytesN<32>,
) -> Option<BytesN<32>> {
    let key = DataKey::AnonymousNullifier(event_id.clone(), nullifier.clone());
    let commitment = env.storage().persistent().get(&key);
    if commitment.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    }
    commitment
}

pub fn save_anonymous_ticket_commitment(
    env: &Env,
    event_id: &Symbol,
    nullifier: &BytesN<32>,
    commitment: &BytesN<32>,
) {
    let key = DataKey::AnonymousNullifier(event_id.clone(), nullifier.clone());
    env.storage().persistent().set(&key, commitment);
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
pub fn save_zk_nullifier(env: &Env, event_id: &Symbol, nullifier: &BytesN<32>) {
    let key = DataKey::ZkNullifier(event_id.clone(), nullifier.clone());
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}
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
pub fn set_zk_verification_config(env: &Env, event_id: &Symbol, config: &ZkVerificationConfig) {
    let key = DataKey::ZkVerificationConfig(event_id.clone());
    env.storage().persistent().set(&key, config);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}
