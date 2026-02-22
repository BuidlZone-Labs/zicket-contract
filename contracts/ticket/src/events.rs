use soroban_sdk::{Address, Env, Symbol};

pub fn emit_ticket_transferred(env: &Env, ticket_id: u64, from: Address, to: Address) {
    let topics = (Symbol::new(env, "ticket_transferred"), ticket_id);
    env.events().publish(topics, (from, to));
}

pub fn emit_ticket_used(env: &Env, ticket_id: u64) {
    let topics = (Symbol::new(env, "ticket_used"), ticket_id);
    env.events().publish(topics, ticket_id);
}
