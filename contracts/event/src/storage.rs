use crate::errors::EventError;
use crate::types::{Event, PrivacyLevel};
use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

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
