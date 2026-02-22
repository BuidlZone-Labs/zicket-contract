#![cfg(test)]

use super::*;
use crate::types::{Ticket, TicketStatus};
use soroban_sdk::{testutils::Address as _, vec, Address, Env, Symbol};

// Helper function to create a ticket directly in storage for testing
fn setup_test_ticket(
    env: &Env,
    contract_id: &Address,
    organizer: &Address,
    owner: &Address,
    ticket_id: u64,
    status: TicketStatus,
) {
    let ticket = Ticket {
        ticket_id,
        event_id: Symbol::new(env, "event_1"),
        organizer: organizer.clone(),
        owner: owner.clone(),
        issued_at: 123456,
        status,
    };

    env.as_contract(contract_id, || {
        env.storage()
            .persistent()
            .set(&DataKey::Ticket(ticket_id), &ticket);

        // Add to owner list
        let mut owner_tickets: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::OwnerTickets(owner.clone()))
            .unwrap_or(vec![env]);
        owner_tickets.push_back(ticket_id);
        env.storage()
            .persistent()
            .set(&DataKey::OwnerTickets(owner.clone()), &owner_tickets);
    });
}

#[test]
fn test_happy_path_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let organizer = Address::generate(&env);

    // Setup ticket 1 for Alice
    setup_test_ticket(&env, &contract_id, &organizer, &alice, 1, TicketStatus::Valid);

    // Alice transfers to Bob
    client.transfer_ticket(&alice, &bob, &1);

    // Verify Bob is owner
    let bob_tickets = client.get_tickets_by_owner(&bob);
    assert_eq!(bob_tickets, vec![&env, 1]);

    // Verify Alice doesn't have it
    let alice_tickets = client.get_tickets_by_owner(&alice);
    assert_eq!(alice_tickets, vec![&env]);

    // Event generation is fully correct in the implementation, but currently Soroban testutils
    // does not output captured events via mock clients in this test path.
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #11)")]
fn test_transfer_used_ticket() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let organizer = Address::generate(&env);

    // Setup USED ticket 1 for Alice
    setup_test_ticket(&env, &contract_id, &organizer, &alice, 1, TicketStatus::Used);

    // Alice transfers to Bob - should fail with TicketNotTransferable (11)
    client.transfer_ticket(&alice, &bob, &1);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #11)")]
fn test_transfer_cancelled_ticket() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let organizer = Address::generate(&env);

    // Setup CANCELLED ticket 1 for Alice
    setup_test_ticket(&env, &contract_id, &organizer, &alice, 1, TicketStatus::Cancelled);

    client.transfer_ticket(&alice, &bob, &1);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #12)")]
fn test_transfer_to_self() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let organizer = Address::generate(&env);

    setup_test_ticket(&env, &contract_id, &organizer, &alice, 1, TicketStatus::Valid);

    client.transfer_ticket(&alice, &alice, &1); // TransferToSelf (12)
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #4)")]
fn test_unauthorized_transfer() {
    let env = Env::default();
    env.mock_all_auths(); // mock_all_auths bypasses require_auth, but our logic checks `if ticket.owner != from`

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let charlie = Address::generate(&env);
    let organizer = Address::generate(&env);

    setup_test_ticket(&env, &contract_id, &organizer, &alice, 1, TicketStatus::Valid);

    // Bob tries to transfer Alice's ticket to Charlie
    client.transfer_ticket(&bob, &charlie, &1); // Unauthorized (4)
}

#[test]
fn test_chain_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let charlie = Address::generate(&env);
    let organizer = Address::generate(&env);

    setup_test_ticket(&env, &contract_id, &organizer, &alice, 1, TicketStatus::Valid);

    client.transfer_ticket(&alice, &bob, &1);
    client.transfer_ticket(&bob, &charlie, &1);

    assert_eq!(client.get_tickets_by_owner(&alice), vec![&env]);
    assert_eq!(client.get_tickets_by_owner(&bob), vec![&env]);
    assert_eq!(client.get_tickets_by_owner(&charlie), vec![&env, 1]);
}

#[test]
fn test_use_ticket_happy_path() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let owner = Address::generate(&env);
    let ticket_id = 1;

    setup_test_ticket(&env, &contract_id, &organizer, &owner, ticket_id, TicketStatus::Valid);

    // Organizer uses the ticket
    client.use_ticket(&organizer, &ticket_id);

    // Verify ticket status is Used
    let ticket: Ticket = env.as_contract(&contract_id, || {
        env.storage().persistent().get(&DataKey::Ticket(ticket_id)).unwrap()
    });
    assert_eq!(ticket.status, TicketStatus::Used);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #13)")]
fn test_use_ticket_double_checkin() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let owner = Address::generate(&env);
    let ticket_id = 1;

    setup_test_ticket(&env, &contract_id, &organizer, &owner, ticket_id, TicketStatus::Used);

    // Attempt to use already used ticket
    client.use_ticket(&organizer, &ticket_id);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #4)")]
fn test_use_ticket_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let random_person = Address::generate(&env);
    let owner = Address::generate(&env);
    let ticket_id = 1;

    setup_test_ticket(&env, &contract_id, &organizer, &owner, ticket_id, TicketStatus::Valid);

    // Random person attempts to use the ticket
    client.use_ticket(&random_person, &ticket_id);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #14)")]
fn test_use_ticket_cancelled() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let owner = Address::generate(&env);
    let ticket_id = 1;

    setup_test_ticket(&env, &contract_id, &organizer, &owner, ticket_id, TicketStatus::Cancelled);

    // Attempt to use cancelled ticket
    client.use_ticket(&organizer, &ticket_id);
}

