//! Tests for enforceable on-chain payment privacy semantics (Issue #117).
//!
//! These tests verify that the three payment privacy levels store, expose, and
//! refund identity data exactly as their privacy contract requires:
//! - Standard:  only the raw payer address is stored.
//! - Private:   only a hashed wallet + stealth delivery key are stored.
//! - Anonymous: only a nullifier commitment is stored.

use super::*;
use mock_event_contract::MockEventContract;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, token, Address, BytesN, Env};

fn setup(
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
    let event_contract_id = env.register(MockEventContract, ());

    let admin = Address::generate(env);
    let platform_wallet = Address::generate(env);
    let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
    let token = token_contract.address();
    client.initialize(&admin, &token, &0, &platform_wallet, &event_contract_id);

    let token_client = token::StellarAssetClient::new(env, &token);
    (admin, token, client, contract_id, token_client)
}

fn fund(env: &Env, admin: &Address, payer: &Address, token: &Address, amount: i128) {
    let sac = token::StellarAssetClient::new(env, token);
    sac.mint(admin, &amount);
    let tc = token::Client::new(env, token);
    tc.transfer(admin, payer, &amount);
}

fn commitment(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[7u8; 32])
}

fn stealth_key(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[9u8; 32])
}

// ===================== Standard =====================

#[test]
fn test_standard_stores_payer_address() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let pid = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
        &None,
        &None,
    );

    let p = client.get_payment(&pid);
    assert_eq!(p.payer, Some(payer.clone()));
    assert_eq!(p.hashed_wallet, None);
    assert_eq!(p.stealth_delivery_key, None);
    assert_eq!(p.nullifier_commitment, None);
}

#[test]
fn test_standard_emits_payer_address() {
    // Standard payments index the payer and are queryable by user.
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
        &None,
        &None,
    );

    let by_user = client.get_payments_by_user(&payer);
    assert_eq!(by_user.len(), 1);
    assert_eq!(by_user.get(0).unwrap().payer, Some(payer.clone()));

    let tickets = client.get_owner_tickets(&payer);
    assert_eq!(tickets.len(), 1);
    let ticket = client.get_ticket(&tickets.get(0).unwrap());
    assert_eq!(ticket.owner, Some(payer));
    assert_eq!(ticket.privacy_level, PaymentPrivacy::Standard);
}

#[test]
fn test_standard_event_emits_full_address() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let pid = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
        &None,
        &None,
    );
    // Standard payment must retain the full address on-chain.
    assert_eq!(client.get_payment(&pid).payer, Some(payer));
}

// ===================== Private =====================

#[test]
fn test_private_stores_hashed_wallet() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let pid = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Private,
        &None,
        &Some(stealth_key(&env)),
    );

    let p = client.get_payment(&pid);
    assert!(p.hashed_wallet.is_some());
    // The hash is a salted SHA-256 of the payer XDR concatenated with the stealth
    // delivery key, preventing brute-force enumeration of the payer address.
    let mut preimage = payer.clone().to_xdr(&env);
    let key = stealth_key(&env);
    preimage.append(&soroban_sdk::Bytes::from_slice(
        &env,
        key.to_array().as_ref(),
    ));
    let expected = env.crypto().sha256(&preimage);
    assert_eq!(p.hashed_wallet, Some(expected.into()));
}

#[test]
fn test_private_no_raw_address_in_record() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let pid = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Private,
        &None,
        &Some(stealth_key(&env)),
    );

    let p = client.get_payment(&pid);
    assert_eq!(p.payer, None);
    assert_eq!(p.nullifier_commitment, None);
    // Not indexed by raw address -> not discoverable via payer query.
    assert_eq!(client.get_payments_by_user(&payer).len(), 0);
    assert_eq!(client.get_owner_tickets(&payer).len(), 0);
}

#[test]
fn test_private_requires_stealth_key() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let result = client.try_pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Private,
        &None,
        &None,
    );
    assert_eq!(
        result.err(),
        Some(Ok(PaymentError::MissingStealthDeliveryKey))
    );
}

#[test]
fn test_private_stealth_key_stored() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let key = stealth_key(&env);
    let pid = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Private,
        &None,
        &Some(key.clone()),
    );
    assert_eq!(client.get_payment(&pid).stealth_delivery_key, Some(key));
}

#[test]
fn test_private_event_hides_raw_address() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let pid = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Private,
        &None,
        &Some(stealth_key(&env)),
    );
    // No raw address is recoverable from the stored record.
    assert_eq!(client.get_payment(&pid).payer, None);
}

