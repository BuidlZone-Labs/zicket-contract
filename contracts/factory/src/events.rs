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
