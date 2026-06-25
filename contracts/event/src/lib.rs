#![no_std]
use payments_contract::{PaymentPrivacy, PaymentsContractClient};
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Symbol};
use ticket_contract::TicketContractClient;

mod errors;
mod events;
mod storage;
mod types;

#[cfg(test)]
mod migration_test;

pub use errors::*;
pub use storage::*;
pub use types::*;

use events::{
    emit_anon_registration, emit_event_cancelled, emit_event_created, emit_event_postponed,
    emit_event_resumed, emit_event_updated, emit_registration, emit_status_changed,
    emit_zk_verified_attendance,
};

// Minimum withdrawal delay (in ledgers) that must be enforced for events
const MIN_WITHDRAWAL_DELAY_LEDGERS: u32 = 100;

// Minimum refund-choice window for a postponement, in ledgers. Set to ~72h
// (51,840 ledgers at ~5s/ledger) so every holder has ample time to opt out.
const MIN_POSTPONEMENT_CHOICE_WINDOW_LEDGERS: u32 = 51_840;

// Maximum refund-choice window, in ledgers (~30 days). Bounds how long escrow can
// stay frozen and keeps the payments-side deadline within its storage TTL.
const MAX_POSTPONEMENT_CHOICE_WINDOW_LEDGERS: u32 = 518_400;

