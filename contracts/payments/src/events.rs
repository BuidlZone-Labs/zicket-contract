use crate::types::{PaymentPrivacy, PaymentRecord, Ticket};
use privacy_utils::{mask_address, MaskedAddress, PrivacyLevel};
use soroban_sdk::{contractevent, Address, BytesN, Env, Symbol};

fn event_type(env: &Env, name: &str) -> Symbol {
    Symbol::new(env, name)
}

/// Map the per-payment `PaymentPrivacy` to the emission `PrivacyLevel` used for masking.
pub fn payment_privacy_to_level(level: &PaymentPrivacy) -> PrivacyLevel {
    match level {
        PaymentPrivacy::Standard => PrivacyLevel::Standard,
        PaymentPrivacy::Private => PrivacyLevel::Private,
        PaymentPrivacy::Anonymous => PrivacyLevel::Anonymous,
    }
}

/// Derive the masked identity that should appear in an event for a payment,
/// honouring the per-payment privacy level. No raw address is ever emitted for
/// Private or Anonymous payments — only the stored hash/commitment is exposed.
fn masked_payment_identity(env: &Env, payment: &PaymentRecord) -> MaskedAddress {
    match payment.privacy_level {
        PaymentPrivacy::Standard => match &payment.payer {
            Some(addr) => MaskedAddress::Full(addr.clone()),
            None => MaskedAddress::Hashed(BytesN::from_array(env, &[0u8; 32])),
        },
        PaymentPrivacy::Private => match &payment.hashed_wallet {
            Some(hash) => MaskedAddress::Hashed(hash.clone()),
            None => MaskedAddress::Hashed(BytesN::from_array(env, &[0u8; 32])),
        },
        PaymentPrivacy::Anonymous => match &payment.nullifier_commitment {
            Some(commitment) => MaskedAddress::Hashed(commitment.clone()),
            None => MaskedAddress::Hashed(BytesN::from_array(env, &[0u8; 32])),
        },
    }
}

/// Derive the masked identity for a ticket, honouring the per-ticket privacy level.
fn masked_ticket_identity(env: &Env, ticket: &Ticket) -> MaskedAddress {
    match ticket.privacy_level {
        PaymentPrivacy::Standard => match &ticket.owner {
            Some(addr) => MaskedAddress::Full(addr.clone()),
            None => MaskedAddress::Hashed(BytesN::from_array(env, &[0u8; 32])),
        },
        PaymentPrivacy::Private => match &ticket.hashed_owner {
            Some(hash) => MaskedAddress::Hashed(hash.clone()),
            None => MaskedAddress::Hashed(BytesN::from_array(env, &[0u8; 32])),
        },
        PaymentPrivacy::Anonymous => match &ticket.nullifier_commitment {
            Some(commitment) => MaskedAddress::Hashed(commitment.clone()),
            None => MaskedAddress::Hashed(BytesN::from_array(env, &[0u8; 32])),
        },
    }
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

/// Emit a payment-received event using the payment's own privacy level so the
/// emitted identity (full address / hashed wallet / nullifier commitment)
/// matches what is stored on-chain for that exact payment.
pub fn emit_payment_received(env: &Env, payment: &PaymentRecord) {
    PaymentReceived {
        event_type: event_type(env, "payment_received"),
        payment_id: payment.payment_id,
        event_id: payment.event_id.clone(),
        payer: masked_payment_identity(env, payment),
        amount: payment.amount,
        token: payment.token.clone(),
        paid_at: payment.paid_at,
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

/// Emit a refund event preserving the original payment's privacy level. The
/// identity exposed is derived from the stored payment record, so an Anonymous
/// payment refunds with its commitment and a Private payment with its hash —
/// never a raw address.
pub fn emit_payment_refunded(env: &Env, payment: &PaymentRecord, amount: i128) {
    PaymentRefunded {
        event_type: event_type(env, "payment_refunded"),
        payment_id: payment.payment_id,
        event_id: payment.event_id.clone(),
        payer: masked_payment_identity(env, payment),
        amount,
        token: payment.token.clone(),
        refunded_at: env.ledger().timestamp(),
    }
    .publish(env);
}

/// Emit a ticket-issued event using the ticket's own privacy level.
pub fn emit_ticket_issued(env: &Env, ticket: &Ticket) {
    TicketIssued {
        event_type: event_type(env, "ticket_issued"),
        ticket_id: ticket.ticket_id,
        event_id: ticket.event_id.clone(),
        owner: masked_ticket_identity(env, ticket),
        payment_id: ticket.payment_id,
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
