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
const MIN_WITHDRAWAL_DELAY_LEDGERS: u32 = 100;
const MIN_POSTPONEMENT_CHOICE_WINDOW_LEDGERS: u32 = 51_840;
const MAX_POSTPONEMENT_CHOICE_WINDOW_LEDGERS: u32 = 518_400;
const MAX_POSTPONEMENTS: u32 = 3;

#[contract]
pub struct EventContract;

#[contractimpl]
impl EventContract {
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
    pub fn create_event(env: Env, params: CreateEventParams) -> Result<Event, EventError> {
        params.organizer.require_auth();
        if params.name.is_empty() {
            return Err(EventError::InvalidInput);
        }
        if params.venue.is_empty() {
            return Err(EventError::InvalidInput);
        }
        let min_date = env.ledger().timestamp() + 86_400;
        if params.event_date <= min_date {
            return Err(EventError::InvalidEventDate);
        }

        if params.event_start_ledger > params.event_end_ledger {
            return Err(EventError::InvalidInput);
        }
        if params.withdrawal_delay_ledgers < MIN_WITHDRAWAL_DELAY_LEDGERS {
            return Err(EventError::InvalidInput);
        }

        // Validate resale royalty (max 2000 bps = 20%)
        if params.resale_royalty_bps > 2000 {
            return Err(EventError::InvalidInput);
        }

        // Validate there is at least 1 tier
        if params.initial_tiers.is_empty() {
            return Err(EventError::InvalidInput);
        }

        // Validate the revenue split if one was provided. An empty split keeps the
        // legacy single-organizer payout behaviour.
        validate_revenue_splits(&params.revenue_splits, &params.organizer)?;

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
            revenue_splits: params.revenue_splits.clone(),
            resale_royalty_bps: params.resale_royalty_bps,
            max_resale_price: params.max_resale_price,
            allow_free_ticket_transfer: params.allow_free_ticket_transfer,
        };

