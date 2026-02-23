use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

use crate::errors::TicketError;
use crate::types::Ticket;

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum DataKey {
    Ticket(u64),
    OwnerTickets(Address),
    EventTickets(Symbol),
    NextTicketId,
}

pub fn get_ticket(env: &Env, ticket_id: u64) -> Result<Ticket, TicketError> {
    env.storage()
        .persistent()
        .get(&DataKey::Ticket(ticket_id))
        .ok_or(TicketError::TicketNotFound)
}

pub fn update_ticket(env: &Env, ticket: &Ticket) {
    env.storage()
        .persistent()
        .set(&DataKey::Ticket(ticket.ticket_id), ticket);
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
