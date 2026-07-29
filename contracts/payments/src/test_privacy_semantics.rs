//! Tests for enforceable on-chain payment privacy semantics (Issue #117).
//!
//! These tests verify that the three payment privacy levels store, expose, and
//! refund identity data exactly as their privacy contract requires:
//! - Standard:  only the raw payer address is stored.
//! - Private:   only a hashed wallet + stealth delivery key are stored.
//! - Anonymous: only a nullifier commitment is stored.

use super::*;
use mock_event_contract::MockEventContract;
use privacy_utils::MaskedAddress;
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::xdr::ContractEventBody;
use soroban_sdk::{
    symbol_short, token, Address, BytesN, Env, Symbol, TryFromVal, TryIntoVal, Val, Vec as SdkVec,
};

/// Decode the `MaskedAddress` carried by an event published in the **last**
/// contract invocation (so call this immediately after `pay_for_ticket`, before
/// any `get_*` query resets the event buffer). The event is matched on its
/// `event_type` field (index 0 of the `vec`-format payload); `field_index` is
/// the position of the masked-identity field. This asserts what actually goes on
/// the wire, not just what is stored, so a regression in `events.rs` is caught.
fn emitted_masked_identity(env: &Env, event_type_name: &str, field_index: u32) -> MaskedAddress {
    let published = env.events().all();
    for event in published.events().iter() {
        let ContractEventBody::V0(body) = &event.body;
        let data_val: Val = match Val::try_from_val(env, &body.data) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let fields: SdkVec<Val> = match data_val.try_into_val(env) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let event_type: Symbol = match fields.get(0).and_then(|v| v.try_into_val(env).ok()) {
            Some(s) => s,
            None => continue,
        };
        if event_type == Symbol::new(env, event_type_name) {
            return fields
                .get(field_index)
                .unwrap()
                .try_into_val(env)
                .expect("masked identity field decodes to MaskedAddress");
        }
    }
    panic!("expected event was not emitted in the last invocation");
}

/// The masked payer identity emitted in the last `payment_received` event.
fn emitted_payment_payer(env: &Env) -> MaskedAddress {
    emitted_masked_identity(env, "payment_received", 3)
}

/// The masked owner identity emitted in the last `ticket_issued` event.
fn emitted_ticket_owner(env: &Env) -> MaskedAddress {
    emitted_masked_identity(env, "ticket_issued", 3)
}

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

    // Capture emitted identities from the pay invocation before any query
    // invocation resets the event buffer. Standard payments emit the full address.
    assert_eq!(
        emitted_payment_payer(&env),
        MaskedAddress::Full(payer.clone())
    );
    assert_eq!(
        emitted_ticket_owner(&env),
        MaskedAddress::Full(payer.clone())
    );

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
    // Emitted identity must be the full (unmasked) address — capture it before
    // the get_payment query invocation resets the event buffer.
    assert_eq!(
        emitted_payment_payer(&env),
        MaskedAddress::Full(payer.clone())
    );
    // Standard payment must also retain the full address on-chain.
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
    assert_eq!(client.get_owner_tickets(&payer).len(), 0);
}

#[test]
fn test_private_requires_stealth_key() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let tc = token::Client::new(&env, &token);
    let payer_before = tc.balance(&payer);
    let contract_before = tc.balance(&cid);

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
    // A post-transfer validation failure must not silently debit any funds.
    assert_eq!(tc.balance(&payer), payer_before);
    assert_eq!(tc.balance(&cid), contract_before);
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
    // Capture emitted identities before the get_payment query resets the buffer.
    // The emitted event exposes a hashed wallet — never a full address.
    let emitted = emitted_payment_payer(&env);
    let emitted_owner = emitted_ticket_owner(&env);
    assert!(!matches!(emitted, MaskedAddress::Full(_)));
    assert!(!matches!(emitted_owner, MaskedAddress::Full(_)));

    // No raw address is recoverable from the stored record, and the emitted
    // masked identity matches the stored hashed wallet exactly.
    let stored = client.get_payment(&pid);
    assert_eq!(stored.payer, None);
    let hashed = stored
        .hashed_wallet
        .expect("private payment stores a hashed wallet");
    assert_eq!(emitted, MaskedAddress::Hashed(hashed));
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
    let (admin, token, client, cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let tc = token::Client::new(&env, &token);
    let payer_before = tc.balance(&payer);
    let contract_before = tc.balance(&cid);

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
    // A post-transfer validation failure must not silently debit any funds.
    assert_eq!(tc.balance(&payer), payer_before);
    assert_eq!(tc.balance(&cid), contract_before);
}

#[test]
fn test_standard_rejects_privacy_material() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    let tc = token::Client::new(&env, &token);
    let payer_before = tc.balance(&payer);
    let contract_before = tc.balance(&cid);

    // Privacy material is mutually exclusive: a Standard payment that also
    // carries Anonymous/Private material is rejected, funds untouched.
    let result = client.try_pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Standard,
        &Some(commitment(&env)),
        &None,
    );
    assert_eq!(result.err(), Some(Ok(PaymentError::PrivacyLevelMismatch)));
    assert_eq!(tc.balance(&payer), payer_before);
    assert_eq!(tc.balance(&cid), contract_before);
}

#[test]
fn test_private_rejects_nullifier_commitment() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, _tc) = setup(&env);
    let payer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    let amount = 1_000i128;
    fund(&env, &admin, &payer, &token, amount);

    // A Private payment must not carry a nullifier commitment (Anonymous material).
    let result = client.try_pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &amount,
        &None,
        &token,
        &PaymentPrivacy::Private,
        &Some(commitment(&env)),
        &Some(stealth_key(&env)),
    );
    assert_eq!(result.err(), Some(Ok(PaymentError::PrivacyLevelMismatch)));
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
    // The emitted events reference only the nullifier commitment — no address,
    // no wallet hash. Capture them before the get_payment query resets the buffer.
    let emitted = emitted_payment_payer(&env);
    assert_eq!(emitted, MaskedAddress::Hashed(commitment(&env)));
    assert!(!matches!(emitted, MaskedAddress::Full(_)));
    assert_eq!(
        emitted_ticket_owner(&env),
        MaskedAddress::Hashed(commitment(&env))
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
