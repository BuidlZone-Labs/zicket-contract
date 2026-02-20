use soroban_sdk::{Address, Env, Symbol};

pub fn emit_ticket_transferred(env: &Env, ticket_id: u64, from: Address, to: Address) {
    let topics = (Symbol::new(env, "ticket_transferred"), ticket_id);
    env.events().publish(topics, (from, to));
}
