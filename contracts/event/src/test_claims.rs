use crate::errors::EventError;
use crate::types::{ClaimSettings, CreateEventParams, EventStatus, PrivacyLevel, TicketTierParams};
use crate::{EventContract, EventContractClient};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{token, Address, Env, String, Symbol};

fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1704067200;
    });
    env
}

/// Registers all contracts, returns (payments_id, token_address, token_admin).
fn setup_contracts(
    env: &Env,
    event_client: &EventContractClient,
    admin: &Address,
    token: &Address,
) -> Address {
    let ticket_contract_id = env.register(ticket_contract::TicketContract, ());
    let payments_contract_id = env.register(payments_contract::PaymentsContract, ());

    let payments_client =
        payments_contract::PaymentsContractClient::new(env, &payments_contract_id);
    let platform_wallet = Address::generate(env);
    payments_client.initialize(admin, token, &0, &platform_wallet, &event_client.address);

    event_client.initialize(admin, &ticket_contract_id, &payments_contract_id);
    payments_contract_id
}

/// Creates a free event (price = 0) and activates it.
fn create_free_event(
    env: &Env,
    client: &EventContractClient,
    organizer: &Address,
    token: &Address,
    event_id: Symbol,
) {
    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: token.clone(),
        event_id: event_id.clone(),
        name: String::from_str(env, "Free Event"),
        description: String::from_str(env, "Free"),
        venue: String::from_str(env, "Venue"),
        event_date: env.ledger().timestamp() + 86_401,
        initial_tiers: soroban_sdk::vec![
            env,
            TicketTierParams {
                name: String::from_str(env, "Free"),
                price: 0,
                capacity: 100,
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 1000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(env),
    };
    client.create_event(&params);
    client.update_event_status(organizer, &event_id, &EventStatus::Active);
}

/// Creates a paid event and activates it.
fn create_paid_event(
    env: &Env,
    client: &EventContractClient,
    organizer: &Address,
    token: &Address,
    event_id: Symbol,
    price: i128,
) {
    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: token.clone(),
        event_id: event_id.clone(),
        name: String::from_str(env, "Paid Event"),
        description: String::from_str(env, "Paid"),
        venue: String::from_str(env, "Venue"),
        event_date: env.ledger().timestamp() + 86_401,
        initial_tiers: soroban_sdk::vec![
            env,
            TicketTierParams {
                name: String::from_str(env, "VIP"),
                price,
                capacity: 100,
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 1000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(env),
    };
    client.create_event(&params);
    client.update_event_status(organizer, &event_id, &EventStatus::Active);
}

// ── set_claim_settings ────────────────────────────────────────────────────────

#[test]
fn test_set_claim_settings_by_organizer() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "ev_cfg");

    setup_contracts(&env, &client, &organizer, &token);
    create_free_event(&env, &client, &organizer, &token, event_id.clone());

    client.set_claim_settings(&organizer, &event_id, &1, &3600);

    let s = client.get_claim_settings(&event_id);
    assert_eq!(s.max_free_claims, 1);
    assert_eq!(s.cooldown_secs, 3600);
}

#[test]
fn test_set_claim_settings_non_organizer_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let intruder = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "ev_cfgf");

    setup_contracts(&env, &client, &organizer, &token);
    create_free_event(&env, &client, &organizer, &token, event_id.clone());

    let result = client.try_set_claim_settings(&intruder, &event_id, &1, &0);
    assert_eq!(result.err(), Some(Ok(EventError::Unauthorized)));
}

#[test]
fn test_get_claim_settings_default_unlimited() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token = Address::generate(&env);
    let event_id = Symbol::new(&env, "ev_dflt");

    setup_contracts(&env, &client, &organizer, &token);
    create_free_event(&env, &client, &organizer, &token, event_id.clone());

    let s = client.get_claim_settings(&event_id);
    assert_eq!(
        s,
        ClaimSettings {
            max_free_claims: 0,
            cooldown_secs: 0
        }
    );
}

// ── Claim limit enforcement ───────────────────────────────────────────────────

#[test]
fn test_free_ticket_no_limit_succeeds() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let event_id = Symbol::new(&env, "ev_nolim");
    let attendee = Address::generate(&env);

    setup_contracts(&env, &client, &organizer, &token_address);
    create_free_event(&env, &client, &organizer, &token_address, event_id.clone());

    // No limits configured: claim should succeed
    client.register_for_event(&1, &attendee, &event_id, &0, &false, &None);
    assert!(client.is_registered(&event_id, &attendee));
}

