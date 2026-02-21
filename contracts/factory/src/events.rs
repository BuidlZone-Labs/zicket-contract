use soroban_sdk::{Address, Env, Symbol};

pub fn emit_factory_initialized(env: &Env, admin: &Address) {
    env.events()
        .publish((Symbol::new(env, "factory_initialized"),), admin.clone());
}
