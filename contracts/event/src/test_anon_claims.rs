use crate::errors::EventError;
use crate::types::{AnonClaimSettings, CreateEventParams, EventStatus, PrivacyLevel, TicketTierParams};
use crate::{EventContract, EventContractClient};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, BytesN, Env, String, Symbol};

fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1_704_067_200;
        li.sequence_number = 1_000;
    });
    env
}

fn setup_contracts(env: &Env, event_client: &EventContractClient, admin: &Address, token: &Address) {
    let ticket_contract_id = env.register(ticket_contract::TicketContract, ());
    let payments_contract_id = env.register(payments_contract::PaymentsContract, ());

    let payments_client =
        payments_contract::PaymentsContractClient::new(env, &payments_contract_id);
    let platform_wallet = Address::generate(env);
    payments_client.initialize(admin, token, &0, &platform_wallet, &event_client.address);

    event_client.initialize(admin, &ticket_contract_id, &payments_contract_id);
}

/// Creates a free, anonymous-enabled event with a given capacity and activates it.
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
    };
    client.create_event(&params);
    client.update_event_status(organizer, &event_id, &EventStatus::Active);
}

fn commitment(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

// ── set_anon_claim_settings ───────────────────────────────────────────────────

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

// ── Basic anonymous claim ─────────────────────────────────────────────────────

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

    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 1));

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
    };
    client.create_event(&params);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    let result = client.try_claim_anonymous_ticket(&event_id, &0, &commitment(&env, 1));
    assert_eq!(result.err(), Some(Ok(EventError::AnonymousClaimsNotEnabled)));
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
    };
    client.create_event(&params);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    let result = client.try_claim_anonymous_ticket(&event_id, &0, &commitment(&env, 1));
    assert_eq!(result.err(), Some(Ok(EventError::InvalidInput)));
}

// ── Commitment uniqueness (duplicate rejection) ───────────────────────────────

#[test]
fn test_anon_commitment_reused_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_dup");

    setup_contracts(&env, &client, &organizer, &token);
    create_anon_free_event(&env, &client, &organizer, &token, event_id.clone(), 50);

    let c = commitment(&env, 42);

    client.claim_anonymous_ticket(&event_id, &0, &c);

    let result = client.try_claim_anonymous_ticket(&event_id, &0, &c);
    assert_eq!(result.err(), Some(Ok(EventError::AnonCommitmentReused)));
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
        client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, i));
    }

    let event = client.get_event(&event_id);
    assert_eq!(event.sold_count, 5);
}

// ── Per-ledger-window rate limit ──────────────────────────────────────────────

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

    // Allow 2 anonymous claims per 100-ledger window.
    client.set_anon_claim_settings(&organizer, &event_id, &2, &100);

    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 1));
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 2));

    let result = client.try_claim_anonymous_ticket(&event_id, &0, &commitment(&env, 3));
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

    // 2 per 100-ledger window; initial sequence = 1_000 → window 10.
    client.set_anon_claim_settings(&organizer, &event_id, &2, &100);

    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 1));
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 2));

    // Window 10 is full. Advance to window 11.
    env.ledger().with_mut(|li| {
        li.sequence_number = 1_100;
    });

    // New window → count resets; claim succeeds.
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 3));

    let event = client.get_event(&event_id);
    assert_eq!(event.sold_count, 3);
}

// ── The key sybil-resistance test ────────────────────────────────────────────

/// Demonstrates that a single source cannot drain event capacity in one transaction
/// batch. Even with five distinct commitments (no reuse), the window rate limit
/// blocks all claims beyond the per-window maximum.
#[test]
fn test_single_source_cannot_drain_capacity_in_one_batch() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "anon_drain");

    setup_contracts(&env, &client, &organizer, &token);

    // Small capacity so the exploit would be devastating if allowed.
    let total_capacity = 5u32;
    create_anon_free_event(
        &env,
        &client,
        &organizer,
        &token,
        event_id.clone(),
        total_capacity,
    );

    // Rate-limit: max 2 claims per 100-ledger window.
    // All five claims happen at sequence 1_000 (same window → window index = 10).
    client.set_anon_claim_settings(&organizer, &event_id, &2, &100);

    // Claims 1 and 2 succeed (within window quota).
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 1));
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 2));

    // Claims 3–5 are blocked by the window rate limit, even though:
    //   • each uses a distinct commitment (no reuse detected), and
    //   • the tier still has 3 remaining slots.
    let r3 = client.try_claim_anonymous_ticket(&event_id, &0, &commitment(&env, 3));
    let r4 = client.try_claim_anonymous_ticket(&event_id, &0, &commitment(&env, 4));
    let r5 = client.try_claim_anonymous_ticket(&event_id, &0, &commitment(&env, 5));

    assert_eq!(r3.err(), Some(Ok(EventError::AnonClaimWindowFull)));
    assert_eq!(r4.err(), Some(Ok(EventError::AnonClaimWindowFull)));
    assert_eq!(r5.err(), Some(Ok(EventError::AnonClaimWindowFull)));

    // Only 2 out of 5 tickets were issued; capacity was not drained.
    let event = client.get_event(&event_id);
    assert_eq!(event.sold_count, 2);
    assert_eq!(event.max_supply, total_capacity);
    assert_eq!(event.max_supply - event.sold_count, 3); // 3 slots still available
}

// ── Capacity enforcement ──────────────────────────────────────────────────────

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

    // No rate limit — fill the event across two windows.
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 1));

    env.ledger().with_mut(|li| {
        li.sequence_number = 1_100;
    });
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 2));

    // Event is now at max_supply; next claim must fail.
    env.ledger().with_mut(|li| {
        li.sequence_number = 1_200;
    });
    let result = client.try_claim_anonymous_ticket(&event_id, &0, &commitment(&env, 3));
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

    // Two tiers, each with capacity 1. Total event capacity = 2, so the
    // event-level check won't fire when only tier 0 is exhausted.
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
    };
    client.create_event(&params);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    // Fill tier 0.
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 1));

    // Tier 0 is sold out; event still has capacity in tier 1.
    let result = client.try_claim_anonymous_ticket(&event_id, &0, &commitment(&env, 2));
    assert_eq!(result.err(), Some(Ok(EventError::TierSoldOut)));
}
