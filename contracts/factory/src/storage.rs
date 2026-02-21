use crate::errors::FactoryError;
use crate::types::DeployedEvent;
use soroban_sdk::{contracttype, Address, BytesN, Env, Symbol, Vec};

const TTL_THRESHOLD: u32 = 60 * 60 * 24 * 30;
const TTL_BUMP: u32 = 60 * 60 * 24 * 30 * 2;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    EventWasm,
    DeployedEvent(Symbol),
    AllEvents,
    OrganizerEvents(Address),
}

pub fn is_initialized(env: &Env) -> bool {
    env.storage().persistent().has(&DataKey::Admin)
        && env.storage().persistent().has(&DataKey::EventWasm)
}

pub fn get_admin(env: &Env) -> Result<Address, FactoryError> {
    env.storage()
        .persistent()
        .get(&DataKey::Admin)
        .ok_or(FactoryError::NotInitialized)
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().persistent().set(&DataKey::Admin, admin);
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::Admin, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_event_wasm_hash(env: &Env) -> Result<BytesN<32>, FactoryError> {
    env.storage()
        .persistent()
        .get(&DataKey::EventWasm)
        .ok_or(FactoryError::NotInitialized)
}

pub fn set_event_wasm_hash(env: &Env, hash: &BytesN<32>) {
    env.storage().persistent().set(&DataKey::EventWasm, hash);
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::EventWasm, TTL_THRESHOLD, TTL_BUMP);
}

pub fn save_deployed_event(env: &Env, event: &DeployedEvent) -> Result<(), FactoryError> {
    let event_key = DataKey::DeployedEvent(event.event_id.clone());
    if env.storage().persistent().has(&event_key) {
        return Err(FactoryError::EventAlreadyDeployed);
    }

    env.storage().persistent().set(&event_key, event);
    env.storage()
        .persistent()
        .extend_ttl(&event_key, TTL_THRESHOLD, TTL_BUMP);

    let mut all_events = get_all_event_ids(env);
    all_events.push_back(event.event_id.clone());
    env.storage()
        .persistent()
        .set(&DataKey::AllEvents, &all_events);
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::AllEvents, TTL_THRESHOLD, TTL_BUMP);

    let organizer_key = DataKey::OrganizerEvents(event.organizer.clone());
    let mut organizer_events = get_organizer_events(env, &event.organizer);
    organizer_events.push_back(event.event_id.clone());
    env.storage()
        .persistent()
        .set(&organizer_key, &organizer_events);
    env.storage()
        .persistent()
        .extend_ttl(&organizer_key, TTL_THRESHOLD, TTL_BUMP);

    Ok(())
}

pub fn get_deployed_event(env: &Env, event_id: &Symbol) -> Result<DeployedEvent, FactoryError> {
    env.storage()
        .persistent()
        .get(&DataKey::DeployedEvent(event_id.clone()))
        .ok_or(FactoryError::EventNotFoundInRegistry)
}

pub fn get_all_event_ids(env: &Env) -> Vec<Symbol> {
    env.storage()
        .persistent()
        .get(&DataKey::AllEvents)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn get_organizer_events(env: &Env, organizer: &Address) -> Vec<Symbol> {
    env.storage()
        .persistent()
        .get(&DataKey::OrganizerEvents(organizer.clone()))
        .unwrap_or_else(|| Vec::new(env))
}
