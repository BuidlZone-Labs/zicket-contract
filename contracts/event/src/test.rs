use crate::errors::EventError;
use crate::types::{
    CreateEventParams, EventStatus, PrivacyLevel, TicketTierParams, UpdateEventParams,
};
use crate::{EventContract, EventContractClient};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{token, Address, Env, String, Symbol};

// ============================================================
// Setup
// ============================================================

fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1704067200; // Jan 1, 2024 (Future relative to now, but static for tests)
    });
    env
}

const BASE_TIMESTAMP: u64 = 1704067200;

fn test_payout_token(env: &Env) -> Address {
    Address::generate(env)
}

// ============================================================
// Tests
// ============================================================

#[test]
fn test_create_event() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);

    let event = client.get_event(&event_id);
    assert_eq!(event.name, String::from_str(&env, "Tech Conference 2024"));
    assert_eq!(event.status, EventStatus::Upcoming);
    assert!(client.get_allow_anonymous(&event_id));
    assert!(!client.get_requires_verification(&event_id));
}

#[test]
fn test_create_event_duplicate_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = Symbol::new(&env, "event_01");
    let name = String::from_str(&env, "Tech Conference 2024");
    let description = String::from_str(&env, "A great conference");
    let venue = String::from_str(&env, "Convention Center");
    // Ensure date is > 24h in future
    let event_date = env.ledger().timestamp() + 86_401;
    let initial_tiers = soroban_sdk::vec![
        &env,
        TicketTierParams {
            name: String::from_str(&env, "General"),
            price: 100_000_000,
            capacity: 500,
        },
    ];

    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: test_payout_token(&env),
        event_id: event_id.clone(),
        name: name.clone(),
        description: description.clone(),
        venue: venue.clone(),
        event_date,
        initial_tiers: initial_tiers.clone(),
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
    };

    // First creation succeeds
    client.create_event(&params);

    // Second creation with same ID fails
    let params_dup = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: test_payout_token(&env),
        event_id: event_id.clone(),
        name: name.clone(), // doesn't matter
        description: description.clone(),
        venue: venue.clone(),
        event_date,
        initial_tiers,
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
    };
    let result = client.try_create_event(&params_dup);
    assert_eq!(result.err(), Some(Ok(EventError::EventAlreadyExists)));
}

#[test]
fn test_create_event_invalid_tickets_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: test_payout_token(&env),
        event_id: Symbol::new(&env, "event_bad"),
        name: String::from_str(&env, "Bad Event"),
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() + 90_000,
        initial_tiers: soroban_sdk::vec![
            &env,
            TicketTierParams {
                name: String::from_str(&env, "General"),
                price: 100,
                capacity: 0, // Invalid
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
    };

    let result = client.try_create_event(&params);
    assert_eq!(result.err(), Some(Ok(EventError::InvalidTicketCount)));
}

#[test]
fn test_create_event_too_many_tickets_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: test_payout_token(&env),
        event_id: Symbol::new(&env, "event_bad"),
        name: String::from_str(&env, "Bad Event"),
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() + 90_000,
        initial_tiers: soroban_sdk::vec![
            &env,
            TicketTierParams {
                name: String::from_str(&env, "General"),
                price: 100,
                capacity: 100_000, // Invalid limit
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
    };

    let result = client.try_create_event(&params);
    assert_eq!(result.err(), Some(Ok(EventError::InvalidTicketCount)));
}

#[test]
fn test_create_event_past_date_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: test_payout_token(&env),
        event_id: Symbol::new(&env, "event_bad"),
        name: String::from_str(&env, "Bad Event"),
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() - 100, // Past
        initial_tiers: soroban_sdk::vec![
            &env,
            TicketTierParams {
                name: String::from_str(&env, "General"),
                price: 100,
                capacity: 100,
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
    };

    let result = client.try_create_event(&params);
    assert_eq!(result.err(), Some(Ok(EventError::InvalidEventDate)));
}

