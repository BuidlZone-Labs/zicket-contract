#![no_std]
mod errors;
mod events;
mod storage;
mod types;

use crate::errors::TicketError;
use crate::storage::DataKey;
use crate::types::{Ticket, TicketStatus};
use soroban_sdk::{contract, contractimpl, vec, Address, Env, Vec};

#[contract]
pub struct TicketContract;

#[contractimpl]
impl TicketContract {
    pub fn transfer_ticket(
        env: Env,
        from: Address,
        to: Address,
        ticket_id: u64,
    ) -> Result<(), TicketError> {
        from.require_auth();

        if from == to {
            return Err(TicketError::TransferToSelf);
        }

        let mut ticket: Ticket = env
            .storage()
            .persistent()
            .get(&DataKey::Ticket(ticket_id))
            .ok_or(TicketError::TicketNotFound)?;

        if ticket.owner != from {
            return Err(TicketError::Unauthorized);
        }

        if ticket.status != TicketStatus::Valid {
            return Err(TicketError::TicketNotTransferable);
        }

        ticket.owner = to.clone();
        env.storage()
            .persistent()
            .set(&DataKey::Ticket(ticket_id), &ticket);

        // Update old owner's list
        let mut from_tickets: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::OwnerTickets(from.clone()))
            .unwrap_or(vec![&env]);

        if let Some(index) = from_tickets.first_index_of(ticket_id) {
            from_tickets.remove(index);
            env.storage()
                .persistent()
                .set(&DataKey::OwnerTickets(from.clone()), &from_tickets);
        }

        // Update new owner's list
        let mut to_tickets: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::OwnerTickets(to.clone()))
            .unwrap_or(vec![&env]);

        to_tickets.push_back(ticket_id);
        env.storage()
            .persistent()
            .set(&DataKey::OwnerTickets(to.clone()), &to_tickets);

        events::emit_ticket_transferred(&env, ticket_id, from, to);

        Ok(())
    }

    pub fn get_tickets_by_owner(env: Env, owner: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::OwnerTickets(owner))
            .unwrap_or(vec![&env])
    }

    pub fn use_ticket(env: Env, organizer: Address, ticket_id: u64) -> Result<(), TicketError> {
        // 1. Require organizer authorization
        organizer.require_auth();

        // 2. Retrieve the ticket from storage
        let mut ticket: Ticket = env
            .storage()
            .persistent()
            .get(&DataKey::Ticket(ticket_id))
            .ok_or(TicketError::TicketNotFound)?;

        // 3. Verify the caller is the event's organizer
        if ticket.organizer != organizer {
            return Err(TicketError::Unauthorized);
        }

        // 4. Verify ticket status is Valid
        match ticket.status {
            TicketStatus::Valid => {}
            TicketStatus::Used => return Err(TicketError::TicketAlreadyUsed),
            TicketStatus::Cancelled => return Err(TicketError::EventNotActive),
        }

        // 5. Update ticket status to Used
        ticket.status = TicketStatus::Used;

        // 6. Save the updated ticket back to storage
        env.storage()
            .persistent()
            .set(&DataKey::Ticket(ticket_id), &ticket);

        // 7. Emit emit_ticket_used
        events::emit_ticket_used(&env, ticket_id);

        Ok(())
    }
}

mod test;
