#![cfg(test)]
use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

#[test]
fn test_mint_and_get_ticket() {
    let env = Env::default();
    let contract_id = env.register_contract(None, TicketContract);
    let client = TicketContractClient::new(&env, &contract_id);

    let event_id = Symbol::new(&env, "event123");
    let owner = Address::generate(&env);

    let ticket_id = client.mint_ticket(&event_id, &owner);
    assert_eq!(ticket_id, 1);

    let ticket = client.get_ticket(&ticket_id);
    assert_eq!(ticket.ticket_id, 1);
    assert_eq!(ticket.event_id, event_id);
    assert_eq!(ticket.owner, owner);
    assert_eq!(ticket.status, TicketStatus::Valid);
}

#[test]
fn test_use_ticket() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, TicketContract);
    let client = TicketContractClient::new(&env, &contract_id);

    let event_id = Symbol::new(&env, "event123");
    let owner = Address::generate(&env);
    let organizer = Address::generate(&env);

    let ticket_id = client.mint_ticket(&event_id, &owner);
    
    client.use_ticket(&ticket_id, &organizer);

    let ticket = client.get_ticket(&ticket_id);
    assert_eq!(ticket.status, TicketStatus::Used);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #2)")]
fn test_use_ticket_already_used() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, TicketContract);
    let client = TicketContractClient::new(&env, &contract_id);

    let event_id = Symbol::new(&env, "event123");
    let owner = Address::generate(&env);
    let organizer = Address::generate(&env);

    let ticket_id = client.mint_ticket(&event_id, &owner);
    client.use_ticket(&ticket_id, &organizer);
    
    // Should panic with TicketAlreadyUsed (Error #2)
    client.use_ticket(&ticket_id, &organizer);
}

#[test]
fn test_query_by_owner_and_event() {
    let env = Env::default();
    let contract_id = env.register_contract(None, TicketContract);
    let client = TicketContractClient::new(&env, &contract_id);

    let event1 = Symbol::new(&env, "event1");
    let event2 = Symbol::new(&env, "event2");
    let owner1 = Address::generate(&env);
    let owner2 = Address::generate(&env);

    client.mint_ticket(&event1, &owner1);
    client.mint_ticket(&event1, &owner2);
    client.mint_ticket(&event2, &owner1);

    let owner1_tickets = client.get_owner_tickets(&owner1);
    assert_eq!(owner1_tickets.len(), 2);
    assert_eq!(owner1_tickets.get(0).unwrap(), 1);
    assert_eq!(owner1_tickets.get(1).unwrap(), 3);

    let event1_tickets = client.get_event_tickets(&event1);
    assert_eq!(event1_tickets.len(), 2);
    assert_eq!(event1_tickets.get(0).unwrap(), 1);
    assert_eq!(event1_tickets.get(1).unwrap(), 2);
}

#[test]
fn test_cancel_ticket() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, TicketContract);
    let client = TicketContractClient::new(&env, &contract_id);

    let event_id = Symbol::new(&env, "event123");
    let owner = Address::generate(&env);

    let ticket_id = client.mint_ticket(&event_id, &owner);
    
    client.cancel_ticket(&ticket_id, &owner);

    let ticket = client.get_ticket(&ticket_id);
    assert_eq!(ticket.status, TicketStatus::Cancelled);
}
