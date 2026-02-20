#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, String, Symbol};

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
    pub fn create_event(
        env: Env,
        organizer: Address,
        event_id: Symbol,
        name: String,
        description: String,
        venue: String,
        event_date: u64,
        total_tickets: u32,
        ticket_price: i128,
    ) -> Result<Event, EventError> {
        // Require organizer authorization
        organizer.require_auth();

        // Validate name and venue are not empty
        if name.len() == 0 {
            return Err(EventError::InvalidInput);
        }
        if venue.len() == 0 {
            return Err(EventError::InvalidInput);
        }

        // Validate event date is at least 24 hours in the future
        let min_date = env.ledger().timestamp() + 86_400; // 24 hours in seconds
        if event_date <= min_date {
            return Err(EventError::InvalidEventDate);
        }

        // Validate ticket count: must be > 0 and < 100,000
        if total_tickets == 0 || total_tickets >= 100_000 {
            return Err(EventError::InvalidTicketCount);
        }

        // Validate ticket price: must be >= 0
        if ticket_price < 0 {
            return Err(EventError::InvalidPrice);
        }

        // Check that event doesn't already exist
        if event_exists(&env, &event_id) {
            return Err(EventError::EventAlreadyExists);
        }

        let event = Event {
            event_id: event_id.clone(),
            organizer: organizer.clone(),
            name: name.clone(),
            description,
            venue: venue.clone(),
            event_date,
            total_tickets,
            tickets_sold: 0,
            ticket_price,
            status: EventStatus::Upcoming,
            created_at: env.ledger().timestamp(),
        };

        save_event(&env, &event_id, &event);
        emit_event_created(
            &env,
            &event_id,
            &organizer,
            &name,
            &venue,
            event_date,
            total_tickets,
            ticket_price,
        );

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
    pub fn update_event_details(
        env: Env,
        organizer: Address,
        event_id: Symbol,
        name: Option<String>,
        description: Option<String>,
        venue: Option<String>,
        event_date: Option<u64>,
        ticket_price: Option<i128>,
    ) -> Result<Event, EventError> {
        organizer.require_auth();

        let mut event = storage::get_event(&env, &event_id)?;

        // Verify caller is the event organizer
        if event.organizer != organizer {
            return Err(EventError::Unauthorized);
        }

        // Verify event status is Upcoming
        if event.status != EventStatus::Upcoming {
            return Err(EventError::EventNotUpdatable);
        }

        // Update fields if provided
        if let Some(n) = name {
            if n.len() == 0 {
                return Err(EventError::InvalidInput);
            }
            event.name = n;
        }
        if let Some(d) = description {
            event.description = d;
        }
        if let Some(v) = venue {
            if v.len() == 0 {
                return Err(EventError::InvalidInput);
            }
            event.venue = v;
        }
        if let Some(date) = event_date {
            let min_date = env.ledger().timestamp() + 86_400; // 24 hours in seconds
            if date <= min_date {
                return Err(EventError::InvalidEventDate);
            }
            event.event_date = date;
        }
        if let Some(price) = ticket_price {
            if price < 0 {
                return Err(EventError::InvalidPrice);
            }
            event.ticket_price = price;
        }

        save_event(&env, &event_id, &event);
        emit_event_updated(
            &env,
            &event_id,
            &event.name,
            &event.description,
            &event.venue,
            event.event_date,
            event.ticket_price,
        );

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

mod test;
