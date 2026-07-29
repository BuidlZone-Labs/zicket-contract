//! Tests for attendee-initiated dispute resolution and refund hooks (Issue #113/attendee path).

use super::*;
use mock_event_contract::MockEventContract;
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{symbol_short, token, Address, BytesN, Env, Symbol};

fn setup(
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

fn fund(env: &Env, admin: &Address, payer: &Address, token: &Address, amount: i128) {
    let sac = token::StellarAssetClient::new(env, token);
    sac.mint(admin, &amount);
    let tc = token::Client::new(env, token);
    tc.transfer(admin, payer, &amount);
}

fn bind_event(
    client: &PaymentsContractClient,
    event_contract: &Address,
    event_id: &Symbol,
    organizer: &Address,
    payout_token: &Address,
    allow_anonymous: bool,
    end_ledger: u32,
) {
    client.sync_event_config(
        event_contract,
        event_id,
        organizer,
        payout_token,
        &allow_anonymous,
        &false,
        &1000,
        &0,
        &0,
        &end_ledger,
        &17280,
        &0,
        &None,
        &false,
    );
}

#[test]
fn test_raise_dispute_success_and_window_rules() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc, event_contract) = setup(&env);
    let organizer = Address::generate(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let end_ledger = 1000u32;

    bind_event(
        &client,
        &event_contract,
        &event_id,
        &organizer,
        &token,
        false,
        end_ledger,
    );
    fund(&env, &admin, &payer, &token, 2000);

    env.ledger().with_mut(|l| l.sequence_number = 500);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &1000,
        &None,
        &token,
        &PaymentPrivacy::Standard,
        &None,
        &None,
    );
    let ticket_id = 1u64;

    // Before event_end_ledger should fail
    let res = client.try_raise_dispute(&ticket_id, &0, &None);
    assert_eq!(res, Err(Ok(PaymentError::DisputeWindowClosed)));

    // At event_end_ledger should succeed
    env.ledger().with_mut(|l| l.sequence_number = end_ledger);
    client.raise_dispute(&ticket_id, &0, &None);

    let ticket = client.get_ticket(&ticket_id);
    let payment = client.get_payment(&ticket.payment_id);
    assert_eq!(payment.status, PaymentStatus::Disputed);

    // Revenue should be reduced while disputed
    assert_eq!(client.get_event_revenue(&event_id), 0);

    // Raising dispute again on same ticket should fail
    let res = client.try_raise_dispute(&ticket_id, &0, &None);
    assert_eq!(res, Err(Ok(PaymentError::DisputeAlreadyExists)));
}

#[test]
fn test_dispute_window_expiration() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc, event_contract) = setup(&env);
    let organizer = Address::generate(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let end_ledger = 1000u32;

    bind_event(
        &client,
        &event_contract,
        &event_id,
        &organizer,
        &token,
        false,
        end_ledger,
    );
    fund(&env, &admin, &payer, &token, 2000);

    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &1000,
        &None,
        &token,
        &PaymentPrivacy::Standard,
        &None,
        &None,
    );
    let ticket_id = 1u64;

    // 7 days in ledgers = 17280 * 7 = 120960. At end_ledger + 120960 window is closed.
    env.ledger()
        .with_mut(|l| l.sequence_number = end_ledger + 120_960);
    let res = client.try_raise_dispute(&ticket_id, &1, &None);
    assert_eq!(res, Err(Ok(PaymentError::DisputeWindowClosed)));
}

#[test]
fn test_invalid_dispute_reason_code() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc, event_contract) = setup(&env);
    let organizer = Address::generate(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let end_ledger = 1000u32;

    bind_event(
        &client,
        &event_contract,
        &event_id,
        &organizer,
        &token,
        false,
        end_ledger,
    );
    fund(&env, &admin, &payer, &token, 2000);

    env.ledger().with_mut(|l| l.sequence_number = end_ledger);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &1000,
        &None,
        &token,
        &PaymentPrivacy::Standard,
        &None,
        &None,
    );

    let res = client.try_raise_dispute(&1u64, &3, &None);
    assert_eq!(res, Err(Ok(PaymentError::InvalidDisputeReason)));
}