#[test]
fn test_create_event_date_less_than_24h_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: test_payout_token(&env),
        event_id: Symbol::new(&env, "event_bad"),
        name: String::from_str(&env, "Bad Event"),
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() + 3600, // Only 1h
        initial_tiers: soroban_sdk::vec![
            &env,
            TicketTierParams {
                name: String::from_str(&env, "General"),
                price: 100,
                capacity: 100,
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
    };

    let result = client.try_create_event(&params);
    assert_eq!(result.err(), Some(Ok(EventError::InvalidEventDate)));
}

#[test]
fn test_create_event_negative_price_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: test_payout_token(&env),
        event_id: Symbol::new(&env, "event_bad"),
        name: String::from_str(&env, "Bad Event"),
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() + 90_000,
        initial_tiers: soroban_sdk::vec![
            &env,
            TicketTierParams {
                name: String::from_str(&env, "General"),
                price: -10, // Invalid
                capacity: 100,
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
    };

    let result = client.try_create_event(&params);
    assert_eq!(result.err(), Some(Ok(EventError::InvalidPrice)));
}

#[test]
fn test_create_event_empty_name_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: test_payout_token(&env),
        event_id: Symbol::new(&env, "event_bad"),
        name: String::from_str(&env, ""), // Empty
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() + 90_000,
        initial_tiers: soroban_sdk::vec![
            &env,
            TicketTierParams {
                name: String::from_str(&env, "General"),
                price: 100,
                capacity: 100,
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
    };

    let result = client.try_create_event(&params);
    assert_eq!(result.err(), Some(Ok(EventError::InvalidInput)));
}

#[test]
fn test_create_event_empty_venue_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: test_payout_token(&env),
        event_id: Symbol::new(&env, "event_bad"),
        name: String::from_str(&env, "Event"),
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, ""), // Empty
        event_date: env.ledger().timestamp() + 90_000,
        initial_tiers: soroban_sdk::vec![
            &env,
            TicketTierParams {
                name: String::from_str(&env, "General"),
                price: 100,
                capacity: 100,
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
    };

    let result = client.try_create_event(&params);
    assert_eq!(result.err(), Some(Ok(EventError::InvalidInput)));
}

#[test]
fn test_get_event_not_found() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);

    let result = client.try_get_event(&Symbol::new(&env, "non_existent"));
    assert_eq!(result.err(), Some(Ok(EventError::EventNotFound)));
}

#[test]
fn test_update_event_status_upcoming_to_active() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);

    // Transitions from Upcoming to Active
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    let status = client.get_event_status(&event_id);
    assert_eq!(status, EventStatus::Active);
}

#[test]
fn test_update_event_status_active_to_completed() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);

    client.update_event_status(&organizer, &event_id, &EventStatus::Active);
    client.update_event_status(&organizer, &event_id, &EventStatus::Completed);

    let status = client.get_event_status(&event_id);
    assert_eq!(status, EventStatus::Completed);
}

#[test]
fn test_invalid_status_transition_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);

    // Try Upcoming -> Completed (Skipping Active) -> Fail
    let result = client.try_update_event_status(&organizer, &event_id, &EventStatus::Completed);
    assert_eq!(result.err(), Some(Ok(EventError::InvalidStatusTransition)));
}

#[test]
fn test_cancel_event() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);

    client.cancel_event(&organizer, &event_id);

    let status = client.get_event_status(&event_id);
    assert_eq!(status, EventStatus::Cancelled);
}

#[test]
fn test_cancel_completed_event_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);

    client.update_event_status(&organizer, &event_id, &EventStatus::Active);
    client.update_event_status(&organizer, &event_id, &EventStatus::Completed);

    // Try cancel -> fail
    let result = client.try_cancel_event(&organizer, &event_id);
    assert_eq!(result.err(), Some(Ok(EventError::InvalidStatusTransition)));
}

#[test]
fn test_unauthorized_cancel() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attacker = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);

    let result = client.try_cancel_event(&attacker, &event_id);
    assert_eq!(result.err(), Some(Ok(EventError::Unauthorized)));
}

// ============================================================
// Update event details tests
// ============================================================

