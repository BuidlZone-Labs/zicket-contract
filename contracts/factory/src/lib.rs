#![no_std]
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Symbol};

mod deployment;
mod errors;
mod events;
mod storage;
mod types;

pub use errors::*;
pub use storage::*;
pub use types::*;

#[contract]
pub struct FactoryContract;

#[contractimpl]
impl FactoryContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        event_wasm_hash: BytesN<32>,
    ) -> Result<(), FactoryError> {
        if storage::is_initialized(&env) {
            return Ok(());
        }

        admin.require_auth();

        storage::set_admin(&env, &admin);
        storage::set_event_wasm_hash(&env, &event_wasm_hash);

        events::emit_factory_initialized(&env, &admin);

        Ok(())
    }

    pub fn get_deployed_event(env: Env, event_id: Symbol) -> Result<DeployedEvent, FactoryError> {
        storage::get_deployed_event(&env, &event_id)
    }
}

#[cfg(test)]
mod test;
