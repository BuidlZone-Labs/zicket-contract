use super::*;
use mock_event_contract::MockEventContract;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, token, Address, Env};

#[test]
fn test_initialize() {
    let env = Env::default();
    let contract_id = env.register(PaymentsContract, ());
    let client = PaymentsContractClient::new(&env, &contract_id);
    let event_contract_id = env.register(MockEventContract, ());

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let platform_wallet = Address::generate(&env);

    client.initialize(&admin, &token, &250, &platform_wallet, &event_contract_id);

    let stored_admin = env
        .as_contract(&contract_id, || storage::get_admin(&env))
        .unwrap();
    let stored_token = env
        .as_contract(&contract_id, || storage::get_accepted_token(&env))
        .unwrap();

    assert_eq!(stored_admin, admin);
    assert_eq!(stored_token, token);
    assert_eq!(client.get_platform_fee_bps(), 250);
}

#[test]
fn test_double_initialization() {
    let env = Env::default();
    let contract_id = env.register(PaymentsContract, ());
    let client = PaymentsContractClient::new(&env, &contract_id);
    let event_contract_id = env.register(MockEventContract, ());

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let platform_wallet = Address::generate(&env);

    client.initialize(&admin, &token, &0, &platform_wallet, &event_contract_id);

    let result = client.try_initialize(&admin, &token, &0, &platform_wallet, &event_contract_id);
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
    let event_contract_id = env.register(MockEventContract, ());

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let platform_wallet = Address::generate(&env);

    client.initialize(&admin, &token, &0, &platform_wallet, &event_contract_id);
    let result = client.try_get_payment(&999);
    assert_eq!(result.err(), Some(Ok(PaymentError::PaymentNotFound)));
}

#[test]
fn test_get_event_revenue_initial() {
    let env = Env::default();
    let contract_id = env.register(PaymentsContract, ());
    let client = PaymentsContractClient::new(&env, &contract_id);
    let event_contract_id = env.register(MockEventContract, ());

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let platform_wallet = Address::generate(&env);

    client.initialize(&admin, &token, &0, &platform_wallet, &event_contract_id);
    let event_id = symbol_short!("EVENT1");
    let revenue = client.get_event_revenue(&event_id);
    assert_eq!(revenue, 0);
}

#[test]
fn test_pause_unpause_and_blocks_sensitive_actions() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    assert!(!client.is_paused());
    client.set_paused(&admin, &true);
    assert!(client.is_paused());

    let status_result = client.try_set_event_status(&admin, &event_id, &EventStatus::Active);
    assert_eq!(status_result.err(), Some(Ok(PaymentError::ContractPaused)));

    let payment_result = client.try_pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    assert_eq!(payment_result.err(), Some(Ok(PaymentError::ContractPaused)));
    assert_eq!(token_client.balance(&payer), amount);

    let wallet = Address::generate(&env);
    let fee_result = client.try_set_platform_fee(&250, &wallet);
    assert_eq!(fee_result.err(), Some(Ok(PaymentError::ContractPaused)));

    client.set_paused(&admin, &false);
    assert!(!client.is_paused());
    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Active);

    let payment_id = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    client.set_paused(&admin, &true);
    let refund_result = client.try_refund(&admin, &payment_id, &None);
    assert_eq!(refund_result.err(), Some(Ok(PaymentError::ContractPaused)));
    assert_eq!(client.get_payment(&payment_id).status, PaymentStatus::Held);
}

#[test]
fn test_pause_requires_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _token, client, _contract_id, _token_contract, _) = setup_contract_with_token(&env);
    let not_admin = Address::generate(&env);

    let result = client.try_set_paused(&not_admin, &true);
    assert_eq!(result.err(), Some(Ok(PaymentError::Unauthorized)));
    assert!(!client.is_paused());

    client.set_paused(&admin, &true);
    assert!(client.is_paused());
}

fn setup_contract_with_token(
    env: &Env,
) -> (
    Address,
    Address,
    PaymentsContractClient<'_>,
    Address,
    token::StellarAssetClient<'_>,
    Address,
) {
    setup_contract_with_fee(env, 0)
}

fn setup_contract_with_fee(
    env: &Env,
    fee_bps: u32,
) -> (
    Address,
    Address,
    PaymentsContractClient<'_>,
    Address,
    token::StellarAssetClient<'_>,
    Address,
) {
    let contract_id = env.register(PaymentsContract, ());
    let client = PaymentsContractClient::new(env, &contract_id);
    let event_contract_id = env.register(MockEventContract, ());

    let admin = Address::generate(env);
    let platform_wallet = Address::generate(env);
    let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
    let token = token_contract.address();
    client.initialize(
        &admin,
        &token,
        &fee_bps,
        &platform_wallet,
        &event_contract_id,
    );

    let token_client = token::StellarAssetClient::new(env, &token);
    (
        admin,
        token,
        client,
        contract_id,
        token_client,
        event_contract_id,
    )
}

fn setup_contract_with_token_and_event(
    env: &Env,
) -> (
    Address,
    Address,
    PaymentsContractClient<'_>,
    Address,
    token::StellarAssetClient<'_>,
    Address,
) {
    let contract_id = env.register(PaymentsContract, ());
    let client = PaymentsContractClient::new(env, &contract_id);
    let event_contract_id = env.register(MockEventContract, ());

    let admin = Address::generate(env);
    let platform_wallet = Address::generate(env);
    let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
    let token = token_contract.address();
    client.initialize(&admin, &token, &0, &platform_wallet, &event_contract_id);

    let token_client = token::StellarAssetClient::new(env, &token);
    (
        admin,
        token,
        client,
        contract_id,
        token_client,
        event_contract_id,
    )
}