#[test]
fn test_free_ticket_claim_limit_exceeded() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let event_id = Symbol::new(&env, "ev_lim");
    let attendee = Address::generate(&env);

    setup_contracts(&env, &client, &organizer, &token_address);
    create_free_event(&env, &client, &organizer, &token_address, event_id.clone());

    // Set limit to 1 free claim per wallet
    client.set_claim_settings(&organizer, &event_id, &1, &0);

    // First claim succeeds and increments count to 1
    client.register_for_event(&1, &attendee, &event_id, &0, &false, &None);

    // Same attendee — now count == max_free_claims == 1, so ClaimLimitExceeded fires
    // (sybil check runs before AlreadyRegistered)
    let result = client.try_register_for_event(&2, &attendee, &event_id, &0, &false, &None);
    assert_eq!(result.err(), Some(Ok(EventError::ClaimLimitExceeded)));
}

#[test]
fn test_free_ticket_different_wallets_independent() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let event_id = Symbol::new(&env, "ev_indep");
    let attendee_a = Address::generate(&env);
    let attendee_b = Address::generate(&env);

    setup_contracts(&env, &client, &organizer, &token_address);
    create_free_event(&env, &client, &organizer, &token_address, event_id.clone());
    client.set_claim_settings(&organizer, &event_id, &1, &0);

    // Both wallets can each claim once independently
    client.register_for_event(&1, &attendee_a, &event_id, &0, &false, &None);
    client.register_for_event(&2, &attendee_b, &event_id, &0, &false, &None);

    assert!(client.is_registered(&event_id, &attendee_a));
    assert!(client.is_registered(&event_id, &attendee_b));
}

// ── Cooldown enforcement ──────────────────────────────────────────────────────

#[test]
fn test_free_ticket_cooldown_active() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let event_id = Symbol::new(&env, "ev_cool");
    let attendee = Address::generate(&env);

    setup_contracts(&env, &client, &organizer, &token_address);
    create_free_event(&env, &client, &organizer, &token_address, event_id.clone());

    // Set cooldown to 3600 seconds, no claim limit (to avoid limit error masking cooldown error)
    client.set_claim_settings(&organizer, &event_id, &0, &3600);

    // First claim succeeds; last_claim timestamp is now recorded
    client.register_for_event(&1, &attendee, &event_id, &0, &false, &None);

    // Immediately re-attempt — cooldown not yet elapsed → ClaimCooldownActive
    // (sybil check runs before AlreadyRegistered)
    let result = client.try_register_for_event(&2, &attendee, &event_id, &0, &false, &None);
    assert_eq!(result.err(), Some(Ok(EventError::ClaimCooldownActive)));
}

#[test]
fn test_free_ticket_cooldown_elapsed_passes_sybil() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let event_id = Symbol::new(&env, "ev_exp");
    let attendee = Address::generate(&env);

    setup_contracts(&env, &client, &organizer, &token_address);
    create_free_event(&env, &client, &organizer, &token_address, event_id.clone());

    // 60 second cooldown, no claim limit
    client.set_claim_settings(&organizer, &event_id, &0, &60);

    // First claim at t=0
    client.register_for_event(&1, &attendee, &event_id, &0, &false, &None);

    // Advance time past cooldown
    env.ledger().with_mut(|li| {
        li.timestamp += 120; // 120s > 60s cooldown
    });

    // Second attempt: cooldown has elapsed, sybil check passes.
    // It should return AlreadyRegistered (not ClaimCooldownActive), confirming
    // the cooldown gate is no longer blocking.
    let result = client.try_register_for_event(&2, &attendee, &event_id, &0, &false, &None);
    assert_eq!(result.err(), Some(Ok(EventError::AlreadyRegistered)));
}

// ── Paid tickets bypass sybil checks ─────────────────────────────────────────

#[test]
fn test_paid_ticket_not_affected_by_claim_settings() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let event_id = Symbol::new(&env, "ev_paid");
    let attendee = Address::generate(&env);

    setup_contracts(&env, &client, &organizer, &token_address);

    let price = 50_000_000i128;
    create_paid_event(
        &env,
        &client,
        &organizer,
        &token_address,
        event_id.clone(),
        price,
    );

    // Extremely restrictive claim settings — should not affect paid tickets
    client.set_claim_settings(&organizer, &event_id, &1, &86400);

    // Fund attendee
    let token_asset_client = token::StellarAssetClient::new(&env, &token_address);
    let token_client = token::Client::new(&env, &token_address);
    token_asset_client.mint(&token_admin, &price);
    token_client.transfer(&token_admin, &attendee, &price);

    // Paid registration should succeed regardless of free-claim settings
    client.register_for_event(&1, &attendee, &event_id, &0, &false, &None);
    assert!(client.is_registered(&event_id, &attendee));
}
