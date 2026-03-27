use soroban_sdk::{contractevent, Address, Env, Symbol};

use crate::types::{
    mask_address, CreateEventParams, Event, EventStatus, MaskedAddress, PrivacyLevel,
};

#[contractevent(data_format = "vec", topics = ["created"])]
pub struct EventCreated {
    pub event_id: Symbol,
    pub organizer: MaskedAddress,
    pub name: soroban_sdk::String,
    pub venue: soroban_sdk::String,
    pub event_date: u64,
    pub tier_count: u32,
    pub created_at: u64,
}

#[contractevent(data_format = "vec", topics = ["updated"])]
pub struct EventUpdated {
    pub event_id: Symbol,
    pub name: soroban_sdk::String,
    pub description: soroban_sdk::String,
    pub venue: soroban_sdk::String,
    pub event_date: u64,
    pub updated_at: u64,
}

#[contractevent(data_format = "vec", topics = ["status"])]
pub struct EventStatusChanged {
    pub event_id: Symbol,
    pub old_status: EventStatus,
    pub new_status: EventStatus,
    pub changed_at: u64,
}

#[contractevent(data_format = "vec", topics = ["ev_cnc"])]
pub struct EventCancelled {
    pub event_id: Symbol,
    pub organizer: Address,
    pub cancelled_at: u64,
}

#[contractevent(data_format = "vec", topics = ["refs_prc"])]
pub struct RefundsProcessed {
    pub event_id: Symbol,
    pub refund_count: u32,
    pub processed_at: u64,
}

#[contractevent(data_format = "vec", topics = ["register"])]
pub struct EventRegistration {
    pub event_id: Symbol,
    pub attendee: MaskedAddress,
    pub tier_id: u32,
    pub tickets_sold: u32,
    pub registered_at: u64,
}

/// Publish a Soroban event when a new event is created.
/// The organizer address is masked according to the event's privacy level.
pub fn emit_event_created(env: &Env, params: &CreateEventParams, level: &PrivacyLevel) {
    EventCreated {
        event_id: params.event_id.clone(),
        organizer: mask_address(env, &params.organizer, level.clone()),
        name: params.name.clone(),
        venue: params.venue.clone(),
        event_date: params.event_date,
        tier_count: params.initial_tiers.len(),
        created_at: env.ledger().timestamp(),
    }
    .publish(env);
}

/// Publish a Soroban event when event details are updated.
pub fn emit_event_updated(env: &Env, event: &Event) {
    EventUpdated {
        event_id: event.event_id.clone(),
        name: event.name.clone(),
        description: event.description.clone(),
        venue: event.venue.clone(),
        event_date: event.event_date,
        updated_at: env.ledger().timestamp(),
    }
    .publish(env);
}

/// Publish a Soroban event when an event status changes.
pub fn emit_status_changed(
    env: &Env,
    event_id: &Symbol,
    old_status: &EventStatus,
    new_status: &EventStatus,
) {
    EventStatusChanged {
        event_id: event_id.clone(),
        old_status: old_status.clone(),
        new_status: new_status.clone(),
        changed_at: env.ledger().timestamp(),
    }
    .publish(env);
}

/// Publish a Soroban event when an event is cancelled.
pub fn emit_event_cancelled(env: &Env, event_id: &Symbol, organizer: &Address) {
    EventCancelled {
        event_id: event_id.clone(),
        organizer: organizer.clone(),
        cancelled_at: env.ledger().timestamp(),
    }
    .publish(env);
}

pub fn emit_refunds_processed(env: &Env, event_id: &Symbol, refund_count: u32) {
    RefundsProcessed {
        event_id: event_id.clone(),
        refund_count,
        processed_at: env.ledger().timestamp(),
    }
    .publish(env);
}

/// Publish a Soroban event when an attendee registers.
/// The attendee address is masked according to the event's privacy level.
pub fn emit_registration(
    env: &Env,
    event_id: &Symbol,
    attendee: &Address,
    tier_id: u32,
    tickets_sold: u32,
    level: &PrivacyLevel,
) {
    EventRegistration {
        event_id: event_id.clone(),
        attendee: mask_address(env, attendee, level.clone()),
        tier_id,
        tickets_sold,
        registered_at: env.ledger().timestamp(),
    }
    .publish(env);
}
