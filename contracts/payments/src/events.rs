use privacy_utils::{mask_address, MaskedAddress, PrivacyLevel};
use soroban_sdk::{contractevent, Address, BytesN, Env, Symbol};

fn event_type(env: &Env, name: &str) -> Symbol {
    Symbol::new(env, name)
}

#[contractevent(data_format = "vec", topics = ["payment"])]
pub struct PaymentReceived {
    pub event_type: Symbol,
    pub payment_id: u64,
    pub event_id: Symbol,
    pub payer: MaskedAddress,
    pub amount: i128,
    pub token: Address,
    pub paid_at: u64,
}

#[contractevent(data_format = "vec", topics = ["receipt_requested"])]
pub struct PaymentReceiptRequested {
    pub event_type: Symbol,
    pub payment_id: u64,
    pub event_id: Symbol,
    pub email_hash: Option<BytesN<32>>,
    pub requested_at: u64,
}

pub fn emit_payment_receipt_requested(
    env: &Env,
    payment_id: u64,
    event_id: Symbol,
    email_hash: Option<BytesN<32>>,
) {
    PaymentReceiptRequested {
        event_type: event_type(env, "receipt_requested"),
        payment_id,
        event_id,
        email_hash,
        requested_at: env.ledger().timestamp(),
    }
    .publish(env);
}

#[contractevent(data_format = "vec", topics = ["refund"])]
pub struct PaymentRefunded {
    pub event_type: Symbol,
    pub payment_id: u64,
    pub event_id: Symbol,
    pub payer: MaskedAddress,
    pub amount: i128,
    pub token: Address,
    pub refunded_at: u64,
}

#[contractevent(data_format = "vec", topics = ["ticket_issued"])]
pub struct TicketIssued {
    pub event_type: Symbol,
    pub ticket_id: u64,
    pub event_id: Symbol,
    pub owner: MaskedAddress,
    pub payment_id: u64,
    pub issued_at: u64,
}

#[contractevent(data_format = "vec", topics = ["withdrawal"])]
pub struct RevenueWithdrawn {
    pub event_type: Symbol,
    pub event_id: Symbol,
    pub organizer: MaskedAddress,
    pub amount: i128,
    pub token: Address,
    pub to: Address,
    pub withdrawn_at: u64,
}

#[contractevent(data_format = "vec", topics = ["escrow_released"])]
pub struct EscrowAutoReleased {
    pub event_type: Symbol,
    pub event_id: Symbol,
    pub organizer: Address,
    pub amount: i128,
    pub released_at: u64,
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
        event_type: event_type(env, "payment_received"),
        payment_id,
        event_id,
        payer: mask_address(env, &payer, level.clone()),
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
    token: Address,
    to: Address,
    level: &PrivacyLevel,
) {
    RevenueWithdrawn {
        event_type: event_type(env, "revenue_withdrawn"),
        event_id,
        organizer: mask_address(env, &organizer, level.clone()),
        amount,
        token,
        to,
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
    token: Address,
    level: &PrivacyLevel,
) {
    PaymentRefunded {
        event_type: event_type(env, "payment_refunded"),
        payment_id,
        event_id,
        payer: mask_address(env, &payer, level.clone()),
        amount,
        token,
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
        event_type: event_type(env, "ticket_issued"),
        ticket_id,
        event_id,
        owner: mask_address(env, &owner, level.clone()),
        payment_id,
        issued_at: env.ledger().timestamp(),
    }
    .publish(env);
}

pub fn emit_escrow_auto_released(env: &Env, event_id: Symbol, organizer: Address, amount: i128) {
    EscrowAutoReleased {
        event_type: event_type(env, "escrow_auto_released"),
        event_id,
        organizer,
        amount,
        released_at: env.ledger().timestamp(),
    }
    .publish(env);
}

#[contractevent(data_format = "vec", topics = ["platform_fee"])]
pub struct PlatformFeeCollected {
    pub event_type: Symbol,
    pub event_id: Symbol,
    pub fee_amount: i128,
    pub organizer_amount: i128,
    pub token: Address,
    pub collected_at: u64,
}

pub fn emit_platform_fee_collected(
    env: &Env,
    event_id: Symbol,
    fee_amount: i128,
    organizer_amount: i128,
    token: Address,
) {
    PlatformFeeCollected {
        event_type: event_type(env, "platform_fee_collected"),
        event_id,
        fee_amount,
        organizer_amount,
        token,
        collected_at: env.ledger().timestamp(),
    }
    .publish(env);
}

#[contractevent(data_format = "vec", topics = ["platform_fee_updated"])]
pub struct PlatformFeeUpdated {
    pub event_type: Symbol,
    pub admin: Address,
    pub old_bps: u32,
    pub new_bps: u32,
    pub updated_at: u64,
}

pub fn emit_platform_fee_updated(env: &Env, admin: Address, old_bps: u32, new_bps: u32) {
    PlatformFeeUpdated {
        event_type: event_type(env, "platform_fee_updated"),
        admin,
        old_bps,
        new_bps,
        updated_at: env.ledger().timestamp(),
    }
    .publish(env);
}

#[contractevent(data_format = "vec", topics = ["platform_withdrawal"])]
pub struct PlatformRevenueWithdrawn {
    pub event_type: Symbol,
    pub event_id: Symbol,
    pub amount: i128,
    pub token: Address,
    pub to: Address,
    pub withdrawn_at: u64,
}

pub fn emit_platform_revenue_withdrawn(
    env: &Env,
    event_id: Symbol,
    amount: i128,
    token: Address,
    to: Address,
) {
    PlatformRevenueWithdrawn {
        event_type: event_type(env, "platform_revenue_withdrawn"),
        event_id,
        amount,
        token,
        to,
        withdrawn_at: env.ledger().timestamp(),
    }
    .publish(env);
}