fn bind_event(
    client: &PaymentsContractClient,
    event_contract: &Address,
    event_id: &soroban_sdk::Symbol,
    organizer: &Address,
    payout_token: &Address,
) {
    client.sync_event_config(
        event_contract,
        event_id,
        organizer,
        payout_token,
        &true,
        &false,
        &0,
        &0,
    );
}

fn set_event_status_for_test(
    client: &PaymentsContractClient<'_>,
    admin: &Address,
    event_id: &soroban_sdk::Symbol,
    status: &EventStatus,
) {
    client.set_event_status(admin, event_id, status);
}

#[test]
fn test_pay_for_ticket() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    let payment_id = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    let payment = client.get_payment(&payment_id);
    assert_eq!(payment.payment_id, payment_id);
    assert_eq!(payment.event_id, event_id);
    assert_eq!(payment.payer, payer);
    assert_eq!(payment.amount, amount);
    assert_eq!(payment.token, token);
    assert_eq!(payment.status, PaymentStatus::Held);
    assert_eq!(payment.privacy_level, PaymentPrivacy::Standard);

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

    let (admin, token, client, _contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    let payment_id = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
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

    let (admin, token, client, _contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount1 = 100_000_000i128;
    let amount2 = 50_000_000i128;

    token_contract.mint(&admin, &(amount1 + amount2));
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &(amount1 + amount2));

    let payment_id_1 = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount1,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    let payment_id_2 = client.pay_for_ticket(
        &2,
        &payer,
        &event_id,
        &amount2,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

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

    let (_admin, token, client, _, _, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");

    let result = client.try_pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &0,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    assert_eq!(result.err(), Some(Ok(PaymentError::InvalidAmount)));
}

#[test]
fn test_pay_for_ticket_invalid_amount_negative() {
    let env = Env::default();
    env.mock_all_auths();

    let (_admin, token, client, _, _, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");

    let result = client.try_pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &-1,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    assert_eq!(result.err(), Some(Ok(PaymentError::InvalidAmount)));
}

#[test]
fn test_pay_for_ticket_rejects_anonymous_when_disabled() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract, event_contract_id) =
        setup_contract_with_token_and_event(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    env.as_contract(&event_contract_id, || {
        client.sync_event_privacy(&event_contract_id, &event_id, &false, &false)
    });
    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    let result = client
        .try_pay_for_ticket_with_options(&1, &payer, &event_id, &amount, &token, &true, &false);
    assert_eq!(
        result.err(),
        Some(Ok(PaymentError::AnonymousPaymentsDisabled))
    );
}

#[test]
fn test_pay_for_ticket_rejects_unverified_when_required() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract, event_contract_id) =
        setup_contract_with_token_and_event(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    env.as_contract(&event_contract_id, || {
        client.sync_event_privacy(&event_contract_id, &event_id, &true, &true)
    });
    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    let result = client
        .try_pay_for_ticket_with_options(&1, &payer, &event_id, &amount, &token, &false, &false);
    assert_eq!(result.err(), Some(Ok(PaymentError::VerificationRequired)));
}

#[test]
fn test_pay_for_ticket_with_options_allows_verified_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, contract_id, token_contract, event_contract_id) =
        setup_contract_with_token_and_event(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    env.as_contract(&event_contract_id, || {
        client.sync_event_privacy(&event_contract_id, &event_id, &false, &true)
    });
    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    let payment_id =
        client.pay_for_ticket_with_options(&1, &payer, &event_id, &amount, &token, &false, &true);

    let payment = client.get_payment(&payment_id);
    assert_eq!(payment.status, PaymentStatus::Held);
    assert_eq!(token_client.balance(&contract_id), amount);
}

