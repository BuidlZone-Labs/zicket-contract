use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::types::{CreateEventParams, Event, EventStatus};

/// Publish a Soroban event when a new event is created.
/// Includes all relevant event data for frontend integration.
pub fn emit_event_created(env: &Env, params: &CreateEventParams) {
    env.events().publish(
        (symbol_short!("created"),),
        (
            params.event_id.clone(),
            params.organizer.clone(),
            params.name.clone(),
            params.venue.clone(),
            params.event_date,
            params.total_tickets,
            params.ticket_price,
        ),
    );
}

/// Publish a Soroban event when event details are updated.
pub fn emit_event_updated(env: &Env, event: &Event) {
    env.events().publish(
        (symbol_short!("updated"),),
        (
            event.event_id.clone(),
            event.name.clone(),
            event.description.clone(),
            event.venue.clone(),
            event.event_date,
            event.ticket_price,
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

pub fn emit_registration(env: &Env, event_id: &Symbol, attendee: &Address, tickets_sold: u32) {
    env.events().publish(
        (symbol_short!("register"),),
        (event_id.clone(), attendee.clone(), tickets_sold),
    );
}
