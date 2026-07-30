use crate::errors::EventError;
use crate::types::{
    AnonClaimSettings, AnonymousTicketClaim, CreateEventParams, EventStatus, PrivacyLevel,
    TicketTierParams,
};
use crate::{DataKey, EventContract, EventContractClient, MAX_ANONYMOUS_PROOF_TTL_LEDGERS};
use soroban_sdk::testutils::{storage::Persistent as _, Address as _, Ledger};
use soroban_sdk::{contract, contractimpl, Address, Bytes, BytesN, Env, String, Symbol};

#[contract]
struct MockAnonymousClaimVerifier;

#[contractimpl]
impl MockAnonymousClaimVerifier {
    pub fn verify(_env: Env, proof: Bytes, public_inputs: Bytes) -> bool {
        !proof.is_empty() && public_inputs.len() == 160
    }
}

fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1_704_067_200;
        li.sequence_number = 1_000;
    });
    env
}

fn setup_contracts(
    env: &Env,
    event_client: &EventContractClient,
    admin: &Address,
    token: &Address,
) {
    let ticket_contract_id = env.register(ticket_contract::TicketContract, ());
    let payments_contract_id = env.register(payments_contract::PaymentsContract, ());

    let payments_client =
        payments_contract::PaymentsContractClient::new(env, &payments_contract_id);
    let platform_wallet = Address::generate(env);
    payments_client.initialize(admin, token, &0, &platform_wallet, &event_client.address);

    let ticket_client = ticket_contract::TicketContractClient::new(env, &ticket_contract_id);
    ticket_client.initialize(admin, &payments_contract_id);

    event_client.initialize(admin, &ticket_contract_id, &payments_contract_id);
    let verifier = env.register(MockAnonymousClaimVerifier, ());
    event_client.set_anonymous_claim_verifier(admin, &verifier);
}
fn create_anon_free_event(
    env: &Env,
    client: &EventContractClient,
    organizer: &Address,
    token: &Address,
    event_id: Symbol,
    capacity: u32,
) {
    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: token.clone(),
        event_id: event_id.clone(),
        name: String::from_str(env, "Anon Free Event"),
        description: String::from_str(env, ""),
        venue: String::from_str(env, "Venue"),
        event_date: env.ledger().timestamp() + 86_401,
        initial_tiers: soroban_sdk::vec![
            env,
            TicketTierParams {
                name: String::from_str(env, "Free"),
                price: 0,
                capacity,
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Anonymous,
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 10_000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(env),
        resale_royalty_bps: 0,
        max_resale_price: None,
        allow_free_ticket_transfer: false,
    };
    client.create_event(&params);
    client.update_event_status(organizer, &event_id, &EventStatus::Active);
}

fn claim(env: &Env, byte: u8) -> AnonymousTicketClaim {
    AnonymousTicketClaim {
        proof: Bytes::from_slice(env, &[byte]),
        nullifier: BytesN::from_array(env, &[byte; 32]),
        ticket_commitment: BytesN::from_array(env, &[byte.wrapping_add(64); 32]),
        expiry_ledger: env
            .ledger()
            .sequence()
            .saturating_add(MAX_ANONYMOUS_PROOF_TTL_LEDGERS),
    }
}

fn fixture_u32(public_inputs: &[u8], offset: usize) -> u32 {
    assert!(public_inputs[offset..offset + 28]
        .iter()
        .all(|byte| *byte == 0));
    u32::from_be_bytes(public_inputs[offset + 28..offset + 32].try_into().unwrap())
}

fn real_claim(env: &Env) -> (u32, AnonymousTicketClaim) {
    let public_inputs = include_bytes!("../../anon-claim-verifier/fixtures/public_inputs");
    (
        fixture_u32(public_inputs, 32),
        AnonymousTicketClaim {
            proof: Bytes::from_slice(
                env,
                include_bytes!("../../anon-claim-verifier/fixtures/proof"),
            ),
            nullifier: BytesN::from_array(env, public_inputs[96..128].try_into().unwrap()),
            ticket_commitment: BytesN::from_array(env, public_inputs[128..160].try_into().unwrap()),
            expiry_ledger: fixture_u32(public_inputs, 64),
        },
    )
}

#[test]
fn test_set_anon_claim_settings_by_organizer() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_cfg");

    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 50);

    client.set_anon_claim_settings(&organizer, &event_id, &10, &100);

    let s = client.get_anon_claim_settings(&event_id);
    assert_eq!(s.max_anon_claims_per_window, 10);
    assert_eq!(s.anon_window_size, 100);
}

#[test]
fn test_set_anon_claim_settings_non_organizer_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let intruder = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_cfgf");

    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 50);

    let result = client.try_set_anon_claim_settings(&intruder, &event_id, &10, &100);
    assert_eq!(result.err(), Some(Ok(EventError::Unauthorized)));
}

