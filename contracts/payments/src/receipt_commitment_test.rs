//! Tests for the zkEmail receipt commitment hook (#119).
//!
//! The commitment is a salted hash of the buyer's email computed off-chain
//! (e.g. `H(email || ticket_id)`). These tests assert that the hash is stored
//! and verifiable, that it is optional, and — critically — that neither the raw
//! email nor the commitment hash is ever emitted in an event.

use super::*;
use mock_event_contract::MockEventContract;
use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{symbol_short, token, Address, BytesN, Env, Symbol};

struct World<'a> {
    env: &'a Env,
    admin: Address,
    token: Address,
    client: PaymentsContractClient<'a>,
}

fn setup(env: &Env, fee_bps: u32) -> World<'_> {
    let contract_id = env.register(PaymentsContract, ());
    let client = PaymentsContractClient::new(env, &contract_id);
    let event_contract = env.register(MockEventContract, ());
    let admin = Address::generate(env);
    let platform_wallet = Address::generate(env);
    let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
    let token = token_contract.address();
    client.initialize(&admin, &token, &fee_bps, &platform_wallet, &event_contract);
    World {
        env,
        admin,
        token,
        client,
    }
}

/// Fund a fresh payer and return its address.
fn funded_payer(w: &World, amount: i128) -> Address {
    let payer = Address::generate(w.env);
    token::StellarAssetClient::new(w.env, &w.token).mint(&payer, &amount);
    payer
}

fn pay(w: &World, payer: &Address, event_id: &Symbol, amount: i128) -> u64 {
    w.client.pay_for_ticket(
        &1,
        payer,
        event_id,
        &amount,
        &None,
        &w.token,
        &PaymentPrivacy::Standard,
    )
}

/// Render every emitted event to a debug string. A 32-byte hash, if emitted,
/// shows up as a contiguous 64-char hex run; absence of that run proves the
/// hash was never published.
fn events_debug(env: &Env) -> std::string::String {
    std::format!("{:?}", env.events().all())
}

/// The hex run that a `BytesN<32>` filled with `byte` produces in event output.
fn hash_hex_run(byte: u8) -> std::string::String {
    std::format!("{:02x}", byte).repeat(32)
}

#[test]
fn test_pay_with_commitment_stores_and_reads_back() {
    let env = Env::default();
    env.mock_all_auths();
    let w = setup(&env, 0);
    let event_id = symbol_short!("EVENT1");
    let payer = funded_payer(&w, 100_000_000);
    let commitment = BytesN::from_array(&env, &[7u8; 32]);

    let pid = w.client.pay_for_ticket_with_commitment(
        &1,
        &payer,
        &event_id,
        &100_000_000,
        &None,
        &w.token,
        &PaymentPrivacy::Standard,
        &Some(commitment.clone()),
    );

    // Stored on the record and exposed via the getter.
    assert_eq!(
        w.client.get_payment(&pid).zk_email_commitment,
        Some(commitment.clone())
    );
    assert_eq!(w.client.get_payment_commitment(&pid), Some(commitment));
}

#[test]
fn test_commitment_is_optional() {
    let env = Env::default();
    env.mock_all_auths();
    let w = setup(&env, 0);
    let event_id = symbol_short!("EVENT1");
    let payer = funded_payer(&w, 100_000_000);

    // Fully anonymous attendee: payment proceeds with no commitment.
    let pid = pay(&w, &payer, &event_id, 100_000_000);
    assert_eq!(w.client.get_payment_commitment(&pid), None);
    assert_eq!(w.client.get_payment(&pid).zk_email_commitment, None);
}

#[test]
fn test_bind_commitment_after_payment() {
    let env = Env::default();
    env.mock_all_auths();
    let w = setup(&env, 0);
    let event_id = symbol_short!("EVENT1");
    let payer = funded_payer(&w, 100_000_000);

    // The realistic flow: pay first (ticket_id assigned), then bind a commitment
    // salted with that ticket_id.
    let pid = pay(&w, &payer, &event_id, 100_000_000);
    assert_eq!(w.client.get_payment_commitment(&pid), None);

    let commitment = BytesN::from_array(&env, &[9u8; 32]);
    w.client.bind_email_commitment(&payer, &pid, &commitment);
    assert_eq!(w.client.get_payment_commitment(&pid), Some(commitment));
}

#[test]
fn test_bind_commitment_is_write_once() {
    let env = Env::default();
    env.mock_all_auths();
    let w = setup(&env, 0);
    let event_id = symbol_short!("EVENT1");
    let payer = funded_payer(&w, 100_000_000);
    let pid = pay(&w, &payer, &event_id, 100_000_000);

    let first = BytesN::from_array(&env, &[1u8; 32]);
    w.client.bind_email_commitment(&payer, &pid, &first);

    let second = BytesN::from_array(&env, &[2u8; 32]);
    let res = w.client.try_bind_email_commitment(&payer, &pid, &second);
    assert_eq!(res.err(), Some(Ok(PaymentError::CommitmentAlreadySet)));
    // Original commitment is unchanged.
    assert_eq!(w.client.get_payment_commitment(&pid), Some(first));
}

