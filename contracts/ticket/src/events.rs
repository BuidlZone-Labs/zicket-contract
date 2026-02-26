use soroban_sdk::{contractevent, Address, Env};

#[contractevent(data_format = "vec", topics = ["ticket_transferred"])]
pub struct TicketTransferred {
    #[topic]
    pub ticket_id: u64,
    pub from: Address,
    pub to: Address,
}

#[contractevent(data_format = "single-value", topics = ["ticket_used"])]
pub struct TicketUsed {
    #[topic]
    pub ticket_id: u64,
}

#[contractevent(data_format = "single-value", topics = ["ticket_minted"])]
pub struct TicketMinted {
    #[topic]
    pub ticket_id: u64,
}

#[contractevent(data_format = "single-value", topics = ["ticket_cancelled"])]
pub struct TicketCancelled {
    #[topic]
    pub ticket_id: u64,
}

pub fn emit_ticket_transferred(env: &Env, ticket_id: u64, from: Address, to: Address) {
    TicketTransferred {
        ticket_id,
        from,
        to,
    }
    .publish(env);
}

pub fn emit_ticket_used(env: &Env, ticket_id: u64) {
    TicketUsed { ticket_id }.publish(env);
}

pub fn emit_ticket_minted(env: &Env, ticket_id: u64) {
    TicketMinted { ticket_id }.publish(env);
}

pub fn emit_ticket_cancelled(env: &Env, ticket_id: u64) {
    TicketCancelled { ticket_id }.publish(env);
}
