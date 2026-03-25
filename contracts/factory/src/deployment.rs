use soroban_sdk::{contractclient, Address, BytesN, Env};

#[contractclient(name = "EventClient")]
#[allow(dead_code)]
pub trait EventContractTrait {
    fn initialize(
        env: Env,
        admin: Address,
        ticket_contract: Address,
        payments_contract: Address,
    ) -> Result<(), crate::errors::FactoryError>;
}

#[allow(dead_code)]
pub fn deploy_event(
    env: &Env,
    organizer: &Address,
    wasm_hash: &BytesN<32>,
    salt: &BytesN<32>,
    ticket_contract: &Address,
    payments_contract: &Address,
) -> Address {
    let deployed_address = env
        .deployer()
        .with_address(env.current_contract_address(), salt.clone())
        .deploy_v2(wasm_hash.clone(), ());

    let client = EventClient::new(env, &deployed_address);
    client.initialize(organizer, ticket_contract, payments_contract);

    deployed_address
}
