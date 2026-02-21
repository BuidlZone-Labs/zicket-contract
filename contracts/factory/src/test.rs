use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, BytesN, Env, Symbol};

fn test_wasm_hash(env: &Env, fill: u8) -> BytesN<32> {
    BytesN::from_array(env, &[fill; 32])
}

#[test]
fn test_initialize_stores_admin_and_event_wasm_hash() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(FactoryContract, ());
    let client = FactoryContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let event_wasm_hash = test_wasm_hash(&env, 7);

    client.initialize(&admin, &event_wasm_hash);

    let stored_admin = env
        .as_contract(&contract_id, || storage::get_admin(&env))
        .unwrap();
    let stored_hash = env
        .as_contract(&contract_id, || storage::get_event_wasm_hash(&env))
        .unwrap();

    assert_eq!(stored_admin, admin);
    assert_eq!(stored_hash, event_wasm_hash);
}

#[test]
fn test_double_initialization_is_noop() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(FactoryContract, ());
    let client = FactoryContractClient::new(&env, &contract_id);

    let admin_one = Address::generate(&env);
    let hash_one = test_wasm_hash(&env, 1);
    let admin_two = Address::generate(&env);
    let hash_two = test_wasm_hash(&env, 2);

    client.initialize(&admin_one, &hash_one);

    let result = client.try_initialize(&admin_two, &hash_two);
    assert!(result.is_ok());

    let stored_admin = env
        .as_contract(&contract_id, || storage::get_admin(&env))
        .unwrap();
    let stored_hash = env
        .as_contract(&contract_id, || storage::get_event_wasm_hash(&env))
        .unwrap();

    assert_eq!(stored_admin, admin_one);
    assert_eq!(stored_hash, hash_one);
}

#[test]
fn test_get_nonexistent_event_returns_error() {
    let env = Env::default();
    let contract_id = env.register(FactoryContract, ());
    let client = FactoryContractClient::new(&env, &contract_id);

    let missing_event_id = Symbol::new(&env, "missing_event");
    let result = client.try_get_deployed_event(&missing_event_id);

    assert_eq!(
        result.err(),
        Some(Ok(FactoryError::EventNotFoundInRegistry))
    );
}