#[test]
fn test_update_event_details() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);

    // Update name and price
    let params = UpdateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: Some(String::from_str(&env, "Updated Conference")),
        description: None,
        venue: None,
        event_date: None,
        allow_anonymous: Some(false),
        requires_verification: Some(true),
    };

    client.update_event_details(&params);

    let event = client.get_event(&event_id);
    assert_eq!(event.name, String::from_str(&env, "Updated Conference"));
    assert!(!event.allow_anonymous);
    assert!(event.requires_verification);
    // Verify other fields remain unchanged
    assert_eq!(event.venue, String::from_str(&env, "Convention Center"));
    let mut capacity = 0;
    for tier in event.tiers.iter() {
        capacity += tier.capacity;
    }
    assert_eq!(capacity, 500);
}

#[test]
fn test_update_event_details_noop() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);
    let original_event = client.get_event(&event_id);

    // Update with all None
    let params = UpdateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: None,
        description: None,
        venue: None,
        event_date: None,
        allow_anonymous: None,
        requires_verification: None,
    };
    client.update_event_details(&params);

    let updated_event = client.get_event(&event_id);
    assert_eq!(original_event, updated_event);
}

#[test]
fn test_update_event_not_found() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let params = UpdateEventParams {
        organizer: organizer.clone(),
        event_id: Symbol::new(&env, "MISSING"),
        name: Some(String::from_str(&env, "New Name")),
        description: None,
        venue: None,
        event_date: None,
        allow_anonymous: None,
        requires_verification: None,
    };

    let result = client.try_update_event_details(&params);
    assert!(result.is_err());
}

#[test]
fn test_update_event_unauthorized() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attacker = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);

    // Attacker tries to update
    let params = UpdateEventParams {
        organizer: attacker.clone(),
        event_id: event_id.clone(),
        name: Some(String::from_str(&env, "Hacked Event")),
        description: None,
        venue: None,
        event_date: None,
        allow_anonymous: None,
        requires_verification: None,
    };

    let result = client.try_update_event_details(&params);
    assert!(result.is_err());
}

#[test]
fn test_update_active_event_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);

    // Activate event
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    // Try update details -> should fail
    let params = UpdateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: Some(String::from_str(&env, "Too Late")),
        description: None,
        venue: None,
        event_date: None,
        allow_anonymous: None,
        requires_verification: None,
    };

    let result = client.try_update_event_details(&params);
    // Expect EventNotUpdatable error
    assert!(result.is_err());
}

#[test]
fn test_update_cancelled_event_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);

    // Cancel event
    client.cancel_event(&organizer, &event_id);

    // Try update details -> should fail
    let params = UpdateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: Some(String::from_str(&env, "Too Late")),
        description: None,
        venue: None,
        event_date: None,
        allow_anonymous: None,
        requires_verification: None,
    };

    let result = client.try_update_event_details(&params);
    assert!(result.is_err());
}

#[test]
fn test_update_invalid_data() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);

    // Empty name
    let params_name = UpdateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: Some(String::from_str(&env, "")),
        description: None,
        venue: None,
        event_date: None,
        allow_anonymous: None,
        requires_verification: None,
    };
    let result = client.try_update_event_details(&params_name);
    assert!(result.is_err());

    // Past date
    let params_date = UpdateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: None,
        description: None,
        venue: None,
        event_date: Some(BASE_TIMESTAMP), // now/past
        allow_anonymous: None,
        requires_verification: None,
    };
    let result_date = client.try_update_event_details(&params_date);
    assert!(result_date.is_err());
}

// ============================================================
// Registration tests
// ============================================================

#[test]
fn test_register_for_event_happy_path() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);

    let (_payments_contract, token, token_admin) =
        setup_registration_contracts(&env, &client, &organizer);
    fund_attendee(&env, &token_admin, &token, &attendee, 100_000_000);

    let event_id = setup_event_with_payout_token(&env, &client, &organizer, &token);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    client.register_for_event(&attendee, &event_id, &0, &false);

    let event = client.get_event(&event_id);
    assert_eq!(event.tiers.get(0).unwrap().sold, 1);

    let registered = client.is_registered(&event_id, &attendee);
    assert!(registered);
}

