#![no_std]
use payments_contract::{PaymentPrivacy, PaymentsContractClient};
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol};
use ticket_contract::TicketContractClient;

mod errors;
mod events;
mod storage;
mod types;

pub use errors::*;
pub use storage::*;
pub use types::*;

use events::{
    emit_event_cancelled, emit_event_created, emit_event_updated, emit_refunds_processed,
    emit_registration, emit_status_changed,
};

#[contract]
pub struct EventContract;

#[contractimpl]
impl EventContract {
    /// Link ticket and payments contracts used for registration flow.
    pub fn initialize(
        env: Env,
        admin: Address,
        ticket_contract: Address,
        payments_contract: Address,
    ) -> Result<(), EventError> {
        admin.require_auth();

        storage::set_admin(&env, &admin);
        storage::set_ticket_contract(&env, &ticket_contract);
        storage::set_payments_contract(&env, &payments_contract);

        Ok(())
    }

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

        // Validate there is at least 1 tier
        if params.initial_tiers.is_empty() {
            return Err(EventError::InvalidInput);
        }

        let mut tiers = soroban_sdk::Vec::new(&env);
        for (current_tier_id, tier_param) in params.initial_tiers.iter().enumerate() {
            if tier_param.name.is_empty() {
                return Err(EventError::InvalidInput);
            }
            if tier_param.capacity == 0 || tier_param.capacity >= 100_000 {
                return Err(EventError::InvalidTicketCount);
            }
            if tier_param.price < 0 {
                return Err(EventError::InvalidPrice);
            }

            tiers.push_back(TicketTier {
                tier_id: current_tier_id as u32,
                name: tier_param.name,
                price: tier_param.price,
                capacity: tier_param.capacity,
                sold: 0,
                reserved: 0,
            });
        }

        // Check that event doesn't already exist
        if event_exists(&env, &params.event_id) {
            return Err(EventError::EventAlreadyExists);
        }

        if has_linked_contracts(&env) {
            let payments_contract = get_payments_contract(&env)?;
            let payments_client = PaymentsContractClient::new(&env, &payments_contract);
            let accepted_token = payments_client.get_accepted_token();

            if params.payout_token != accepted_token {
                return Err(EventError::InvalidPayoutToken);
            }
        }

        let event = Event {
            event_id: params.event_id.clone(),
            organizer: params.organizer.clone(),
            payout_token: params.payout_token.clone(),
            name: params.name.clone(),
            description: params.description.clone(),
            venue: params.venue.clone(),
            event_date: params.event_date,
            allow_anonymous: params.allow_anonymous,
            requires_verification: params.requires_verification,
            tiers,
            status: EventStatus::Upcoming,
            created_at: env.ledger().timestamp(),
            privacy_level: params.privacy_level.clone(),
        };

        save_event(&env, &params.event_id, &event);
        if has_linked_contracts(&env) {
            let payments_contract = get_payments_contract(&env)?;
            let payments_client = PaymentsContractClient::new(&env, &payments_contract);
            payments_client.sync_event_config(
                &env.current_contract_address(),
                &params.event_id,
                &params.organizer,
                &params.payout_token,
                &params.allow_anonymous,
                &params.requires_verification,
            );
        }
        let privacy = storage::get_event_privacy(&env, &params.event_id);
        emit_event_created(&env, &params, &privacy);

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
        if let Some(allow_anonymous) = params.allow_anonymous {
            event.allow_anonymous = allow_anonymous;
        }
        if let Some(requires_verification) = params.requires_verification {
            event.requires_verification = requires_verification;
        }

        save_event(&env, &params.event_id, &event);
        if has_linked_contracts(&env) {
            let payments_contract = get_payments_contract(&env)?;
            let payments_client = PaymentsContractClient::new(&env, &payments_contract);
            payments_client.sync_event_config(
                &env.current_contract_address(),
                &params.event_id,
                &event.organizer,
                &event.payout_token,
                &event.allow_anonymous,
                &event.requires_verification,
            );
        }
        emit_event_updated(&env, &event);

