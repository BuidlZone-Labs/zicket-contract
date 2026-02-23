use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Bytes, BytesN, Env, Symbol};

const EVENT_WASM: &[u8] =
    include_bytes!("../../../target/wasm32v1-none/release/mock_event_contract.wasm");

fn setup_factory(env: &Env) -> (FactoryContractClient<'_>, Address) {
    let contract_id = env.register(FactoryContract, ());
    let client = FactoryContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let wasm_hash = env
        .deployer()
        .upload_contract_wasm(Bytes::from_slice(env, EVENT_WASM));
    client.initialize(&admin, &wasm_hash);

    (client, admin)
}

fn salt(env: &Env, fill: u8) -> BytesN<32> {
    BytesN::from_array(env, &[fill; 32])
}

#[test]
fn test_initialize_stores_admin_and_event_wasm_hash() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(FactoryContract, ());
    let client = FactoryContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let wasm_hash = BytesN::from_array(&env, &[7; 32]);

    client.initialize(&admin, &wasm_hash);

    let stored_admin = env
        .as_contract(&contract_id, || storage::get_admin(&env))
        .unwrap();
    let stored_hash = env
        .as_contract(&contract_id, || storage::get_event_wasm_hash(&env))
        .unwrap();

    assert_eq!(stored_admin, admin);
    assert_eq!(stored_hash, wasm_hash);
}

#[test]
fn test_double_initialization_is_noop() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(FactoryContract, ());
    let client = FactoryContractClient::new(&env, &contract_id);

    let admin_one = Address::generate(&env);
    let hash_one = BytesN::from_array(&env, &[1; 32]);
    let admin_two = Address::generate(&env);
    let hash_two = BytesN::from_array(&env, &[2; 32]);

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

    let result = client.try_get_deployed_event(&Symbol::new(&env, "missing"));
    assert_eq!(
        result.err(),
        Some(Ok(FactoryError::EventNotFoundInRegistry))
    );
}

#[test]
fn test_deploy_event_returns_address_and_stores_record() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin) = setup_factory(&env);
    let organizer = Address::generate(&env);
    let event_id = Symbol::new(&env, "evt_1");
    let ticket = Address::generate(&env);
    let payments = Address::generate(&env);

    let addr = client.deploy_event(&organizer, &event_id, &salt(&env, 1), &ticket, &payments);

    let record = client.get_deployed_event(&event_id);
    assert_eq!(record.contract_address, addr);
    assert_eq!(record.organizer, organizer);
    assert_eq!(record.event_id, event_id);
}

#[test]
fn test_get_event_address() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin) = setup_factory(&env);
    let organizer = Address::generate(&env);
    let event_id = Symbol::new(&env, "evt_addr");
    let ticket = Address::generate(&env);
    let payments = Address::generate(&env);

    let deployed = client.deploy_event(&organizer, &event_id, &salt(&env, 10), &ticket, &payments);
    let queried = client.get_event_address(&event_id);

    assert_eq!(deployed, queried);
}

#[test]
fn test_get_all_events() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin) = setup_factory(&env);
    let organizer = Address::generate(&env);
    let ticket = Address::generate(&env);
    let payments = Address::generate(&env);

    let id1 = Symbol::new(&env, "all_1");
    let id2 = Symbol::new(&env, "all_2");
    let id3 = Symbol::new(&env, "all_3");

    client.deploy_event(&organizer, &id1, &salt(&env, 1), &ticket, &payments);
    client.deploy_event(&organizer, &id2, &salt(&env, 2), &ticket, &payments);
    client.deploy_event(&organizer, &id3, &salt(&env, 3), &ticket, &payments);

    let all = client.get_all_events();
    assert_eq!(all.len(), 3);
    assert_eq!(all.get(0).unwrap(), id1);
    assert_eq!(all.get(1).unwrap(), id2);
    assert_eq!(all.get(2).unwrap(), id3);
}

#[test]
fn test_get_organizer_events_filtering() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin) = setup_factory(&env);
    let org_a = Address::generate(&env);
    let org_b = Address::generate(&env);
    let ticket = Address::generate(&env);
    let payments = Address::generate(&env);

    let a1 = Symbol::new(&env, "a_1");
    let a2 = Symbol::new(&env, "a_2");
    let b1 = Symbol::new(&env, "b_1");

    client.deploy_event(&org_a, &a1, &salt(&env, 1), &ticket, &payments);
    client.deploy_event(&org_a, &a2, &salt(&env, 2), &ticket, &payments);
    client.deploy_event(&org_b, &b1, &salt(&env, 3), &ticket, &payments);

    let a_events = client.get_organizer_events(&org_a);
    assert_eq!(a_events.len(), 2);
    assert_eq!(a_events.get(0).unwrap(), a1);
    assert_eq!(a_events.get(1).unwrap(), a2);

    let b_events = client.get_organizer_events(&org_b);
    assert_eq!(b_events.len(), 1);
    assert_eq!(b_events.get(0).unwrap(), b1);
}

#[test]
fn test_duplicate_event_id_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin) = setup_factory(&env);
    let organizer = Address::generate(&env);
    let event_id = Symbol::new(&env, "dup_evt");
    let ticket = Address::generate(&env);
    let payments = Address::generate(&env);

    client.deploy_event(&organizer, &event_id, &salt(&env, 1), &ticket, &payments);

    let result = client.try_deploy_event(&organizer, &event_id, &salt(&env, 2), &ticket, &payments);
    assert_eq!(result.err(), Some(Ok(FactoryError::EventAlreadyDeployed)));
}

#[test]
fn test_get_event_address_nonexistent() {
    let env = Env::default();
    let contract_id = env.register(FactoryContract, ());
    let client = FactoryContractClient::new(&env, &contract_id);

    let result = client.try_get_event_address(&Symbol::new(&env, "nope"));
    assert_eq!(
        result.err(),
        Some(Ok(FactoryError::EventNotFoundInRegistry))
    );
}