#[test]
fn test_anon_claim_settings_default_unlimited() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_dflt");

    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 50);

    let s = client.get_anon_claim_settings(&event_id);
    assert_eq!(
        s,
        AnonClaimSettings {
            max_anon_claims_per_window: 0,
            anon_window_size: 0,
        }
    );
}

#[test]
fn test_anonymous_claim_verifier_is_admin_set_and_immutable() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let intruder = Address::generate(&env);
    client.initialize(&admin, &Address::generate(&env), &Address::generate(&env));
    let verifier = env.register(MockAnonymousClaimVerifier, ());
    let replacement = env.register(MockAnonymousClaimVerifier, ());

    let unauthorized = client.try_set_anonymous_claim_verifier(&intruder, &verifier);
    assert_eq!(unauthorized.err(), Some(Ok(EventError::Unauthorized)));

    client.set_anonymous_claim_verifier(&admin, &verifier);
    let replace = client.try_set_anonymous_claim_verifier(&admin, &replacement);
    assert_eq!(
        replace.err(),
        Some(Ok(EventError::AnonymousClaimVerifierAlreadyConfigured))
    );
    assert_eq!(client.get_anonymous_claim_verifier(), verifier);
}

#[test]
fn test_anonymous_claim_verifier_ttl_renews_on_read() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &Address::generate(&env), &Address::generate(&env));
    let verifier = env.register(MockAnonymousClaimVerifier, ());
    client.set_anonymous_claim_verifier(&admin, &verifier);

    let key = DataKey::AnonymousClaimVerifier;
    let initial_ttl = env.as_contract(&contract_id, || env.storage().persistent().get_ttl(&key));
    env.ledger()
        .with_mut(|li| li.sequence_number += initial_ttl / 2 + 1);
    let before_read = env.as_contract(&contract_id, || env.storage().persistent().get_ttl(&key));

    assert_eq!(client.get_anonymous_claim_verifier(), verifier);

    let after_read = env.as_contract(&contract_id, || env.storage().persistent().get_ttl(&key));
    assert!(after_read > before_read);
}

#[test]
fn test_anon_claim_basic_success() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_ok");
    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 10);
    client.claim_anonymous_ticket(&event_id, &0, &claim(&env, 1));

    let event = client.get_event(&event_id);
    assert_eq!(event.sold_count, 1);
}

#[test]
fn test_anon_claim_rejects_non_anonymous_event() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_dis");
    setup_contracts(&env, &client, &organizer, &token);

    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: token.clone(),
        event_id: event_id.clone(),
        name: String::from_str(&env, "Non-Anon Event"),
        description: String::from_str(&env, ""),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() + 86_401,
        initial_tiers: soroban_sdk::vec![
            &env,
            TicketTierParams {
                name: String::from_str(&env, "Free"),
                price: 0,
                capacity: 10,
            },
        ],
        allow_anonymous: false,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 10_000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(&env),
        resale_royalty_bps: 0,
        max_resale_price: None,
        allow_free_ticket_transfer: false,
    };
    client.create_event(&params);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    let result = client.try_claim_anonymous_ticket(&event_id, &0, &claim(&env, 1));
    assert_eq!(
        result.err(),
        Some(Ok(EventError::AnonymousClaimsNotEnabled))
    );
}

#[test]
fn test_anon_claim_paid_tier_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_paid");
    setup_contracts(&env, &client, &organizer, &token);

    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: token.clone(),
        event_id: event_id.clone(),
        name: String::from_str(&env, "Paid Anon Event"),
        description: String::from_str(&env, ""),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() + 86_401,
        initial_tiers: soroban_sdk::vec![
            &env,
            TicketTierParams {
                name: String::from_str(&env, "VIP"),
                price: 100,
                capacity: 10,
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Anonymous,
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 10_000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(&env),
        resale_royalty_bps: 0,
        max_resale_price: None,
        allow_free_ticket_transfer: false,
    };
    client.create_event(&params);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    let result = client.try_claim_anonymous_ticket(&event_id, &0, &claim(&env, 1));
    assert_eq!(result.err(), Some(Ok(EventError::InvalidInput)));
}

#[test]
fn test_exact_proof_replay_is_idempotent() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_dup");
    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 50);

    let claim = claim(&env, 42);
    client.claim_anonymous_ticket(&event_id, &0, &claim);
    env.ledger().with_mut(|li| li.sequence_number = 20_001);
    client.claim_anonymous_ticket(&event_id, &0, &claim);

    assert_eq!(client.get_event(&event_id).sold_count, 1);
}

#[test]
fn test_expired_anonymous_proof_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_exp");

    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 50);

    let mut expired = claim(&env, 1);
    expired.expiry_ledger = 999;
    let result = client.try_claim_anonymous_ticket(&event_id, &0, &expired);

    assert_eq!(result.err(), Some(Ok(EventError::AnonymousProofExpired)));
    assert_eq!(client.get_event(&event_id).sold_count, 0);
}

