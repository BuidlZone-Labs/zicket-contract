#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String, Symbol};

/// Helper to create a test event through the client.
fn setup_event(env: &Env, client: &EventContractClient, organizer: &Address) -> Symbol {
    let event_id = Symbol::new(env, "EVT001");
    let name = String::from_str(env, "Blockchain Conference");
    let description = String::from_str(env, "Annual developer conference");
    let venue = String::from_str(env, "Convention Center");
    let event_date: u64 = 1735689600;
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

#[test]
fn test_create_event() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_event(&env, &client, &organizer);

    let event = client.get_event(&event_id);
    assert_eq!(event.event_id, event_id);
    assert_eq!(event.organizer, organizer);
    assert_eq!(event.total_tickets, 500);
    assert_eq!(event.tickets_sold, 0);
    assert_eq!(event.ticket_price, 150_000_000);
    assert_eq!(event.status, EventStatus::Upcoming);
}

#[test]
fn test_create_event_duplicate_fails() {
    let env = Env::default();
    env.mock_all_auths();
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
        &1735689600,
        &500,
        &150_000_000,
    );
    assert!(result.is_err());
}

#[test]
fn test_create_event_invalid_tickets_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let result = client.try_create_event(
        &organizer,
        &Symbol::new(&env, "EVT002"),
        &String::from_str(&env, "Bad Event"),
        &String::from_str(&env, "Desc"),
        &String::from_str(&env, "Venue"),
        &1735689600,
        &0, // zero tickets
        &100,
    );
    assert!(result.is_err());
}

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
    let env = Env::default();
    env.mock_all_auths();
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
    let env = Env::default();
    env.mock_all_auths();
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
    let env = Env::default();
    env.mock_all_auths();
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
    let env = Env::default();
    env.mock_all_auths();
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
    let env = Env::default();
    env.mock_all_auths();
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
    let env = Env::default();
    env.mock_all_auths();
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
