#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

#[contracttype]
#[derive(Clone)]
enum DataKey {
    EventConfig(Symbol),
}

#[contracttype]
#[derive(Clone)]
struct EventConfig {
    allow_anonymous: bool,
    requires_verification: bool,
}

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

    pub fn configure_event(
        env: Env,
        event_id: Symbol,
        allow_anonymous: bool,
        requires_verification: bool,
    ) {
        let config = EventConfig {
            allow_anonymous,
            requires_verification,
        };
        env.storage()
            .persistent()
            .set(&DataKey::EventConfig(event_id), &config);
    }

    pub fn get_allow_anonymous(env: Env, event_id: Symbol) -> bool {
        env.storage()
            .persistent()
            .get::<_, EventConfig>(&DataKey::EventConfig(event_id))
            .map(|config| config.allow_anonymous)
            .unwrap_or(true)
    }

    pub fn get_requires_verification(env: Env, event_id: Symbol) -> bool {
        env.storage()
            .persistent()
            .get::<_, EventConfig>(&DataKey::EventConfig(event_id))
            .map(|config| config.requires_verification)
            .unwrap_or(false)
    }
}
