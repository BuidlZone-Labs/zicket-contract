use super::*;
use crate::storage::DataKey;
use crate::types::{Ticket, TicketStatus};
use soroban_sdk::{testutils::Address as _, vec, Address, Env, Symbol, Vec};
fn setup_test_ticket(
    env: &Env,
    contract_id: &Address,
    organizer: &Address,
    owner: &Address,
    ticket_id: u64,
    status: TicketStatus,
) {
    setup_test_ticket_with_transferable(
        env,
        contract_id,
        organizer,
        owner,
        ticket_id,
        status,
        true,
    );
}
fn setup_test_ticket_with_transferable(
    env: &Env,
    contract_id: &Address,
    organizer: &Address,
    owner: &Address,
    ticket_id: u64,
    status: TicketStatus,
    is_transferable: bool,
) {
    let ticket = Ticket {
        ticket_id,
        event_id: Symbol::new(env, "event_1"),
        organizer: organizer.clone(),
        owner: owner.clone(),
        issued_at: 123456,
        status,
        is_transferable,
        is_used: false,
    };

    env.as_contract(contract_id, || {
        env.storage()
            .persistent()
            .set(&DataKey::Ticket(ticket_id), &ticket);
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
    setup_test_ticket(
        &env,
        &contract_id,
        &organizer,
        &alice,
        1,
        TicketStatus::Valid,
    );
    client.transfer_ticket(&alice, &bob, &1);
    let bob_tickets = client.get_tickets_by_owner(&bob);
    assert_eq!(bob_tickets, vec![&env, 1]);
    let alice_tickets = client.get_tickets_by_owner(&alice);
    assert_eq!(alice_tickets, vec![&env]);
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
    setup_test_ticket(
        &env,
        &contract_id,
        &organizer,
        &alice,
        1,
        TicketStatus::Used,
    );
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
    setup_test_ticket(
        &env,
        &contract_id,
        &organizer,
        &alice,
        1,
        TicketStatus::Cancelled,
    );

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

    setup_test_ticket(
        &env,
        &contract_id,
        &organizer,
        &alice,
        1,
        TicketStatus::Valid,
    );

    client.transfer_ticket(&alice, &alice, &1);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #4)")]
fn test_unauthorized_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let charlie = Address::generate(&env);
    let organizer = Address::generate(&env);

    setup_test_ticket(
        &env,
        &contract_id,
        &organizer,
        &alice,
        1,
        TicketStatus::Valid,
    );
    client.transfer_ticket(&bob, &charlie, &1);
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

    setup_test_ticket(
        &env,
        &contract_id,
        &organizer,
        &alice,
        1,
        TicketStatus::Valid,
    );

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

    setup_test_ticket(
        &env,
        &contract_id,
        &organizer,
        &owner,
        ticket_id,
        TicketStatus::Valid,
    );
    client.use_ticket(&organizer, &ticket_id);
    let ticket: Ticket = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Ticket(ticket_id))
            .expect("ticket should exist")
    });
    assert_eq!(ticket.status, TicketStatus::Used);
    assert!(ticket.is_used);
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
    let ticket = Ticket {
        ticket_id,
        event_id: Symbol::new(&env, "event_1"),
        organizer: organizer.clone(),
        owner: owner.clone(),
        issued_at: 123456,
        status: TicketStatus::Valid,
        is_transferable: true,
        is_used: true,
    };

    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .set(&DataKey::Ticket(ticket_id), &ticket);
    });
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

    setup_test_ticket(
        &env,
        &contract_id,
        &organizer,
        &owner,
        ticket_id,
        TicketStatus::Valid,
    );
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

    setup_test_ticket(
        &env,
        &contract_id,
        &organizer,
        &owner,
        ticket_id,
        TicketStatus::Cancelled,
    );
    client.use_ticket(&organizer, &ticket_id);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #11)")]
fn test_transfer_disabled_ticket() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let organizer = Address::generate(&env);
    setup_test_ticket_with_transferable(
        &env,
        &contract_id,
        &organizer,
        &alice,
        1,
        TicketStatus::Valid,
        false,
    );
    client.transfer_ticket(&alice, &bob, &1);
}

#[test]
fn test_transfer_enabled_ticket() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let organizer = Address::generate(&env);
    setup_test_ticket(
        &env,
        &contract_id,
        &organizer,
        &alice,
        1,
        TicketStatus::Valid,
    );
    client.transfer_ticket(&alice, &bob, &1);
    let bob_tickets = client.get_tickets_by_owner(&bob);
    assert_eq!(bob_tickets, vec![&env, 1]);
    let alice_tickets = client.get_tickets_by_owner(&alice);
    assert_eq!(alice_tickets, vec![&env]);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #11)")]
fn test_transfer_used_ticket_via_is_used() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let organizer = Address::generate(&env);
    let ticket = Ticket {
        ticket_id: 1,
        event_id: Symbol::new(&env, "event_1"),
        organizer: organizer.clone(),
        owner: alice.clone(),
        issued_at: 123456,
        status: TicketStatus::Valid,
        is_transferable: true,
        is_used: true,
    };

    env.as_contract(&contract_id, || {
        env.storage().persistent().set(&DataKey::Ticket(1), &ticket);
    });
    client.transfer_ticket(&alice, &bob, &1);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #13)")]
fn test_cancel_used_ticket_via_is_used() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let organizer = Address::generate(&env);
    let ticket = Ticket {
        ticket_id: 1,
        event_id: Symbol::new(&env, "event_1"),
        organizer: organizer.clone(),
        owner: owner.clone(),
        issued_at: 123456,
        status: TicketStatus::Valid,
        is_transferable: true,
        is_used: true,
    };

    env.as_contract(&contract_id, || {
        env.storage().persistent().set(&DataKey::Ticket(1), &ticket);
    });
    client.cancel_ticket(&1, &owner);
}

#[test]
fn test_set_recovery_key() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let organizer = Address::generate(&env);

    setup_test_ticket(
        &env,
        &contract_id,
        &organizer,
        &alice,
        1,
        TicketStatus::Valid,
    );

    let pub_key = soroban_sdk::BytesN::from_array(&env, &[1; 32]);
    client.set_recovery_key(&alice, &1, &pub_key);
    let stored_key: Option<soroban_sdk::BytesN<32>> = env.as_contract(&contract_id, || {
        env.storage().persistent().get(&DataKey::RecoveryKey(1))
    });
    assert_eq!(stored_key.unwrap().to_array(), pub_key.to_array());
}

#[test]
#[should_panic]
fn test_recover_ticket_invalid_signature() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let organizer = Address::generate(&env);

    setup_test_ticket(
        &env,
        &contract_id,
        &organizer,
        &alice,
        1,
        TicketStatus::Valid,
    );

    let pub_key = soroban_sdk::BytesN::from_array(&env, &[1; 32]);
    client.set_recovery_key(&alice, &1, &pub_key);

    let invalid_signature = soroban_sdk::BytesN::from_array(&env, &[2; 64]);
    client.recover_ticket(&1, &bob, &invalid_signature);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #17)")]
fn test_recover_ticket_no_key_set() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TicketContract, ());
    let client = TicketContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let organizer = Address::generate(&env);

    setup_test_ticket(
        &env,
        &contract_id,
        &organizer,
        &alice,
        1,
        TicketStatus::Valid,
    );

    let invalid_signature = soroban_sdk::BytesN::from_array(&env, &[2; 64]);
    client.recover_ticket(&1, &bob, &invalid_signature);
}
