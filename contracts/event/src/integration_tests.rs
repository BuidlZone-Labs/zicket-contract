use crate::types::{CreateEventParams, EventStatus, TicketTierParams};
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

fn create_active_event(
    env: &Env,
    client: &EventContractClient,
    organizer: &Address,
    event_id: Symbol,
) {
    let params = CreateEventParams {
        organizer: organizer.clone(),
        event_id: event_id.clone(),
        name: String::from_str(env, "Cross Contract Event"),
        description: String::from_str(env, "Integration test event"),
        venue: String::from_str(env, "Main Hall"),
        event_date: env.ledger().timestamp() + 86_401,
        initial_tiers: soroban_sdk::vec![
            env,
            TicketTierParams {
                name: String::from_str(env, "General"),
                price: 100_000_000,
                capacity: 10,
            },
        ],
    };

    client.create_event(&params);
    client.update_event_status(organizer, &event_id, &EventStatus::Active);
}

#[test]
fn test_registration_cross_contract_happy_path() {
    let env = setup_env();

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);

    let event_contract_id = env.register(EventContract, ());
    let event_client = EventContractClient::new(&env, &event_contract_id);

    let ticket_contract_id = env.register(crate::ticket_contract::WASM, ());
    let ticket_client = crate::TicketContractClient::new(&env, &ticket_contract_id);

    let payments_contract_id = env.register(crate::payments_contract::WASM, ());
    let payments_client = crate::PaymentsContractClient::new(&env, &payments_contract_id);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);
    let token_client = token::Client::new(&env, &token_address);

    payments_client.initialize(&organizer, &token_address);
    event_client.initialize(&organizer, &ticket_contract_id, &payments_contract_id);

    let price = 100_000_000i128;
    token_admin_client.mint(&token_admin, &price);
    token_client.transfer(&token_admin, &attendee, &price);

    let event_id = Symbol::new(&env, "evt_cc_1");
    create_active_event(&env, &event_client, &organizer, event_id.clone());

    event_client.register_for_event(&attendee, &event_id, &0);

    let attendee_balance = token_client.balance(&attendee);
    assert_eq!(attendee_balance, 0);

    let escrow_balance = token_client.balance(&payments_contract_id);
    assert_eq!(escrow_balance, price);

    let event = event_client.get_event(&event_id);
    assert_eq!(event.tiers.get(0).unwrap().sold, 1);

    let attendee_tickets = ticket_client.get_tickets_by_owner(&attendee);
    assert_eq!(attendee_tickets.len(), 1);

    let registered = event_client.is_registered(&event_id, &attendee);
    assert!(registered);
}

#[test]
fn test_registration_reverts_if_minting_fails() {
    let env = setup_env();

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);

    let event_contract_id = env.register(EventContract, ());
    let event_client = EventContractClient::new(&env, &event_contract_id);

    let payments_contract_id = env.register(crate::payments_contract::WASM, ());
    let payments_client = crate::PaymentsContractClient::new(&env, &payments_contract_id);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);
    let token_client = token::Client::new(&env, &token_address);

    payments_client.initialize(&organizer, &token_address);
    // Intentionally link the ticket contract to the payments contract to force mint failure.
    event_client.initialize(&organizer, &payments_contract_id, &payments_contract_id);

    let price = 100_000_000i128;
    token_admin_client.mint(&token_admin, &price);
    token_client.transfer(&token_admin, &attendee, &price);

    let event_id = Symbol::new(&env, "evt_cc_2");
    create_active_event(&env, &event_client, &organizer, event_id.clone());

    let result = event_client.try_register_for_event(&attendee, &event_id, &0);
    assert!(result.is_err());

    let attendee_balance = token_client.balance(&attendee);
    assert_eq!(attendee_balance, price);

    let escrow_balance = token_client.balance(&payments_contract_id);
    assert_eq!(escrow_balance, 0);

    let revenue = payments_client.get_event_revenue(&event_id);
    assert_eq!(revenue, 0);

    let event = event_client.get_event(&event_id);
    assert_eq!(event.tiers.get(0).unwrap().sold, 0);

    let registered = event_client.is_registered(&event_id, &attendee);
    assert!(!registered);
}

#[test]
fn test_cancel_event_triggers_refunds() {
    let env = setup_env();

    let organizer = Address::generate(&env);
    let attendee1 = Address::generate(&env);
    let attendee2 = Address::generate(&env);

    let event_contract_id = env.register(EventContract, ());
    let event_client = EventContractClient::new(&env, &event_contract_id);

    let ticket_contract_id = env.register(ticket_contract::TicketContract, ());
    let payments_contract_id = env.register(payments_contract::PaymentsContract, ());
    let payments_client =
        payments_contract::PaymentsContractClient::new(&env, &payments_contract_id);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);
    let token_client = token::Client::new(&env, &token_address);

    payments_client.initialize(&organizer, &token_address);
    event_client.initialize(&organizer, &ticket_contract_id, &payments_contract_id);

    let price = 100_000_000i128;
    token_admin_client.mint(&token_admin, &(price * 2));
    token_client.transfer(&token_admin, &attendee1, &price);
    token_client.transfer(&token_admin, &attendee2, &price);

    let event_id = Symbol::new(&env, "evt_refund_1");
    create_active_event(&env, &event_client, &organizer, event_id.clone());

    event_client.register_for_event(&attendee1, &event_id, &0);
    event_client.register_for_event(&attendee2, &event_id, &0);

    assert_eq!(token_client.balance(&attendee1), 0);
    assert_eq!(token_client.balance(&attendee2), 0);
    assert_eq!(token_client.balance(&payments_contract_id), price * 2);
    assert_eq!(payments_client.get_event_revenue(&event_id), price * 2);

    // Cancel event - should trigger refunds
    event_client.cancel_event(&organizer, &event_id);

    // Check event status
    assert_eq!(
        event_client.get_event_status(&event_id),
        EventStatus::Cancelled
    );

    // Check balances restored
    assert_eq!(token_client.balance(&attendee1), price);
    assert_eq!(token_client.balance(&attendee2), price);
    assert_eq!(token_client.balance(&payments_contract_id), 0);
    assert_eq!(payments_client.get_event_revenue(&event_id), 0);
}
