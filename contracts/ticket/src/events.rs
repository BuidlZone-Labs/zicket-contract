use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent(data_format = "vec", topics = ["ticket_transferred"])]
pub struct TicketTransferred {
    pub ticket_id: u64,
    pub event_id: Symbol,
    pub from: Address,
    pub to: Address,
    pub transferred_at: u64,
}

#[contractevent(data_format = "vec", topics = ["ticket_used"])]
pub struct TicketUsed {
    pub ticket_id: u64,
    pub event_id: Symbol,
    pub owner: Address,
    pub used_at: u64,
}

#[contractevent(data_format = "vec", topics = ["ticket_minted"])]
pub struct TicketMinted {
    pub ticket_id: u64,
    pub event_id: Symbol,
    pub owner: Address,
    pub organizer: Address,
    pub issued_at: u64,
}

#[contractevent(data_format = "vec", topics = ["ticket_cancelled"])]
pub struct TicketCancelled {
    pub ticket_id: u64,
    pub event_id: Symbol,
    pub owner: Address,
    pub cancelled_at: u64,
}

#[contractevent(data_format = "vec", topics = ["ticket_recovery_key_set"])]
pub struct TicketRecoveryKeySet {
    pub ticket_id: u64,
    pub owner: Address,
    pub set_at: u64,
}

#[contractevent(data_format = "vec", topics = ["ticket_recovered"])]
pub struct TicketRecovered {
    pub ticket_id: u64,
    pub old_owner: Address,
    pub new_owner: Address,
    pub recovered_at: u64,
}

pub fn emit_ticket_transferred(
    env: &Env,
    ticket_id: u64,
    event_id: Symbol,
    from: Address,
    to: Address,
) {
    TicketTransferred {
        ticket_id,
        event_id,
        from,
        to,
        transferred_at: env.ledger().timestamp(),
    }
    .publish(env);
}

pub fn emit_ticket_used(env: &Env, ticket_id: u64, event_id: Symbol, owner: Address) {
    TicketUsed {
        ticket_id,
        event_id,
        owner,
        used_at: env.ledger().timestamp(),
    }
    .publish(env);
}

pub fn emit_ticket_minted(
    env: &Env,
    ticket_id: u64,
    event_id: Symbol,
    owner: Address,
    organizer: Address,
    issued_at: u64,
) {
    TicketMinted {
        ticket_id,
        event_id,
        owner,
        organizer,
        issued_at,
    }
    .publish(env);
}

pub fn emit_ticket_cancelled(env: &Env, ticket_id: u64, event_id: Symbol, owner: Address) {
    TicketCancelled {
        ticket_id,
        event_id,
        owner,
        cancelled_at: env.ledger().timestamp(),
    }
    .publish(env);
}

pub fn emit_ticket_recovery_key_set(env: &Env, ticket_id: u64, owner: Address) {
    TicketRecoveryKeySet {
        ticket_id,
        owner,
        set_at: env.ledger().timestamp(),
    }
    .publish(env);
}

pub fn emit_ticket_recovered(env: &Env, ticket_id: u64, old_owner: Address, new_owner: Address) {
    TicketRecovered {
        ticket_id,
        old_owner,
        new_owner,
        recovered_at: env.ledger().timestamp(),
    }
    .publish(env);
}
