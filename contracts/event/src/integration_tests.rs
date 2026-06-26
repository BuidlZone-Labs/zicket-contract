use crate::types::{CreateEventParams, EventStatus, PrivacyLevel, TicketTierParams};
use crate::{EventContract, EventContractClient};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{token, Address, BytesN, Env, String, Symbol};

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
    payout_token: &Address,
    event_id: Symbol,
) {
    let params = CreateEventParams {
        organizer: organizer.clone(),
        payout_token: payout_token.clone(),
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
        allow_anonymous: true,
        requires_verification: false,
        privacy_level: PrivacyLevel::Standard,
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 1000,
        withdrawal_delay_ledgers: 17280,
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

    let ticket_contract_id = env.register(ticket_contract::TicketContract, ());
    let ticket_client = ticket_contract::TicketContractClient::new(&env, &ticket_contract_id);

    let payments_contract_id = env.register(payments_contract::PaymentsContract, ());
    let payments_client =
        payments_contract::PaymentsContractClient::new(&env, &payments_contract_id);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);
    let token_client = token::Client::new(&env, &token_address);

    let platform_wallet = Address::generate(&env);
    payments_client.initialize(
        &organizer,
        &token_address,
        &0,
        &platform_wallet,
        &event_contract_id,
    );
    event_client.initialize(&organizer, &ticket_contract_id, &payments_contract_id);

    let price = 100_000_000i128;
    token_admin_client.mint(&token_admin, &price);
    token_client.transfer(&token_admin, &attendee, &price);

    let event_id = Symbol::new(&env, "evt_cc_1");
    create_active_event(
        &env,
        &event_client,
        &organizer,
        &token_address,
        event_id.clone(),
    );

    event_client.register_for_event(&1, &attendee, &event_id, &0, &false, &None);

    let attendee_balance = token_client.balance(&attendee);
    assert_eq!(attendee_balance, 0);

    let escrow_balance = token_client.balance(&payments_contract_id);
    assert_eq!(escrow_balance, price);

    let event = event_client.get_event(&event_id);
    assert_eq!(event.payout_token, token_address);
    assert_eq!(event.tiers.get(0).unwrap().sold, 1);

    let attendee_tickets = ticket_client.get_tickets_by_owner(&attendee);
    assert_eq!(attendee_tickets.len(), 1);

    // Payment contract also issues a receipt-style ticket record linked to payment.
    let payment_owner_tickets = payments_client.get_owner_tickets(&attendee);
    assert_eq!(payment_owner_tickets.len(), 1);
    let payment_ticket_id = payment_owner_tickets.get(0).unwrap();
    let payment_ticket = payments_client.get_ticket(&payment_ticket_id);
    assert_eq!(payment_ticket.owner, attendee);
    assert_eq!(payment_ticket.event_id, event_id);

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

    let payments_contract_id = env.register(payments_contract::PaymentsContract, ());
    let payments_client =
        payments_contract::PaymentsContractClient::new(&env, &payments_contract_id);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);
    let token_client = token::Client::new(&env, &token_address);

    let platform_wallet = Address::generate(&env);
    payments_client.initialize(
        &organizer,
        &token_address,
        &0,
        &platform_wallet,
        &event_contract_id,
    );
    // Intentionally link the ticket contract to the payments contract to force mint failure.
    event_client.initialize(&organizer, &payments_contract_id, &payments_contract_id);

    let price = 100_000_000i128;
    token_admin_client.mint(&token_admin, &price);
    token_client.transfer(&token_admin, &attendee, &price);

    let event_id = Symbol::new(&env, "evt_cc_2");
    create_active_event(
        &env,
        &event_client,
        &organizer,
        &token_address,
        event_id.clone(),
    );

    let result = event_client.try_register_for_event(&1, &attendee, &event_id, &0, &false, &None);
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

    let platform_wallet = Address::generate(&env);
    payments_client.initialize(
        &organizer,
        &token_address,
        &0,
        &platform_wallet,
        &event_contract_id,
    );
    event_client.initialize(&organizer, &ticket_contract_id, &payments_contract_id);

    let price = 100_000_000i128;
    token_admin_client.mint(&token_admin, &(price * 2));
    token_client.transfer(&token_admin, &attendee1, &price);
    token_client.transfer(&token_admin, &attendee2, &price);

    let event_id = Symbol::new(&env, "evt_refund_1");
    create_active_event(
        &env,
        &event_client,
        &organizer,
        &token_address,
        event_id.clone(),
    );

    event_client.register_for_event(&1, &attendee1, &event_id, &0, &false, &None);
    event_client.register_for_event(&2, &attendee2, &event_id, &0, &false, &None);

    assert_eq!(token_client.balance(&attendee1), 0);
    assert_eq!(token_client.balance(&attendee2), 0);
    assert_eq!(token_client.balance(&payments_contract_id), price * 2);
    assert_eq!(payments_client.get_event_revenue(&event_id), price * 2);

    // Cancel event
    event_client.cancel_event(&organizer, &event_id);

    // Check event status
    assert_eq!(
        event_client.get_event_status(&event_id),
        EventStatus::Cancelled
    );

    // Claim refunds
    let payment_owner_tickets_1 = payments_client.get_owner_tickets(&attendee1);
    let ticket_1 = payments_client.get_ticket(&payment_owner_tickets_1.get(0).unwrap());
    payments_client.claim_refund(&attendee1, &ticket_1.payment_id);

    let payment_owner_tickets_2 = payments_client.get_owner_tickets(&attendee2);
    let ticket_2 = payments_client.get_ticket(&payment_owner_tickets_2.get(0).unwrap());
    payments_client.claim_refund(&attendee2, &ticket_2.payment_id);

    // Check balances restored
    assert_eq!(token_client.balance(&attendee1), price);
    assert_eq!(token_client.balance(&attendee2), price);
    assert_eq!(token_client.balance(&payments_contract_id), 0);
    assert_eq!(payments_client.get_event_revenue(&event_id), 0);
}

