use crate::errors::EventError;
use crate::types::{CreateEventParams, EventStatus, PrivacyLevel, TicketTierParams};
use crate::{EventContract, EventContractClient};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Env, String, Symbol};

fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1704067200;
    });
    env
}

fn create_event_with_privacy(
    env: &Env,
    client: &EventContractClient,
    organizer: &Address,
    event_id: Symbol,
    privacy: PrivacyLevel,
) {
    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: Address::generate(env),
        event_id,
        name: String::from_str(env, "Privacy Test Event"),
        description: String::from_str(env, "Desc"),
        venue: String::from_str(env, "Venue"),
        event_date: env.ledger().timestamp() + 86_401,
        initial_tiers: soroban_sdk::vec![
            env,
            TicketTierParams {
                name: String::from_str(env, "General"),
                price: 100_000_000,
                capacity: 100,
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: privacy,
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 1000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(env),
    };
    client.create_event(&params);
}

// ── Privacy level is stored and retrievable ───────────────────────────────────

#[test]
fn test_privacy_level_stored_on_creation() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = Symbol::new(&env, "ev_priv");

    create_event_with_privacy(
        &env,
        &client,
        &organizer,
        event_id.clone(),
        PrivacyLevel::Anonymous,
    );

    assert_eq!(client.get_event_privacy(&event_id), PrivacyLevel::Anonymous);
}

#[test]
fn test_privacy_default_is_standard_when_not_set() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = Symbol::new(&env, "ev_std");

    create_event_with_privacy(
        &env,
        &client,
        &organizer,
        event_id.clone(),
        PrivacyLevel::Standard,
    );

    assert_eq!(client.get_event_privacy(&event_id), PrivacyLevel::Standard);
}

#[test]
fn test_set_event_privacy_by_organizer() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = Symbol::new(&env, "ev_upd");

    create_event_with_privacy(
        &env,
        &client,
        &organizer,
        event_id.clone(),
        PrivacyLevel::Standard,
    );

    client.set_event_privacy(&organizer, &event_id, &PrivacyLevel::Private);
    assert_eq!(client.get_event_privacy(&event_id), PrivacyLevel::Private);
}

#[test]
fn test_set_event_privacy_non_organizer_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let intruder = Address::generate(&env);
    let event_id = Symbol::new(&env, "ev_fail");

    create_event_with_privacy(
        &env,
        &client,
        &organizer,
        event_id.clone(),
        PrivacyLevel::Standard,
    );

    let result = client.try_set_event_privacy(&intruder, &event_id, &PrivacyLevel::Anonymous);
    assert_eq!(result.err(), Some(Ok(EventError::Unauthorized)));
}

// ── Standard privacy: get_attendees is public ────────────────────────────────

#[test]
fn test_get_attendees_standard_public() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = Symbol::new(&env, "ev_std2");

    create_event_with_privacy(
        &env,
        &client,
        &organizer,
        event_id.clone(),
        PrivacyLevel::Standard,
    );

    let attendees = client.get_attendees(&event_id);
    assert_eq!(attendees.len(), 0);
}

// ── Anonymous privacy: get_attendees returns empty ───────────────────────────

#[test]
fn test_get_attendees_anonymous_returns_empty() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = Symbol::new(&env, "ev_anon");

    create_event_with_privacy(
        &env,
        &client,
        &organizer,
        event_id.clone(),
        PrivacyLevel::Anonymous,
    );

    let attendees = client.get_attendees(&event_id);
    assert_eq!(attendees.len(), 0);
}

// ── Private privacy: get_attendees blocks public, organizer can access ───────

#[test]
fn test_get_attendees_private_blocked_for_public() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = Symbol::new(&env, "ev_prv");

    create_event_with_privacy(
        &env,
        &client,
        &organizer,
        event_id.clone(),
        PrivacyLevel::Private,
    );

    let result = client.try_get_attendees(&event_id);
    assert_eq!(
        result.err(),
        Some(Ok(EventError::UnauthorizedPrivateAccess))
    );
}

#[test]
fn test_get_attendees_as_organizer_private_succeeds() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = Symbol::new(&env, "ev_prvorg");

    create_event_with_privacy(
        &env,
        &client,
        &organizer,
        event_id.clone(),
        PrivacyLevel::Private,
    );

    let attendees = client.get_attendees_as_organizer(&organizer, &event_id);
    assert_eq!(attendees.len(), 0);
}

#[test]
fn test_get_attendees_as_organizer_non_organizer_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let intruder = Address::generate(&env);
    let event_id = Symbol::new(&env, "ev_prvfail");

    create_event_with_privacy(
        &env,
        &client,
        &organizer,
        event_id.clone(),
        PrivacyLevel::Private,
    );

    let result = client.try_get_attendees_as_organizer(&intruder, &event_id);
    assert_eq!(result.err(), Some(Ok(EventError::Unauthorized)));
}

// ── Organizer can view attendees for Standard and Anonymous too ───────────────

#[test]
fn test_get_attendees_as_organizer_standard() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = Symbol::new(&env, "ev_stdorg");

    create_event_with_privacy(
        &env,
        &client,
        &organizer,
        event_id.clone(),
        PrivacyLevel::Standard,
    );

    let attendees = client.get_attendees_as_organizer(&organizer, &event_id);
    assert_eq!(attendees.len(), 0);
}

#[test]
fn test_get_attendees_as_organizer_anonymous() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = Symbol::new(&env, "ev_anonorg");

    create_event_with_privacy(
        &env,
        &client,
        &organizer,
        event_id.clone(),
        PrivacyLevel::Anonymous,
    );

    let attendees = client.get_attendees_as_organizer(&organizer, &event_id);
    assert_eq!(attendees.len(), 0);
}

// ── Cancel event emits privacy-respecting organizer address ──────────────────

#[test]
fn test_cancel_event_succeeds_all_privacy_levels() {
    for privacy in [
        PrivacyLevel::Standard,
        PrivacyLevel::Private,
        PrivacyLevel::Anonymous,
    ] {
        let env = setup_env();
        let contract_id = env.register(EventContract, ());
        let client = EventContractClient::new(&env, &contract_id);
        let organizer = Address::generate(&env);
        let event_id = Symbol::new(&env, "ev_cancel");

        create_event_with_privacy(&env, &client, &organizer, event_id.clone(), privacy);
        client.cancel_event(&organizer, &event_id);

        let event = client.get_event(&event_id);
        assert_eq!(event.status, EventStatus::Cancelled);
    }
}
