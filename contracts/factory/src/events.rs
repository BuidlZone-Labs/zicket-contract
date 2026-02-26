use soroban_sdk::{contractevent, Address, Env};

#[contractevent(data_format = "single-value", topics = ["factory_initialized"])]
pub struct FactoryInitialized {
    pub admin: Address,
}

pub fn emit_factory_initialized(env: &Env, admin: &Address) {
    FactoryInitialized {
        admin: admin.clone(),
    }
    .publish(env);
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
