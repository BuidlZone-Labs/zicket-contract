use soroban_sdk::{Address, Env, Symbol};

pub fn emit_factory_initialized(env: &Env, admin: &Address) {
    env.events()
        .publish((Symbol::new(env, "factory_initialized"),), admin.clone());
}

pub fn emit_event_deployed(
    env: &Env,
    event_id: &Symbol,
    contract_address: &Address,
    organizer: &Address,
) {
    env.events().publish(
        (Symbol::new(env, "event_deployed"), event_id.clone()),
        (contract_address.clone(), organizer.clone()),
    );
}
