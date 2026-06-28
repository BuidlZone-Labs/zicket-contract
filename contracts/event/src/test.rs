use crate::errors::EventError;
use crate::types::{
    CreateEventParams, EventStatus, PrivacyLevel, TicketTierParams, UpdateEventParams,
};
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

const BASE_TIMESTAMP: u64 = 1704067200;

fn test_payout_token(env: &Env) -> Address {
    Address::generate(env)
}

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
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 1000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(&env),
        resale_royalty_bps: 0,
        max_resale_price: None,
        allow_free_ticket_transfer: false,
    };

    // First creation succeeds
    client.create_event(&params);
    let params_dup = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: test_payout_token(&env),
        event_id: event_id.clone(),
        name: name.clone(),
        description: description.clone(),
        venue: venue.clone(),
        event_date,
        initial_tiers,
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 1000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(&env),
        resale_royalty_bps: 0,
        max_resale_price: None,
        allow_free_ticket_transfer: false,
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
                capacity: 0,
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 1000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(&env),
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
                capacity: 100_000,
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 1000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(&env),
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
        event_date: env.ledger().timestamp() - 100,
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
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 1000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(&env),
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
        event_date: env.ledger().timestamp() + 3600,
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
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 1000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(&env),
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
                price: -10,
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
        revenue_splits: soroban_sdk::Vec::new(&env),
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
        name: String::from_str(&env, ""),
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
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 1000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(&env),
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
        venue: String::from_str(&env, ""),
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
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 1000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(&env),
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

#[test]
fn test_update_event_details() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);
    let params = UpdateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: Some(String::from_str(&env, "Updated Conference")),
        description: None,
        venue: None,
        event_date: None,
        allow_anonymous: Some(false),
        requires_verification: Some(true),
        max_tickets_per_user: None,
        resale_royalty_bps: None,
        max_resale_price: None,
        allow_free_ticket_transfer: None,
    };

    client.update_event_details(&params);

    let event = client.get_event(&event_id);
    assert_eq!(event.name, String::from_str(&env, "Updated Conference"));
    assert!(!event.allow_anonymous);
    assert!(event.requires_verification);
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
    let params = UpdateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: None,
        description: None,
        venue: None,
        event_date: None,
        allow_anonymous: None,
        requires_verification: None,
        max_tickets_per_user: None,
        resale_royalty_bps: None,
        max_resale_price: None,
        allow_free_ticket_transfer: None,
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
        max_tickets_per_user: None,
        resale_royalty_bps: None,
        max_resale_price: None,
        allow_free_ticket_transfer: None,
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
    let params = UpdateEventParams {
        organizer: attacker.clone(),
        event_id: event_id.clone(),
        name: Some(String::from_str(&env, "Hacked Event")),
        description: None,
        venue: None,
        event_date: None,
        allow_anonymous: None,
        requires_verification: None,
        max_tickets_per_user: None,
        resale_royalty_bps: None,
        max_resale_price: None,
        allow_free_ticket_transfer: None,
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
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);
    let params = UpdateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: Some(String::from_str(&env, "Too Late")),
        description: None,
        venue: None,
        event_date: None,
        allow_anonymous: None,
        requires_verification: None,
        max_tickets_per_user: None,
        resale_royalty_bps: None,
        max_resale_price: None,
        allow_free_ticket_transfer: None,
    };

    let result = client.try_update_event_details(&params);
    assert!(result.is_err());
}

