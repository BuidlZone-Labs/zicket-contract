#![no_std]
mod errors;
mod events;
mod storage;
mod types;

#[cfg(test)]
mod migration_test;

#[cfg(test)]
mod test;

use crate::errors::TicketError;
use crate::storage::DataKey;
pub use crate::types::{Ticket, TicketStatus};
use soroban_sdk::{contract, contractimpl, vec, xdr::ToXdr, Address, BytesN, Env, Symbol, Vec};

#[contract]
pub struct TicketContract;

#[contractimpl]
impl TicketContract {
    pub fn mint_ticket(
        env: Env,
        event_id: Symbol,
        organizer: Address,
        owner: Address,
    ) -> Result<u64, TicketError> {
        let ticket_id = read_next_ticket_id(&env);

        let ticket = Ticket {
            ticket_id,
            event_id: event_id.clone(),
            organizer,
            owner: owner.clone(),
            issued_at: env.ledger().timestamp(),
            status: TicketStatus::Valid,
            is_transferable: true,
            is_used: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Ticket(ticket_id), &ticket);

        let mut owner_tickets: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::OwnerTickets(owner.clone()))
            .unwrap_or(vec![&env]);
        owner_tickets.push_back(ticket_id);
        env.storage()
            .persistent()
            .set(&DataKey::OwnerTickets(owner), &owner_tickets);

