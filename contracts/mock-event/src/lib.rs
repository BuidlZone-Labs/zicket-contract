#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct MockEventContract;

#[contractimpl]
impl MockEventContract {
    pub fn initialize(
        _env: Env,
        _admin: Address,
        _ticket_contract: Address,
        _payments_contract: Address,
    ) {
    }
}