#[test]
fn test_update_cancelled_event_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);
    client.cancel_event(&organizer, &event_id);
    let params = UpdateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: Some(String::from_str(&env, "Too Late")),
        description: None,
        venue: None,
        event_date: None,
        allow_anonymous: None,
        requires_verification: None,
        max_tickets_per_user: None,
        resale_royalty_bps: None,
        max_resale_price: None,
        allow_free_ticket_transfer: None,
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
    let params_name = UpdateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: Some(String::from_str(&env, "")),
        description: None,
        venue: None,
        event_date: None,
        allow_anonymous: None,
        requires_verification: None,
        max_tickets_per_user: None,
        resale_royalty_bps: None,
        max_resale_price: None,
        allow_free_ticket_transfer: None,
    };
    let result = client.try_update_event_details(&params_name);
    assert!(result.is_err());
    let params_date = UpdateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: None,
        description: None,
        venue: None,
        event_date: Some(BASE_TIMESTAMP),
        allow_anonymous: None,
        requires_verification: None,
        max_tickets_per_user: None,
        resale_royalty_bps: None,
        max_resale_price: None,
        allow_free_ticket_transfer: None,
    };
    let result_date = client.try_update_event_details(&params_date);
    assert!(result_date.is_err());
}

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

    client.register_for_event(&1, &attendee, &event_id, &0, &false, &None);

    let event = client.get_event(&event_id);
    assert_eq!(event.tiers.get(0).unwrap().sold, 1);
    assert_eq!(event.max_supply, 500);
    assert_eq!(event.sold_count, 1);

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

    let result = client.try_register_for_event(&1, &attendee, &event_id, &0, &false, &None);
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
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 1000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(&env),
    };
    client.create_event(&params);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    client.register_for_event(&1, &attendee1, &event_id, &0, &false, &None);
    let event = client.get_event(&event_id);
    assert_eq!(event.sold_count, 1);
    assert_eq!(event.max_supply, 1);

    let result = client.try_register_for_event(&2, &attendee2, &event_id, &0, &false, &None);
    assert_eq!(result.err(), Some(Ok(EventError::EventSoldOut)));
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

    client.register_for_event(&1, &attendee, &event_id, &0, &false, &None);
    let result = client.try_register_for_event(&2, &attendee, &event_id, &0, &false, &None);
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

    let result = client.try_register_for_event(&1, &attendee, &event_id, &0, &false, &None);
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

    client.register_for_event(&1, &attendee1, &event_id, &0, &false, &None);
    client.register_for_event(&2, &attendee2, &event_id, &0, &false, &None);

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
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 1000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(env),
        resale_royalty_bps: 0,
        max_resale_price: None,
        allow_free_ticket_transfer: false,
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
    let platform_wallet = Address::generate(env);
    payments_client.initialize(admin, &token, &0, &platform_wallet, &event_client.address);

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

    client.reserve_ticket(&attendee, &event_id, &0, &None);

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
    client.reserve_ticket(&attendee, &event_id, &0, &None);
    client.register_for_event(&1, &attendee, &event_id, &0, &false, &None);

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
                capacity: 1,
            },
        ],
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 1000,
        withdrawal_delay_ledgers: 17280,
        revenue_splits: soroban_sdk::Vec::new(&env),
    };
    client.create_event(&params);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);
    client.reserve_ticket(&attendee, &event_id, &0, &None);

    let event = client.get_event(&event_id);
    assert_eq!(event.tiers.get(0).unwrap().reserved, 1);
    let attendee_2 = Address::generate(&env);
    let result = client.try_reserve_ticket(&attendee_2, &event_id, &0, &None);
    assert_eq!(result.err(), Some(Ok(EventError::TierSoldOut)));
    env.ledger().with_mut(|li| {
        li.timestamp += 1000;
    });
    client.release_expired_reservation(&event_id, &attendee);

    let event_after = client.get_event(&event_id);
    assert_eq!(event_after.tiers.get(0).unwrap().reserved, 0);
    client.reserve_ticket(&attendee_2, &event_id, &0, &None);
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
    client.reserve_ticket(&attendee, &event_id, &0, &None);
    env.ledger().with_mut(|li| {
        li.timestamp += 1000;
    });
    let result = client.try_register_for_event(&1, &attendee, &event_id, &0, &false, &None);
    assert_eq!(result.err(), Some(Ok(EventError::ReservationExpired)));
}

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