#[test]
#[should_panic(expected = "Auth")]
fn test_pay_for_ticket_unauthorized() {
    let env = Env::default();

    let (_admin, token, client, _, _, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
}

#[test]
fn test_pay_for_ticket_multiple_payments() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, contract_id, token_contract, _) = setup_contract_with_token(&env);
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

    let payment_id1 = client.pay_for_ticket(
        &1,
        &payer1,
        &event_id1,
        &amount1,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    let payment_id2 = client.pay_for_ticket(
        &2,
        &payer2,
        &event_id2,
        &amount2,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    let payment_id3 = client.pay_for_ticket(
        &3,
        &payer1,
        &event_id1,
        &amount3,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

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

    let (admin, token, client, contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    let payment_id = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

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

    let (admin, token, client, contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    // 1. Setup funds and pay for ticket
    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    assert_eq!(token_client.balance(&payer), 0);
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

    let (admin, token, client, _contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    let payment_id = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    let _not_admin = Address::generate(&env);
    let result = client.try_refund(&_not_admin, &payment_id, &None);
    assert_eq!(result.err(), Some(Ok(PaymentError::Unauthorized)));
}

#[test]
fn test_refund_after_withdrawal() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _, token_contract, event_contract) =
        setup_contract_with_token_and_event(&env);
    let payer = Address::generate(&env);
    let token_client = token::Client::new(&env, &token);
    let organizer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    // First withdrawal
    token_contract.mint(&admin, &(amount * 3));
    token_client.transfer(&admin, &payer, &(amount * 3));
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    client.withdraw_revenue(&event_id, &organizer);

    bind_event(&client, &event_contract, &event_id, &organizer, &token);
    let payment_id = client.pay_for_ticket(
        &123,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    // Set status to complete and withdraw
    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Completed);
    client.withdraw(&organizer, &event_id);

    let result = client.try_refund(&admin, &payment_id, &None);
    assert_eq!(
        result.err(),
        Some(Ok(PaymentError::PaymentAlreadyProcessed))
    );
}

#[test]
fn test_withdraw_happy_path() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, contract_id, token_contract, event_contract) =
        setup_contract_with_token_and_event(&env);
    let payer1 = Address::generate(&env);
    let payer2 = Address::generate(&env);
    let organizer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount1 = 100_000_000i128;
    let amount2 = 50_000_000i128;

    token_contract.mint(&admin, &(amount1 + amount2));
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer1, &amount1);
    token_client.transfer(&admin, &payer2, &amount2);

    bind_event(&client, &event_contract, &event_id, &organizer, &token);
    let pid1 = client.pay_for_ticket(
        &1,
        &payer1,
        &event_id,
        &amount1,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    let pid2 = client.pay_for_ticket(
        &1,
        &payer2,
        &event_id,
        &amount2,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Completed);
    client.withdraw(&organizer, &event_id);

    assert_eq!(token_client.balance(&organizer), amount1 + amount2);
    assert_eq!(token_client.balance(&contract_id), 0);
    assert_eq!(client.get_event_revenue(&event_id), 0);

    let p1 = client.get_payment(&pid1);
    let p2 = client.get_payment(&pid2);
    assert_eq!(p1.status, PaymentStatus::Released);
    assert_eq!(p2.status, PaymentStatus::Released);
}

#[test]
fn test_withdraw_no_revenue() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _, _, event_contract) = setup_contract_with_token_and_event(&env);
    let organizer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");

    bind_event(&client, &event_contract, &event_id, &organizer, &token);
    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Completed);
    let result = client.try_withdraw(&organizer, &event_id);
    assert_eq!(result.err(), Some(Ok(PaymentError::NoRevenue)));
}

#[test]
fn test_mixed_refund_then_withdraw() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, contract_id, token_contract, event_contract) =
        setup_contract_with_token_and_event(&env);
    let payer1 = Address::generate(&env);
    let payer2 = Address::generate(&env);
    let payer3 = Address::generate(&env);
    let organizer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount1 = 100_000_000i128;
    let amount2 = 50_000_000i128;
    let amount3 = 75_000_000i128;

    let total = amount1 + amount2 + amount3;
    token_contract.mint(&admin, &total);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer1, &amount1);
    token_client.transfer(&admin, &payer2, &amount2);
    token_client.transfer(&admin, &payer3, &amount3);

    bind_event(&client, &event_contract, &event_id, &organizer, &token);
    let pid1 = client.pay_for_ticket(
        &1,
        &payer1,
        &event_id,
        &amount1,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    let pid2 = client.pay_for_ticket(
        &1,
        &payer2,
        &event_id,
        &amount2,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    let pid3 = client.pay_for_ticket(
        &1,
        &payer3,
        &event_id,
        &amount3,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    // Refund payment 2
    client.refund(&admin, &pid2, &None);
    assert_eq!(client.get_event_revenue(&event_id), amount1 + amount3);
    assert_eq!(token_client.balance(&payer2), amount2);

    // Withdraw remaining
    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Completed);
    client.withdraw(&organizer, &event_id);

    assert_eq!(token_client.balance(&organizer), amount1 + amount3);
    assert_eq!(token_client.balance(&contract_id), 0);
    assert_eq!(client.get_event_revenue(&event_id), 0);

    let p1 = client.get_payment(&pid1);
    let p2 = client.get_payment(&pid2);
    let p3 = client.get_payment(&pid3);
    assert_eq!(p1.status, PaymentStatus::Released);
    assert_eq!(p2.status, PaymentStatus::Refunded);
    assert_eq!(p3.status, PaymentStatus::Released);
}

#[test]
fn test_refund_reduces_revenue_correctly() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _, token_contract, _) = setup_contract_with_token(&env);
    let payer1 = Address::generate(&env);
    let payer2 = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount1 = 100_000_000i128;
    let amount2 = 50_000_000i128;

    token_contract.mint(&admin, &(amount1 + amount2));
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer1, &amount1);
    token_client.transfer(&admin, &payer2, &amount2);

    let pid1 = client.pay_for_ticket(
        &1,
        &payer1,
        &event_id,
        &amount1,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    client.pay_for_ticket(
        &1,
        &payer2,
        &event_id,
        &amount2,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    assert_eq!(client.get_event_revenue(&event_id), amount1 + amount2);

    client.refund(&admin, &pid1, &None);
    assert_eq!(client.get_event_revenue(&event_id), amount2);
}

#[test]
fn test_query_payments() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract, event_contract) =
        setup_contract_with_token_and_event(&env);
    let payer1 = Address::generate(&env);
    let payer2 = Address::generate(&env);
    let event1 = symbol_short!("E1");
    let event2 = symbol_short!("E2");
    let organizer = Address::generate(&env);

    let amount = 1000i128;
    token_contract.mint(&admin, &(amount * 4));
    let token_utils = token::Client::new(&env, &token);
    token_utils.transfer(&admin, &payer1, &(amount * 2));
    token_utils.transfer(&admin, &payer2, &(amount * 2));

    bind_event(&client, &event_contract, &event1, &organizer, &token);
    bind_event(&client, &event_contract, &event2, &organizer, &token);

    // P1 -> E1
    client.pay_for_ticket(
        &1,
        &payer1,
        &event1,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    // P1 -> E2
    client.pay_for_ticket(
        &2,
        &payer1,
        &event2,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    // P2 -> E1
    client.pay_for_ticket(
        &1,
        &payer2,
        &event1,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    // P2 -> E2
    client.pay_for_ticket(
        &2,
        &payer2,
        &event2,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    // Query by Event E1
    let e1_payments = client.get_payments_by_event(&event1);
    assert_eq!(e1_payments.len(), 2);
    assert_eq!(e1_payments.get(0).unwrap().event_id, event1);
    assert_eq!(e1_payments.get(1).unwrap().event_id, event1);
    assert_ne!(
        e1_payments.get(0).unwrap().payer,
        e1_payments.get(1).unwrap().payer
    );

    // Query by User P1
    let p1_payments = client.get_payments_by_user(&payer1);
    assert_eq!(p1_payments.len(), 2);
    assert_eq!(p1_payments.get(0).unwrap().payer, payer1);
    assert_eq!(p1_payments.get(1).unwrap().payer, payer1);
    assert_ne!(
        p1_payments.get(0).unwrap().event_id,
        p1_payments.get(1).unwrap().event_id
    );
}

#[test]
fn test_pay_for_ticket_with_email_hash() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;
    let email_hash = BytesN::from_array(&env, &[1u8; 32]);

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Active);

    let payment_id = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &Some(email_hash.clone()),
        &token,
        &PaymentPrivacy::Standard,
    );

    let payment = client.get_payment(&payment_id);
    assert_eq!(payment.amount, amount);
}