#[test]
fn test_admin_approve_refund() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc, event_contract) = setup(&env);
    let organizer = Address::generate(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let end_ledger = 1000u32;

    bind_event(
        &client,
        &event_contract,
        &event_id,
        &organizer,
        &token,
        false,
        end_ledger,
    );
    fund(&env, &admin, &payer, &token, 2000);

    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &1000,
        &None,
        &token,
        &PaymentPrivacy::Standard,
        &None,
        &None,
    );
    let ticket_id = 1u64;

    env.ledger().with_mut(|l| l.sequence_number = end_ledger);
    client.raise_dispute(&ticket_id, &2, &None);

    let tc = token::Client::new(&env, &token);
    let balance_before = tc.balance(&payer);

    client.approve_refund(&ticket_id);

    let payment = client.get_payment(&1u64);
    assert_eq!(payment.status, PaymentStatus::Refunded);
    assert_eq!(tc.balance(&payer), balance_before + 1000);
}

#[test]
fn test_admin_reject_dispute() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc, event_contract) = setup(&env);
    let organizer = Address::generate(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let end_ledger = 1000u32;

    bind_event(
        &client,
        &event_contract,
        &event_id,
        &organizer,
        &token,
        false,
        end_ledger,
    );
    fund(&env, &admin, &payer, &token, 2000);

    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &1000,
        &None,
        &token,
        &PaymentPrivacy::Standard,
        &None,
        &None,
    );
    let ticket_id = 1u64;

    env.ledger().with_mut(|l| l.sequence_number = end_ledger);
    client.raise_dispute(&ticket_id, &1, &None);

    assert_eq!(client.get_event_revenue(&event_id), 0);

    client.reject_dispute(&ticket_id);

    let payment = client.get_payment(&1u64);
    assert_eq!(payment.status, PaymentStatus::Held);
    assert_eq!(client.get_event_revenue(&event_id), 1000);
}

#[test]
fn test_dispute_14_day_timeout_auto_releases_to_organizer() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc, event_contract) = setup(&env);
    let organizer = Address::generate(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let end_ledger = 1000u32;

    bind_event(
        &client,
        &event_contract,
        &event_id,
        &organizer,
        &token,
        false,
        end_ledger,
    );
    fund(&env, &admin, &payer, &token, 2000);

    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &1000,
        &None,
        &token,
        &PaymentPrivacy::Standard,
        &None,
        &None,
    );
    let ticket_id = 1u64;

    env.ledger().with_mut(|l| l.sequence_number = end_ledger);
    client.raise_dispute(&ticket_id, &0, &None);
    assert_eq!(client.get_event_revenue(&event_id), 0);

    // Advance 14 days in ledgers (17280 * 14 = 241920)
    env.ledger()
        .with_mut(|l| l.sequence_number = end_ledger + 241_920);

    client.process_dispute_timeouts(&event_id);

    let payment = client.get_payment(&1u64);
    assert_eq!(payment.status, PaymentStatus::Held);
    assert_eq!(client.get_event_revenue(&event_id), 1000);
}

#[test]
fn test_anonymous_ticket_dispute() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc, event_contract) = setup(&env);
    let organizer = Address::generate(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let end_ledger = 1000u32;

    bind_event(
        &client,
        &event_contract,
        &event_id,
        &organizer,
        &token,
        true,
        end_ledger,
    );
    fund(&env, &admin, &payer, &token, 2000);

    let preimage = soroban_sdk::Bytes::from_slice(&env, &[1u8; 32]);
    let commitment: BytesN<32> = env.crypto().sha256(&preimage).into();
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &1000,
        &None,
        &token,
        &PaymentPrivacy::Anonymous,
        &Some(commitment),
        &None,
    );
    let ticket_id = 1u64;

    env.ledger().with_mut(|l| l.sequence_number = end_ledger);
    // Raise dispute on anonymous ticket via ticket_id only
    client.raise_dispute(&ticket_id, &2, &Some(preimage));

    let payment = client.get_payment(&1u64);
    assert_eq!(payment.status, PaymentStatus::Disputed);
}