#[test]
fn test_register_for_event_not_active_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);

    let (_payments_contract, token, token_admin) =
        setup_registration_contracts(&env, &client, &organizer);
    fund_attendee(&env, &token_admin, &token, &attendee, 100_000_000);

    let event_id = setup_event_with_payout_token(&env, &client, &organizer, &token);

    let result = client.try_register_for_event(&attendee, &event_id, &0, &false);
    assert_eq!(result.err(), Some(Ok(EventError::EventNotActive)));
}

#[test]
fn test_register_for_event_sold_out_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attendee1 = Address::generate(&env);
    let attendee2 = Address::generate(&env);

    let (_payments_contract, token, token_admin) =
        setup_registration_contracts(&env, &client, &organizer);
    fund_attendee(&env, &token_admin, &token, &attendee1, 100);
    fund_attendee(&env, &token_admin, &token, &attendee2, 100);

    let event_id = Symbol::new(&env, "event_02");
    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: token.clone(),
        event_id: event_id.clone(),
        name: String::from_str(&env, "One Ticket"),
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() + 86_401,
        initial_tiers: soroban_sdk::vec![
            &env,
            TicketTierParams {
                name: String::from_str(&env, "General"),
                price: 100,
                capacity: 1,
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
    };
    client.create_event(&params);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    client.register_for_event(&attendee1, &event_id, &0, &false);
    let result = client.try_register_for_event(&attendee2, &event_id, &0, &false);
    assert_eq!(result.err(), Some(Ok(EventError::TierSoldOut)));
}

#[test]
fn test_register_for_event_duplicate_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);

    let (_payments_contract, token, token_admin) =
        setup_registration_contracts(&env, &client, &organizer);
    fund_attendee(&env, &token_admin, &token, &attendee, 200_000_000);

    let event_id = setup_event_with_payout_token(&env, &client, &organizer, &token);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    client.register_for_event(&attendee, &event_id, &0, &false);
    let result = client.try_register_for_event(&attendee, &event_id, &0, &false);
    assert_eq!(result.err(), Some(Ok(EventError::AlreadyRegistered)));
}

#[test]
fn test_register_for_event_cancelled_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);

    let (_payments_contract, token, token_admin) =
        setup_registration_contracts(&env, &client, &organizer);
    fund_attendee(&env, &token_admin, &token, &attendee, 100_000_000);

    let event_id = setup_event_with_payout_token(&env, &client, &organizer, &token);
    client.cancel_event(&organizer, &event_id);

    let result = client.try_register_for_event(&attendee, &event_id, &0, &false);
    assert_eq!(result.err(), Some(Ok(EventError::EventNotActive)));
}

#[test]
fn test_get_attendees() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attendee1 = Address::generate(&env);
    let attendee2 = Address::generate(&env);

    let (_payments_contract, token, token_admin) =
        setup_registration_contracts(&env, &client, &organizer);
    fund_attendee(&env, &token_admin, &token, &attendee1, 100_000_000);
    fund_attendee(&env, &token_admin, &token, &attendee2, 100_000_000);

    let event_id = setup_event_with_payout_token(&env, &client, &organizer, &token);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    client.register_for_event(&attendee1, &event_id, &0, &false);
    client.register_for_event(&attendee2, &event_id, &0, &false);

    let attendees = client.get_attendees(&event_id);
    assert_eq!(attendees.len(), 2);
    assert_eq!(attendees.get(0).unwrap(), attendee1);
    assert_eq!(attendees.get(1).unwrap(), attendee2);
}

fn setup_event(env: &Env, client: &EventContractClient, organizer: &Address) -> Symbol {
    setup_event_with_payout_token(env, client, organizer, &test_payout_token(env))
}

