use soroban_sdk::{contractevent, Address, Env, Symbol};

use crate::types::{
    mask_address, CreateEventParams, Event, EventStatus, MaskedAddress, PrivacyLevel, ZkClaimType,
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
    pub organizer: MaskedAddress,
    pub cancelled_at: u64,
}

#[contractevent(data_format = "vec", topics = ["refs_prc"])]
pub struct _RefundsProcessed {
    pub event_id: Symbol,
    pub refund_count: u32,
    pub processed_at: u64,
}

#[contractevent(data_format = "vec", topics = ["ev_pp"])]
pub struct EventPostponed {
    pub event_id: Symbol,
    pub new_date_ledger: u64,
    pub choice_deadline_ledger: u64,
    pub postpone_count: u32,
    pub postponed_at: u64,
}

#[contractevent(data_format = "vec", topics = ["ev_rsm"])]
pub struct EventResumed {
    pub event_id: Symbol,
    pub new_start_ledger: u32,
    pub new_end_ledger: u32,
    pub resumed_at: u64,
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
/// The organizer address is masked according to the event's privacy level.
pub fn emit_event_cancelled(
    env: &Env,
    event_id: &Symbol,
    organizer: &Address,
    level: &PrivacyLevel,
) {
    EventCancelled {
        event_id: event_id.clone(),
        organizer: mask_address(env, organizer, level.clone()),
        cancelled_at: env.ledger().timestamp(),
    }
    .publish(env);
}

// pub fn emit_refunds_processed(env: &Env, event_id: &Symbol, refund_count: u32) {
//     RefundsProcessed {
//         event_id: event_id.clone(),
//         refund_count,
//         processed_at: env.ledger().timestamp(),
//     }
//     .publish(env);
// }

/// Publish a Soroban event when an event is postponed (rescheduled).
///
/// Carries no address-derivable fields, so it is privacy-safe for all levels.
pub fn emit_event_postponed(
    env: &Env,
    event_id: &Symbol,
    new_date_ledger: u64,
    choice_deadline_ledger: u64,
    postpone_count: u32,
) {
    EventPostponed {
        event_id: event_id.clone(),
        new_date_ledger,
        choice_deadline_ledger,
        postpone_count,
        postponed_at: env.ledger().timestamp(),
    }
    .publish(env);
}

/// Publish a Soroban event when a postponed event is finalized back to `Active`
/// on its new schedule.
pub fn emit_event_resumed(
    env: &Env,
    event_id: &Symbol,
    new_start_ledger: u32,
    new_end_ledger: u32,
) {
    EventResumed {
        event_id: event_id.clone(),
        new_start_ledger,
        new_end_ledger,
        resumed_at: env.ledger().timestamp(),
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

#[contractevent(data_format = "vec", topics = ["anon_reg"])]
pub struct AnonEventRegistration {
    pub event_id: Symbol,
    pub tier_id: u32,
    pub tickets_sold: u32,
    pub registered_at: u64,
}

/// Publish a Soroban event for an anonymous (no-wallet) free ticket claim.
/// No attendee identifier is emitted — the commitment is kept off-chain.
pub fn emit_anon_registration(env: &Env, event_id: &Symbol, tier_id: u32, tickets_sold: u32) {
    AnonEventRegistration {
        event_id: event_id.clone(),
        tier_id,
        tickets_sold,
        registered_at: env.ledger().timestamp(),
    }
    .publish(env);
}

// ── zkPassport events ───────────────────────────────────────────────────────────

/// Emitted when an attendee successfully registers via a valid zkPassport proof.
///
/// # Privacy design
/// - The **nullifier** is deliberately omitted from the event payload. Publishing
///   it on-chain would allow any observer to correlate proof submissions across
///   events. Callers who need the nullifier can read it from storage directly.
/// - Only `claim_type` is emitted so that indexers can aggregate verification
///   statistics without linking individual attendees.
#[contractevent(data_format = "vec", topics = ["zk_attend"])]
pub struct ZkVerifiedAttendance {
    pub event_id: Symbol,
    pub claim_type: ZkClaimType,
    pub tier_id: u32,
    pub tickets_sold: u32,
    pub registered_at: u64,
}

/// Publish a Soroban event when a zkPassport-verified attendee registers.
/// The nullifier and raw proof are intentionally not included in the event
/// data to prevent correlation attacks.
pub fn emit_zk_verified_attendance(
    env: &Env,
    event_id: &Symbol,
    claim_type: &ZkClaimType,
    tier_id: u32,
    tickets_sold: u32,
) {
    ZkVerifiedAttendance {
        event_id: event_id.clone(),
        claim_type: claim_type.clone(),
        tier_id,
        tickets_sold,
        registered_at: env.ledger().timestamp(),
    }
    .publish(env);
}