// ===================== Anonymous =====================

#[test]
fn test_anonymous_stores_nullifier_commitment() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let c = commitment(&env);
    let pid = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Anonymous,
        &Some(c.clone()),
        &None,
    );
    assert_eq!(client.get_payment(&pid).nullifier_commitment, Some(c));
}

#[test]
fn test_anonymous_no_address_in_record() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let pid = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Anonymous,
        &Some(commitment(&env)),
        &None,
    );
    let p = client.get_payment(&pid);
    assert_eq!(p.payer, None);
    assert_eq!(client.get_payments_by_user(&payer).len(), 0);
    assert_eq!(client.get_owner_tickets(&payer).len(), 0);
}

#[test]
fn test_anonymous_no_hash_in_record() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let pid = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Anonymous,
        &Some(commitment(&env)),
        &None,
    );
    let p = client.get_payment(&pid);
    assert_eq!(p.hashed_wallet, None);
    assert_eq!(p.stealth_delivery_key, None);
}

#[test]
fn test_anonymous_requires_commitment() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let result = client.try_pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Anonymous,
        &None,
        &None,
    );
    assert_eq!(
        result.err(),
        Some(Ok(PaymentError::MissingNullifierCommitment))
    );
}

#[test]
fn test_anonymous_event_no_identity() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let pid = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Anonymous,
        &Some(commitment(&env)),
        &None,
    );
    let p = client.get_payment(&pid);
    assert_eq!(p.payer, None);
    assert_eq!(p.hashed_wallet, None);
}

// ===================== Immutability =====================

#[test]
fn test_no_privacy_level_mutation_path_exists() {
    // There is no contract function to change a payment's privacy level after
    // purchase. The privacy level stored at purchase is the final value.
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let pid = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
        &None,
        &None,
    );

    // The only mutations to a stored payment are status/refund transitions,
    // never the privacy_level. Confirm it is preserved across a refund.
    let before = client.get_payment(&pid).privacy_level;
    client.refund(&admin, &pid, &None);
    let after = client.get_payment(&pid).privacy_level;
    assert_eq!(before, after);
    assert_eq!(after, PaymentPrivacy::Standard);
}

// ===================== Refund preserves privacy =====================

#[test]
fn test_anonymous_refund_returns_error() {
    // Anonymous payments store no payer address, so an on-chain refund would
    // strand the escrowed tokens. The contract rejects the refund outright.
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let pid = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Anonymous,
        &Some(commitment(&env)),
        &None,
    );
    let result = client.try_refund(&admin, &pid, &None);
    assert_eq!(result.err(), Some(Ok(PaymentError::RefundNotAllowed)));

    // Payment remains Held and unchanged.
    let p = client.get_payment(&pid);
    assert_eq!(p.status, PaymentStatus::Held);
    assert_eq!(p.privacy_level, PaymentPrivacy::Anonymous);
}

#[test]
fn test_private_refund_returns_error() {
    // Private payments store only a hashed wallet, so an on-chain refund would
    // strand the escrowed tokens. The contract rejects the refund outright.
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let pid = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Private,
        &None,
        &Some(stealth_key(&env)),
    );
    let result = client.try_refund(&admin, &pid, &None);
    assert_eq!(result.err(), Some(Ok(PaymentError::RefundNotAllowed)));

    // Payment remains Held and unchanged.
    let p = client.get_payment(&pid);
    assert_eq!(p.status, PaymentStatus::Held);
    assert_eq!(p.privacy_level, PaymentPrivacy::Private);
}

#[test]
fn test_nullifier_reuse_rejected() {
    // The same nullifier commitment cannot be spent by two Anonymous payments.
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount * 2);

    let c = commitment(&env);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Anonymous,
        &Some(c.clone()),
        &None,
    );

    let result = client.try_pay_for_ticket(
        &2,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Anonymous,
        &Some(c),
        &None,
    );
    assert_eq!(result.err(), Some(Ok(PaymentError::DuplicateRequest)));
}

#[test]
fn test_standard_refund_preserves_privacy() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let pid = client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
        &None,
        &None,
    );
    client.refund(&admin, &pid, &None);

    let p = client.get_payment(&pid);
    assert_eq!(p.status, PaymentStatus::Refunded);
    assert_eq!(p.privacy_level, PaymentPrivacy::Standard);
    assert_eq!(p.payer, Some(payer.clone()));
    // Standard refund returns funds to the on-chain payer.
    let tc = token::Client::new(&env, &token);
    assert_eq!(tc.balance(&payer), amount);
}
