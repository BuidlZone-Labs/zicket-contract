use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, token, Address, Env};

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

fn setup_contract_with_token(
    env: &Env,
) -> (
    Address,
    Address,
    PaymentsContractClient<'_>,
    Address,
    token::StellarAssetClient<'_>,
) {
    let contract_id = env.register(PaymentsContract, ());
    let client = PaymentsContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
    let token = token_contract.address();
    client.initialize(&admin, &token);

    let token_client = token::StellarAssetClient::new(env, &token);
    (admin, token, client, contract_id, token_client)
}

#[test]
fn test_pay_for_ticket() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, contract_id, token_contract) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    let payment_id = client.pay_for_ticket(&payer, &event_id, &amount);

    let payment = client.get_payment(&payment_id);
    assert_eq!(payment.payment_id, payment_id);
    assert_eq!(payment.event_id, event_id);
    assert_eq!(payment.payer, payer);
    assert_eq!(payment.amount, amount);
    assert_eq!(payment.token, token);
    assert_eq!(payment.status, PaymentStatus::Held);

    let owner_tickets = client.get_owner_tickets(&payer);
    assert_eq!(owner_tickets.len(), 1);
    let ticket_id = owner_tickets.get(0).unwrap();
    let ticket = client.get_ticket(&ticket_id);
    assert_eq!(ticket.ticket_id, ticket_id);
    assert_eq!(ticket.event_id, event_id);
    assert_eq!(ticket.owner, payer);
    assert_eq!(ticket.payment_id, payment_id);

    let contract_balance = token_client.balance(&contract_id);
    assert_eq!(contract_balance, amount);

    let payer_balance = token_client.balance(&payer);
    assert_eq!(payer_balance, 0);

    let revenue = client.get_event_revenue(&event_id);
    assert_eq!(revenue, amount);
}

#[test]
fn test_payment_issues_ticket_and_links_payment() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    let payment_id = client.pay_for_ticket(&payer, &event_id, &amount);
    let owner_tickets = client.get_owner_tickets(&payer);

    assert_eq!(owner_tickets.len(), 1);
    let ticket_id = owner_tickets.get(0).unwrap();

    let ticket = client.get_ticket(&ticket_id);
    assert_eq!(ticket.ticket_id, ticket_id);
    assert_eq!(ticket.event_id, event_id);
    assert_eq!(ticket.owner, payer);
    assert_eq!(ticket.payment_id, payment_id);
}

#[test]
fn test_multiple_payments_create_distinct_tickets_for_owner() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount1 = 100_000_000i128;
    let amount2 = 50_000_000i128;

    token_contract.mint(&admin, &(amount1 + amount2));
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &(amount1 + amount2));

    let payment_id_1 = client.pay_for_ticket(&payer, &event_id, &amount1);
    let payment_id_2 = client.pay_for_ticket(&payer, &event_id, &amount2);

    let owner_tickets = client.get_owner_tickets(&payer);
    assert_eq!(owner_tickets.len(), 2);

    let ticket_1 = client.get_ticket(&owner_tickets.get(0).unwrap());
    let ticket_2 = client.get_ticket(&owner_tickets.get(1).unwrap());

    assert_ne!(ticket_1.ticket_id, ticket_2.ticket_id);
    assert_eq!(ticket_1.owner, payer);
    assert_eq!(ticket_2.owner, payer);
    assert_eq!(ticket_1.payment_id, payment_id_1);
    assert_eq!(ticket_2.payment_id, payment_id_2);
}

#[test]
fn test_pay_for_ticket_invalid_amount_zero() {
    let env = Env::default();
    env.mock_all_auths();

    let (_admin, _token, client, _, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");

    let result = client.try_pay_for_ticket(&payer, &event_id, &0);
    assert_eq!(result.err(), Some(Ok(PaymentError::InvalidAmount)));
}

#[test]
fn test_pay_for_ticket_invalid_amount_negative() {
    let env = Env::default();
    env.mock_all_auths();

    let (_admin, _token, client, _, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");

    let result = client.try_pay_for_ticket(&payer, &event_id, &-1);
    assert_eq!(result.err(), Some(Ok(PaymentError::InvalidAmount)));
}

#[test]
#[should_panic(expected = "Auth")]
fn test_pay_for_ticket_unauthorized() {
    let env = Env::default();

    let (_admin, _token, client, _, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    client.pay_for_ticket(&payer, &event_id, &amount);
}

#[test]
fn test_pay_for_ticket_multiple_payments() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, contract_id, token_contract) = setup_contract_with_token(&env);
    let payer1 = Address::generate(&env);
    let payer2 = Address::generate(&env);
    let event_id1 = symbol_short!("EVENT1");
    let event_id2 = symbol_short!("EVENT2");
    let amount1 = 100_000_000i128;
    let amount2 = 200_000_000i128;
    let amount3 = 50_000_000i128;

    token_contract.mint(&admin, &(amount1 + amount2 + amount3));
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer1, &(amount1 + amount3));
    token_client.transfer(&admin, &payer2, &amount2);

    let payment_id1 = client.pay_for_ticket(&payer1, &event_id1, &amount1);
    let payment_id2 = client.pay_for_ticket(&payer2, &event_id2, &amount2);
    let payment_id3 = client.pay_for_ticket(&payer1, &event_id1, &amount3);

    assert_eq!(payment_id1, 1);
    assert_eq!(payment_id2, 2);
    assert_eq!(payment_id3, 3);

    let payment1 = client.get_payment(&payment_id1);
    assert_eq!(payment1.event_id, event_id1);
    assert_eq!(payment1.payer, payer1);
    assert_eq!(payment1.amount, amount1);
    assert_eq!(payment1.status, PaymentStatus::Held);

    let payment2 = client.get_payment(&payment_id2);
    assert_eq!(payment2.event_id, event_id2);
    assert_eq!(payment2.payer, payer2);
    assert_eq!(payment2.amount, amount2);
    assert_eq!(payment2.status, PaymentStatus::Held);

    let payment3 = client.get_payment(&payment_id3);
    assert_eq!(payment3.event_id, event_id1);
    assert_eq!(payment3.payer, payer1);
    assert_eq!(payment3.amount, amount3);
    assert_eq!(payment3.status, PaymentStatus::Held);

    let revenue1 = client.get_event_revenue(&event_id1);
    assert_eq!(revenue1, amount1 + amount3);

    let revenue2 = client.get_event_revenue(&event_id2);
    assert_eq!(revenue2, amount2);

    let contract_balance = token_client.balance(&contract_id);
    assert_eq!(contract_balance, amount1 + amount2 + amount3);
}

