use soroban_sdk::{contracttype, Address, BytesN, Env, Symbol, Vec};

use crate::errors::TicketError;
use crate::types::Ticket;

const TTL_THRESHOLD: u32 = 60 * 60 * 24 * 30;
const TTL_BUMP: u32 = 60 * 60 * 24 * 30 * 2;
#[allow(dead_code)]
const CURRENT_VERSION: u32 = 1;

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum DataKey {
    Ticket(u64),
    /// Map-based: Individual owner-ticket relationship
    OwnerTicket(Address, u64),
    /// Map-based: Individual event-ticket relationship
    EventTicket(Symbol, u64),
    NextTicketId,
    ContractVersion,
    Admin,
    RecoveryKey(u64),
    PaymentsContract,
}

pub fn get_ticket(env: &Env, ticket_id: u64) -> Result<Ticket, TicketError> {
    env.storage()
        .persistent()
        .get(&DataKey::Ticket(ticket_id))
        .ok_or(TicketError::TicketNotFound)
}

pub fn update_ticket(env: &Env, ticket: &Ticket) {
    env.storage()
        .persistent()
        .set(&DataKey::Ticket(ticket.ticket_id), ticket);
}

/// Add an owner-ticket relationship (map-based)
pub fn add_owner_ticket(env: &Env, owner: &Address, ticket_id: u64) {
    env.storage()
        .persistent()
        .set(&DataKey::OwnerTicket(owner.clone(), ticket_id), &true);
    env.storage().persistent().extend_ttl(
        &DataKey::OwnerTicket(owner.clone(), ticket_id),
        TTL_THRESHOLD,
        TTL_BUMP,
    );
}

/// Check if an owner has a specific ticket (map-based lookup)
#[allow(dead_code)]
pub fn has_owner_ticket(env: &Env, owner: &Address, ticket_id: u64) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::OwnerTicket(owner.clone(), ticket_id))
}

/// Remove an owner-ticket relationship (map-based)
pub fn remove_owner_ticket(env: &Env, owner: &Address, ticket_id: u64) {
    env.storage()
        .persistent()
        .remove(&DataKey::OwnerTicket(owner.clone(), ticket_id));
}

/// Add an event-ticket relationship (map-based)
pub fn add_event_ticket(env: &Env, event_id: &Symbol, ticket_id: u64) {
    env.storage()
        .persistent()
        .set(&DataKey::EventTicket(event_id.clone(), ticket_id), &true);
    env.storage().persistent().extend_ttl(
        &DataKey::EventTicket(event_id.clone(), ticket_id),
        TTL_THRESHOLD,
        TTL_BUMP,
    );
}

/// Check if an event has a specific ticket (map-based lookup)
#[allow(dead_code)]
pub fn has_event_ticket(env: &Env, event_id: &Symbol, ticket_id: u64) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::EventTicket(event_id.clone(), ticket_id))
}

/// Get all tickets by owner
/// Note: This returns an empty vector since map-based storage requires full scanning
/// which is expensive and should be avoided in production.
/// Consider tracking tickets in a separate collection if listing is required.
pub fn get_tickets_by_owner(env: &Env, _owner: Address) -> Vec<u64> {
    Vec::new(env)
}

/// Get all tickets by event
/// Note: This returns an empty vector since map-based storage requires full scanning
/// which is expensive and should be avoided in production.
/// Consider tracking tickets in a separate collection if listing is required.
pub fn get_tickets_by_event(env: &Env, _event_id: Symbol) -> Vec<u64> {
    Vec::new(env)
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
#[allow(dead_code)]
pub fn verify_version(env: &Env) -> Result<(), TicketError> {
    let version = get_contract_version(env);
    if version > CURRENT_VERSION {
        return Err(TicketError::UnsupportedVersion);
    }
    Ok(())
}

pub fn get_recovery_key(env: &Env, ticket_id: u64) -> Option<BytesN<32>> {
    env.storage()
        .persistent()
        .get(&DataKey::RecoveryKey(ticket_id))
}

pub fn set_recovery_key(env: &Env, ticket_id: u64, public_key: &BytesN<32>) {
    env.storage()
        .persistent()
        .set(&DataKey::RecoveryKey(ticket_id), public_key);
}

pub fn remove_recovery_key(env: &Env, ticket_id: u64) {
    env.storage()
        .persistent()
        .remove(&DataKey::RecoveryKey(ticket_id));
}

pub fn get_payments_contract(env: &Env) -> Result<Address, TicketError> {
    env.storage()
        .persistent()
        .get(&DataKey::PaymentsContract)
        .ok_or(TicketError::Unauthorized) // or specific error if available
}

pub fn set_payments_contract(env: &Env, payments_contract: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::PaymentsContract, payments_contract);
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::PaymentsContract, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_admin(env: &Env) -> Result<Address, TicketError> {
    env.storage()
        .persistent()
        .get(&DataKey::Admin)
        .ok_or(TicketError::Unauthorized)
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().persistent().set(&DataKey::Admin, admin);
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::Admin, TTL_THRESHOLD, TTL_BUMP);
}