        save_event(&env, &params.event_id, &event);
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
                &event.resale_royalty_bps,
                &event.max_resale_price,
                &event.allow_free_ticket_transfer,
            );
            // Register the (immutable) revenue split alongside the event config so
            // the payments contract can pay each recipient independently.
            if !params.revenue_splits.is_empty() {
                payments_client.sync_revenue_splits(
                    &env.current_contract_address(),
                    &params.event_id,
                    &params.revenue_splits,
                );
            }
        }
        let privacy = storage::get_event_privacy(&env, &params.event_id);
        emit_event_created(&env, &params, &privacy);

        Ok(event)
    }
    pub fn get_event(env: Env, event_id: Symbol) -> Result<Event, EventError> {
        storage::get_event(&env, &event_id)
    }
    pub fn get_event_status(env: Env, event_id: Symbol) -> Result<EventStatus, EventError> {
        let event = storage::get_event(&env, &event_id)?;
        Ok(event.status)
    }
    pub fn update_event_details(env: Env, params: UpdateEventParams) -> Result<Event, EventError> {
        params.organizer.require_auth();

        let mut event = storage::get_event(&env, &params.event_id)?;
        if event.organizer != params.organizer {
            return Err(EventError::Unauthorized);
        }
        if event.status != EventStatus::Upcoming {
            return Err(EventError::EventNotUpdatable);
        }
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
            let min_date = env.ledger().timestamp() + 86_400;
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

        if let Some(bps) = params.resale_royalty_bps {
            if event.sold_count > 0 {
                return Err(EventError::EventNotUpdatable); // Cannot change royalty after selling starts
            }
            if bps > 2000 {
                return Err(EventError::InvalidInput);
            }
            event.resale_royalty_bps = bps;
        }
        if let Some(cap_opt) = params.max_resale_price {
            if event.sold_count > 0 {
                return Err(EventError::EventNotUpdatable);
            }
            if let Some(cap) = cap_opt {
                if cap < 0 && cap != -1 {
                    return Err(EventError::InvalidInput);
                }
                if cap == -1 {
                    event.max_resale_price = None;
                } else {
                    event.max_resale_price = Some(cap);
                }
            } else {
                event.max_resale_price = None;
            }
        }
        if let Some(allow_transfer) = params.allow_free_ticket_transfer {
            event.allow_free_ticket_transfer = allow_transfer;
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
                &event.resale_royalty_bps,
                &event.max_resale_price,
                &event.allow_free_ticket_transfer,
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
    pub fn update_event_status(
        env: Env,
        organizer: Address,
        event_id: Symbol,
        new_status: EventStatus,
    ) -> Result<(), EventError> {
        organizer.require_auth();

        let mut event = storage::get_event(&env, &event_id)?;
        if event.organizer != organizer {
            return Err(EventError::Unauthorized);
        }
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
    pub fn cancel_event(env: Env, organizer: Address, event_id: Symbol) -> Result<(), EventError> {
        organizer.require_auth();

        let mut event = storage::get_event(&env, &event_id)?;
        if event.organizer != organizer {
            return Err(EventError::Unauthorized);
        }
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
        if has_linked_contracts(&env) {
            let payments_contract = get_payments_contract(&env)?;
            let payments_client = PaymentsContractClient::new(&env, &payments_contract);

            payments_client.cancel_event(&event_id, &organizer);
        }

        Ok(())
    }
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
        if event.status != EventStatus::Active {
            return Err(EventError::InvalidStatusTransition);
        }
        let count = storage::get_postpone_count(&env, &event_id);
        if count >= MAX_POSTPONEMENTS {
            return Err(EventError::MaxPostponementsReached);
        }
        if choice_window_ledgers < MIN_POSTPONEMENT_CHOICE_WINDOW_LEDGERS {
            return Err(EventError::PostponementWindowTooShort);
        }
        if choice_window_ledgers > MAX_POSTPONEMENT_CHOICE_WINDOW_LEDGERS {
            return Err(EventError::InvalidPostponementDate);
        }
        if new_date_ledger > u32::MAX as u64 {
            return Err(EventError::InvalidPostponementDate);
        }

        let current_ledger = env.ledger().sequence();
        let choice_deadline_ledger = current_ledger
            .checked_add(choice_window_ledgers)
            .ok_or(EventError::InvalidPostponementDate)?;
        if new_date_ledger <= choice_deadline_ledger as u64 {
            return Err(EventError::InvalidPostponementDate);
        }

        let old_status = event.status.clone();
        event.status = EventStatus::Postponed;
        update_event(&env, &event_id, &event)?;

        let postpone_count = count + 1;
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
        if has_linked_contracts(&env) {
            let payments_contract = get_payments_contract(&env)?;
            let payments_client = PaymentsContractClient::new(&env, &payments_contract);
            payments_client.postpone_event(&event_id, &organizer, &choice_deadline_ledger);
        }

        Ok(())
    }
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
        if (env.ledger().sequence() as u64) <= info.choice_deadline_ledger {
            return Err(EventError::PostponementWindowOpen);
        }
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
    pub fn get_postponement(env: Env, event_id: Symbol) -> Result<PostponementInfo, EventError> {
        storage::get_event(&env, &event_id)?;
        storage::get_postponement(&env, &event_id).ok_or(EventError::EventNotPostponed)
    }
    pub fn request_postponement_refund(
        env: Env,
        attendee: Address,
        ticket_id: u64,
    ) -> Result<(), EventError> {
        attendee.require_auth();

        let payments_contract = get_payments_contract(&env)?;
        let payments_client = PaymentsContractClient::new(&env, &payments_contract);
        let event_id = payments_client.get_ticket(&ticket_id).event_id;

        let event = storage::get_event(&env, &event_id)?;
        if event.status != EventStatus::Postponed {
            return Err(EventError::EventNotPostponed);
        }
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
        payments_client.request_postponement_refund(&attendee, &ticket_id);
        ticket_client.cancel_ticket(&revocable, &attendee);
        if !has_valid_ticket_for_event(&ticket_client, &attendee, &event_id) {
            storage::remove_registration(&env, &event_id, &attendee);
        }

        Ok(())
    }
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
        if storage::has_reservation(&env, &event_id, &attendee) {
            let reservation = storage::get_reservation(&env, &event_id, &attendee)?;
            if reservation.expires_at > env.ledger().timestamp() {
                return Ok(());
            } else {
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
        let expires_at = env.ledger().timestamp() + 900;
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
    pub fn release_expired_reservation(
        env: Env,
        event_id: Symbol,
        attendee: Address,
    ) -> Result<(), EventError> {
        let reservation = storage::get_reservation(&env, &event_id, &attendee)?;

        if reservation.expires_at > env.ledger().timestamp() {
            return Err(EventError::InvalidInput);
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
                return Err(EventError::InvalidInput);
            }

            for i in 0..event.tiers.len() {
                let tier = event.tiers.get(i).ok_or(EventError::TierNotFound)?;
                if tier.tier_id == tier_id {
                    tier_index = Some(i);
                    break;
                }
            }
        } else {
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
    pub fn withdraw_revenue(
        env: Env,
        organizer: Address,
        event_id: Symbol,
    ) -> Result<(), EventError> {
        organizer.require_auth();

        let event = storage::get_event(&env, &event_id)?;
        if event.organizer != organizer {
            return Err(EventError::Unauthorized);
        }
        if event.status != EventStatus::Completed {
            return Err(EventError::InvalidStatusTransition);
        }

        let payments_contract = storage::get_payments_contract(&env)?;
        let payments_client = PaymentsContractClient::new(&env, &payments_contract);
        payments_client.withdraw_revenue(&event_id, &organizer);

        Ok(())
    }
    pub fn get_withdrawal_history(
        env: Env,
        event_id: Symbol,
    ) -> Result<soroban_sdk::Vec<payments_contract::WithdrawalRecord>, EventError> {
        storage::get_event(&env, &event_id)?;
        let payments_contract = storage::get_payments_contract(&env)?;
        let payments_client = PaymentsContractClient::new(&env, &payments_contract);
        Ok(payments_client.get_withdrawal_history(&event_id))
    }
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
    pub fn get_event_privacy(env: Env, event_id: Symbol) -> PrivacyLevel {
        storage::get_event_privacy(&env, &event_id)
    }
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
    pub fn get_claim_settings(env: Env, event_id: Symbol) -> ClaimSettings {
        storage::get_claim_settings(&env, &event_id)
    }
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
        if storage::has_anon_commitment(&env, &event_id, &commitment) {
            return Err(EventError::AnonCommitmentReused);
        }
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
        let mut tier = event.tiers.get(index).ok_or(EventError::TierNotFound)?;

        if event.sold_count >= event.max_supply {
            return Err(EventError::EventSoldOut);
        }
        if tier.sold + tier.reserved >= tier.capacity {
            return Err(EventError::TierSoldOut);
        }
        storage::save_anon_commitment(&env, &event_id, &commitment);

        tier.sold += 1;
        event.sold_count += 1;
        event.tiers.set(index, tier.clone());
        storage::update_event(&env, &event_id, &event)?;

        emit_anon_registration(&env, &event_id, tier_id, tier.sold);

        Ok(())
    }
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
    pub fn get_anon_claim_settings(env: Env, event_id: Symbol) -> AnonClaimSettings {
        storage::get_anon_claim_settings(&env, &event_id)
    }
    pub fn verify_and_attend(
        env: Env,
        event_id: Symbol,
        tier_id: u32,
        claim: ZkPassportClaim,
    ) -> Result<(), EventError> {
        let mut event = storage::get_event(&env, &event_id)?;
        if event.status != EventStatus::Active {
            return Err(EventError::EventNotActive);
        }
        if !event.requires_verification {
            return Err(EventError::ZkVerificationRequired);
        }
        let zk_config = storage::get_zk_verification_config(&env, &event_id);
        if !zk_config.enabled {
            return Err(EventError::ZkVerificationRequired);
        }
        let current_ledger = env.ledger().sequence();
        if claim.expiry_ledger < current_ledger {
            return Err(EventError::ZkProofExpired);
        }
        if zk_config.required_claim_type != ZkClaimType::Any
            && claim.claim_type != zk_config.required_claim_type
        {
            return Err(EventError::ZkClaimTypeMismatch);
        }
        if storage::has_zk_nullifier(&env, &event_id, &claim.nullifier) {
            return Err(EventError::ZkNullifierReused);
        }
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
        if event.sold_count >= event.max_supply {
            return Err(EventError::EventSoldOut);
        }
        if tier.sold + tier.reserved >= tier.capacity {
            return Err(EventError::TierSoldOut);
        }
        if tier.price > 0 && has_linked_contracts(&env) {
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
        if has_linked_contracts(&env) {
            let ticket_contract = get_ticket_contract(&env)?;
            let ticket_client = TicketContractClient::new(&env, &ticket_contract);
            ticket_client.mint_ticket(
                &event.event_id,
                &event.organizer,
                &env.current_contract_address(),
            );
        }
        storage::save_zk_nullifier(&env, &event_id, &claim.nullifier);
        tier.sold += 1;
        event.sold_count += 1;
        event.tiers.set(index, tier.clone());
        storage::update_event(&env, &event_id, &event)?;
        emit_zk_verified_attendance(&env, &event_id, &claim.claim_type, tier_id, tier.sold);

        Ok(())
    }
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
    pub fn get_zk_config(env: Env, event_id: Symbol) -> ZkVerificationConfig {
        storage::get_zk_verification_config(&env, &event_id)
    }
    pub fn is_nullifier_used(env: Env, event_id: Symbol, nullifier: BytesN<32>) -> bool {
        storage::has_zk_nullifier(&env, &event_id, &nullifier)
    }
    pub fn contract_version(env: Env) -> u32 {
        storage::get_contract_version(&env)
    }
    pub fn migrate(env: Env, admin: Address) -> Result<u32, EventError> {
        admin.require_auth();

        let current_admin = storage::get_admin(&env)?;
        if current_admin != admin {
            return Err(EventError::Unauthorized);
        }

        let current_version = storage::get_contract_version(&env);
        let new_version = current_version + 1;
        match current_version {
            0 => {
                storage::set_contract_version(&env, 1);
            }
            1 => {
                storage::set_contract_version(&env, 2);
            }
            2 => {
                storage::set_contract_version(&env, 3);
            }
            _ => {
                return Err(EventError::UnsupportedVersion);
            }
        }

        Ok(new_version)
    }

    /// Get the configured revenue split for an event as `(Address, basis_points)`.
    /// An empty result means the event uses the legacy single-organizer payout.
    pub fn get_revenue_splits(
        env: Env,
        event_id: Symbol,
    ) -> Result<soroban_sdk::Vec<(Address, u32)>, EventError> {
        let event = storage::get_event(&env, &event_id)?;
        Ok(event.revenue_splits)
    }

    /// Withdraw the caller's allocated share of a split event's revenue.
    /// Proxies to the payments contract, which performs the actual settlement and
    /// transfer. Any configured recipient may call this independently.
    pub fn withdraw_split(
        env: Env,
        recipient: Address,
        event_id: Symbol,
    ) -> Result<(), EventError> {
        recipient.require_auth();
        storage::get_event(&env, &event_id)?;

        let payments_contract = storage::get_payments_contract(&env)?;
        let payments_client = PaymentsContractClient::new(&env, &payments_contract);
        payments_client.withdraw_split(&recipient, &event_id);

        Ok(())
    }

    /// Flag a co-host wallet as compromised. Only the primary organizer (the
    /// event organizer / split index 0) may call this. Proxies to the payments
    /// contract, which freezes the flagged recipient's share in escrow.
    pub fn flag_cohost(
        env: Env,
        primary_organizer: Address,
        event_id: Symbol,
        recipient: Address,
    ) -> Result<(), EventError> {
        primary_organizer.require_auth();

        let event = storage::get_event(&env, &event_id)?;
        // The primary organizer always retains admin rights regardless of split
        // percentage: the event's organizer is the split's index-0 recipient.
        if event.organizer != primary_organizer {
            return Err(EventError::Unauthorized);
        }

        let payments_contract = storage::get_payments_contract(&env)?;
        let payments_client = PaymentsContractClient::new(&env, &payments_contract);
        payments_client.flag_cohost(&primary_organizer, &event_id, &recipient);

        Ok(())
    }
}

/// Validate a revenue split. An empty split is allowed (legacy single-organizer
/// payout). When non-empty: 1–5 recipients, basis points summing to exactly
/// 10000, no zero allocations, no duplicate recipients, and index 0 must be the
/// primary organizer.
fn validate_revenue_splits(
    splits: &soroban_sdk::Vec<(Address, u32)>,
    organizer: &Address,
) -> Result<(), EventError> {
    let len = splits.len();
    if len == 0 {
        return Ok(());
    }
    if len > 5 {
        return Err(EventError::InvalidRevenueSplit);
    }

    let (first, _) = splits.get(0).ok_or(EventError::InvalidRevenueSplit)?;
    if first != *organizer {
        return Err(EventError::InvalidRevenueSplit);
    }

    let mut total: u32 = 0;
    for i in 0..len {
        let (recipient, bps) = splits.get(i).ok_or(EventError::InvalidRevenueSplit)?;
        if bps == 0 {
            return Err(EventError::InvalidRevenueSplit);
        }
        total = total
            .checked_add(bps)
            .ok_or(EventError::InvalidRevenueSplit)?;
        for j in 0..i {
            let (other, _) = splits.get(j).ok_or(EventError::InvalidRevenueSplit)?;
            if other == recipient {
                return Err(EventError::InvalidRevenueSplit);
            }
        }
    }

    if total != 10_000 {
        return Err(EventError::InvalidRevenueSplit);
    }

    Ok(())
}

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
