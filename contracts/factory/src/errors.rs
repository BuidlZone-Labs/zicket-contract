use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum FactoryError {
    Unauthorized = 1,
    EventAlreadyDeployed = 2,
    EventNotFoundInRegistry = 3,
    NotInitialized = 4,
}
