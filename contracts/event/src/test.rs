#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger, Address, Env, String, Symbol};

/// Base timestamp used in tests (some time in the future).
const BASE_TIMESTAMP: u64 = 1_700_000_000;

/// Helper: set up env with a known ledger timestamp.
fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = BASE_TIMESTAMP;
    });
    env
}

/// Helper to create a valid test event through the client.
fn setup_event(env: &Env, client: &EventContractClient, organizer: &Address) -> Symbol {
    let event_id = Symbol::new(env, "EVT001");
    let name = String::from_str(env, "Blockchain Conference");
    let description = String::from_str(env, "Annual developer conference");
    let venue = String::from_str(env, "Convention Center");
    let event_date: u64 = BASE_TIMESTAMP + 86_400 + 3600; // 25 hours in the future
    let total_tickets: u32 = 500;
    let ticket_price: i128 = 150_000_000;

    client.create_event(
        organizer,
        &event_id,
        &name,
        &description,
        &venue,
        &event_date,
        &total_tickets,
        &ticket_price,
    );

    event_id
}

// ============================================================
// Successful creation
// ============================================================

#[test]
fn test_create_event() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);

    let event = client.get_event(&event_id);
    assert_eq!(event.event_id, event_id);
    assert_eq!(event.organizer, organizer);
    assert_eq!(event.name, String::from_str(&env, "Blockchain Conference"));
    assert_eq!(event.venue, String::from_str(&env, "Convention Center"));
    assert_eq!(event.event_date, BASE_TIMESTAMP + 86_400 + 3600);
    assert_eq!(event.total_tickets, 500);
    assert_eq!(event.tickets_sold, 0);
    assert_eq!(event.ticket_price, 150_000_000);
    assert_eq!(event.status, EventStatus::Upcoming);
    assert_eq!(event.created_at, BASE_TIMESTAMP);
}

// ============================================================
// Validation tests for create_event
// ============================================================

#[test]
fn test_create_event_duplicate_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    setup_event(&env, &client, &organizer);

    // Creating the same event again should fail
    let result = client.try_create_event(
        &organizer,
        &Symbol::new(&env, "EVT001"),
        &String::from_str(&env, "Duplicate"),
        &String::from_str(&env, "Desc"),
        &String::from_str(&env, "Venue"),
        &(BASE_TIMESTAMP + 86_400 + 3600),
        &500,
        &150_000_000,
    );
    assert!(result.is_err());
}

#[test]
fn test_create_event_invalid_tickets_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let result = client.try_create_event(
        &organizer,
        &Symbol::new(&env, "EVT002"),
        &String::from_str(&env, "Bad Event"),
        &String::from_str(&env, "Desc"),
        &String::from_str(&env, "Venue"),
        &(BASE_TIMESTAMP + 86_400 + 3600),
        &0, // zero tickets
        &100,
    );
    assert!(result.is_err());
}

#[test]
fn test_create_event_too_many_tickets_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let result = client.try_create_event(
        &organizer,
        &Symbol::new(&env, "EVT003"),
        &String::from_str(&env, "Big Event"),
        &String::from_str(&env, "Desc"),
        &String::from_str(&env, "Arena"),
        &(BASE_TIMESTAMP + 86_400 + 3600),
        &100_000, // exactly 100,000 — should fail (must be < 100,000)
        &100,
    );
    assert!(result.is_err());
}

#[test]
fn test_create_event_negative_price_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let result = client.try_create_event(
        &organizer,
        &Symbol::new(&env, "EVT004"),
        &String::from_str(&env, "Negative Price"),
        &String::from_str(&env, "Desc"),
        &String::from_str(&env, "Venue"),
        &(BASE_TIMESTAMP + 86_400 + 3600),
        &100,
        &-1, // negative price
    );
    assert!(result.is_err());
}

#[test]
fn test_create_event_empty_name_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let result = client.try_create_event(
        &organizer,
        &Symbol::new(&env, "EVT005"),
        &String::from_str(&env, ""), // empty name
        &String::from_str(&env, "Desc"),
        &String::from_str(&env, "Venue"),
        &(BASE_TIMESTAMP + 86_400 + 3600),
        &100,
        &100,
    );
    assert!(result.is_err());
}