#[test]
fn test_registration_with_email_hook() {
    let env = setup_env();
    env.mock_all_auths();

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);

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

    let platform_wallet = Address::generate(&env);
    payments_client.initialize(
        &organizer,
        &token_address,
        &0,
        &platform_wallet,
        &event_contract_id,
    );
    event_client.initialize(&organizer, &ticket_contract_id, &payments_contract_id);

    let price = 100_000_000i128;
    token_admin_client.mint(&token_admin, &price);
    let token_client = token::Client::new(&env, &token_address);
    token_client.transfer(&token_admin, &attendee, &price);

    let event_id = Symbol::new(&env, "evt_hook_1");
    create_active_event(
        &env,
        &event_client,
        &organizer,
        &token_address,
        event_id.clone(),
    );

    let email_hash = BytesN::from_array(&env, &[2u8; 32]);
    event_client.register_for_event(
        &1,
        &attendee,
        &event_id,
        &0,
        &false,
        &Some(email_hash.clone()),
    );

    let registered = event_client.is_registered(&event_id, &attendee);
    assert!(registered);
}

const MIN_WINDOW: u32 = 51_840;
const PRICE: i128 = 100_000_000;

#[allow(clippy::type_complexity)]
fn setup_linked(
    env: &Env,
) -> (
    EventContractClient<'_>,
    payments_contract::PaymentsContractClient<'_>,
    ticket_contract::TicketContractClient<'_>,
    token::Client<'_>,
    token::StellarAssetClient<'_>,
    Address, // token address
    Address, // organizer
    Address, // payments contract id
) {
    let organizer = Address::generate(env);

    let event_contract_id = env.register(EventContract, ());
    let event_client = EventContractClient::new(env, &event_contract_id);

    let ticket_contract_id = env.register(ticket_contract::TicketContract, ());
    let ticket_client = ticket_contract::TicketContractClient::new(env, &ticket_contract_id);

    let payments_contract_id = env.register(payments_contract::PaymentsContract, ());
    let payments_client =
        payments_contract::PaymentsContractClient::new(env, &payments_contract_id);

    let token_admin = Address::generate(env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_admin_client = token::StellarAssetClient::new(env, &token_address);
    let token_client = token::Client::new(env, &token_address);

    let platform_wallet = Address::generate(env);
    payments_client.initialize(
        &organizer,
        &token_address,
        &0,
        &platform_wallet,
        &event_contract_id,
    );
    event_client.initialize(&organizer, &ticket_contract_id, &payments_contract_id);

    (
        event_client,
        payments_client,
        ticket_client,
        token_client,
        token_admin_client,
        token_address,
        organizer,
        payments_contract_id,
    )
}

fn fund(token_admin_client: &token::StellarAssetClient, to: &Address, amount: i128) {
    token_admin_client.mint(to, &amount);
}

#[test]
fn test_postpone_full_refund_and_resume() {
    let env = setup_env();
    env.ledger().with_mut(|li| li.sequence_number = 100);

    let (
        event_client,
        payments_client,
        ticket_client,
        token_client,
        token_admin_client,
        token_address,
        organizer,
        payments_id,
    ) = setup_linked(&env);

    let attendee1 = Address::generate(&env);
    let attendee2 = Address::generate(&env);
    fund(&token_admin_client, &attendee1, PRICE);
    fund(&token_admin_client, &attendee2, PRICE);

    let event_id = Symbol::new(&env, "evt_pp_1");
    create_active_event(
        &env,
        &event_client,
        &organizer,
        &token_address,
        event_id.clone(),
    );

    event_client.register_for_event(&1, &attendee1, &event_id, &0, &false, &None);
    event_client.register_for_event(&2, &attendee2, &event_id, &0, &false, &None);
    assert_eq!(token_client.balance(&payments_id), PRICE * 2);
    assert_eq!(payments_client.get_event_revenue(&event_id), PRICE * 2);

    let new_date = 100 + MIN_WINDOW as u64 + 10_000;
    event_client.postpone_event(&organizer, &event_id, &new_date, &MIN_WINDOW);
    assert_eq!(
        event_client.get_event_status(&event_id),
        EventStatus::Postponed
    );

    // The minted (entry) ticket for attendee1 before opting out.
    let minted1 = ticket_client
        .get_tickets_by_owner(&attendee1)
        .get(0)
        .unwrap();

    let t1 = payments_client
        .get_owner_tickets(&attendee1)
        .get(0)
        .unwrap();
    let payment1 = payments_client.get_ticket(&t1).payment_id;

    // Opt out through the event contract (orchestrates refund + revocation).
    // The event is derived from the payment ticket, so no event_id argument.
    event_client.request_postponement_refund(&attendee1, &t1);

    assert_eq!(token_client.balance(&attendee1), PRICE);
    assert_eq!(token_client.balance(&payments_id), PRICE);
    assert_eq!(payments_client.get_event_revenue(&event_id), PRICE);
    assert_eq!(
        payments_client.get_payment(&payment1).status,
        payments_contract::PaymentStatus::Refunded
    );

    // Refunded holder loses participation: registration dropped and ticket cancelled.
    assert!(!event_client.is_registered(&event_id, &attendee1));
    assert_eq!(
        ticket_client.get_ticket(&minted1).status,
        ticket_contract::TicketStatus::Cancelled
    );

    // Non-acting holder keeps their place.
    assert!(event_client.is_registered(&event_id, &attendee2));

    env.ledger()
        .with_mut(|li| li.sequence_number = 100 + MIN_WINDOW + 1);
    event_client.finalize_postponement(&organizer, &event_id);
    assert_eq!(
        event_client.get_event_status(&event_id),
        EventStatus::Active
    );
    let ev = event_client.get_event(&event_id);
    assert_eq!(ev.event_start_ledger, new_date as u32);

    assert!(event_client.is_registered(&event_id, &attendee2));

    event_client.update_event_status(&organizer, &event_id, &EventStatus::Completed);
    event_client.withdraw_revenue(&organizer, &event_id);
    assert_eq!(token_client.balance(&organizer), PRICE);
    assert_eq!(token_client.balance(&payments_id), 0);
}

#[test]
fn test_postponed_event_blocks_all_withdrawals() {
    let env = setup_env();
    env.ledger().with_mut(|li| li.sequence_number = 100);

    let (
        event_client,
        payments_client,
        _ticket_client,
        _token_client,
        token_admin_client,
        token_address,
        organizer,
        _payments_id,
    ) = setup_linked(&env);

    let attendee = Address::generate(&env);
    fund(&token_admin_client, &attendee, PRICE);

    let event_id = Symbol::new(&env, "evt_pp_2");
    create_active_event(
        &env,
        &event_client,
        &organizer,
        &token_address,
        event_id.clone(),
    );
    event_client.register_for_event(&1, &attendee, &event_id, &0, &false, &None);

    let new_date = 100 + MIN_WINDOW as u64 + 10_000;
    event_client.postpone_event(&organizer, &event_id, &new_date, &MIN_WINDOW);

    // Event-contract withdrawal path (gated on Completed).
    let res = event_client.try_withdraw_revenue(&organizer, &event_id);
    assert!(res.is_err());

    // Every direct payments-contract release path is frozen while Postponed.
    let res = payments_client.try_withdraw(&organizer, &event_id);
    assert!(res.is_err());
    let res = payments_client.try_withdraw_revenue(&event_id, &organizer);
    assert_eq!(
        res.err(),
        Some(Ok(payments_contract::PaymentError::EventNotActive))
    );
    let res = payments_client.try_withdraw_token(&organizer, &event_id, &token_address);
    assert!(res.is_err());
}

#[test]
fn test_postponement_refund_after_window_closed_fails() {
    let env = setup_env();
    env.ledger().with_mut(|li| li.sequence_number = 100);

    let (
        event_client,
        payments_client,
        _ticket_client,
        _token_client,
        token_admin_client,
        token_address,
        organizer,
        _payments_id,
    ) = setup_linked(&env);

    let attendee = Address::generate(&env);
    fund(&token_admin_client, &attendee, PRICE);

    let event_id = Symbol::new(&env, "evt_pp_3");
    create_active_event(
        &env,
        &event_client,
        &organizer,
        &token_address,
        event_id.clone(),
    );
    event_client.register_for_event(&1, &attendee, &event_id, &0, &false, &None);

    let new_date = 100 + MIN_WINDOW as u64 + 10_000;
    event_client.postpone_event(&organizer, &event_id, &new_date, &MIN_WINDOW);

    env.ledger()
        .with_mut(|li| li.sequence_number = 100 + MIN_WINDOW + 1);
    let t = payments_client.get_owner_tickets(&attendee).get(0).unwrap();
    let res = payments_client.try_request_postponement_refund(&attendee, &t);
    assert_eq!(
        res.err(),
        Some(Ok(
            payments_contract::PaymentError::PostponementWindowClosed
        ))
    );
}

#[test]
fn test_postponement_refund_rejects_non_owner() {
    let env = setup_env();
    env.ledger().with_mut(|li| li.sequence_number = 100);

    let (
        event_client,
        payments_client,
        _ticket_client,
        _token_client,
        token_admin_client,
        token_address,
        organizer,
        _payments_id,
    ) = setup_linked(&env);

    let attendee = Address::generate(&env);
    let attacker = Address::generate(&env);
    fund(&token_admin_client, &attendee, PRICE);

    let event_id = Symbol::new(&env, "evt_pp_4");
    create_active_event(
        &env,
        &event_client,
        &organizer,
        &token_address,
        event_id.clone(),
    );
    event_client.register_for_event(&1, &attendee, &event_id, &0, &false, &None);

    let new_date = 100 + MIN_WINDOW as u64 + 10_000;
    event_client.postpone_event(&organizer, &event_id, &new_date, &MIN_WINDOW);

    let t = payments_client.get_owner_tickets(&attendee).get(0).unwrap();
    let res = payments_client.try_request_postponement_refund(&attacker, &t);
    assert_eq!(
        res.err(),
        Some(Ok(payments_contract::PaymentError::Unauthorized))
    );
}

#[test]
fn test_postponement_refund_requires_revocable_ticket() {
    // If the holder has no valid, unused entry ticket to give up (e.g. it was
    // already cancelled/transferred), the refund must be rejected — and no money
    // must move.
    let env = setup_env();
    env.ledger().with_mut(|li| li.sequence_number = 100);

    let (
        event_client,
        payments_client,
        ticket_client,
        token_client,
        token_admin_client,
        token_address,
        organizer,
        payments_id,
    ) = setup_linked(&env);

    let attendee = Address::generate(&env);
    fund(&token_admin_client, &attendee, PRICE);

    let event_id = Symbol::new(&env, "evt_pp_5");
    create_active_event(
        &env,
        &event_client,
        &organizer,
        &token_address,
        event_id.clone(),
    );
    event_client.register_for_event(&1, &attendee, &event_id, &0, &false, &None);

    let new_date = 100 + MIN_WINDOW as u64 + 10_000;
    event_client.postpone_event(&organizer, &event_id, &new_date, &MIN_WINDOW);

    // Holder cancels their entry ticket, so nothing is revocable.
    let minted = ticket_client
        .get_tickets_by_owner(&attendee)
        .get(0)
        .unwrap();
    ticket_client.cancel_ticket(&minted, &attendee);

    let t = payments_client.get_owner_tickets(&attendee).get(0).unwrap();
    let res = event_client.try_request_postponement_refund(&attendee, &t);
    assert_eq!(res.err(), Some(Ok(crate::EventError::NoRefundableTicket)));

    // No refund was issued: escrow and payment status are unchanged.
    assert_eq!(token_client.balance(&attendee), 0);
    assert_eq!(token_client.balance(&payments_id), PRICE);
    assert_eq!(
        payments_client.get_payment(&t).status,
        payments_contract::PaymentStatus::Held
    );
}

#[test]
fn test_postponement_refund_is_event_scoped() {
    // A refund must revoke access only for the event the payment ticket belongs to,
    // never for a different event the caller also participates in.
    let env = setup_env();
    env.ledger().with_mut(|li| li.sequence_number = 100);

    let (
        event_client,
        payments_client,
        ticket_client,
        _token_client,
        token_admin_client,
        token_address,
        organizer,
        _payments_id,
    ) = setup_linked(&env);

    let attendee = Address::generate(&env);
    fund(&token_admin_client, &attendee, PRICE * 2);

    let event_a = Symbol::new(&env, "evt_a");
    let event_b = Symbol::new(&env, "evt_b");
    create_active_event(
        &env,
        &event_client,
        &organizer,
        &token_address,
        event_a.clone(),
    );
    create_active_event(
        &env,
        &event_client,
        &organizer,
        &token_address,
        event_b.clone(),
    );

    event_client.register_for_event(&1, &attendee, &event_a, &0, &false, &None);
    event_client.register_for_event(&2, &attendee, &event_b, &0, &false, &None);

    let new_date = 100 + MIN_WINDOW as u64 + 10_000;
    event_client.postpone_event(&organizer, &event_a, &new_date, &MIN_WINDOW);
    event_client.postpone_event(&organizer, &event_b, &new_date, &MIN_WINDOW);

    // Locate the payments receipt ticket that belongs to event B.
    let tickets = payments_client.get_owner_tickets(&attendee);
    let mut ticket_b = None;
    for i in 0..tickets.len() {
        let tid = tickets.get(i).unwrap();
        if payments_client.get_ticket(&tid).event_id == event_b {
            ticket_b = Some(tid);
        }
    }
    let ticket_b = ticket_b.unwrap();

    // Refunding event B's ticket revokes participation in B only — A is untouched.
    event_client.request_postponement_refund(&attendee, &ticket_b);

    assert!(!event_client.is_registered(&event_b, &attendee));
    assert!(event_client.is_registered(&event_a, &attendee));

    // Event A's minted ticket stays valid; event B's is cancelled.
    for tid in ticket_client.get_tickets_by_owner(&attendee).iter() {
        let minted = ticket_client.get_ticket(&tid);
        if minted.event_id == event_a {
            assert_eq!(minted.status, ticket_contract::TicketStatus::Valid);
        } else if minted.event_id == event_b {
            assert_eq!(minted.status, ticket_contract::TicketStatus::Cancelled);
        }
    }
}

#[test]
fn test_withdraw_revenue_integration() {
    let env = setup_env();
    env.mock_all_auths();

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);

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

    let platform_wallet = Address::generate(&env);
    payments_client.initialize(
        &organizer,
        &token_address,
        &0,
        &platform_wallet,
        &event_contract_id,
    );
    event_client.initialize(&organizer, &ticket_contract_id, &payments_contract_id);

    let price = 100_000_000i128;
    token_admin_client.mint(&token_admin, &price);
    let token_client = token::Client::new(&env, &token_address);
    token_client.transfer(&token_admin, &attendee, &price);

    let event_id = Symbol::new(&env, "evt_withdraw_1");
    create_active_event(
        &env,
        &event_client,
        &organizer,
        &token_address,
        event_id.clone(),
    );

    // Register attendee
    event_client.register_for_event(&1, &attendee, &event_id, &0, &false, &None);
    assert_eq!(token_client.balance(&payments_contract_id), price);

    // Complete event to allow withdrawal
    event_client.update_event_status(&organizer, &event_id, &EventStatus::Completed);
    event_client.withdraw_revenue(&organizer, &event_id);

    assert_eq!(token_client.balance(&organizer), price);
    assert_eq!(token_client.balance(&payments_contract_id), 0);
}
