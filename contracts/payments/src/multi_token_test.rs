use super::*;
use mock_event_contract::MockEventContract;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, token, Address, Env};

fn setup_contract_with_two_tokens(
    env: &Env,
) -> (
    Address,
    Address,
    Address,
    PaymentsContractClient<'_>,
    Address,
    Address,
    token::StellarAssetClient<'_>,
    token::StellarAssetClient<'_>,
) {
    let contract_id = env.register(PaymentsContract, ());
    let client = PaymentsContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let token1_contract = env.register_stellar_asset_contract_v2(admin.clone());
    let token1 = token1_contract.address();
    let token2_contract = env.register_stellar_asset_contract_v2(admin.clone());
    let token2 = token2_contract.address();

    let event_contract_id = env.register(MockEventContract, ());
    let platform_wallet = Address::generate(env);
    client.initialize(&admin, &token1, &0, &platform_wallet, &event_contract_id);

    let token1_client = token::StellarAssetClient::new(env, &token1);
    let token2_client = token::StellarAssetClient::new(env, &token2);

    (
        admin,
        token1,
        token2,
        client,
        contract_id,
        event_contract_id,
        token1_client,
        token2_client,
    )
}

#[test]
fn test_multi_token_payments() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token1, token2, client, contract_id, _event_contract, token1_client, token2_client) =
        setup_contract_with_two_tokens(&env);
    let payer1 = Address::generate(&env);
    let payer2 = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount1 = 100_000_000i128;
    let amount2 = 200_000_000i128;

    // Setup tokens
    token1_client.mint(&admin, &amount1);
    token2_client.mint(&admin, &amount2);

    let token1_transfer_client = token::Client::new(&env, &token1);
    let token2_transfer_client = token::Client::new(&env, &token2);

    token1_transfer_client.transfer(&admin, &payer1, &amount1);
    token2_transfer_client.transfer(&admin, &payer2, &amount2);

    // Pay with different tokens
    let payment_id1 = client.pay_for_ticket(
        &1,
        &payer1,
        &event_id,
        &amount1,
        &None,
        &token1,
        &PaymentPrivacy::Standard,
    );
    let payment_id2 = client.pay_for_ticket(
        &1,
        &payer2,
        &event_id,
        &amount2,
        &None,
        &token2,
        &PaymentPrivacy::Standard,
    );

    // Verify payments
    let payment1 = client.get_payment(&payment_id1);
    let payment2 = client.get_payment(&payment_id2);

    assert_eq!(payment1.token, token1);
    assert_eq!(payment1.amount, amount1);
    assert_eq!(payment2.token, token2);
    assert_eq!(payment2.amount, amount2);

    // Verify token-specific revenue tracking
    assert_eq!(client.get_event_token_revenue(&event_id, &token1), amount1);
    assert_eq!(client.get_event_token_revenue(&event_id, &token2), amount2);

    // Verify total revenue (should be sum of both)
    assert_eq!(client.get_event_revenue(&event_id), amount1 + amount2);

    // Verify event tokens list
    let event_tokens = client.get_event_tokens(&event_id);
    assert_eq!(event_tokens.len(), 2);
    assert!(event_tokens.contains(&token1));
    assert!(event_tokens.contains(&token2));

    // Verify contract balances
    assert_eq!(token1_transfer_client.balance(&contract_id), amount1);
    assert_eq!(token2_transfer_client.balance(&contract_id), amount2);
}

#[test]
fn test_multi_token_refund_updates_only_the_paid_token_bucket() {
    let env = Env::default();
    env.mock_all_auths();

    let (
        admin,
        token1,
        token2,
        client,
        _contract_id,
        _event_contract,
        token1_client,
        token2_client,
    ) = setup_contract_with_two_tokens(&env);
    let payer1 = Address::generate(&env);
    let payer2 = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount1 = 100_000_000i128;
    let amount2 = 200_000_000i128;

    token1_client.mint(&admin, &amount1);
    token2_client.mint(&admin, &amount2);

    let token1_transfer_client = token::Client::new(&env, &token1);
    let token2_transfer_client = token::Client::new(&env, &token2);
    token1_transfer_client.transfer(&admin, &payer1, &amount1);
    token2_transfer_client.transfer(&admin, &payer2, &amount2);

    let payment_id1 = client.pay_for_ticket(
        &1,
        &payer1,
        &event_id,
        &amount1,
        &None,
        &token1,
        &PaymentPrivacy::Standard,
    );
    client.pay_for_ticket(
        &2,
        &payer2,
        &event_id,
        &amount2,
        &None,
        &token2,
        &PaymentPrivacy::Standard,
    );

    client.refund(&admin, &payment_id1, &None);

    assert_eq!(client.get_event_token_revenue(&event_id, &token1), 0);
    assert_eq!(client.get_event_token_revenue(&event_id, &token2), amount2);
    assert_eq!(client.get_event_revenue(&event_id), amount2);
}

#[test]
fn test_withdraw_uses_only_the_event_payout_token_revenue() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token1, token2, client, contract_id, event_contract, token1_client, token2_client) =
        setup_contract_with_two_tokens(&env);
    let payer1 = Address::generate(&env);
    let payer2 = Address::generate(&env);
    let organizer = Address::generate(&env);
    let event_id = symbol_short!("EVENT1");
    let amount1 = 100_000_000i128;
    let amount2 = 200_000_000i128;

    token1_client.mint(&admin, &amount1);
    token2_client.mint(&admin, &amount2);

    let token1_transfer_client = token::Client::new(&env, &token1);
    let token2_transfer_client = token::Client::new(&env, &token2);
    token1_transfer_client.transfer(&admin, &payer1, &amount1);
    token2_transfer_client.transfer(&admin, &payer2, &amount2);

    client.sync_event_config(
        &event_contract,
        &event_id,
        &organizer,
        &token1,
        &true,
        &false,
        &0,
        &0,
    );
    client.pay_for_ticket(
        &1,
        &payer1,
        &event_id,
        &amount1,
        &None,
        &token1,
        &PaymentPrivacy::Standard,
    );
    let second_payment_id = client.pay_for_ticket(
        &2,
        &payer2,
        &event_id,
        &amount2,
        &None,
        &token2,
        &PaymentPrivacy::Standard,
    );

    client.set_event_status(&admin, &event_id, &EventStatus::Completed);
    client.withdraw(&organizer, &event_id);

    let second_payment = client.get_payment(&second_payment_id);
    assert_eq!(second_payment.status, PaymentStatus::Held);
    assert_eq!(client.get_event_token_revenue(&event_id, &token1), 0);
    assert_eq!(client.get_event_token_revenue(&event_id, &token2), amount2);
    assert_eq!(client.get_event_revenue(&event_id), amount2);
    assert_eq!(token1_transfer_client.balance(&organizer), amount1);
    assert_eq!(token2_transfer_client.balance(&contract_id), amount2);
}
