use soroban_sdk::{contracttype, Env, Address, Symbol, Vec};
use crate::types::{Ticket, TicketStatus};
use crate::errors::TicketError;

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Ticket(u64),
    OwnerTickets(Address),
    EventTickets(Symbol),
    NextTicketId,
}

pub fn mint_ticket(env: &Env, event_id: Symbol, owner: Address) -> u64 {
    let ticket_id = get_next_ticket_id(env);
    
    let ticket = Ticket {
        ticket_id,
        event_id: event_id.clone(),
        owner: owner.clone(),
        issued_at: env.ledger().timestamp(),
        status: TicketStatus::Valid,
    };

    // Store the ticket
    env.storage().persistent().set(&DataKey::Ticket(ticket_id), &ticket);

    // Update owner tickets
    let mut owner_tickets: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::OwnerTickets(owner.clone()))
        .unwrap_or(Vec::new(env));
    owner_tickets.push_back(ticket_id);
    env.storage().persistent().set(&DataKey::OwnerTickets(owner), &owner_tickets);

    // Update event tickets
    let mut event_tickets: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::EventTickets(event_id.clone()))
        .unwrap_or(Vec::new(env));
    event_tickets.push_back(ticket_id);
    env.storage().persistent().set(&DataKey::EventTickets(event_id), &event_tickets);

    // Increment next ticket ID
    increment_ticket_id(env);

    ticket_id
}

pub fn get_ticket(env: &Env, ticket_id: u64) -> Result<Ticket, TicketError> {
    env.storage()
        .persistent()
        .get(&DataKey::Ticket(ticket_id))
        .ok_or(TicketError::TicketNotFound)
}

pub fn update_ticket(env: &Env, ticket: &Ticket) {
    env.storage().persistent().set(&DataKey::Ticket(ticket.ticket_id), ticket);
}

pub fn get_tickets_by_owner(env: &Env, owner: Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::OwnerTickets(owner))
        .unwrap_or(Vec::new(env))
}

pub fn get_tickets_by_event(env: &Env, event_id: Symbol) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::EventTickets(event_id))
        .unwrap_or(Vec::new(env))
}

fn get_next_ticket_id(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::NextTicketId)
        .unwrap_or(1)
}

fn increment_ticket_id(env: &Env) {
    let next_id = get_next_ticket_id(env) + 1;
    env.storage().persistent().set(&DataKey::NextTicketId, &next_id);
}