// ============================================================
// Issue #43: Escrow Timeout / Auto-Release Tests
// ============================================================

#[test]
fn test_set_event_end_time_success() {
    let env = Env::default();
    env.mock_all_auths();

    let (_admin, _token, client, _contract_id, _token_contract, _) =
        setup_contract_with_token(&env);
    let organizer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let event_end_time: u64 = 1704067200 + 86_400;

    let admin = _admin;
    client.set_event_end_time(&admin, &event_id, &organizer, &event_end_time);

    env.ledger().with_mut(|li| {
        li.timestamp = 1704067200;
    });
    let result = client.try_release_if_expired(&event_id);
    assert_eq!(result.err(), Some(Ok(PaymentError::EscrowNotExpired)));
}

#[test]
fn test_release_if_expired_before_deadline_fails() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1704067200;
    });

    let (admin, token, client, _contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let organizer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;
    let event_end_time: u64 = env.ledger().timestamp() + 86_400;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    client.set_event_end_time(&admin, &event_id, &organizer, &event_end_time);

    let result = client.try_release_if_expired(&event_id);
    assert_eq!(result.err(), Some(Ok(PaymentError::EscrowNotExpired)));
}

#[test]
fn test_release_if_expired_after_deadline_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1704067200;
    });

    let (admin, token, client, contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let organizer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;
    let event_end_time: u64 = env.ledger().timestamp() + 86_400;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    client.set_event_end_time(&admin, &event_id, &organizer, &event_end_time);

    env.ledger().with_mut(|li| {
        li.timestamp = event_end_time + 1;
    });

    client.release_if_expired(&event_id);

    assert_eq!(token_client.balance(&contract_id), 0);
    assert_eq!(token_client.balance(&organizer), amount);
    assert_eq!(client.get_event_revenue(&event_id), 0);
}

#[test]
fn test_release_if_expired_no_double_payout() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1704067200;
    });

    let (admin, token, client, _contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let organizer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;
    let event_end_time: u64 = env.ledger().timestamp() + 86_400;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    client.set_event_end_time(&admin, &event_id, &organizer, &event_end_time);

    env.ledger().with_mut(|li| {
        li.timestamp = event_end_time + 1;
    });

    client.release_if_expired(&event_id);

    let result = client.try_release_if_expired(&event_id);
    assert_eq!(result.err(), Some(Ok(PaymentError::EscrowAlreadyReleased)));
    assert_eq!(token_client.balance(&organizer), amount);
}

#[test]
fn test_release_if_expired_not_configured() {
    let env = Env::default();
    env.mock_all_auths();

    let (_admin, _token, client, _contract_id, _token_contract, _) =
        setup_contract_with_token(&env);
    let event_id = symbol_short!("EVENT1");

    let result = client.try_release_if_expired(&event_id);
    assert_eq!(result.err(), Some(Ok(PaymentError::EscrowNotConfigured)));
}

#[test]
fn test_release_if_expired_no_held_funds_still_marks_released() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1704067200;
    });

    let (admin, _token, client, _contract_id, _token_contract, _) = setup_contract_with_token(&env);
    let organizer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let event_end_time: u64 = env.ledger().timestamp() + 86_400;

    client.set_event_end_time(&admin, &event_id, &organizer, &event_end_time);

    env.ledger().with_mut(|li| {
        li.timestamp = event_end_time + 1;
    });

    client.release_if_expired(&event_id);

    let result = client.try_release_if_expired(&event_id);
    assert_eq!(result.err(), Some(Ok(PaymentError::EscrowAlreadyReleased)));
}

// ============================================================
// Issue #53: Privacy-Preserving Event Emissions Tests
// ============================================================

#[test]
fn test_payments_privacy_default_is_standard() {
    use super::PrivacyLevel;

    let env = Env::default();
    env.mock_all_auths();

    let (_admin, _token, client, _contract_id, _token_contract, _) =
        setup_contract_with_token(&env);
    let event_id = symbol_short!("EVENT1");

    let level = client.get_event_privacy(&event_id);
    assert_eq!(level, PrivacyLevel::Standard);
}

