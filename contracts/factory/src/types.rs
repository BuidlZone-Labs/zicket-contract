use soroban_sdk::{contracttype, Address, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeployedEvent {
    pub event_id: Symbol,
    pub contract_address: Address,
    pub organizer: Address,
    pub deployed_at: u64,
}