        Ok(event)
    }

    pub fn get_allow_anonymous(env: Env, event_id: Symbol) -> bool {
        storage::get_event(&env, &event_id).unwrap().allow_anonymous
    }

    pub fn get_requires_verification(env: Env, event_id: Symbol) -> bool {
        storage::get_event(&env, &event_id)
            .unwrap()
            .requires_verification
    }

    /// Add a new ticket tier to an Upcoming event. Only the organizer can do this.
    pub fn add_ticket_tier(
        env: Env,
        organizer: Address,
        event_id: Symbol,
        name: soroban_sdk::String,
        price: i128,
        capacity: u32,
    ) -> Result<TicketTier, EventError> {
        organizer.require_auth();

        let mut event = storage::get_event(&env, &event_id)?;

        if event.organizer != organizer {
            return Err(EventError::Unauthorized);
        }

        if event.status != EventStatus::Upcoming {
            return Err(EventError::EventNotUpdatable);
        }

        if name.is_empty() {
            return Err(EventError::InvalidInput);
        }
        if capacity == 0 || capacity >= 100_000 {
            return Err(EventError::InvalidTicketCount);
        }
        if price < 0 {
            return Err(EventError::InvalidPrice);
        }

        let new_tier_id = event.tiers.len();
        let new_tier = TicketTier {
            tier_id: new_tier_id,
            name,
            price,
            capacity,
            sold: 0,
            reserved: 0,
        };

        event.tiers.push_back(new_tier.clone());

        save_event(&env, &event_id, &event);

        Ok(new_tier)
    }

    /// Update an existing ticket tier of an Upcoming event.
    pub fn update_tier(
        env: Env,
        organizer: Address,
        event_id: Symbol,
        tier_id: u32,
        name: Option<soroban_sdk::String>,
        price: Option<i128>,
        capacity: Option<u32>,
    ) -> Result<(), EventError> {
        organizer.require_auth();

        let mut event = storage::get_event(&env, &event_id)?;

        if event.organizer != organizer {
            return Err(EventError::Unauthorized);
        }

        if event.status != EventStatus::Upcoming {
            return Err(EventError::EventNotUpdatable);
        }

        let mut found = false;
        for i in 0..event.tiers.len() {
            let mut tier = event.tiers.get(i).ok_or(EventError::TierNotFound)?;
            if tier.tier_id == tier_id {
                if let Some(n) = name.clone() {
                    if n.is_empty() {
                        return Err(EventError::InvalidInput);
                    }
                    tier.name = n;
                }
                if let Some(p) = price {
                    if p < 0 {
                        return Err(EventError::InvalidPrice);
                    }
                    tier.price = p;
                }
                if let Some(c) = capacity {
                    if c == 0 || c >= 100_000 {
                        return Err(EventError::InvalidTicketCount);
                    }
                    tier.capacity = c;
                }
                event.tiers.set(i, tier);
                found = true;
                break;
            }
        }

        if !found {
            return Err(EventError::TierNotFound);
        }

        save_event(&env, &event_id, &event);
        Ok(())
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
        emit_event_cancelled(&env, &event_id, &organizer);

        // Process refunds if contracts are linked
        if has_linked_contracts(&env) {
            let admin = storage::get_admin(&env)?;
            let payments_contract = get_payments_contract(&env)?;
            let payments_client = PaymentsContractClient::new(&env, &payments_contract);

            let payment_ids = payments_client.get_event_payments(&event_id);
            let mut refund_count = 0;

            for payment_id in payment_ids.iter() {
                payments_client.refund(&admin, &payment_id);
                refund_count += 1;
            }

            emit_refunds_processed(&env, &event_id, refund_count);
        }

        Ok(())
    }

    /// Reserve a ticket for a specific tier. The reservation is valid for 15 minutes.
    pub fn reserve_ticket(
        env: Env,
        attendee: Address,
        event_id: Symbol,
        tier_id: u32,
    ) -> Result<(), EventError> {
        attendee.require_auth();

        let mut event = storage::get_event(&env, &event_id)?;

        if event.status != EventStatus::Active {
            return Err(EventError::EventNotActive);
        }

        if storage::is_registered(&env, &event_id, &attendee) {
            return Err(EventError::AlreadyRegistered);
        }

        // Check if user already has an active reservation
        if storage::has_reservation(&env, &event_id, &attendee) {
            let reservation = storage::get_reservation(&env, &event_id, &attendee)?;
            if reservation.expires_at > env.ledger().timestamp() {
                // Already has an active reservation
                return Ok(());
            } else {
                // Reservation expired, we'll replace it.
                // First decrement the old reserved count.
                let mut found = false;
                for i in 0..event.tiers.len() {
                    let mut tier = event.tiers.get(i).ok_or(EventError::TierNotFound)?;
                    if tier.tier_id == reservation.tier_id {
                        if tier.reserved > 0 {
                            tier.reserved -= 1;
                        }
                        event.tiers.set(i, tier);
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err(EventError::TierNotFound);
                }
            }
        }

        let mut tier_index = None;
        for i in 0..event.tiers.len() {
            let tier = event.tiers.get(i).ok_or(EventError::TierNotFound)?;
            if tier.tier_id == tier_id {
                tier_index = Some(i);
                break;
            }
        }

        let index = tier_index.ok_or(EventError::TierNotFound)?;
        let mut tier = event.tiers.get(index).ok_or(EventError::TierNotFound)?;

        if tier.sold + tier.reserved >= tier.capacity {
            return Err(EventError::TierSoldOut);
        }

        // Create reservation
        let expires_at = env.ledger().timestamp() + 900; // 15 minutes
        let reservation = Reservation {
            tier_id,
            expires_at,
        };

        storage::save_reservation(&env, &event_id, &attendee, &reservation);

        tier.reserved += 1;
        event.tiers.set(index, tier);
        storage::save_event(&env, &event_id, &event);

        Ok(())
    }

    /// Release an expired reservation.
    pub fn release_expired_reservation(
        env: Env,
        event_id: Symbol,
        attendee: Address,
    ) -> Result<(), EventError> {
        let reservation = storage::get_reservation(&env, &event_id, &attendee)?;

        if reservation.expires_at > env.ledger().timestamp() {
            return Err(EventError::InvalidInput); // Not expired yet
        }

        let mut event = storage::get_event(&env, &event_id)?;
        let mut found = false;
        for i in 0..event.tiers.len() {
            let mut tier = event.tiers.get(i).ok_or(EventError::TierNotFound)?;
            if tier.tier_id == reservation.tier_id {
                if tier.reserved > 0 {
                    tier.reserved -= 1;
                }
                event.tiers.set(i, tier);
                found = true;
                break;
            }
        }

        if !found {
            return Err(EventError::TierNotFound);
        }

        storage::remove_reservation(&env, &event_id, &attendee);
        storage::save_event(&env, &event_id, &event);

        Ok(())
    }

    pub fn register_for_event(
        env: Env,
        attendee: Address,
        event_id: Symbol,
        tier_id: u32,
        _is_verified: bool,
    ) -> Result<(), EventError> {
        attendee.require_auth();

        let mut event = storage::get_event(&env, &event_id)?;

        if event.status != EventStatus::Active {
            return Err(EventError::EventNotActive);
        }

        if storage::is_registered(&env, &event_id, &attendee) {
            return Err(EventError::AlreadyRegistered);
        }

        let has_res = storage::has_reservation(&env, &event_id, &attendee);
        let mut tier_index = None;

        if has_res {
            let reservation = storage::get_reservation(&env, &event_id, &attendee)?;
            if reservation.expires_at < env.ledger().timestamp() {
                return Err(EventError::ReservationExpired);
            }
            if reservation.tier_id != tier_id {
                return Err(EventError::InvalidInput); // Trying to pay for a different tier than reserved
            }

            for i in 0..event.tiers.len() {
                let tier = event.tiers.get(i).ok_or(EventError::TierNotFound)?;
                if tier.tier_id == tier_id {
                    tier_index = Some(i);
                    break;
                }
            }
        } else {
            // Instant purchase without reservation (if capacity allows)
            for i in 0..event.tiers.len() {
                let tier = event.tiers.get(i).ok_or(EventError::TierNotFound)?;
                if tier.tier_id == tier_id {
                    tier_index = Some(i);
                    break;
                }
            }
        }

        let index = tier_index.ok_or(EventError::TierNotFound)?;
        let mut tier = event.tiers.get(index).ok_or(EventError::TierNotFound)?;

        if !has_res && tier.sold + tier.reserved >= tier.capacity {
            return Err(EventError::TierSoldOut);
        }

        let payments_contract = storage::get_payments_contract(&env)?;
        let ticket_contract = storage::get_ticket_contract(&env)?;

        if tier.price > 0 {
            let payments_client = PaymentsContractClient::new(&env, &payments_contract);
            let token = payments_client.get_accepted_token();
            payments_client.pay_for_ticket(
                &attendee,
                &event_id,
                &tier.price,
                &token,
                &PaymentPrivacy::Standard,
            );
        }

        let ticket_client = TicketContractClient::new(&env, &ticket_contract);
        ticket_client.mint_ticket(&event.event_id, &event.organizer, &attendee);

        storage::save_registration(&env, &event_id, &attendee);

        if has_res {
            if tier.reserved > 0 {
                tier.reserved -= 1;
            }
            storage::remove_reservation(&env, &event_id, &attendee);
        }

        tier.sold += 1;
        event.tiers.set(index, tier.clone());
        update_event(&env, &event_id, &event)?;
        let privacy = storage::get_event_privacy(&env, &event_id);
        emit_registration(&env, &event_id, &attendee, tier_id, tier.sold, &privacy);

        Ok(())
    }

    pub fn is_registered(
        env: Env,
        event_id: Symbol,
        attendee: Address,
    ) -> Result<bool, EventError> {
        storage::get_event(&env, &event_id)?;
        Ok(storage::is_registered(&env, &event_id, &attendee))
    }

    pub fn get_attendees(
        env: Env,
        event_id: Symbol,
    ) -> Result<soroban_sdk::Vec<Address>, EventError> {
        storage::get_event(&env, &event_id)?;
        Ok(storage::get_attendees(&env, &event_id))
    }

    /// Withdraw revenue for a completed event. Only the organizer can do this.
    pub fn withdraw_revenue(
        env: Env,
        organizer: Address,
        event_id: Symbol,
    ) -> Result<(), EventError> {
        organizer.require_auth();

        let event = storage::get_event(&env, &event_id)?;

        // Verify caller is the event organizer
        if event.organizer != organizer {
            return Err(EventError::Unauthorized);
        }

        // Revenue can only be withdrawn for completed events (optional, but safer)
        if event.status != EventStatus::Completed {
            return Err(EventError::InvalidStatusTransition);
        }

        let payments_contract = storage::get_payments_contract(&env)?;
        let payments_client = PaymentsContractClient::new(&env, &payments_contract);

        // This calls the payment contract to transfer funds and record the history
        payments_client.withdraw_revenue(&event_id, &organizer);

        Ok(())
    }

    /// Get all withdrawal history for an event.
    pub fn get_withdrawal_history(
        env: Env,
        event_id: Symbol,
    ) -> Result<soroban_sdk::Vec<payments_contract::WithdrawalRecord>, EventError> {
        storage::get_event(&env, &event_id)?;
        let payments_contract = storage::get_payments_contract(&env)?;
        let payments_client = PaymentsContractClient::new(&env, &payments_contract);
        Ok(payments_client.get_withdrawal_history(&event_id))
    }

    /// Set the privacy level for an event. Only the organizer can change this.
    pub fn set_event_privacy(
        env: Env,
        organizer: Address,
        event_id: Symbol,
        level: PrivacyLevel,
    ) -> Result<(), EventError> {
        organizer.require_auth();

        let event = storage::get_event(&env, &event_id)?;
        if event.organizer != organizer {
            return Err(EventError::Unauthorized);
        }

        storage::set_event_privacy(&env, &event_id, &level);
        Ok(())
    }

    /// Get the privacy level for an event.
    pub fn get_event_privacy(env: Env, event_id: Symbol) -> PrivacyLevel {
        storage::get_event_privacy(&env, &event_id)
    }
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod integration_tests;