#[test]
fn test_anonymous_proof_expiry_horizon_boundaries() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_ttl");

    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 50);

    client.claim_anonymous_ticket(&event_id, &0, &claim(&env, 1));

    let mut too_far = claim(&env, 2);
    too_far.expiry_ledger = too_far.expiry_ledger.saturating_add(1);
    let result = client.try_claim_anonymous_ticket(&event_id, &0, &too_far);

    assert_eq!(
        result.err(),
        Some(Ok(EventError::AnonymousProofExpiryTooFar))
    );
    assert_eq!(client.get_event(&event_id).sold_count, 1);
}

#[test]
fn test_same_nullifier_with_different_commitment_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_null");

    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 50);

    let original = claim(&env, 42);
    client.claim_anonymous_ticket(&event_id, &0, &original);
    let mut changed = original.clone();
    changed.ticket_commitment = BytesN::from_array(&env, &[99; 32]);

    let result = client.try_claim_anonymous_ticket(&event_id, &0, &changed);
    assert_eq!(result.err(), Some(Ok(EventError::AnonymousNullifierReused)));
    assert_eq!(client.get_event(&event_id).sold_count, 1);
}

#[test]
fn test_invalid_anonymous_proof_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_badpf");

    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 50);

    let mut invalid = claim(&env, 1);
    invalid.proof = Bytes::new(&env);
    let result = client.try_claim_anonymous_ticket(&event_id, &0, &invalid);

    assert_eq!(result.err(), Some(Ok(EventError::AnonymousProofInvalid)));
    assert_eq!(client.get_event(&event_id).sold_count, 0);
}

#[test]
fn test_real_ultrahonk_proof_claims_ticket_end_to_end() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_ok");

    let ticket_contract_id = env.register(ticket_contract::TicketContract, ());
    let payments_contract_id = env.register(payments_contract::PaymentsContract, ());
    let payments_client =
        payments_contract::PaymentsContractClient::new(&env, &payments_contract_id);
    let platform_wallet = Address::generate(&env);
    payments_client.initialize(&organizer, &token, &0, &platform_wallet, &client.address);
    client.initialize(&organizer, &ticket_contract_id, &payments_contract_id);
    let verifier = env.register(anon_claim_verifier::AnonymousClaimVerifier, ());
    client.set_anonymous_claim_verifier(&organizer, &verifier);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 10);

    let (tier_id, claim) = real_claim(&env);
    let expected_scope = include_bytes!("../../anon-claim-verifier/fixtures/public_inputs");
    assert_eq!(
        client.get_anonymous_claim_scope(&event_id).to_array(),
        expected_scope[..32]
    );
    let wrong_event_id = Symbol::new(&env, "anon_no");
    create_anon_free_event(
        &env,
        &client,
        &organizer,
        &token,
        wrong_event_id.clone(),
        10,
    );
    let wrong_event_result = client.try_claim_anonymous_ticket(&wrong_event_id, &tier_id, &claim);
    assert_eq!(
        wrong_event_result.err(),
        Some(Ok(EventError::AnonymousProofInvalid))
    );
    assert_eq!(client.get_event(&wrong_event_id).sold_count, 0);

    client.claim_anonymous_ticket(&event_id, &tier_id, &claim);

    assert_eq!(client.get_event(&event_id).sold_count, 1);
    assert_eq!(
        client.get_anonymous_ticket_commitment(&event_id, &claim.nullifier),
        Some(claim.ticket_commitment)
    );
}

#[test]
fn test_distinct_commitments_each_accepted_once() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_dist");
    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 50);

    for i in 1u8..=5 {
        client.claim_anonymous_ticket(&event_id, &0, &claim(&env, i));
    }

    let event = client.get_event(&event_id);
    assert_eq!(event.sold_count, 5);
}

#[test]
fn test_anon_window_rate_limit_blocks_excess_claims() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_wlim");
    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 50);
    client.set_anon_claim_settings(&organizer, &event_id, &2, &100);

    client.claim_anonymous_ticket(&event_id, &0, &claim(&env, 1));
    client.claim_anonymous_ticket(&event_id, &0, &claim(&env, 2));

    let result = client.try_claim_anonymous_ticket(&event_id, &0, &claim(&env, 3));
    assert_eq!(result.err(), Some(Ok(EventError::AnonClaimWindowFull)));
}

