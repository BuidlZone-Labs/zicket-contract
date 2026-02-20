#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol};

mod errors;
mod events;
mod storage;
mod types;

pub use errors::*;
pub use storage::*;
pub use types::*;

use events::{emit_event_cancelled, emit_event_created, emit_event_updated, emit_status_changed};

#[contract]
pub struct EventContract;

#[contractimpl]
impl EventContract {
    /// Create a new event. The organizer must authorize the transaction.
    pub fn create_event(env: Env, params: CreateEventParams) -> Result<Event, EventError> {
        // Require organizer authorization
        params.organizer.require_auth();

        // Validate name and venue are not empty
        if params.name.is_empty() {
            return Err(EventError::InvalidInput);
        }
        if params.venue.is_empty() {
            return Err(EventError::InvalidInput);
        }

        // Validate event date is at least 24 hours in the future
        let min_date = env.ledger().timestamp() + 86_400; // 24 hours in seconds
        if params.event_date <= min_date {
            return Err(EventError::InvalidEventDate);
        }

        // Validate ticket count: must be > 0 and < 100,000
        if params.total_tickets == 0 || params.total_tickets >= 100_000 {
            return Err(EventError::InvalidTicketCount);
        }

        // Validate ticket price: must be >= 0
        if params.ticket_price < 0 {
            return Err(EventError::InvalidPrice);
        }

        // Check that event doesn't already exist
        if event_exists(&env, &params.event_id) {
            return Err(EventError::EventAlreadyExists);
        }

        let event = Event {
            event_id: params.event_id.clone(),
            organizer: params.organizer.clone(),
            name: params.name.clone(),
            description: params.description.clone(),
            venue: params.venue.clone(),
            event_date: params.event_date,
            total_tickets: params.total_tickets,
            tickets_sold: 0,
            ticket_price: params.ticket_price,
            status: EventStatus::Upcoming,
            created_at: env.ledger().timestamp(),
        };

        save_event(&env, &params.event_id, &event);
        emit_event_created(&env, &params);

        Ok(event)
    }

    /// Retrieve an event by its ID.
    pub fn get_event(env: Env, event_id: Symbol) -> Result<Event, EventError> {
        storage::get_event(&env, &event_id)
    }

    /// Get the status of an event.
    pub fn get_event_status(env: Env, event_id: Symbol) -> Result<EventStatus, EventError> {
        let event = storage::get_event(&env, &event_id)?;
        Ok(event.status)
    }

    /// Update event details. Only the organizer can do this, and only for Upcoming events.
    pub fn update_event_details(env: Env, params: UpdateEventParams) -> Result<Event, EventError> {
        params.organizer.require_auth();

        let mut event = storage::get_event(&env, &params.event_id)?;

        // Verify caller is the event organizer
        if event.organizer != params.organizer {
            return Err(EventError::Unauthorized);
        }

        // Verify event status is Upcoming
        if event.status != EventStatus::Upcoming {
            return Err(EventError::EventNotUpdatable);
        }

        // Update fields if provided
        if let Some(n) = params.name {
            if n.is_empty() {
                return Err(EventError::InvalidInput);
            }
            event.name = n;
        }
        if let Some(d) = params.description {
            event.description = d;
        }
        if let Some(v) = params.venue {
            if v.is_empty() {
                return Err(EventError::InvalidInput);
            }
            event.venue = v;
        }
        if let Some(date) = params.event_date {
            let min_date = env.ledger().timestamp() + 86_400; // 24 hours in seconds
            if date <= min_date {
                return Err(EventError::InvalidEventDate);
            }
            event.event_date = date;
        }
        if let Some(price) = params.ticket_price {
            if price < 0 {
                return Err(EventError::InvalidPrice);
            }
            event.ticket_price = price;
        }

        save_event(&env, &params.event_id, &event);
        emit_event_updated(&env, &event);

        Ok(event)
    }

    /// Update the status of an event. Only the organizer can do this.
    /// Valid transitions: Upcoming -> Active, Active -> Completed.
    pub fn update_event_status(
        env: Env,
        organizer: Address,
        event_id: Symbol,
        new_status: EventStatus,
    ) -> Result<(), EventError> {
        organizer.require_auth();

        let mut event = storage::get_event(&env, &event_id)?;

        // Verify caller is the event organizer
        if event.organizer != organizer {
            return Err(EventError::Unauthorized);
        }

        // Validate status transitions
        let valid_transition = matches!(
            (&event.status, &new_status),
            (EventStatus::Upcoming, EventStatus::Active)
                | (EventStatus::Active, EventStatus::Completed)
        );

        if !valid_transition {
            return Err(EventError::InvalidStatusTransition);
        }

        let old_status = event.status.clone();
        event.status = new_status.clone();

        update_event(&env, &event_id, &event)?;
        emit_status_changed(&env, &event_id, &old_status, &new_status);

        Ok(())
    }

    /// Cancel an event. Only the organizer can cancel.
    /// Cannot cancel an already completed event.
    pub fn cancel_event(env: Env, organizer: Address, event_id: Symbol) -> Result<(), EventError> {
        organizer.require_auth();

        let mut event = storage::get_event(&env, &event_id)?;

        // Verify caller is the event organizer
        if event.organizer != organizer {
            return Err(EventError::Unauthorized);
        }

        // Cannot cancel a completed or already cancelled event
        if matches!(
            event.status,
            EventStatus::Completed | EventStatus::Cancelled
        ) {
            return Err(EventError::InvalidStatusTransition);
        }

        let old_status = event.status.clone();
        event.status = EventStatus::Cancelled;

        update_event(&env, &event_id, &event)?;
        emit_status_changed(&env, &event_id, &old_status, &EventStatus::Cancelled);
        emit_event_cancelled(&env, &event_id);

        Ok(())
    }
}

#[cfg(test)]
mod test;