// Maximum number of times a single event may be postponed. The issue does not
// mandate a specific number — it only flags "postpone indefinitely to avoid
// refunds" as a spec flaw — so this is an anti-abuse bound: once exhausted the
// organizer must run the event (-> Completed) or cancel it (-> Cancelled, full
// refund). Each postponement independently opens a fresh refund window.
const MAX_POSTPONEMENTS: u32 = 3;

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

        if params.event_start_ledger > params.event_end_ledger {
            return Err(EventError::InvalidInput);
        }

        // Enforce minimum withdrawal delay to prevent bypass at creation time
        if params.withdrawal_delay_ledgers < MIN_WITHDRAWAL_DELAY_LEDGERS {
            return Err(EventError::InvalidInput);
        }

        // Validate there is at least 1 tier
        if params.initial_tiers.is_empty() {
            return Err(EventError::InvalidInput);
        }

        let mut tiers = soroban_sdk::Vec::new(&env);
        let mut max_supply = 0u32;
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
            max_supply = max_supply
                .checked_add(tier_param.capacity)
                .ok_or(EventError::InvalidTicketCount)?;

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
            max_tickets_per_user: params.max_tickets_per_user,
            max_supply,
            sold_count: 0,
            event_start_ledger: params.event_start_ledger,
            event_end_ledger: params.event_end_ledger,
            withdrawal_delay_ledgers: params.withdrawal_delay_ledgers,
        };

        save_event(&env, &params.event_id, &event);
        // Persist privacy level under its own key so get_event_privacy always
        // returns the value chosen at creation without a separate set_event_privacy call.
        storage::set_event_privacy(&env, &params.event_id, &params.privacy_level);
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
                &params.max_tickets_per_user,
                &event.max_supply,
                &event.event_start_ledger,
                &event.event_end_ledger,
                &event.withdrawal_delay_ledgers,
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
        if let Some(max_tickets) = params.max_tickets_per_user {
            event.max_tickets_per_user = max_tickets;
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
                &event.max_tickets_per_user,
                &event.max_supply,
                &event.event_start_ledger,
                &event.event_end_ledger,
                &event.withdrawal_delay_ledgers,
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
        event.max_supply = event
            .max_supply
            .checked_add(capacity)
            .ok_or(EventError::InvalidTicketCount)?;

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
                    if c < tier.sold {
                        return Err(EventError::InvalidTicketCount);
                    }
                    let new_max_supply = event
                        .max_supply
                        .checked_sub(tier.capacity)
                        .and_then(|supply| supply.checked_add(c))
                        .ok_or(EventError::InvalidTicketCount)?;
                    if new_max_supply < event.sold_count {
                        return Err(EventError::InvalidTicketCount);
                    }
                    tier.capacity = c;
                    event.max_supply = new_max_supply;
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
        let privacy = storage::get_event_privacy(&env, &event_id);
        emit_event_cancelled(&env, &event_id, &organizer, &privacy);

        // Process refunds if contracts are linked
        if has_linked_contracts(&env) {
            let payments_contract = get_payments_contract(&env)?;
            let payments_client = PaymentsContractClient::new(&env, &payments_contract);

            payments_client.cancel_event(&event_id, &organizer);
        }

        Ok(())
    }

    /// Postpone (reschedule) an active event to a new date instead of cancelling it.
    ///
    /// This is a distinct lifecycle path from cancellation: the event continues to
    /// exist and tickets stay valid for the new date. Only an `Active` event can be
    /// postponed (`Completed`/`Cancelled`/`Upcoming` are rejected). Postponing opens
    /// a refund-choice window of at least `MIN_POSTPONEMENT_CHOICE_WINDOW_LEDGERS`
    /// (~72h) during which any holder may opt out for a full refund via the payments
    /// contract's `request_postponement_refund`. While `Postponed`, every revenue
    /// withdrawal path is blocked (here and in the payments contract).
    ///
    /// `new_date_ledger` must fall strictly after the choice window closes, so
    /// holders always get their full decision window before the rescheduled date.
    /// An event may be postponed at most `MAX_POSTPONEMENTS` times.
    pub fn postpone_event(
        env: Env,
        organizer: Address,
        event_id: Symbol,
        new_date_ledger: u64,
        choice_window_ledgers: u32,
    ) -> Result<(), EventError> {
        organizer.require_auth();

        let mut event = storage::get_event(&env, &event_id)?;

        if event.organizer != organizer {
            return Err(EventError::Unauthorized);
        }

        // Only an Active event can be postponed.
        if event.status != EventStatus::Active {
            return Err(EventError::InvalidStatusTransition);
        }

        // Anti-abuse: cap the number of postponements.
        let count = storage::get_postpone_count(&env, &event_id);
        if count >= MAX_POSTPONEMENTS {
            return Err(EventError::MaxPostponementsReached);
        }

        // Enforce the mandatory minimum (and a sane maximum) refund-choice window.
        if choice_window_ledgers < MIN_POSTPONEMENT_CHOICE_WINDOW_LEDGERS {
            return Err(EventError::PostponementWindowTooShort);
        }
        if choice_window_ledgers > MAX_POSTPONEMENT_CHOICE_WINDOW_LEDGERS {
            return Err(EventError::InvalidPostponementDate);
        }

        // The new date is a ledger sequence; it must fit u32 since the event
        // schedule (`event_start_ledger`/`event_end_ledger`) is stored as u32.
        if new_date_ledger > u32::MAX as u64 {
            return Err(EventError::InvalidPostponementDate);
        }

        let current_ledger = env.ledger().sequence();
        let choice_deadline_ledger = current_ledger
            .checked_add(choice_window_ledgers)
            .ok_or(EventError::InvalidPostponementDate)?;

        // The new date must be strictly after the choice window closes.
        if new_date_ledger <= choice_deadline_ledger as u64 {
            return Err(EventError::InvalidPostponementDate);
        }

        let old_status = event.status.clone();
        event.status = EventStatus::Postponed;
        update_event(&env, &event_id, &event)?;

        let postpone_count = count + 1;
        // The active window data and the cumulative anti-abuse counter are stored
        // separately: the window record is cleared on resume, while the counter
        // persists for the lifetime of the event to enforce `MAX_POSTPONEMENTS`.
        storage::set_postpone_count(&env, &event_id, postpone_count);
        storage::set_postponement(
            &env,
            &event_id,
            &PostponementInfo {
                new_date_ledger,
                choice_deadline_ledger: choice_deadline_ledger as u64,
            },
        );

        emit_status_changed(&env, &event_id, &old_status, &EventStatus::Postponed);
        emit_event_postponed(
            &env,
            &event_id,
            new_date_ledger,
            choice_deadline_ledger as u64,
            postpone_count,
        );

        // Freeze escrow and open the refund window on the payments side.
        if has_linked_contracts(&env) {
            let payments_contract = get_payments_contract(&env)?;
            let payments_client = PaymentsContractClient::new(&env, &payments_contract);
            payments_client.postpone_event(&event_id, &organizer, &choice_deadline_ledger);
        }

        Ok(())
    }

    /// Finalize a postponement once its refund-choice window has closed, returning
    /// the event to `Active` on its new schedule.
    ///
    /// Restricted to the organizer (matching every other state-changing entrypoint).
    /// The organizer is economically motivated to call it: revenue stays frozen
    /// until the event is resumed and subsequently `Completed`. The ledger schedule
    /// (`event_start_ledger` / `event_end_ledger`) is shifted to the new date while
    /// preserving the original event duration, and the new schedule is synced to the
    /// payments contract so escrow/withdrawal timing is recomputed against it. The
    /// active postponement record is cleared so getters no longer expose stale data.
    pub fn finalize_postponement(
        env: Env,
        organizer: Address,
        event_id: Symbol,
    ) -> Result<(), EventError> {
        organizer.require_auth();

        let mut event = storage::get_event(&env, &event_id)?;

        if event.organizer != organizer {
            return Err(EventError::Unauthorized);
        }

        if event.status != EventStatus::Postponed {
            return Err(EventError::EventNotPostponed);
        }

        let info =
            storage::get_postponement(&env, &event_id).ok_or(EventError::EventNotPostponed)?;

        // The choice window must have closed.
        if (env.ledger().sequence() as u64) <= info.choice_deadline_ledger {
            return Err(EventError::PostponementWindowOpen);
        }

        // Shift the ledger schedule to the new date, preserving the original duration.
        // `new_date_ledger` was validated to fit u32 at postponement time.
        let duration = event
            .event_end_ledger
            .saturating_sub(event.event_start_ledger);
        let new_start_ledger =
            u32::try_from(info.new_date_ledger).map_err(|_| EventError::InvalidPostponementDate)?;
        let new_end_ledger = new_start_ledger
            .checked_add(duration)
            .ok_or(EventError::InvalidPostponementDate)?;
        event.event_start_ledger = new_start_ledger;
        event.event_end_ledger = new_end_ledger;

        event.status = EventStatus::Active;
        update_event(&env, &event_id, &event)?;

        // Clear the active window record; the cumulative postpone counter is retained.
        storage::remove_postponement(&env, &event_id);

        emit_status_changed(
            &env,
            &event_id,
            &EventStatus::Postponed,
            &EventStatus::Active,
        );
        emit_event_resumed(&env, &event_id, new_start_ledger, new_end_ledger);

        if has_linked_contracts(&env) {
            let payments_contract = get_payments_contract(&env)?;
            let payments_client = PaymentsContractClient::new(&env, &payments_contract);
            // Push the new schedule, then resume the event back to Active.
            payments_client.sync_event_config(
                &env.current_contract_address(),
                &event_id,
                &event.organizer,
                &event.payout_token,
                &event.allow_anonymous,
                &event.requires_verification,
                &event.max_tickets_per_user,
                &event.max_supply,
                &event.event_start_ledger,
                &event.event_end_ledger,
                &event.withdrawal_delay_ledgers,
            );
            payments_client.resume_event(&event_id, &event.organizer);
        }

        Ok(())
    }

    /// Read the active postponement record for an event. Only present while the
    /// event is `Postponed`; returns `EventNotPostponed` once it has been resumed
    /// or if it was never postponed (so callers never see stale window data).
    pub fn get_postponement(env: Env, event_id: Symbol) -> Result<PostponementInfo, EventError> {
        storage::get_event(&env, &event_id)?;
        storage::get_postponement(&env, &event_id).ok_or(EventError::EventNotPostponed)
    }

    /// Opt a ticket holder out of a postponed event for a full refund, and revoke
    /// their participation so they cannot also attend the rescheduled event.
    ///
    /// Orchestrated here rather than directly on the payments contract because only
    /// the event contract is linked to both payments and ticket contracts. The
    /// target event is derived from the payment ticket itself (not a caller-supplied
    /// argument) so the refund and the access-revocation always concern the same
    /// event. Sequence: the payments contract performs the full token refund
    /// (verifying ownership, `Held` status and that the choice window is still open —
    /// reverting the whole call otherwise); one of the attendee's valid minted
    /// tickets for the event is cancelled; and the registration is dropped only once
    /// the attendee has no remaining valid ticket for the event (so a single-ticket
    /// refund does not strip a holder who still has other valid tickets).
    /// `ticket_id` is the payments-side receipt ticket id (as returned by
    /// `payments::get_owner_tickets`).
    pub fn request_postponement_refund(
        env: Env,
        attendee: Address,
        ticket_id: u64,
    ) -> Result<(), EventError> {
        attendee.require_auth();

        let payments_contract = get_payments_contract(&env)?;
        let payments_client = PaymentsContractClient::new(&env, &payments_contract);

        // Derive the event from the payment ticket so refund and revocation can
        // never target different events.
        let event_id = payments_client.get_ticket(&ticket_id).event_id;

        let event = storage::get_event(&env, &event_id)?;
        if event.status != EventStatus::Postponed {
            return Err(EventError::EventNotPostponed);
        }

        // Locate a revocable (valid, unused) minted ticket for this event BEFORE
        // issuing any refund, so we never refund a holder who has nothing to give up
        // (e.g. their entry ticket was already used or transferred away). The
        // attendee owns the ticket, so their auth on this call covers the
        // owner-gated cancellation that follows.
        let ticket_contract = get_ticket_contract(&env)?;
        let ticket_client = TicketContractClient::new(&env, &ticket_contract);
        let mut revocable: Option<u64> = None;
        for tid in ticket_client.get_tickets_by_owner(&attendee).iter() {
            let minted = ticket_client.get_ticket(&tid);
            if minted.event_id == event_id
                && !minted.is_used
                && minted.status == ticket_contract::TicketStatus::Valid
            {
                revocable = Some(tid);
                break;
            }
        }
        let revocable = revocable.ok_or(EventError::NoRefundableTicket)?;

        // Full refund (verifies ownership, Held status and open window; reverts otherwise).
        payments_client.request_postponement_refund(&attendee, &ticket_id);

        // Revoke the entry ticket that we proved exists above.
        ticket_client.cancel_ticket(&revocable, &attendee);

        // Drop the registration only when no valid ticket for this event remains.
        if !has_valid_ticket_for_event(&ticket_client, &attendee, &event_id) {
            storage::remove_registration(&env, &event_id, &attendee);
        }

        Ok(())
    }

    /// Reserve a ticket for a specific tier. The reservation is valid for 15 minutes.
    pub fn reserve_ticket(
        env: Env,
        attendee: Address,
        event_id: Symbol,
        tier_id: u32,
        _email_hash: Option<BytesN<32>>,
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
        nonce: u64,
        attendee: Address,
        event_id: Symbol,
        tier_id: u32,
        _is_verified: bool,
        _email_hash: Option<BytesN<32>>,
    ) -> Result<(), EventError> {
        attendee.require_auth();

        let mut event = storage::get_event(&env, &event_id)?;

        if event.status != EventStatus::Active {
            return Err(EventError::EventNotActive);
        }

        // Sybil-resistance: check free-claim limits before registration state,
        // so the limit fires even if the wallet has already registered (prevents
        // gaming via cancellation/re-registration in future flows) and avoids
        // leaking registration status through the error path on Anonymous events.
        {
            let mut req_price: Option<i128> = None;
            for t in event.tiers.iter() {
                if t.tier_id == tier_id {
                    req_price = Some(t.price);
                    break;
                }
            }
            if req_price == Some(0) {
                let now = env.ledger().timestamp();
                let settings = storage::get_claim_settings(&env, &event_id);
                if settings.max_free_claims > 0 {
                    let count = storage::get_free_claim_count(&env, &event_id, &attendee);
                    if count >= settings.max_free_claims {
                        return Err(EventError::ClaimLimitExceeded);
                    }
                }
                if settings.cooldown_secs > 0 {
                    let last = storage::get_last_free_claim(&env, &event_id, &attendee);
                    if last > 0 && now < last + settings.cooldown_secs {
                        return Err(EventError::ClaimCooldownActive);
                    }
                }
            }
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

        if event.sold_count >= event.max_supply {
            return Err(EventError::EventSoldOut);
        }

        if !has_res && tier.sold + tier.reserved >= tier.capacity {
            return Err(EventError::TierSoldOut);
        }

        let payments_contract = storage::get_payments_contract(&env)?;
        let ticket_contract = storage::get_ticket_contract(&env)?;

        if tier.price > 0 {
            let payments_client = PaymentsContractClient::new(&env, &payments_contract);
            let token = payments_client.get_accepted_token();

            payments_client.pay_for_ticket(
                &nonce,
                &attendee,
                &event_id,
                &tier.price,
                &_email_hash,
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

        // Record free-claim usage after a successful zero-price registration
        if tier.price == 0 {
            storage::increment_free_claim_count(&env, &event_id, &attendee);
            storage::set_last_free_claim(&env, &event_id, &attendee, env.ledger().timestamp());
        }

        tier.sold += 1;
        event.sold_count += 1;
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

    /// Get the public attendee list for an event.
    ///
    /// - `Standard`: returns the full list of attendee addresses.
    /// - `Private`: returns `UnauthorizedPrivateAccess` — organizer must use
    ///   `get_attendees_as_organizer` instead.
    /// - `Anonymous`: returns an empty list; attendee identities are never exposed.
    pub fn get_attendees(
        env: Env,
        event_id: Symbol,
    ) -> Result<soroban_sdk::Vec<Address>, EventError> {
        storage::get_event(&env, &event_id)?;
        let privacy = storage::get_event_privacy(&env, &event_id);
        match privacy {
            PrivacyLevel::Standard => Ok(storage::get_attendees(&env, &event_id)),
            PrivacyLevel::Private => Err(EventError::UnauthorizedPrivateAccess),
            PrivacyLevel::Anonymous => Ok(soroban_sdk::Vec::new(&env)),
        }
    }

    /// Organizer-only view of the attendee list for Private events.
    ///
    /// Requires organizer authorization. Works for all privacy levels.
    pub fn get_attendees_as_organizer(
        env: Env,
        organizer: Address,
        event_id: Symbol,
    ) -> Result<soroban_sdk::Vec<Address>, EventError> {
        organizer.require_auth();
        let event = storage::get_event(&env, &event_id)?;
        if event.organizer != organizer {
            return Err(EventError::Unauthorized);
        }
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

    /// Configure sybil-resistance limits for free ticket claims on an event.
    ///
    /// - `max_free_claims`: max free tickets a single wallet may claim. 0 = unlimited.
    /// - `cooldown_secs`: minimum seconds between consecutive free claims. 0 = no cooldown.
    ///
    /// Only the event organizer may call this.
    pub fn set_claim_settings(
        env: Env,
        organizer: Address,
        event_id: Symbol,
        max_free_claims: u32,
        cooldown_secs: u64,
    ) -> Result<(), EventError> {
        organizer.require_auth();
        let event = storage::get_event(&env, &event_id)?;
        if event.organizer != organizer {
            return Err(EventError::Unauthorized);
        }
        storage::set_claim_settings(
            &env,
            &event_id,
            &ClaimSettings {
                max_free_claims,
                cooldown_secs,
            },
        );
        Ok(())
    }

    /// Get the current sybil-resistance settings for an event.
    pub fn get_claim_settings(env: Env, event_id: Symbol) -> ClaimSettings {
        storage::get_claim_settings(&env, &event_id)
    }

    /// Claim a free ticket anonymously — no wallet or account required.
    ///
    /// The caller submits a `commitment` (e.g. SHA-256 of user_secret ‖ event_id ‖ nonce).
    /// The contract rejects duplicate commitments and enforces an organizer-configured
    /// per-ledger-window rate limit so a single source cannot drain capacity in one batch.
    ///
    /// Capacity (event and tier) is decremented on success; no ticket NFT is minted.
    /// The stored commitment is the on-chain attendance proof.
    pub fn claim_anonymous_ticket(
        env: Env,
        event_id: Symbol,
        tier_id: u32,
        commitment: BytesN<32>,
    ) -> Result<(), EventError> {
        let mut event = storage::get_event(&env, &event_id)?;

        if event.status != EventStatus::Active {
            return Err(EventError::EventNotActive);
        }

        if !event.allow_anonymous {
            return Err(EventError::AnonymousClaimsNotEnabled);
        }

        // Locate the tier and enforce the free-only constraint.
        let mut tier_index = None;
        for i in 0..event.tiers.len() {
            let t = event.tiers.get(i).ok_or(EventError::TierNotFound)?;
            if t.tier_id == tier_id {
                if t.price != 0 {
                    return Err(EventError::InvalidInput);
                }
                tier_index = Some(i);
                break;
            }
        }
        let index = tier_index.ok_or(EventError::TierNotFound)?;

        // Reject duplicate commitments (prevents trivial sybil via commitment reuse).
        if storage::has_anon_commitment(&env, &event_id, &commitment) {
            return Err(EventError::AnonCommitmentReused);
        }

        // Per-ledger-window rate limit: prevents draining capacity in a single batch.
        let anon_settings = storage::get_anon_claim_settings(&env, &event_id);
        if anon_settings.max_anon_claims_per_window > 0 && anon_settings.anon_window_size > 0 {
            let current_window = env.ledger().sequence() / anon_settings.anon_window_size;
            let mut state = storage::get_anon_window_state(&env, &event_id);
            if state.window_index != current_window {
                state.window_index = current_window;
                state.count = 0;
            }
            if state.count >= anon_settings.max_anon_claims_per_window {
                return Err(EventError::AnonClaimWindowFull);
            }
            state.count += 1;
            storage::set_anon_window_state(&env, &event_id, &state);
        }

        // Capacity checks.
        let mut tier = event.tiers.get(index).ok_or(EventError::TierNotFound)?;

        if event.sold_count >= event.max_supply {
            return Err(EventError::EventSoldOut);
        }
        if tier.sold + tier.reserved >= tier.capacity {
            return Err(EventError::TierSoldOut);
        }

        // Commit: record the commitment and update sold counts.
        storage::save_anon_commitment(&env, &event_id, &commitment);

        tier.sold += 1;
        event.sold_count += 1;
        event.tiers.set(index, tier.clone());
        storage::update_event(&env, &event_id, &event)?;

        emit_anon_registration(&env, &event_id, tier_id, tier.sold);

        Ok(())
    }

    /// Configure the per-ledger-window rate limit for anonymous free claims on an event.
    ///
    /// - `max_anon_claims_per_window`: max anonymous tickets claimable per window. 0 = unlimited.
    /// - `anon_window_size`: window size in ledgers (e.g. 100). 0 = no window limit.
    ///
    /// Only the event organizer may call this.
    ///
    /// ⚠️ These settings can be changed at any time by the organizer. Setting either to 0
    /// disables rate limiting. Consider locking these after event launch.
    pub fn set_anon_claim_settings(
        env: Env,
        organizer: Address,
        event_id: Symbol,
        max_anon_claims_per_window: u32,
        anon_window_size: u32,
    ) -> Result<(), EventError> {
        organizer.require_auth();
        let event = storage::get_event(&env, &event_id)?;
        if event.organizer != organizer {
            return Err(EventError::Unauthorized);
        }
        storage::set_anon_claim_settings(
            &env,
            &event_id,
            &AnonClaimSettings {
                max_anon_claims_per_window,
                anon_window_size,
            },
        );
        Ok(())
    }

    /// Get the current anonymous claim rate-limit settings for an event.
    pub fn get_anon_claim_settings(env: Env, event_id: Symbol) -> AnonClaimSettings {
        storage::get_anon_claim_settings(&env, &event_id)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // zkPassport Verification
    // ─────────────────────────────────────────────────────────────────────────

    /// Register an attendee for a zkPassport-gated event by submitting a
    /// zero-knowledge passport proof.
    ///
    /// # Verification flow
    ///
    /// 1. Asserts the event exists and is `Active`.
    /// 2. Asserts `event.requires_verification == true` — non-gated events must
    ///    use `register_for_event` instead.
    /// 3. Asserts the organizer has enabled zkPassport via `set_zk_config`.
    /// 4. Checks `claim.expiry_ledger >= env.ledger().sequence()` — stale proofs
    ///    are rejected immediately.
    /// 5. If the organizer specified a `required_claim_type`, verifies the claim
    ///    type matches.
    /// 6. Checks the nullifier has not already been used for this event.
    /// 7. Checks capacity and duplicate registration.
    /// 8. Mints a ticket via the ticket contract (if linked).
    /// 9. Saves **only** the nullifier — the raw `proof` bytes are discarded.
    /// 10. Emits `ZkVerifiedAttendance` (nullifier omitted from event payload).
    ///
    /// # Privacy guarantees
    /// - Proof bytes are NEVER written to the ledger.
    /// - The nullifier prevents reuse without revealing identity.
    /// - The emitted event contains only `claim_type`, not the nullifier, to
    ///   prevent cross-event correlation.
    pub fn verify_and_attend(
        env: Env,
        event_id: Symbol,
        tier_id: u32,
        claim: ZkPassportClaim,
    ) -> Result<(), EventError> {
        let mut event = storage::get_event(&env, &event_id)?;

        // 1. Event must be Active.
        if event.status != EventStatus::Active {
            return Err(EventError::EventNotActive);
        }

        // 2. This path is only for verification-gated events.
        if !event.requires_verification {
            return Err(EventError::ZkVerificationRequired);
        }

        // 3. Organizer must have explicitly enabled zkPassport for this event.
        let zk_config = storage::get_zk_verification_config(&env, &event_id);
        if !zk_config.enabled {
            return Err(EventError::ZkVerificationRequired);
        }

        // 4. Proof must not be expired.
        let current_ledger = env.ledger().sequence();
        if claim.expiry_ledger < current_ledger {
            return Err(EventError::ZkProofExpired);
        }

        // 5. Validate claim type if the organizer specified one.
        if let Some(required_type) = &zk_config.required_claim_type {
            if &claim.claim_type != required_type {
                return Err(EventError::ZkClaimTypeMismatch);
            }
        }

        // 6. Nullifier must be fresh — no proof reuse allowed.
        if storage::has_zk_nullifier(&env, &event_id, &claim.nullifier) {
            return Err(EventError::ZkNullifierReused);
        }

        // 7. Guard duplicate registration.
        // NOTE: We intentionally do NOT check is_registered for anonymous paths;
        // for verified paths the ticket contract enforces uniqueness per-address.
        // We rely on nullifier uniqueness as the primary sybil guard here.

        // Locate the requested tier.
        let mut tier_index = None;
        for i in 0..event.tiers.len() {
            let t = event.tiers.get(i).ok_or(EventError::TierNotFound)?;
            if t.tier_id == tier_id {
                tier_index = Some(i);
                break;
            }
        }
        let index = tier_index.ok_or(EventError::TierNotFound)?;
        let mut tier = event.tiers.get(index).ok_or(EventError::TierNotFound)?;

        // Capacity checks.
        if event.sold_count >= event.max_supply {
            return Err(EventError::EventSoldOut);
        }
        if tier.sold + tier.reserved >= tier.capacity {
            return Err(EventError::TierSoldOut);
        }

        // 8. Handle payment for paid tiers.
        if tier.price > 0 {
            if has_linked_contracts(&env) {
                let payments_contract = get_payments_contract(&env)?;
                let payments_client = PaymentsContractClient::new(&env, &payments_contract);
                let token = payments_client.get_accepted_token();
                payments_client.pay_for_ticket(
                    &0u64,
                    &env.current_contract_address(),
                    &event_id,
                    &tier.price,
                    &None::<BytesN<32>>,
                    &token,
                    &PaymentPrivacy::Standard,
                );
            }
        }

        // Mint the ticket NFT if the ticket contract is linked.
        if has_linked_contracts(&env) {
            let ticket_contract = get_ticket_contract(&env)?;
            let ticket_client = TicketContractClient::new(&env, &ticket_contract);
            ticket_client.mint_ticket(&event.event_id, &event.organizer, &env.current_contract_address());
        }

        // 9. Persist ONLY the nullifier — proof bytes are never stored.
        storage::save_zk_nullifier(&env, &event_id, &claim.nullifier);

        // Update sold counts.
        tier.sold += 1;
        event.sold_count += 1;
        event.tiers.set(index, tier.clone());
        storage::update_event(&env, &event_id, &event)?;

        // 10. Emit event — nullifier deliberately excluded from payload.
        emit_zk_verified_attendance(&env, &event_id, &claim.claim_type, tier_id, tier.sold);

        Ok(())
    }

    /// Configure the zkPassport verification settings for an event.
    ///
    /// - `enabled: true` activates the `verify_and_attend` path.
    /// - `required_claim_type`: if `Some`, only that claim category is accepted.
    ///   `None` allows any valid ZK claim type.
    ///
    /// Only the event organizer may call this. The event must exist.
    pub fn set_zk_config(
        env: Env,
        organizer: Address,
        event_id: Symbol,
        config: ZkVerificationConfig,
    ) -> Result<(), EventError> {
        organizer.require_auth();
        let event = storage::get_event(&env, &event_id)?;
        if event.organizer != organizer {
            return Err(EventError::Unauthorized);
        }
        storage::set_zk_verification_config(&env, &event_id, &config);
        Ok(())
    }

    /// Retrieve the current zkPassport verification configuration for an event.
    pub fn get_zk_config(env: Env, event_id: Symbol) -> ZkVerificationConfig {
        storage::get_zk_verification_config(&env, &event_id)
    }

    /// Check whether a specific nullifier has already been recorded (spent) for
    /// the given event. Useful for off-chain relayers to pre-screen proofs before
    /// submitting a transaction.
    pub fn is_nullifier_used(env: Env, event_id: Symbol, nullifier: BytesN<32>) -> bool {
        storage::has_zk_nullifier(&env, &event_id, &nullifier)
    }

    /// Get the current contract version.
    pub fn contract_version(env: Env) -> u32 {
        storage::get_contract_version(&env)
    }

    /// Migrate the contract to a new version. Only admin can call this.
    pub fn migrate(env: Env, admin: Address) -> Result<u32, EventError> {
        admin.require_auth();

        let current_admin = storage::get_admin(&env)?;
        if current_admin != admin {
            return Err(EventError::Unauthorized);
        }

        let current_version = storage::get_contract_version(&env);
        let new_version = current_version + 1;

        // Perform any necessary migrations based on version transitions
        match current_version {
            0 => {
                // First migration: initialize version tracking
                storage::set_contract_version(&env, 1);
            }
            1 => {
                // Future migrations can be added here
                storage::set_contract_version(&env, 2);
            }
            2 => {
                // v2 -> v3 migration
                storage::set_contract_version(&env, 3);
            }
            _ => {
                return Err(EventError::UnsupportedVersion);
            }
        }

        Ok(new_version)
    }
}

/// Whether `attendee` still holds at least one `Valid`, unused minted ticket for
/// `event_id`. Used to decide if a postponement refund should drop the attendee's
/// event registration (only once their last valid ticket is gone).
fn has_valid_ticket_for_event(
    ticket_client: &TicketContractClient,
    attendee: &Address,
    event_id: &Symbol,
) -> bool {
    for tid in ticket_client.get_tickets_by_owner(attendee).iter() {
        let minted = ticket_client.get_ticket(&tid);
        if minted.event_id == *event_id
            && !minted.is_used
            && minted.status == ticket_contract::TicketStatus::Valid
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod integration_tests;

#[cfg(test)]
mod test_privacy;

#[cfg(test)]
mod test_claims;

#[cfg(test)]
mod test_anon_claims;

#[cfg(test)]
mod test_zk_passport;