#[test]
fn test_create_event_minimum_withdrawal_delay_enforced() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = Symbol::new(&env, "TESTEVENT");
    let payout_token = test_payout_token(&env);
    let params = CreateEventParams {
        event_id: event_id.clone(),
        organizer: organizer.clone(),
        payout_token: payout_token.clone(),
        name: String::from_str(&env, "Test Event"),
        description: String::from_str(&env, "A test event"),
        venue: String::from_str(&env, "Test Venue"),
        event_date: env.ledger().timestamp() + 100_000,
        allow_anonymous: false,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
        initial_tiers: {
            let mut tiers = soroban_sdk::Vec::new(&env);
            tiers.push_back(TicketTierParams {
                name: String::from_str(&env, "VIP"),
                price: 1000,
                capacity: 100,
            });
            tiers
        },
        max_tickets_per_user: 0,
        event_start_ledger: 100,
        event_end_ledger: 200,
        withdrawal_delay_ledgers: 0, // Below minimum!
        revenue_splits: soroban_sdk::Vec::new(&env),
    };

    let result = client.try_create_event(&params);
    assert_eq!(result.err(), Some(Ok(EventError::InvalidInput)));
    let params_99 = CreateEventParams {
        withdrawal_delay_ledgers: 99,
        ..params.clone()
    };

    let result = client.try_create_event(&params_99);
    assert_eq!(result.err(), Some(Ok(EventError::InvalidInput)));
    let params_100 = CreateEventParams {
        withdrawal_delay_ledgers: 100,
        ..params.clone()
    };

    let result = client.try_create_event(&params_100);
    assert!(result.is_ok());
    let params_200 = CreateEventParams {
        event_id: Symbol::new(&env, "TSTEVENT2"),
        withdrawal_delay_ledgers: 200,
        ..params.clone()
    };

    let result = client.try_create_event(&params_200);
    assert!(result.is_ok());
}
const MIN_WINDOW: u32 = 51_840;
fn setup_active_event(env: &Env, client: &EventContractClient, organizer: &Address) -> Symbol {
    let event_id = setup_event(env, client, organizer);
    client.update_event_status(organizer, &event_id, &EventStatus::Active);
    event_id
}

fn at_sequence(env: &Env, sequence: u32) {
    env.ledger().with_mut(|li| li.sequence_number = sequence);
}

#[test]
fn test_postpone_active_event_opens_window() {
    let env = setup_env();
    at_sequence(&env, 100);
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_active_event(&env, &client, &organizer);

    client.postpone_event(&organizer, &event_id, &60_000, &MIN_WINDOW);

    assert_eq!(client.get_event_status(&event_id), EventStatus::Postponed);
    let info = client.get_postponement(&event_id);
    assert_eq!(info.new_date_ledger, 60_000);
    assert_eq!(info.choice_deadline_ledger, 100 + MIN_WINDOW as u64);
}

#[test]
fn test_postpone_rejects_non_active_states() {
    let env = setup_env();
    at_sequence(&env, 100);
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_event(&env, &client, &organizer);
    let res = client.try_postpone_event(&organizer, &event_id, &60_000, &MIN_WINDOW);
    assert_eq!(res.err(), Some(Ok(EventError::InvalidStatusTransition)));
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);
    client.update_event_status(&organizer, &event_id, &EventStatus::Completed);
    let res = client.try_postpone_event(&organizer, &event_id, &60_000, &MIN_WINDOW);
    assert_eq!(res.err(), Some(Ok(EventError::InvalidStatusTransition)));
}

#[test]
fn test_postpone_rejects_cancelled() {
    let env = setup_env();
    at_sequence(&env, 100);
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_active_event(&env, &client, &organizer);

    client.cancel_event(&organizer, &event_id);
    let res = client.try_postpone_event(&organizer, &event_id, &60_000, &MIN_WINDOW);
    assert_eq!(res.err(), Some(Ok(EventError::InvalidStatusTransition)));
}

#[test]
fn test_postpone_rejects_short_window() {
    let env = setup_env();
    at_sequence(&env, 100);
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_active_event(&env, &client, &organizer);

    let res = client.try_postpone_event(&organizer, &event_id, &60_000, &(MIN_WINDOW - 1));
    assert_eq!(res.err(), Some(Ok(EventError::PostponementWindowTooShort)));
}

#[test]
fn test_postpone_rejects_new_date_inside_window() {
    let env = setup_env();
    at_sequence(&env, 100);
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_active_event(&env, &client, &organizer);
    let deadline = 100 + MIN_WINDOW as u64;
    let res = client.try_postpone_event(&organizer, &event_id, &deadline, &MIN_WINDOW);
    assert_eq!(res.err(), Some(Ok(EventError::InvalidPostponementDate)));
}

#[test]
fn test_postpone_requires_organizer() {
    let env = setup_env();
    at_sequence(&env, 100);
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attacker = Address::generate(&env);
    let event_id = setup_active_event(&env, &client, &organizer);

    let res = client.try_postpone_event(&attacker, &event_id, &60_000, &MIN_WINDOW);
    assert_eq!(res.err(), Some(Ok(EventError::Unauthorized)));
}

#[test]
fn test_finalize_before_window_closes_fails() {
    let env = setup_env();
    at_sequence(&env, 100);
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_active_event(&env, &client, &organizer);

    client.postpone_event(&organizer, &event_id, &60_000, &MIN_WINDOW);
    let res = client.try_finalize_postponement(&organizer, &event_id);
    assert_eq!(res.err(), Some(Ok(EventError::PostponementWindowOpen)));
}

