use soroban_sdk::{symbol_short, Address, Env, String, Symbol};

use crate::types::EventStatus;

/// Publish a Soroban event when a new event is created.
/// Includes all relevant event data for frontend integration.
pub fn emit_event_created(
    env: &Env,
    event_id: &Symbol,
    organizer: &Address,
    name: &String,
    venue: &String,
    event_date: u64,
    total_tickets: u32,
    ticket_price: i128,
) {
    env.events().publish(
        (symbol_short!("created"),),
        (
            event_id.clone(),
            organizer.clone(),
            name.clone(),
            venue.clone(),
            event_date,
            total_tickets,
            ticket_price,
        ),
    );
}

/// Publish a Soroban event when an event status changes.
pub fn emit_status_changed(
    env: &Env,
    event_id: &Symbol,
    old_status: &EventStatus,
    new_status: &EventStatus,
) {
    env.events().publish(
        (symbol_short!("status"),),
        (event_id.clone(), old_status.clone(), new_status.clone()),
    );
}

/// Publish a Soroban event when an event is cancelled.
pub fn emit_event_cancelled(env: &Env, event_id: &Symbol) {
    env.events()
        .publish((symbol_short!("cancel"),), (event_id.clone(),));
}
