use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Address, Env};

#[test]
fn test_initialize() {
    let env = Env::default();
    let contract_id = env.register(PaymentsContract, ());
    let client = PaymentsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    client.initialize(&admin, &token);

    let stored_admin = env
        .as_contract(&contract_id, || storage::get_admin(&env))
        .unwrap();
    let stored_token = env
        .as_contract(&contract_id, || storage::get_accepted_token(&env))
        .unwrap();

    assert_eq!(stored_admin, admin);
    assert_eq!(stored_token, token);
}

#[test]
fn test_double_initialization() {
    let env = Env::default();
    let contract_id = env.register(PaymentsContract, ());
    let client = PaymentsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    client.initialize(&admin, &token);

    let result = client.try_initialize(&admin, &token);
    assert!(result.is_ok());

    let stored_admin = env
        .as_contract(&contract_id, || storage::get_admin(&env))
        .unwrap();
    let stored_token = env
        .as_contract(&contract_id, || storage::get_accepted_token(&env))
        .unwrap();
    assert_eq!(stored_admin, admin);
    assert_eq!(stored_token, token);
}

#[test]
fn test_get_nonexistent_payment() {
    let env = Env::default();
    let contract_id = env.register(PaymentsContract, ());
    let client = PaymentsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    client.initialize(&admin, &token);
    let result = client.try_get_payment(&999);
    assert_eq!(result.err(), Some(Ok(PaymentError::PaymentNotFound)));
}

#[test]
fn test_get_event_revenue_initial() {
    let env = Env::default();
    let contract_id = env.register(PaymentsContract, ());
    let client = PaymentsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    client.initialize(&admin, &token);
    let event_id = symbol_short!("EVENT1");
    let revenue = client.get_event_revenue(&event_id);
    assert_eq!(revenue, 0);
}