#[test]
fn test_payments_set_privacy_level_private() {
    use super::PrivacyLevel;

    let env = Env::default();
    env.mock_all_auths();

    let (admin, _token, client, _contract_id, _token_contract, _) = setup_contract_with_token(&env);
    let event_id = symbol_short!("EVENT1");

    client.set_event_privacy(&admin, &event_id, &PrivacyLevel::Private);
    assert_eq!(client.get_event_privacy(&event_id), PrivacyLevel::Private);
}

#[test]
fn test_payments_set_privacy_level_anonymous() {
    use super::PrivacyLevel;

    let env = Env::default();
    env.mock_all_auths();

    let (admin, _token, client, _contract_id, _token_contract, _) = setup_contract_with_token(&env);
    let event_id = symbol_short!("EVENT1");

    client.set_event_privacy(&admin, &event_id, &PrivacyLevel::Anonymous);
    assert_eq!(client.get_event_privacy(&event_id), PrivacyLevel::Anonymous);
}

#[test]
fn test_payments_set_privacy_unauthorized() {
    use super::PrivacyLevel;

    let env = Env::default();
    env.mock_all_auths();

    let (_admin, _token, client, _contract_id, _token_contract, _) =
        setup_contract_with_token(&env);
    let intruder = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");

    let result = client.try_set_event_privacy(&intruder, &event_id, &PrivacyLevel::Anonymous);
    assert_eq!(result.err(), Some(Ok(PaymentError::Unauthorized)));
}

#[test]
fn test_pay_for_ticket_anonymous_privacy() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    let payment_id = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Anonymous,
    );

    let payment = client.get_payment(&payment_id);
    assert_eq!(payment.privacy_level, PaymentPrivacy::Anonymous);
    assert_eq!(payment.payer, payer);
    assert_eq!(payment.amount, amount);
    assert_eq!(payment.status, PaymentStatus::Held);
}

#[test]
fn test_pay_for_ticket_private_privacy() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 50_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    let payment_id = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Private,
    );

    let payment = client.get_payment(&payment_id);
    assert_eq!(payment.privacy_level, PaymentPrivacy::Private);
    assert_eq!(payment.payer, payer);
    assert_eq!(payment.amount, amount);
    assert_eq!(payment.status, PaymentStatus::Held);
}

#[test]
fn test_pay_for_ticket_standard_privacy() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 75_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    let payment_id = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    let payment = client.get_payment(&payment_id);
    assert_eq!(payment.privacy_level, PaymentPrivacy::Standard);
    assert_eq!(payment.payer, payer);
    assert_eq!(payment.amount, amount);
    assert_eq!(payment.status, PaymentStatus::Held);
}

#[test]
fn test_anonymous_event_does_not_expose_payer() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    let payment_id = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Anonymous,
    );

    // Verify the payment was recorded with Anonymous privacy on-chain
    let payment = client.get_payment(&payment_id);
    assert_eq!(payment.privacy_level, PaymentPrivacy::Anonymous);
    // The payer is still stored on-chain for refund/admin purposes,
    // but the emitted event uses PaymentReceivedAnonymous (no payer field)
    assert_eq!(payment.payer, payer);
    assert!(payment_id > 0);
}

#[test]
fn test_privacy_levels_stored_correctly() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 50_000_000i128;

    token_contract.mint(&admin, &(amount * 3));
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &(amount * 3));

    let pid_anon = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Anonymous,
    );
    let pid_priv = client.pay_for_ticket(
        &2,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Private,
    );
    let pid_std = client.pay_for_ticket(
        &3,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    assert_eq!(
        client.get_payment(&pid_anon).privacy_level,
        PaymentPrivacy::Anonymous
    );
    assert_eq!(
        client.get_payment(&pid_priv).privacy_level,
        PaymentPrivacy::Private
    );
    assert_eq!(
        client.get_payment(&pid_std).privacy_level,
        PaymentPrivacy::Standard
    );
}

#[test]
fn test_refund_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let _not_admin = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    let payment_id = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    let result = client.try_refund(&_not_admin, &payment_id, &None);
    assert_eq!(result.err(), Some(Ok(PaymentError::Unauthorized)));
}

#[test]
fn test_mask_address_standard_returns_full() {
    use privacy_utils::{mask_address, MaskedAddress, PrivacyLevel};

    let env = Env::default();
    let addr = Address::generate(&env);
    let result = mask_address(&env, &addr, PrivacyLevel::Standard);
    assert_eq!(result, MaskedAddress::Full(addr));
}

#[test]
fn test_mask_address_private_returns_partial() {
    use privacy_utils::{mask_address, MaskedAddress, PrivacyLevel};

    let env = Env::default();
    let addr = Address::generate(&env);
    let result = mask_address(&env, &addr, PrivacyLevel::Private);
    assert!(matches!(result, MaskedAddress::Partial(_)));
}

#[test]
fn test_mask_address_anonymous_returns_hashed() {
    use privacy_utils::{mask_address, MaskedAddress, PrivacyLevel};

    let env = Env::default();
    let addr = Address::generate(&env);
    let result = mask_address(&env, &addr, PrivacyLevel::Anonymous);
    assert!(matches!(result, MaskedAddress::Hashed(_)));
}

#[test]
fn test_refund_nonexistent_payment() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _token, client, _, _, _) = setup_contract_with_token(&env);
    let result = client.try_refund(&admin, &999, &None);
    assert_eq!(result.err(), Some(Ok(PaymentError::PaymentNotFound)));
}

