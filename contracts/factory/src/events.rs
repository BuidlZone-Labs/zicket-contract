use soroban_sdk::{contractevent, Address, Env, Symbol};

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactoryInitialized {
    pub admin: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventDeployed {
    pub event_id: Symbol,
    pub contract_address: Address,
    pub organizer: Address,
}

pub fn emit_event_deployed(
    env: &Env,
    event_id: Symbol,
    contract_address: Address,
    organizer: Address,
) {
    EventDeployed {
        event_id,
        contract_address,
        organizer,
    }
    .publish(env);
}