#[test]
fn test_create_event_empty_venue_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let result = client.try_create_event(
        &organizer,
        &Symbol::new(&env, "EVT006"),
        &String::from_str(&env, "Valid Name"),
        &String::from_str(&env, "Desc"),
        &String::from_str(&env, ""), // empty venue
        &(BASE_TIMESTAMP + 86_400 + 3600),
        &100,
        &100,
    );
    assert!(result.is_err());
}

#[test]
fn test_create_event_past_date_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let result = client.try_create_event(
        &organizer,
        &Symbol::new(&env, "EVT007"),
        &String::from_str(&env, "Past Event"),
        &String::from_str(&env, "Desc"),
        &String::from_str(&env, "Venue"),
        &(BASE_TIMESTAMP - 3600), // 1 hour in the past
        &100,
        &100,
    );
    assert!(result.is_err());
}

#[test]
fn test_create_event_date_less_than_24h_fails() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let result = client.try_create_event(
        &organizer,
        &Symbol::new(&env, "EVT008"),
        &String::from_str(&env, "Too Soon"),
        &String::from_str(&env, "Desc"),
        &String::from_str(&env, "Venue"),
        &(BASE_TIMESTAMP + 3600), // only 1 hour ahead, need 24h
        &100,
        &100,
    );
    assert!(result.is_err());
}

// ============================================================
// Event status and lifecycle tests
// ============================================================

#[test]
fn test_get_event_not_found() {
    let env = Env::default();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);

    let result = client.try_get_event(&Symbol::new(&env, "MISSING"));
    assert!(result.is_err());
}

#[test]
fn test_update_event_status_upcoming_to_active() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);

    // Upcoming -> Active
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

    // Upcoming -> Active -> Completed
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

    // Upcoming -> Completed (invalid, must go through Active first)
    let result = client.try_update_event_status(&organizer, &event_id, &EventStatus::Completed);
    assert!(result.is_err());
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

    // Upcoming -> Active -> Completed
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);
    client.update_event_status(&organizer, &event_id, &EventStatus::Completed);

    // Cancelling a completed event should fail
    let result = client.try_cancel_event(&organizer, &event_id);
    assert!(result.is_err());
}

#[test]
fn test_unauthorized_cancel() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let attacker = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);

    // Attacker tries to cancel — auth is mocked, but our contract checks
    // that the caller address matches the event organizer
    let result = client.try_cancel_event(&attacker, &event_id);
    assert!(result.is_err());
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
    client.update_event_details(
        &organizer,
        &event_id,
        &Some(String::from_str(&env, "Updated Conference")),
        &None,              // description unchanged
        &None,              // venue unchanged
        &None,              // date unchanged
        &Some(200_000_000), // new price
    );

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
    client.update_event_details(&organizer, &event_id, &None, &None, &None, &None, &None);

    let updated_event = client.get_event(&event_id);
    assert_eq!(original_event, updated_event);
}

#[test]
fn test_update_event_not_found() {
    let env = setup_env(); // Mistake in previous tests calling setup_env but this is missing a fn?
                           // Ah, setup_env defined in test.rs lines 10-17.
                           // Checking if layout_env exists. No.
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let result = client.try_update_event_details(
        &organizer,
        &Symbol::new(&env, "MISSING"),
        &Some(String::from_str(&env, "New Name")),
        &None,
        &None,
        &None,
        &None,
    );
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
    let result = client.try_update_event_details(
        &attacker,
        &event_id,
        &Some(String::from_str(&env, "Hacked Event")),
        &None,
        &None,
        &None,
        &None,
    );
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
    let result = client.try_update_event_details(
        &organizer,
        &event_id,
        &Some(String::from_str(&env, "Too Late")),
        &None,
        &None,
        &None,
        &None,
    );
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
    let result = client.try_update_event_details(
        &organizer,
        &event_id,
        &Some(String::from_str(&env, "Too Late")),
        &None,
        &None,
        &None,
        &None,
    );
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
    let result = client.try_update_event_details(
        &organizer,
        &event_id,
        &Some(String::from_str(&env, "")),
        &None,
        &None,
        &None,
        &None,
    );
    assert!(result.is_err());

    // Past date
    let result_date = client.try_update_event_details(
        &organizer,
        &event_id,
        &None,
        &None,
        &None,
        &Some(BASE_TIMESTAMP), // now/past
        &None,
    );
    assert!(result_date.is_err());

    // Negative price
    let result_price = client.try_update_event_details(
        &organizer,
        &event_id,
        &None,
        &None,
        &None,
        &None,
        &Some(-100),
    );
    assert!(result_price.is_err());
}