#[test]
fn test_withdraw_unauthorized_organizer_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract, event_contract) =
        setup_contract_with_token_and_event(&env);
    let payer = Address::generate(&env);
    let organizer = Address::generate(&env);
    let attacker = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    bind_event(&client, &event_contract, &event_id, &organizer, &token);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Completed);

    let result = client.try_withdraw(&attacker, &event_id);
    assert_eq!(result.err(), Some(Ok(PaymentError::UnauthorizedWithdrawal)));
    assert_eq!(token_client.balance(&organizer), 0);
}

#[test]
fn test_sync_event_config_invalid_payout_token_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (_admin, token, client, _contract_id, _token_contract, event_contract) =
        setup_contract_with_token_and_event(&env);
    let organizer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let invalid_token = Address::generate(&env);

    let result = client.try_sync_event_config(
        &event_contract,
        &event_id,
        &organizer,
        &invalid_token,
        &true,
        &false,
        &0,
        &0,
    );
    assert_eq!(result.err(), Some(Ok(PaymentError::InvalidPayoutToken)));

    let stored = client.get_accepted_token();
    assert_eq!(stored, token);
}

#[test]
fn test_double_withdraw_rejected_after_revenue_cleared() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract, event_contract) =
        setup_contract_with_token_and_event(&env);
    let payer = Address::generate(&env);
    let organizer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    bind_event(&client, &event_contract, &event_id, &organizer, &token);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Completed);
    client.withdraw(&organizer, &event_id);

    let result = client.try_withdraw(&organizer, &event_id);
    assert_eq!(result.err(), Some(Ok(PaymentError::NoRevenue)));
    assert_eq!(token_client.balance(&organizer), amount);
}

#[test]
fn test_pay_after_event_completed_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENTCPL");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Completed);
    let result = client
        .try_pay_for_ticket_with_options(&1, &payer, &event_id, &amount, &token, &true, &false);
    assert_eq!(result.err(), Some(Ok(PaymentError::EventNotActive)));
}

#[test]
fn test_withdraw_before_completion_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract, event_contract) =
        setup_contract_with_token_and_event(&env);
    let payer = Address::generate(&env);
    let organizer = Address::generate(&env);
    let event_id = symbol_short!("EVENTACT");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    bind_event(&client, &event_contract, &event_id, &organizer, &token);
    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Active);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    let result = client.try_withdraw(&organizer, &event_id);
    assert_eq!(result.err(), Some(Ok(PaymentError::EventNotCompleted)));
}

#[test]
fn test_refund_on_cancelled_event_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, contract_id, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENTCAN");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Active);
    let payment_id = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Cancelled);
    client.refund(&admin, &payment_id, &None);

    let payment = client.get_payment(&payment_id);
    assert_eq!(payment.status, PaymentStatus::Refunded);
    assert_eq!(token_client.balance(&payer), amount);
    assert_eq!(token_client.balance(&contract_id), 0);
}

