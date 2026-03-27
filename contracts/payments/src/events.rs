use soroban_sdk::{contractevent, Address, Env, Symbol};

use crate::types::PrivacyLevel;

/// Returns Some(address) for Standard, None for Private or Anonymous.
pub fn mask_address(_env: &Env, address: &Address, level: &PrivacyLevel) -> Option<Address> {
    match level {
        PrivacyLevel::Standard => Some(address.clone()),
        PrivacyLevel::Private | PrivacyLevel::Anonymous => None,
    }
}

#[contractevent(data_format = "vec", topics = ["payment"])]
pub struct PaymentReceived {
    pub payment_id: u64,
    pub event_id: Symbol,
    pub payer: Option<Address>,
    pub amount: i128,
    pub token: Address,
    pub paid_at: u64,
}

#[contractevent(data_format = "vec", topics = ["refund"])]
pub struct PaymentRefunded {
    pub payment_id: u64,
    pub event_id: Symbol,
    pub payer: Option<Address>,
    pub amount: i128,
    pub refunded_at: u64,
}

#[contractevent(data_format = "vec", topics = ["ticket_issued"])]
pub struct TicketIssued {
    pub ticket_id: u64,
    pub event_id: Symbol,
    pub owner: Option<Address>,
    pub payment_id: u64,
}

#[contractevent(data_format = "vec", topics = ["withdrawal"])]
pub struct RevenueWithdrawn {
    pub event_id: Symbol,
    pub organizer: Option<Address>,
    pub amount: i128,
    pub withdrawn_at: u64,
}

#[allow(clippy::too_many_arguments)]
pub fn emit_payment_received(
    env: &Env,
    payment_id: u64,
    event_id: Symbol,
    payer: Address,
    amount: i128,
    token: Address,
    paid_at: u64,
    level: &PrivacyLevel,
) {
    PaymentReceived {
        payment_id,
        event_id,
        payer: mask_address(env, &payer, level),
        amount,
        token,
        paid_at,
    }
    .publish(env);
}

pub fn emit_revenue_withdrawn(
    env: &Env,
    event_id: Symbol,
    organizer: Address,
    amount: i128,
    level: &PrivacyLevel,
) {
    RevenueWithdrawn {
        event_id,
        organizer: mask_address(env, &organizer, level),
        amount,
        withdrawn_at: env.ledger().timestamp(),
    }
    .publish(env);
}

pub fn emit_payment_refunded(
    env: &Env,
    payment_id: u64,
    event_id: Symbol,
    payer: Address,
    amount: i128,
    level: &PrivacyLevel,
) {
    PaymentRefunded {
        payment_id,
        event_id,
        payer: mask_address(env, &payer, level),
        amount,
        refunded_at: env.ledger().timestamp(),
    }
    .publish(env);
}

pub fn emit_ticket_issued(
    env: &Env,
    ticket_id: u64,
    event_id: Symbol,
    owner: Address,
    payment_id: u64,
    level: &PrivacyLevel,
) {
    TicketIssued {
        ticket_id,
        event_id,
        owner: mask_address(env, &owner, level),
        payment_id,
    }
    .publish(env);
}

#[contractevent(data_format = "vec", topics = ["escrow_released"])]
pub struct EscrowAutoReleased {
    pub event_id: Symbol,
    pub organizer: Address,
    pub amount: i128,
}

pub fn emit_escrow_auto_released(env: &Env, event_id: Symbol, organizer: Address, amount: i128) {
    EscrowAutoReleased {
        event_id,
        organizer,
        amount,
    }
    .publish(env);
}
