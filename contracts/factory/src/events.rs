use privacy_utils::{mask_address, MaskedAddress, PrivacyLevel};
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
    pub organizer: MaskedAddress,
}

pub fn emit_event_deployed(
    env: &Env,
    event_id: Symbol,
    contract_address: Address,
    organizer: Address,
    privacy_level: PrivacyLevel,
) {
    EventDeployed {
        event_id,
        contract_address,
        organizer: mask_address(env, &organizer, privacy_level),
    }
    .publish(env);
}
