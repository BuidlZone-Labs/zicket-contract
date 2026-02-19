use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::types::EventStatus;

/// Publish a Soroban event when a new event is created.
pub fn emit_event_created(env: &Env, event_id: &Symbol, organizer: &Address) {
    env.events().publish(
        (symbol_short!("created"),),
        (event_id.clone(), organizer.clone()),
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