fn setup_event_with_payout_token(
    env: &Env,
    client: &EventContractClient,
    organizer: &Address,
    payout_token: &Address,
) -> Symbol {
    let event_id = Symbol::new(env, "event_01");
    let name = String::from_str(env, "Tech Conference 2024");
    let description = String::from_str(env, "A great conference");
    let venue = String::from_str(env, "Convention Center");
    // Ensure date is > 24h in future
    let event_date = env.ledger().timestamp() + 86_401;
    let initial_tiers = soroban_sdk::vec![
        env,
        TicketTierParams {
            name: String::from_str(env, "General"),
            price: 100_000_000,
            capacity: 500,
        },
    ];

    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: payout_token.clone(),
        event_id: event_id.clone(),
        name,
        description,
        venue,
        event_date,
        initial_tiers,
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
    };

    client.create_event(&params);
    event_id
}

fn setup_registration_contracts(
    env: &Env,
    event_client: &EventContractClient,
    admin: &Address,
) -> (Address, Address, Address) {
    let ticket_contract_id = env.register(ticket_contract::TicketContract, ());
    let payments_contract_id = env.register(payments_contract::PaymentsContract, ());

    let token_admin = Address::generate(env);
    let token = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let payments_client =
        payments_contract::PaymentsContractClient::new(env, &payments_contract_id);
    payments_client.initialize(admin, &token, &event_client.address);

    event_client.initialize(admin, &ticket_contract_id, &payments_contract_id);

    (payments_contract_id, token, token_admin)
}

fn fund_attendee(
    env: &Env,
    token_admin: &Address,
    token: &Address,
    attendee: &Address,
    amount: i128,
) {
    let asset_admin = token::StellarAssetClient::new(env, token);
    let token_client = token::Client::new(env, token);
    asset_admin.mint(token_admin, &amount);
    token_client.transfer(token_admin, attendee, &amount);
}

// ============================================================
// Reservation Tests
// ============================================================

#[test]
fn test_reserve_ticket_success() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);

    let (_payments_contract, token, token_admin) =
        setup_registration_contracts(&env, &client, &organizer);
    fund_attendee(&env, &token_admin, &token, &attendee, 100_000_000);

    let event_id = setup_event_with_payout_token(&env, &client, &organizer, &token);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    client.reserve_ticket(&attendee, &event_id, &0);

    let event = client.get_event(&event_id);
    let tier = event.tiers.get(0).unwrap();
    assert_eq!(tier.reserved, 1);
    assert_eq!(tier.sold, 0);
}

#[test]
fn test_reserve_and_pay_success() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);

    let (_payments_contract, token, token_admin) =
        setup_registration_contracts(&env, &client, &organizer);
    fund_attendee(&env, &token_admin, &token, &attendee, 100_000_000);

    let event_id = setup_event_with_payout_token(&env, &client, &organizer, &token);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    // 1. Reserve
    client.reserve_ticket(&attendee, &event_id, &0);

    // 2. Pay
    client.register_for_event(&attendee, &event_id, &0, &false);

    let event = client.get_event(&event_id);
    let tier = event.tiers.get(0).unwrap();
    assert_eq!(tier.reserved, 0);
    assert_eq!(tier.sold, 1);

    let registered = client.is_registered(&event_id, &attendee);
    assert!(registered);
}

#[test]
fn test_reserve_expire_and_available_again() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);

    let event_id = Symbol::new(&env, "event_limit");
    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: test_payout_token(&env),
        event_id: event_id.clone(),
        name: String::from_str(&env, "Limit Event"),
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() + 86_401,
        initial_tiers: soroban_sdk::vec![
            &env,
            TicketTierParams {
                name: String::from_str(&env, "VIP"),
                price: 100,
                capacity: 1, // Only 1 spot
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
    };
    client.create_event(&params);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    // 1. Reserve
    client.reserve_ticket(&attendee, &event_id, &0);

    let event = client.get_event(&event_id);
    assert_eq!(event.tiers.get(0).unwrap().reserved, 1);

    // 2. Try to reserve again by another user -> should fail (Sold out/Reserved out)
    let attendee_2 = Address::generate(&env);
    let result = client.try_reserve_ticket(&attendee_2, &event_id, &0);
    assert_eq!(result.err(), Some(Ok(EventError::TierSoldOut)));

    // 3. Move time forward 16 minutes (beyond 15 min expiry)
    env.ledger().with_mut(|li| {
        li.timestamp += 1000;
    });

    // 4. Release expired
    client.release_expired_reservation(&event_id, &attendee);

    let event_after = client.get_event(&event_id);
    assert_eq!(event_after.tiers.get(0).unwrap().reserved, 0);

    // 5. Now attendee_2 can reserve
    client.reserve_ticket(&attendee_2, &event_id, &0);
    assert_eq!(
        client.get_event(&event_id).tiers.get(0).unwrap().reserved,
        1
    );
}

