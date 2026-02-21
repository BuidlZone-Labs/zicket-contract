use crate::errors::EventError;
use crate::types::{CreateEventParams, EventStatus, UpdateEventParams};
use crate::{EventContract, EventContractClient};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Env, String, Symbol};

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
    let total_tickets = 500;
    let ticket_price = 100_000_000;

    let params = CreateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: name.clone(),
        description: description.clone(),
        venue: venue.clone(),
        event_date,
        total_tickets,
        ticket_price,
    };

    // First creation succeeds
    client.create_event(&params);

    // Second creation with same ID fails
    let params_dup = CreateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: name.clone(), // doesn't matter
        description: description.clone(),
        venue: venue.clone(),
        event_date,
        total_tickets,
        ticket_price,
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
        event_id: Symbol::new(&env, "event_bad"),
        name: String::from_str(&env, "Bad Event"),
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() + 90_000,
        total_tickets: 0, // Invalid
        ticket_price: 100,
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
        event_id: Symbol::new(&env, "event_bad"),
        name: String::from_str(&env, "Bad Event"),
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() + 90_000,
        total_tickets: 100_000, // Invalid limit
        ticket_price: 100,
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
        event_id: Symbol::new(&env, "event_bad"),
        name: String::from_str(&env, "Bad Event"),
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() - 100, // Past
        total_tickets: 100,
        ticket_price: 100,
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
        event_id: Symbol::new(&env, "event_bad"),
        name: String::from_str(&env, "Bad Event"),
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() + 3600, // Only 1h
        total_tickets: 100,
        ticket_price: 100,
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
        event_id: Symbol::new(&env, "event_bad"),
        name: String::from_str(&env, "Bad Event"),
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() + 90_000,
        total_tickets: 100,
        ticket_price: -10, // Invalid
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
        event_id: Symbol::new(&env, "event_bad"),
        name: String::from_str(&env, ""), // Empty
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() + 90_000,
        total_tickets: 100,
        ticket_price: 100,
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
        event_id: Symbol::new(&env, "event_bad"),
        name: String::from_str(&env, "Event"),
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, ""), // Empty
        event_date: env.ledger().timestamp() + 90_000,
        total_tickets: 100,
        ticket_price: 100,
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
        ticket_price: Some(200_000_000),
    };

    client.update_event_details(&params);

    let event = client.get_event(&event_id);
    assert_eq!(event.name, String::from_str(&env, "Updated Conference"));
    assert_eq!(event.ticket_price, 200_000_000);
    // Verify other fields remain unchanged
    assert_eq!(event.venue, String::from_str(&env, "Convention Center"));
    assert_eq!(event.total_tickets, 500);
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
        ticket_price: None,
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
        ticket_price: None,
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
        ticket_price: None,
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
        ticket_price: None,
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
        ticket_price: None,
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
        ticket_price: None,
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
        ticket_price: None,
    };
    let result_date = client.try_update_event_details(&params_date);
    assert!(result_date.is_err());

    // Negative price
    let params_price = UpdateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: None,
        description: None,
        venue: None,
        event_date: None,
        ticket_price: Some(-100),
    };
    let result_price = client.try_update_event_details(&params_price);
    assert!(result_price.is_err());
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

    let event_id = setup_event(&env, &client, &organizer);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    client.register_for_event(&attendee, &event_id);

    let event = client.get_event(&event_id);
    assert_eq!(event.tickets_sold, 1);

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

    let event_id = setup_event(&env, &client, &organizer);

    let result = client.try_register_for_event(&attendee, &event_id);
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

    let event_id = Symbol::new(&env, "event_02");
    let params = CreateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: String::from_str(&env, "One Ticket"),
        description: String::from_str(&env, "Desc"),
        venue: String::from_str(&env, "Venue"),
        event_date: env.ledger().timestamp() + 86_401,
        total_tickets: 1,
        ticket_price: 100,
    };
    client.create_event(&params);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    client.register_for_event(&attendee1, &event_id);
    let result = client.try_register_for_event(&attendee2, &event_id);
    assert_eq!(result.err(), Some(Ok(EventError::EventSoldOut)));
}

#[test]
fn test_register_for_event_duplicate_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    client.register_for_event(&attendee, &event_id);
    let result = client.try_register_for_event(&attendee, &event_id);
    assert_eq!(result.err(), Some(Ok(EventError::AlreadyRegistered)));
}

#[test]
fn test_register_for_event_cancelled_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);
    client.cancel_event(&organizer, &event_id);

    let result = client.try_register_for_event(&attendee, &event_id);
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

    let event_id = setup_event(&env, &client, &organizer);
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    client.register_for_event(&attendee1, &event_id);
    client.register_for_event(&attendee2, &event_id);

    let attendees = client.get_attendees(&event_id);
    assert_eq!(attendees.len(), 2);
    assert_eq!(attendees.get(0).unwrap(), attendee1);
    assert_eq!(attendees.get(1).unwrap(), attendee2);
}

fn setup_event(env: &Env, client: &EventContractClient, organizer: &Address) -> Symbol {
    let event_id = Symbol::new(env, "event_01");
    let name = String::from_str(env, "Tech Conference 2024");
    let description = String::from_str(env, "A great conference");
    let venue = String::from_str(env, "Convention Center");
    // Ensure date is > 24h in future
    let event_date = env.ledger().timestamp() + 86_401;
    let total_tickets = 500;
    let ticket_price = 100_000_000;

    let params = CreateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name,
        description,
        venue,
        event_date,
        total_tickets,
        ticket_price,
    };

    client.create_event(&params);
    event_id
}
