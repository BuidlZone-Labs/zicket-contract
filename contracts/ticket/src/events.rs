use soroban_sdk::{symbol_short, Address, Env, Symbol};

pub fn emit_ticket_minted(env: &Env, ticket_id: u64, event_id: Symbol, owner: Address) {
    let topics = (symbol_short!("minted"), event_id, owner);
    env.events().publish(topics, ticket_id);
}

pub fn emit_ticket_used(env: &Env, ticket_id: u64) {
    let topics = (symbol_short!("used"),);
    env.events().publish(topics, ticket_id);
}

pub fn emit_ticket_cancelled(env: &Env, ticket_id: u64) {
    let topics = (symbol_short!("cancelled"),);
    env.events().publish(topics, ticket_id);
}