#[test]
fn test_finalize_requires_organizer() {
    let env = setup_env();
    at_sequence(&env, 100);
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attacker = Address::generate(&env);
    let event_id = setup_active_event(&env, &client, &organizer);

    client.postpone_event(&organizer, &event_id, &60_000, &MIN_WINDOW);
    at_sequence(&env, 100 + MIN_WINDOW + 1);
    let res = client.try_finalize_postponement(&attacker, &event_id);
    assert_eq!(res.err(), Some(Ok(EventError::Unauthorized)));
}

#[test]
fn test_finalize_requires_postponed_state() {
    let env = setup_env();
    at_sequence(&env, 100);
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_active_event(&env, &client, &organizer);

    let res = client.try_finalize_postponement(&organizer, &event_id);
    assert_eq!(res.err(), Some(Ok(EventError::EventNotPostponed)));
}

#[test]
fn test_finalize_resumes_active_and_shifts_schedule() {
    let env = setup_env();
    at_sequence(&env, 100);
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_active_event(&env, &client, &organizer);
    let before = client.get_event(&event_id);
    let duration = before.event_end_ledger - before.event_start_ledger;

    client.postpone_event(&organizer, &event_id, &60_000, &MIN_WINDOW);
    at_sequence(&env, 100 + MIN_WINDOW + 1);
    client.finalize_postponement(&organizer, &event_id);

    assert_eq!(client.get_event_status(&event_id), EventStatus::Active);
    let after = client.get_event(&event_id);
    assert_eq!(after.event_start_ledger, 60_000);
    assert_eq!(after.event_end_ledger, 60_000 + duration);
    let res = client.try_get_postponement(&event_id);
    assert_eq!(res.err(), Some(Ok(EventError::EventNotPostponed)));
}

#[test]
fn test_postpone_rejects_out_of_range_new_date() {
    let env = setup_env();
    at_sequence(&env, 100);
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_active_event(&env, &client, &organizer);
    let too_far = u32::MAX as u64 + 1;
    let res = client.try_postpone_event(&organizer, &event_id, &too_far, &MIN_WINDOW);
    assert_eq!(res.err(), Some(Ok(EventError::InvalidPostponementDate)));
}

#[test]
fn test_postpone_rejects_window_above_max() {
    let env = setup_env();
    at_sequence(&env, 100);
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_active_event(&env, &client, &organizer);
    let res = client.try_postpone_event(&organizer, &event_id, &10_000_000, &518_401);
    assert_eq!(res.err(), Some(Ok(EventError::InvalidPostponementDate)));
}

#[test]
fn test_postpone_count_capped() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_active_event(&env, &client, &organizer);
    let mut seq = 10u32;
    for _ in 0..3 {
        at_sequence(&env, seq);
        let new_date = (seq + MIN_WINDOW) as u64 + 10_000;
        client.postpone_event(&organizer, &event_id, &new_date, &MIN_WINDOW);
        seq += MIN_WINDOW + 1;
        at_sequence(&env, seq);
        client.finalize_postponement(&organizer, &event_id);
    }
    at_sequence(&env, seq);
    let new_date = (seq + MIN_WINDOW) as u64 + 10_000;
    let res = client.try_postpone_event(&organizer, &event_id, &new_date, &MIN_WINDOW);
    assert_eq!(res.err(), Some(Ok(EventError::MaxPostponementsReached)));
}

#[test]
fn test_cancel_allowed_from_postponed() {
    let env = setup_env();
    at_sequence(&env, 100);
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_active_event(&env, &client, &organizer);

    client.postpone_event(&organizer, &event_id, &60_000, &MIN_WINDOW);
    client.cancel_event(&organizer, &event_id);
    assert_eq!(client.get_event_status(&event_id), EventStatus::Cancelled);
}

#[test]
fn test_withdraw_revenue_blocked_while_postponed() {
    let env = setup_env();
    at_sequence(&env, 100);
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let event_id = setup_active_event(&env, &client, &organizer);

    client.postpone_event(&organizer, &event_id, &60_000, &MIN_WINDOW);
    let res = client.try_withdraw_revenue(&organizer, &event_id);
    assert_eq!(res.err(), Some(Ok(EventError::InvalidStatusTransition)));
}