#[test]
#[should_panic]
fn test_idempotent_payment() {
    let env = Env::default();
    env.mock_all_auths();

    let (_admin, token, client, _, _, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    // First attempt succeeds
    let nonce = 12345u64;
    client.pay_for_ticket(
        &nonce,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    // Check storage manually
    let contract_id = client.address.clone();
    let has_in_storage = env.as_contract(&contract_id, || storage::has_nonce(&env, &payer, nonce));
    assert!(
        has_in_storage,
        "Nonce should be in storage after first call"
    );

    // First attempt succeeds
    let nonce = 123456u64;
    client.pay_for_ticket(
        &nonce,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    // Second attempt with same nonce fails (should panic with DuplicateRequest)
    client.pay_for_ticket(
        &nonce,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
}

#[test]
#[should_panic]
fn test_idempotent_payment_with_options() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _, _, _) = setup_contract_with_token(&env);

    let payer = Address::generate(&env);
    let event_id = Symbol::new(&env, "EVENT1");
    let amount = 100_000_000i128;
    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Active);

    // First attempt succeeds
    let nonce = 54321u64;
    client.pay_for_ticket_with_options(&nonce, &payer, &event_id, &amount, &token, &true, &false);

    // Second attempt with same nonce fails
    client.pay_for_ticket_with_options(&nonce, &payer, &event_id, &amount, &token, &true, &false);
}

// ===== Max Tickets Per User Tests =====

#[test]
fn test_max_tickets_per_user_enforcement() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract, event_contract_id) =
        setup_contract_with_token_and_event(&env);
    let payer1 = Address::generate(&env);
    let payer2 = Address::generate(&env);
    let organizer = Address::generate(&env);
    let event_id = symbol_short!("LIMIT_EV");
    let amount = 100_000_000i128;

    // Mint plenty of tokens for tests
    token_contract.mint(&admin, &(amount * 5));
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer1, &(amount * 3));
    token_client.transfer(&admin, &payer2, &amount);

    // Sync config with limit of 2 tickets per user
    client.sync_event_config(
        &event_contract_id,
        &event_id,
        &organizer,
        &token,
        &true,
        &false,
        &2,
        &0,
    );

    // Payer 1: First ticket -> Success
    client.pay_for_ticket(
        &1,
        &payer1,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    // Payer 1: Second ticket -> Success
    client.pay_for_ticket(
        &2,
        &payer1,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    // Payer 1: Third ticket -> Failure
    let result = client.try_pay_for_ticket(
        &3,
        &payer1,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    assert_eq!(result.err(), Some(Ok(PaymentError::MaxTicketsReached)));

    // Payer 2: First ticket -> Success (limit is per user)
    client.pay_for_ticket(
        &1,
        &payer2,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    assert_eq!(client.get_owner_tickets(&payer1).len(), 2);
    assert_eq!(client.get_owner_tickets(&payer2).len(), 1);
}

#[test]
fn test_event_supply_enforcement() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _contract_id, token_contract, event_contract_id) =
        setup_contract_with_token_and_event(&env);
    let payer1 = Address::generate(&env);
    let payer2 = Address::generate(&env);
    let payer3 = Address::generate(&env);
    let organizer = Address::generate(&env);
    let event_id = symbol_short!("SUPPLY");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &(amount * 3));
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer1, &amount);
    token_client.transfer(&admin, &payer2, &amount);
    token_client.transfer(&admin, &payer3, &amount);

    client.sync_event_config(
        &event_contract_id,
        &event_id,
        &organizer,
        &token,
        &true,
        &false,
        &0,
        &2,
    );

    client.pay_for_ticket(
        &1,
        &payer1,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    client.pay_for_ticket(
        &1,
        &payer2,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    let config = client.get_event_config(&event_id);
    assert_eq!(config.max_supply, 2);
    assert_eq!(config.sold_count, 2);

    let result = client.try_pay_for_ticket(
        &1,
        &payer3,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    assert_eq!(result.err(), Some(Ok(PaymentError::EventSoldOut)));
    assert_eq!(token_client.balance(&payer3), amount);
}

// ===== Platform Fee Tests =====

#[test]
fn test_initialize_invalid_fee_bps() {
    let env = Env::default();
    let contract_id = env.register(PaymentsContract, ());
    let client = PaymentsContractClient::new(&env, &contract_id);
    let event_contract_id = env.register(MockEventContract, ());

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let platform_wallet = Address::generate(&env);

    let result = client.try_initialize(
        &admin,
        &token,
        &10_001,
        &platform_wallet,
        &event_contract_id,
    );
    assert_eq!(result.err(), Some(Ok(PaymentError::InvalidFeeBps)));
}

#[test]
fn test_withdraw_revenue_with_fee() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1704067200;
    });

    // 2.5% fee = 250 bps
    let (admin, token, client, contract_id, token_contract, _) = setup_contract_with_fee(&env, 250);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128; // 100 tokens

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    // Withdraw revenue — fee should be deducted
    let organizer = Address::generate(&env);
    client.withdraw_revenue(&event_id, &organizer);

    // Fee = 100_000_000 * 250 / 10_000 = 2_500_000
    let expected_fee = 2_500_000i128;
    let expected_organizer = amount - expected_fee;

    // Organizer gets amount minus fee
    assert_eq!(token_client.balance(&organizer), expected_organizer);
    // Contract still holds the fee portion
    assert_eq!(token_client.balance(&contract_id), expected_fee);
    // Platform revenue accumulated
    assert_eq!(client.get_platform_revenue(&event_id), expected_fee);
    // Event revenue is reset
    assert_eq!(client.get_event_revenue(&event_id), 0);

    // Withdrawal history records organizer amount (after fee)
    let history = client.get_withdrawal_history(&event_id);
    assert_eq!(history.len(), 1);
    assert_eq!(history.get(0).unwrap().amount, expected_organizer);
}

#[test]
fn test_withdraw_revenue_zero_fee() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1704067200;
    });

    // 0% fee
    let (admin, token, client, contract_id, token_contract, _) = setup_contract_with_fee(&env, 0);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    let organizer = Address::generate(&env);
    client.withdraw_revenue(&event_id, &organizer);

    // Full amount to organizer, nothing retained
    assert_eq!(token_client.balance(&organizer), amount);
    assert_eq!(token_client.balance(&contract_id), 0);
    assert_eq!(client.get_platform_revenue(&event_id), 0);
}

#[test]
fn test_withdraw_platform_revenue() {
    let env = Env::default();
    env.mock_all_auths();

    // 5% fee = 500 bps
    let (admin, token, client, contract_id, token_contract, _) = setup_contract_with_fee(&env, 500);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 200_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    // Withdraw revenue — fee accumulated
    let organizer = Address::generate(&env);
    client.withdraw_revenue(&event_id, &organizer);

    // Fee = 200_000_000 * 500 / 10_000 = 10_000_000
    let expected_fee = 10_000_000i128;
    assert_eq!(client.get_platform_revenue(&event_id), expected_fee);
    assert_eq!(token_client.balance(&contract_id), expected_fee);

    // Withdraw platform revenue
    client.withdraw_platform_revenue(&event_id);

    // Platform revenue reset, contract balance zero
    assert_eq!(client.get_platform_revenue(&event_id), 0);
    assert_eq!(token_client.balance(&contract_id), 0);
}

#[test]
fn test_withdraw_platform_revenue_no_revenue() {
    let env = Env::default();
    env.mock_all_auths();

    let (_admin, _token, client, _, _, _) = setup_contract_with_fee(&env, 500);
    let event_id = symbol_short!("EVENT1");

    let result = client.try_withdraw_platform_revenue(&event_id);
    assert_eq!(result.err(), Some(Ok(PaymentError::NoPlatformRevenue)));
}

#[test]
fn test_set_platform_fee() {
    let env = Env::default();
    env.mock_all_auths();

    let (_admin, _token, client, _, _, _) = setup_contract_with_fee(&env, 250);

    assert_eq!(client.get_platform_fee_bps(), 250);

    let new_wallet = Address::generate(&env);
    client.set_platform_fee(&500, &new_wallet);

    assert_eq!(client.get_platform_fee_bps(), 500);
}