#[test]
fn test_anon_window_resets_after_ledger_advance() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_wrst");
    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 50);
    client.set_anon_claim_settings(&organizer, &event_id, &2, &100);

    client.claim_anonymous_ticket(&event_id, &0, &claim(&env, 1));
    client.claim_anonymous_ticket(&event_id, &0, &claim(&env, 2));
    env.ledger().with_mut(|li| {
        li.sequence_number = 1_100;
    });
    client.claim_anonymous_ticket(&event_id, &0, &claim(&env, 3));

    let event = client.get_event(&event_id);
    assert_eq!(event.sold_count, 3);
}
#[test]
fn test_anon_window_straddle_boundary() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_strd");
    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 50);
    client.set_anon_claim_settings(&organizer, &event_id, &1, &10);
    env.ledger().with_mut(|li| li.sequence_number = 1_009);
    client.claim_anonymous_ticket(&event_id, &0, &claim(&env, 1));
    env.ledger().with_mut(|li| li.sequence_number = 1_010);
    client.claim_anonymous_ticket(&event_id, &0, &claim(&env, 2));

    let event = client.get_event(&event_id);
    assert_eq!(event.sold_count, 2);
}
#[test]
fn test_single_source_rate_limited_per_window() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_drain");
    setup_contracts(&env, &client, &organizer, &token);
    let total_capacity = 5u32;
    create_anon_free_event(
        &env,
        &client,
        &organizer,
        &token,
        event_id.clone(),
        total_capacity,
    );
    client.set_anon_claim_settings(&organizer, &event_id, &2, &100);
    client.claim_anonymous_ticket(&event_id, &0, &claim(&env, 1));
    client.claim_anonymous_ticket(&event_id, &0, &claim(&env, 2));
    let r3 = client.try_claim_anonymous_ticket(&event_id, &0, &claim(&env, 3));
    let r4 = client.try_claim_anonymous_ticket(&event_id, &0, &claim(&env, 4));
    let r5 = client.try_claim_anonymous_ticket(&event_id, &0, &claim(&env, 5));

    assert_eq!(r3.err(), Some(Ok(EventError::AnonClaimWindowFull)));
    assert_eq!(r4.err(), Some(Ok(EventError::AnonClaimWindowFull)));
    assert_eq!(r5.err(), Some(Ok(EventError::AnonClaimWindowFull)));
    let event = client.get_event(&event_id);
    assert_eq!(event.sold_count, 2);
    assert_eq!(event.max_supply, total_capacity);
    assert_eq!(event.max_supply - event.sold_count, 3);
}

#[test]
fn test_anon_claim_event_sold_out() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_sol");
    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 2);
    client.claim_anonymous_ticket(&event_id, &0, &claim(&env, 1));

    env.ledger().with_mut(|li| {
        li.sequence_number = 1_100;
    });
    client.claim_anonymous_ticket(&event_id, &0, &claim(&env, 2));
    env.ledger().with_mut(|li| {
        li.sequence_number = 1_200;
    });
    let result = client.try_claim_anonymous_ticket(&event_id, &0, &claim(&env, 3));
    assert_eq!(result.err(), Some(Ok(EventError::EventSoldOut)));
}

#[test]
fn test_anon_claim_tier_sold_out() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_tso");
    setup_contracts(&env, &client, &organizer, &token);
    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: token.clone(),
        event_id: event_id.clone(),
        name: String::from_str(&env, "Two-Tier Anon Event"),
        description: String::from_str(&env, ""),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() + 86_401,
        initial_tiers: soroban_sdk::vec![
            &env,
            TicketTierParams {
                name: String::from_str(&env, "Tier0"),
                price: 0,
                capacity: 1,
            },
            TicketTierParams {
                name: String::from_str(&env, "Tier1"),
                price: 0,
                capacity: 1,
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Anonymous,
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 10_000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(&env),
        resale_royalty_bps: 0,
        max_resale_price: None,
        allow_free_ticket_transfer: false,
    };
    client.create_event(&params);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);
    client.claim_anonymous_ticket(&event_id, &0, &claim(&env, 1));
    let result = client.try_claim_anonymous_ticket(&event_id, &0, &claim(&env, 2));
    assert_eq!(result.err(), Some(Ok(EventError::TierSoldOut)));
}

#[test]
fn test_front_running_commitment_theft_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_front");
    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 10);

    let copied_claim = claim(&env, 7);
    client.claim_anonymous_ticket(&event_id, &0, &copied_claim);
    client.claim_anonymous_ticket(&event_id, &0, &copied_claim);

    let event = client.get_event(&event_id);
    assert_eq!(event.sold_count, 1);
    assert_eq!(
        client.get_anonymous_ticket_commitment(&event_id, &copied_claim.nullifier),
        Some(copied_claim.ticket_commitment)
    );
}

#[test]
fn test_anonymous_claim_requires_no_address_auth() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_noauth");
    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 10);

    env.set_auths(&[]);
    client.claim_anonymous_ticket(&event_id, &0, &claim(&env, 1));
    assert_eq!(client.get_event(&event_id).sold_count, 1);
}
