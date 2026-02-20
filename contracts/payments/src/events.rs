use soroban_sdk::{symbol_short, Address, Env, Symbol};

pub fn emit_payment_received(
    env: &Env,
    payment_id: u64,
    event_id: Symbol,
    payer: Address,
    amount: i128,
) {
    env.events().publish(
        (symbol_short!("payment"),),
        (payment_id, event_id, payer, amount),
    );
}