#[test]
fn test_bind_commitment_requires_payer_ownership() {
    let env = Env::default();
    env.mock_all_auths();
    let w = setup(&env, 0);
    let event_id = symbol_short!("EVENT1");
    let payer = funded_payer(&w, 100_000_000);
    let pid = pay(&w, &payer, &event_id, 100_000_000);

    let attacker = Address::generate(&env);
    let commitment = BytesN::from_array(&env, &[3u8; 32]);
    let res = w
        .client
        .try_bind_email_commitment(&attacker, &pid, &commitment);
    assert_eq!(res.err(), Some(Ok(PaymentError::Unauthorized)));
}

#[test]
fn test_bind_commitment_rejected_after_refund() {
    let env = Env::default();
    env.mock_all_auths();
    let w = setup(&env, 0);
    let event_id = symbol_short!("EVENT1");
    let payer = funded_payer(&w, 100_000_000);
    let pid = pay(&w, &payer, &event_id, 100_000_000);

    // Admin fully refunds the payment.
    w.client.refund(&w.admin, &pid, &None);

    let commitment = BytesN::from_array(&env, &[4u8; 32]);
    let res = w
        .client
        .try_bind_email_commitment(&payer, &pid, &commitment);
    assert_eq!(res.err(), Some(Ok(PaymentError::CommitmentNotAllowed)));
}

#[test]
fn test_verify_email_commitment_matches_and_mismatches() {
    let env = Env::default();
    env.mock_all_auths();
    let w = setup(&env, 0);
    let event_id = symbol_short!("EVENT1");
    let payer = funded_payer(&w, 100_000_000);
    let pid = pay(&w, &payer, &event_id, 100_000_000);

    let commitment = BytesN::from_array(&env, &[5u8; 32]);
    w.client.bind_email_commitment(&payer, &pid, &commitment);

    // A relayer recomputes H(email || ticket_id) and verifies it on-chain.
    assert!(w.client.verify_email_commitment(&pid, &commitment));
    let wrong = BytesN::from_array(&env, &[6u8; 32]);
    assert!(!w.client.verify_email_commitment(&pid, &wrong));
}

#[test]
fn test_verify_returns_false_when_no_commitment() {
    let env = Env::default();
    env.mock_all_auths();
    let w = setup(&env, 0);
    let event_id = symbol_short!("EVENT1");
    let payer = funded_payer(&w, 100_000_000);
    let pid = pay(&w, &payer, &event_id, 100_000_000);

    let candidate = BytesN::from_array(&env, &[8u8; 32]);
    assert!(!w.client.verify_email_commitment(&pid, &candidate));
}

#[test]
fn test_commitment_is_stored_but_never_emitted() {
    let env = Env::default();
    env.mock_all_auths();
    let w = setup(&env, 0);
    let event_id = symbol_short!("EVENT1");
    let payer = funded_payer(&w, 100_000_000);

    // `email_hash` (a legacy receipt hash) IS emitted in PaymentReceiptRequested;
    // the new zkEmail `commitment` must NOT be.
    let receipt_hash = BytesN::from_array(&env, &[0xAAu8; 32]);
    let commitment = BytesN::from_array(&env, &[0xBBu8; 32]);

    let pid = w.client.pay_for_ticket_with_commitment(
        &1,
        &payer,
        &event_id,
        &100_000_000,
        &Some(receipt_hash.clone()),
        &w.token,
        &PaymentPrivacy::Standard,
        &Some(commitment.clone()),
    );

    let dbg = events_debug(&env);
    // Positive control: the legacy receipt hash IS emitted, so our detector works.
    assert!(
        dbg.contains(&hash_hex_run(0xAA)),
        "receipt hash should appear in events (positive control)"
    );
    // Guarantee: the zkEmail commitment is never emitted in any event.
    assert!(
        !dbg.contains(&hash_hex_run(0xBB)),
        "zkEmail commitment must not appear in any emitted event"
    );
    let _ = &receipt_hash;
    // But it is durably stored and retrievable.
    assert_eq!(w.client.get_payment_commitment(&pid), Some(commitment));
}

#[test]
fn test_bind_event_does_not_leak_commitment() {
    let env = Env::default();
    env.mock_all_auths();
    let w = setup(&env, 0);
    let event_id = symbol_short!("EVENT1");
    let payer = funded_payer(&w, 100_000_000);
    let pid = pay(&w, &payer, &event_id, 100_000_000);

    let commitment = BytesN::from_array(&env, &[0xCDu8; 32]);
    w.client.bind_email_commitment(&payer, &pid, &commitment);

    // The ReceiptCommitmentBound event carries only ids/timestamp, never the hash.
    assert!(!events_debug(&env).contains(&hash_hex_run(0xCD)));
}
