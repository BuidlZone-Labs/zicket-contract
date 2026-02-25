use soroban_sdk::{contractevent, Address};

#[contractevent]
pub struct FactoryInitialized {
    pub admin: Address,
}
