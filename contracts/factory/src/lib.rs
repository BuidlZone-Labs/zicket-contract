#![no_std]
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Symbol, Vec, IntoVal, vec};

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

    pub fn deploy_event(
        env: Env,
        organizer: Address,
        event_id: Symbol,
        salt: BytesN<32>,
        ticket_contract: Address,
        payments_contract: Address,
    ) -> Result<Address, FactoryError> {
        organizer.require_auth();

        if storage::get_deployed_event(&env, &event_id).is_ok() {
            return Err(FactoryError::EventAlreadyDeployed);
        }

        let wasm_hash = storage::get_event_wasm_hash(&env)?;
        let contract_address = deployment::deploy_event_contract(&env, &salt, &wasm_hash);

        let init_args = vec![
            &env,
            organizer.clone().into_val(&env),
            ticket_contract.into_val(&env),
            payments_contract.into_val(&env),
        ];
        env.invoke_contract::<()>(&contract_address, &Symbol::new(&env, "initialize"), init_args);

        let deployed_event = DeployedEvent {
            event_id: event_id.clone(),
            contract_address: contract_address.clone(),
            organizer: organizer.clone(),
            deployed_at: env.ledger().timestamp(),
        };

        storage::save_deployed_event(&env, &deployed_event)?;

        events::emit_event_deployed(&env, &event_id, &contract_address, &organizer);

        Ok(contract_address)
    }

    pub fn get_event_address(env: Env, event_id: Symbol) -> Result<Address, FactoryError> {
        let event = storage::get_deployed_event(&env, &event_id)?;
        Ok(event.contract_address)
    }

    pub fn get_all_events(env: Env) -> Vec<Symbol> {
        storage::get_all_event_ids(&env)
    }

    pub fn get_organizer_events(env: Env, organizer: Address) -> Vec<Symbol> {
        storage::get_organizer_events(&env, &organizer)
    }
}

#[cfg(test)]
mod test;