#[test]
fn test_set_platform_fee_invalid_bps() {
    let env = Env::default();
    env.mock_all_auths();

    let (_admin, _token, client, _, _, _) = setup_contract_with_fee(&env, 250);
    let new_wallet = Address::generate(&env);

    let result = client.try_set_platform_fee(&10_001, &new_wallet);
    assert_eq!(result.err(), Some(Ok(PaymentError::InvalidFeeBps)));
}

#[test]
fn test_organizer_withdraw_with_fee() {
    let env = Env::default();
    env.mock_all_auths();

    // 10% fee = 1000 bps
    let (admin, token, client, contract_id, token_contract, event_contract) =
        setup_contract_with_fee(&env, 1000);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    // Set up event config for organizer withdraw
    let organizer = Address::generate(&env);
    bind_event(&client, &event_contract, &event_id, &organizer, &token);

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    // Mark event as completed, then organizer withdraws via `withdraw` function
    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Completed);
    client.withdraw(&organizer, &event_id);

    // Fee = 100_000_000 * 1000 / 10_000 = 10_000_000
    let expected_fee = 10_000_000i128;
    let expected_organizer = amount - expected_fee;

    assert_eq!(token_client.balance(&organizer), expected_organizer);
    assert_eq!(token_client.balance(&contract_id), expected_fee);
    assert_eq!(client.get_platform_revenue(&event_id), expected_fee);
}

#[test]
fn test_platform_revenue_accumulates_across_withdrawals() {
    let env = Env::default();
    env.mock_all_auths();

    // 5% fee = 500 bps
    let (admin, token, client, _contract_id, token_contract, _) =
        setup_contract_with_fee(&env, 500);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;
    let token_client = token::Client::new(&env, &token);
    let organizer = Address::generate(&env);

    // First cycle
    token_contract.mint(&admin, &amount);
    token_client.transfer(&admin, &payer, &amount);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    client.withdraw_revenue(&event_id, &organizer);

    let fee_per_cycle = 5_000_000i128; // 100M * 500 / 10000
    assert_eq!(client.get_platform_revenue(&event_id), fee_per_cycle);

    // Second cycle
    token_contract.mint(&admin, &amount);
    token_client.transfer(&admin, &payer, &amount);
    client.pay_for_ticket(
        &2,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    client.withdraw_revenue(&event_id, &organizer);

    // Platform revenue should accumulate
    assert_eq!(client.get_platform_revenue(&event_id), fee_per_cycle * 2);
}

#[test]
fn test_replay_attack_rejected_detailed() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &(amount * 2));
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &(amount * 2));

    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Active);

    let nonce = 101u64;

    // First attempt succeeds
    client.pay_for_ticket(
        &nonce,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    // Second attempt with same nonce fails with DuplicateRequest
    let result = client.try_pay_for_ticket(
        &nonce,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    assert_eq!(result.err(), Some(Ok(PaymentError::DuplicateRequest)));
    assert_eq!(client.get_owner_tickets(&payer).len(), 1);
    assert_eq!(token_client.balance(&payer), amount);
}

#[test]
fn test_zero_nonce_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &(amount * 2));
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &(amount * 2));

    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Active);

    let nonce = 0u64;

    let result = client.try_pay_for_ticket(
        &nonce,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    assert_eq!(result.err(), Some(Ok(PaymentError::NonceRequired)));

    assert_eq!(client.get_owner_tickets(&payer).len(), 0);
    assert_eq!(token_client.balance(&payer), amount * 2);
}

#[test]
fn test_partial_refund_success() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Active);

    let payment_id = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    // Initial revenue
    assert_eq!(client.get_event_revenue(&event_id), amount);

    // 1. Partial refund 30%
    let partial_1 = 30_000_000i128;
    client.refund(&admin, &payment_id, &Some(partial_1));

    assert_eq!(token_client.balance(&payer), partial_1);
    assert_eq!(client.get_event_revenue(&event_id), amount - partial_1);

    let payment = client.get_payment(&payment_id);
    assert_eq!(payment.refunded_amount, partial_1);
    assert_eq!(payment.status, PaymentStatus::Held);

    // 2. Partial refund another 20%
    let partial_2 = 20_000_000i128;
    client.refund(&admin, &payment_id, &Some(partial_2));

    assert_eq!(token_client.balance(&payer), partial_1 + partial_2);
    assert_eq!(
        client.get_event_revenue(&event_id),
        amount - partial_1 - partial_2
    );

    // 3. Full refund remaining (passing None)
    client.refund(&admin, &payment_id, &None);

    assert_eq!(token_client.balance(&payer), amount);
    assert_eq!(client.get_event_revenue(&event_id), 0);

    let final_payment = client.get_payment(&payment_id);
    assert_eq!(final_payment.refunded_amount, amount);
    assert_eq!(final_payment.status, PaymentStatus::Refunded);
}

#[test]
fn test_partial_refund_exceeds_amount_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client, _, token_contract, _) = setup_contract_with_token(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount = 100_000_000i128;

    token_contract.mint(&admin, &amount);
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&admin, &payer, &amount);

    set_event_status_for_test(&client, &admin, &event_id, &EventStatus::Active);

    let payment_id = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    // Try to refund more than original amount
    let result = client.try_refund(&admin, &payment_id, &Some(amount + 1));
    assert!(result.is_err());
}