#[test]
fn test_pay_with_expired_reservation_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);

    let (_payments_contract, token, token_admin) =
        setup_registration_contracts(&env, &client, &organizer);
    fund_attendee(&env, &token_admin, &token, &attendee, 100_000_000);

    let event_id = setup_event_with_payout_token(&env, &client, &organizer, &token);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    // 1. Reserve
    client.reserve_ticket(&attendee, &event_id, &0);

    // 2. Move time forward
    env.ledger().with_mut(|li| {
        li.timestamp += 1000;
    });

    // 3. Try to pay -> should fail
    let result = client.try_register_for_event(&attendee, &event_id, &0, &false);
    assert_eq!(result.err(), Some(Ok(EventError::ReservationExpired)));
}

// ============================================================
// Issue #53: Privacy-Preserving Event Emissions Tests
// ============================================================

#[test]
fn test_privacy_default_is_standard() {
    use crate::types::PrivacyLevel;

    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_event(&env, &client, &organizer);

    let level = client.get_event_privacy(&event_id);
    assert_eq!(level, PrivacyLevel::Standard);
}

#[test]
fn test_set_privacy_level_standard() {
    use crate::types::PrivacyLevel;

    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_event(&env, &client, &organizer);

    client.set_event_privacy(&organizer, &event_id, &PrivacyLevel::Standard);
    assert_eq!(client.get_event_privacy(&event_id), PrivacyLevel::Standard);
}

#[test]
fn test_set_privacy_level_private() {
    use crate::types::PrivacyLevel;

    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_event(&env, &client, &organizer);

    client.set_event_privacy(&organizer, &event_id, &PrivacyLevel::Private);
    assert_eq!(client.get_event_privacy(&event_id), PrivacyLevel::Private);
}

#[test]
fn test_set_privacy_level_anonymous() {
    use crate::types::PrivacyLevel;

    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_event(&env, &client, &organizer);

    client.set_event_privacy(&organizer, &event_id, &PrivacyLevel::Anonymous);
    assert_eq!(client.get_event_privacy(&event_id), PrivacyLevel::Anonymous);
}

#[test]
fn test_set_privacy_unauthorized() {
    use crate::types::PrivacyLevel;

    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attacker = Address::generate(&env);
    let event_id = setup_event(&env, &client, &organizer);

    let result = client.try_set_event_privacy(&attacker, &event_id, &PrivacyLevel::Anonymous);
    assert_eq!(result.err(), Some(Ok(EventError::Unauthorized)));
}

#[test]
fn test_mask_address_standard_returns_full() {
    use privacy_utils::{mask_address, MaskedAddress, PrivacyLevel};

    let env = setup_env();
    let addr = Address::generate(&env);
    let result = mask_address(&env, &addr, PrivacyLevel::Standard);
    assert_eq!(result, MaskedAddress::Full(addr));
}

#[test]
fn test_mask_address_private_returns_partial() {
    use privacy_utils::{mask_address, MaskedAddress, PrivacyLevel};

    let env = setup_env();
    let addr = Address::generate(&env);
    let result = mask_address(&env, &addr, PrivacyLevel::Private);
    assert!(matches!(result, MaskedAddress::Partial(_)));
}

#[test]
fn test_mask_address_anonymous_returns_hashed() {
    use privacy_utils::{mask_address, MaskedAddress, PrivacyLevel};

    let env = setup_env();
    let addr = Address::generate(&env);
    let result = mask_address(&env, &addr, PrivacyLevel::Anonymous);
    assert!(matches!(result, MaskedAddress::Hashed(_)));
}
