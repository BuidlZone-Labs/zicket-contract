#![no_std]
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Symbol};

mod deployment;
mod errors;
mod events;
mod storage;
mod types;

pub use errors::*;
pub use events::*;
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
        ticket_contract: Address,
        payments_contract: Address,
    ) -> Result<(), FactoryError> {
        if storage::is_initialized(&env) {
            return Ok(());
        }

        admin.require_auth();

        storage::set_admin(&env, &admin);
        storage::set_event_wasm_hash(&env, &event_wasm_hash);
        storage::set_ticket_contract(&env, &ticket_contract);
        storage::set_payments_contract(&env, &payments_contract);

        FactoryInitialized {
            admin: admin.clone(),
        }
        .publish(&env);

        Ok(())
    }

    pub fn deploy_event(
        env: Env,
        organizer: Address,
        event_id: Symbol,
        salt: BytesN<32>,
    ) -> Result<Address, FactoryError> {
        organizer.require_auth();

        if storage::get_deployed_event(&env, &event_id).is_ok() {
            return Err(FactoryError::EventAlreadyDeployed);
        }

        let wasm_hash = storage::get_event_wasm_hash(&env)?;
        let ticket_contract = storage::get_ticket_contract(&env)?;
        let payments_contract = storage::get_payments_contract(&env)?;

        let contract_address = deployment::deploy_event(
            &env,
            &organizer,
            &wasm_hash,
            &salt,
            &ticket_contract,
            &payments_contract,
        );

        let deployed_event = DeployedEvent {
            event_id: event_id.clone(),
            contract_address: contract_address.clone(),
            organizer: organizer.clone(),
            deployed_at: env.ledger().timestamp(),
        };

        storage::save_deployed_event(&env, &deployed_event)?;

        events::emit_event_deployed(
            &env,
            event_id,
            contract_address.clone(),
            organizer,
            privacy_utils::PrivacyLevel::Standard,
        );

        Ok(contract_address)
    }

    pub fn get_deployed_event(env: Env, event_id: Symbol) -> Result<DeployedEvent, FactoryError> {
        storage::get_deployed_event(&env, &event_id)
    }

    pub fn get_event_address(env: Env, event_id: Symbol) -> Result<Address, FactoryError> {
        let deployed = storage::get_deployed_event(&env, &event_id)?;
        Ok(deployed.contract_address)
    }

    pub fn get_all_events(env: Env) -> soroban_sdk::Vec<Symbol> {
        storage::get_all_event_ids(&env)
    }

    pub fn get_organizer_events(env: Env, organizer: Address) -> soroban_sdk::Vec<Symbol> {
        storage::get_organizer_events(&env, &organizer)
    }
}

#[cfg(test)]
mod test;
