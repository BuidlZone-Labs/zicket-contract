use crate::errors::EventError;
use crate::types::{
    AnonClaimSettings, CreateEventParams, EventStatus, PrivacyLevel, TicketTierParams,
};
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

    event_client.initialize(admin, &ticket_contract_id, &payments_contract_id);
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
    };
    client.create_event(&params);
    client.update_event_status(organizer, &event_id, &EventStatus::Active);
}

fn commitment(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
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
        event_start_ledger: 0,
        event_end_ledger: 10_000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(&env),
    };
    client.create_event(&params);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    let result = client.try_claim_anonymous_ticket(&event_id, &0, &commitment(&env, 1));
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
    };
    client.create_event(&params);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    let result = client.try_claim_anonymous_ticket(&event_id, &0, &commitment(&env, 1));
    assert_eq!(result.err(), Some(Ok(EventError::InvalidInput)));
}

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
    client.set_anon_claim_settings(&organizer, &event_id, &2, &100);

    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 1));
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 2));
    env.ledger().with_mut(|li| {
        li.sequence_number = 1_100;
    });
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 3));

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
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 1));
    env.ledger().with_mut(|li| li.sequence_number = 1_010);
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 2));

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
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 1));
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 2));
    let r3 = client.try_claim_anonymous_ticket(&event_id, &0, &commitment(&env, 3));
    let r4 = client.try_claim_anonymous_ticket(&event_id, &0, &commitment(&env, 4));
    let r5 = client.try_claim_anonymous_ticket(&event_id, &0, &commitment(&env, 5));

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
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 1));

    env.ledger().with_mut(|li| {
        li.sequence_number = 1_100;
    });
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 2));
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
    };
    client.create_event(&params);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);
    client.claim_anonymous_ticket(&event_id, &0, &commitment(&env, 1));
    let result = client.try_claim_anonymous_ticket(&event_id, &0, &commitment(&env, 2));
    assert_eq!(result.err(), Some(Ok(EventError::TierSoldOut)));
}
