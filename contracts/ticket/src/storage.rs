use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

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
    OwnerTickets(Address),
    EventTickets(Symbol),
    NextTicketId,
    ContractVersion,
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

pub fn get_tickets_by_owner(env: &Env, owner: Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::OwnerTickets(owner))
        .unwrap_or(Vec::new(env))
}

pub fn get_tickets_by_event(env: &Env, event_id: Symbol) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::EventTickets(event_id))
        .unwrap_or(Vec::new(env))
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
#[allow(dead_code)]
pub fn verify_version(env: &Env) -> Result<(), TicketError> {
    let version = get_contract_version(env);
    if version > CURRENT_VERSION {
        return Err(TicketError::UnsupportedVersion);
    }
    Ok(())
}