        let mut event_tickets: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::EventTickets(event_id.clone()))
            .unwrap_or(vec![&env]);
        event_tickets.push_back(ticket_id);
        env.storage()
            .persistent()
            .set(&DataKey::EventTickets(event_id), &event_tickets);

        write_next_ticket_id(&env, ticket_id + 1);
        events::emit_ticket_minted(
            &env,
            ticket_id,
            ticket.event_id.clone(),
            ticket.owner.clone(),
            ticket.organizer.clone(),
            ticket.issued_at,
        );

        Ok(ticket_id)
    }

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

        if !ticket.is_transferable {
            return Err(TicketError::TicketNotTransferable);
        }

        if ticket.is_used {
            return Err(TicketError::TicketNotTransferable);
        }

        if ticket.status != TicketStatus::Valid {
            return Err(TicketError::TicketNotTransferable);
        }

        ticket.owner = to.clone();
        env.storage()
            .persistent()
            .set(&DataKey::Ticket(ticket_id), &ticket);
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
        let mut to_tickets: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::OwnerTickets(to.clone()))
            .unwrap_or(vec![&env]);

        to_tickets.push_back(ticket_id);
        env.storage()
            .persistent()
            .set(&DataKey::OwnerTickets(to.clone()), &to_tickets);

        events::emit_ticket_transferred(&env, ticket_id, ticket.event_id.clone(), from, to);

        Ok(())
    }

    pub fn get_tickets_by_owner(env: Env, owner: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::OwnerTickets(owner))
            .unwrap_or(vec![&env])
    }

    pub fn use_ticket(env: Env, organizer: Address, ticket_id: u64) -> Result<(), TicketError> {
        organizer.require_auth();
        let mut ticket: Ticket = env
            .storage()
            .persistent()
            .get(&DataKey::Ticket(ticket_id))
            .ok_or(TicketError::TicketNotFound)?;
        if ticket.organizer != organizer {
            return Err(TicketError::Unauthorized);
        }
        if ticket.is_used {
            return Err(TicketError::TicketAlreadyUsed);
        }

        match ticket.status {
            TicketStatus::Valid => {}
            TicketStatus::Cancelled => return Err(TicketError::EventNotActive),
            TicketStatus::Used => return Err(TicketError::TicketAlreadyUsed),
        }
        ticket.is_used = true;
        ticket.status = TicketStatus::Used;
        env.storage()
            .persistent()
            .set(&DataKey::Ticket(ticket_id), &ticket);
        events::emit_ticket_used(
            &env,
            ticket_id,
            ticket.event_id.clone(),
            ticket.owner.clone(),
        );

        Ok(())
    }
    pub fn get_ticket(env: Env, ticket_id: u64) -> Result<Ticket, TicketError> {
        storage::get_ticket(&env, ticket_id)
    }
    pub fn get_owner_tickets(env: Env, owner: Address) -> Vec<u64> {
        storage::get_tickets_by_owner(&env, owner)
    }
    pub fn get_event_tickets(env: Env, event_id: Symbol) -> Vec<u64> {
        storage::get_tickets_by_event(&env, event_id)
    }
    pub fn cancel_ticket(env: Env, ticket_id: u64, caller: Address) -> Result<(), TicketError> {
        caller.require_auth();

        let mut ticket = storage::get_ticket(&env, ticket_id)?;

        if caller != ticket.owner {
            return Err(TicketError::Unauthorized);
        }

        if ticket.is_used {
            return Err(TicketError::TicketAlreadyUsed);
        }

        if ticket.status != TicketStatus::Valid {
            return Err(TicketError::TicketAlreadyUsed);
        }

        ticket.status = TicketStatus::Cancelled;
        storage::update_ticket(&env, &ticket);

        events::emit_ticket_cancelled(
            &env,
            ticket_id,
            ticket.event_id.clone(),
            ticket.owner.clone(),
        );

        Ok(())
    }

    pub fn set_recovery_key(
        env: Env,
        owner: Address,
        ticket_id: u64,
        public_key: BytesN<32>,
    ) -> Result<(), TicketError> {
        owner.require_auth();

        let ticket = storage::get_ticket(&env, ticket_id)?;

        if ticket.owner != owner {
            return Err(TicketError::Unauthorized);
        }

        if ticket.is_used || ticket.status != TicketStatus::Valid {
            return Err(TicketError::TicketNotTransferable);
        }

        storage::set_recovery_key(&env, ticket_id, &public_key);
        events::emit_ticket_recovery_key_set(&env, ticket_id, owner);

        Ok(())
    }

    pub fn recover_ticket(
        env: Env,
        ticket_id: u64,
        new_owner: Address,
        signature: BytesN<64>,
    ) -> Result<(), TicketError> {
        let mut ticket = storage::get_ticket(&env, ticket_id)?;

        if ticket.is_used || ticket.status != TicketStatus::Valid {
            return Err(TicketError::TicketNotTransferable);
        }

        let public_key =
            storage::get_recovery_key(&env, ticket_id).ok_or(TicketError::RecoveryKeyNotFound)?;

        let message = new_owner.clone().to_xdr(&env);
        env.crypto()
            .ed25519_verify(&public_key, &message, &signature);

        let old_owner = ticket.owner.clone();
        ticket.owner = new_owner.clone();
        storage::update_ticket(&env, &ticket);
        let mut old_owner_tickets = storage::get_tickets_by_owner(&env, old_owner.clone());
        if let Some(index) = old_owner_tickets.first_index_of(ticket_id) {
            old_owner_tickets.remove(index);
            env.storage().persistent().set(
                &DataKey::OwnerTickets(old_owner.clone()),
                &old_owner_tickets,
            );
        }
        let mut new_owner_tickets = storage::get_tickets_by_owner(&env, new_owner.clone());
        new_owner_tickets.push_back(ticket_id);
        env.storage().persistent().set(
            &DataKey::OwnerTickets(new_owner.clone()),
            &new_owner_tickets,
        );
        storage::remove_recovery_key(&env, ticket_id);

        events::emit_ticket_recovered(&env, ticket_id, old_owner, new_owner);

        Ok(())
    }

    pub fn set_payments_contract(env: Env, admin: Address, payments_contract: Address) -> Result<(), TicketError> {
        if let Ok(stored_admin) = storage::get_admin(&env) {
            if admin != stored_admin {
                return Err(TicketError::Unauthorized);
            }
        } else {
            storage::set_admin(&env, &admin);
        }
        admin.require_auth();
        storage::set_payments_contract(&env, &payments_contract);
        Ok(())
    }

    pub fn admin_transfer_ticket(
        env: Env,
        admin: Address,
        from: Address,
        to: Address,
        ticket_id: u64,
    ) -> Result<(), TicketError> {
        admin.require_auth();

        let payments_contract = storage::get_payments_contract(&env)?;
        if admin != payments_contract {
            return Err(TicketError::Unauthorized);
        }

        let mut ticket = storage::get_ticket(&env, ticket_id)?;

        if ticket.owner != from {
            return Err(TicketError::Unauthorized);
        }

        if !ticket.is_transferable || ticket.is_used || ticket.status != TicketStatus::Valid {
            return Err(TicketError::TicketNotTransferable);
        }

        ticket.owner = to.clone();
        storage::update_ticket(&env, &ticket);

        // Update old owner's list
        let mut from_tickets = storage::get_tickets_by_owner(&env, from.clone());
        if let Some(index) = from_tickets.first_index_of(ticket_id) {
            from_tickets.remove(index);
            env.storage()
                .persistent()
                .set(&DataKey::OwnerTickets(from.clone()), &from_tickets);
        }

        // Update new owner's list
        let mut to_tickets = storage::get_tickets_by_owner(&env, to.clone());
        to_tickets.push_back(ticket_id);
        env.storage()
            .persistent()
            .set(&DataKey::OwnerTickets(to.clone()), &to_tickets);

        events::emit_ticket_transferred(&env, ticket_id, ticket.event_id.clone(), from, to);

        Ok(())
    }

    /// Get the current contract version.
    pub fn contract_version(env: Env) -> u32 {
        storage::get_contract_version(&env)
    }
    pub fn migrate(env: Env, caller: Address) -> Result<u32, TicketError> {
        caller.require_auth();

        let current_version = storage::get_contract_version(&env);
        let new_version = current_version + 1;
        match current_version {
            0 => {
                storage::set_contract_version(&env, 1);
            }
            1 => {
                storage::set_contract_version(&env, 2);
            }
            2 => {
                storage::set_contract_version(&env, 3);
            }
            _ => {
                return Err(TicketError::UnsupportedVersion);
            }
        }

        Ok(new_version)
    }
}

fn read_next_ticket_id(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::NextTicketId)
        .unwrap_or(1)
}

fn write_next_ticket_id(env: &Env, next_id: u64) {
    env.storage()
        .persistent()
        .set(&DataKey::NextTicketId, &next_id);
}