#[test]
fn test_pay_for_ticket_query_record() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1704067200;
    });

    let (admin, token, client, contract_id, token_contract) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    let payment_id = client.pay_for_ticket(&payer, &event_id, &amount);

    let payment = env
        .as_contract(&contract_id, || storage::get_payment(&env, payment_id))
        .unwrap();

    assert_eq!(payment.payment_id, payment_id);
    assert_eq!(payment.event_id, event_id);
    assert_eq!(payment.payer, payer);
    assert_eq!(payment.amount, amount);
    assert_eq!(payment.token, token);
    assert_eq!(payment.status, PaymentStatus::Held);
    assert!(payment.paid_at > 0);
}

#[test]
fn test_withdraw_revenue_success() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1704067200;
    });

    let (admin, token, client, contract_id, token_contract) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    // 1. Setup funds and pay for ticket
    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);
    client.pay_for_ticket(&payer, &event_id, &amount);

    assert_eq!(token_client.balance(&contract_id), amount);
    assert_eq!(client.get_event_revenue(&event_id), amount);

    // 2. Withdraw revenue
    let organizer = Address::generate(&env);
    client.withdraw_revenue(&event_id, &organizer);

    // 3. Verify balances
    assert_eq!(token_client.balance(&contract_id), 0);
    assert_eq!(token_client.balance(&organizer), amount);
    assert_eq!(client.get_event_revenue(&event_id), 0);

    // 4. Verify withdrawal history
    let history = client.get_withdrawal_history(&event_id);
    assert_eq!(history.len(), 1);
    let record = history.get(0).unwrap();
    assert_eq!(record.amount, amount);
    assert_eq!(record.organizer, organizer);
    assert_eq!(record.timestamp, 1704067200);

    // 5. Try to withdraw again -> should fail as revenue is 0
    let result = client.try_withdraw_revenue(&event_id, &organizer);
    assert!(result.is_err());
}

#[test]
fn test_multiple_withdrawals_tracked() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    let token_client = token::Client::new(&env, &token);
    let organizer = Address::generate(&env);

    // First withdrawal
    token_contract.mint(&admin, &amount);
    token_client.transfer(&admin, &payer, &amount);
    client.pay_for_ticket(&payer, &event_id, &amount);
    client.withdraw_revenue(&event_id, &organizer);

    // Second withdrawal
    token_contract.mint(&admin, &amount);
    token_client.transfer(&admin, &payer, &amount);
    client.pay_for_ticket(&payer, &event_id, &amount);
    client.withdraw_revenue(&event_id, &organizer);

    let history = client.get_withdrawal_history(&event_id);
    assert_eq!(history.len(), 2);
    assert_eq!(history.get(0).unwrap().amount, amount);
    assert_eq!(history.get(1).unwrap().amount, amount);
}

#[test]
fn test_pay_for_ticket_transfer_failure_no_state_change() {
    let env = Env::default();
    env.mock_all_auths();

    let (_admin, _token, client, contract_id, _token_contract) =
        setup_contract_with_token(&env);

    // payer has zero balance — transfer will fail
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    let result = client.try_pay_for_ticket(&payer, &event_id, &amount);

    // Should return TransferFailed
    assert_eq!(result.err(), Some(Ok(PaymentError::TransferFailed)));

    // No payment record created
    let payment_result = client.try_get_payment(&1);
    assert_eq!(payment_result.err(), Some(Ok(PaymentError::PaymentNotFound)));

    // Revenue unchanged
    let revenue = client.get_event_revenue(&event_id);
    assert_eq!(revenue, 0);

    // No tickets issued
    let tickets = client.get_owner_tickets(&payer);
    assert_eq!(tickets.len(), 0);

    // Contract holds no tokens
    let token_address = env
        .as_contract(&contract_id, || storage::get_accepted_token(&env))
        .unwrap();
    let token_client = token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&contract_id), 0);
}

#[test]
fn test_pay_for_ticket_partial_funds_no_state_change() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _token, client, contract_id, token_contract) =
        setup_contract_with_token(&env);

    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;
    let partial = 50_000_000i128; // only half the required amount

    // Mint only partial funds to payer
    token_contract.mint(&admin, &partial);
    let token_address = env
        .as_contract(&contract_id, || storage::get_accepted_token(&env))
        .unwrap();
    let token_client = token::Client::new(&env, &token_address);
    token_client.transfer(&admin, &payer, &partial);

    // Attempt to pay full amount — should fail
    let result = client.try_pay_for_ticket(&payer, &event_id, &amount);
    assert_eq!(result.err(), Some(Ok(PaymentError::TransferFailed)));

    // No state written
    assert_eq!(client.get_event_revenue(&event_id), 0);
    assert_eq!(client.get_owner_tickets(&payer).len(), 0);
    assert_eq!(token_client.balance(&contract_id), 0);
    // Payer still holds their partial funds
    assert_eq!(token_client.balance(&payer), partial);
}
