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
use soroban_sdk::{contract, contractimpl, xdr::ToXdr, Address, BytesN, Env, Symbol, Vec};

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
        if let Ok(event_contract) = storage::get_event_contract(&env) {
            event_contract.require_auth();
        } else if let Ok(payments_contract) = storage::get_payments_contract(&env) {
            payments_contract.require_auth();
        } else {
            organizer.require_auth();
        }

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

        // Use map-based indexing instead of vector storage
        storage::add_owner_ticket(&env, &owner, ticket_id);
        storage::add_event_ticket(&env, &event_id, ticket_id);

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

        // Use map-based indexing: remove from old owner, add to new owner
        storage::remove_owner_ticket(&env, &from, ticket_id);
        storage::add_owner_ticket(&env, &to, ticket_id);

        events::emit_ticket_transferred(&env, ticket_id, ticket.event_id.clone(), from, to);

        Ok(())
    }

    pub fn get_tickets_by_owner(env: Env, owner: Address) -> Vec<u64> {
        storage::get_tickets_by_owner(&env, owner)
    }

    pub fn use_ticket(
        env: Env,
        organizer: Address,
        owner: Address,
        ticket_id: u64,
    ) -> Result<(), TicketError> {
        organizer.require_auth();
        owner.require_auth();
        let mut ticket: Ticket = env
            .storage()
            .persistent()
            .get(&DataKey::Ticket(ticket_id))
            .ok_or(TicketError::TicketNotFound)?;
        if ticket.organizer != organizer {
            return Err(TicketError::Unauthorized);
        }
        if ticket.owner != owner {
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

        // Use map-based indexing: remove from old owner, add to new owner
        storage::remove_owner_ticket(&env, &old_owner, ticket_id);
        storage::add_owner_ticket(&env, &new_owner, ticket_id);
        storage::remove_recovery_key(&env, ticket_id);

        events::emit_ticket_recovered(&env, ticket_id, old_owner, new_owner);

        Ok(())
    }

    pub fn initialize(
        env: Env,
        admin: Address,
        payments_contract: Address,
    ) -> Result<(), TicketError> {
        if storage::get_admin(&env).is_ok() {
            return Err(TicketError::Unauthorized);
        }
        admin.require_auth();
        storage::set_admin(&env, &admin);
        storage::set_payments_contract(&env, &payments_contract);
        Ok(())
    }

    pub fn set_payments_contract(
        env: Env,
        admin: Address,
        payments_contract: Address,
    ) -> Result<(), TicketError> {
        admin.require_auth();
        let stored_admin = storage::get_admin(&env)?;
        if admin != stored_admin {
            return Err(TicketError::Unauthorized);
        }
        storage::set_payments_contract(&env, &payments_contract);
        Ok(())
    }

    pub fn set_event_contract(
        env: Env,
        admin: Address,
        event_contract: Address,
    ) -> Result<(), TicketError> {
        admin.require_auth();
        let stored_admin = storage::get_admin(&env)?;
        if admin != stored_admin {
            return Err(TicketError::Unauthorized);
        }
        storage::set_event_contract(&env, &event_contract);
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

        // Use map-based indexing: remove from old owner, add to new owner
        storage::remove_owner_ticket(&env, &from, ticket_id);
        storage::add_owner_ticket(&env, &to, ticket_id);

        events::emit_ticket_transferred(&env, ticket_id, ticket.event_id.clone(), from, to);

        Ok(())
    }

    /// Get the current contract version.
    pub fn contract_version(env: Env) -> u32 {
        storage::get_contract_version(&env)
    }
    /// Migrate contract storage to the next version.
    ///
    /// Only the stored admin (set via `set_payments_contract`) may call this;
    /// any other caller is rejected with `TicketError::Unauthorized`.
    pub fn migrate(env: Env, caller: Address) -> Result<u32, TicketError> {
        caller.require_auth();

        let admin = storage::get_admin(&env)?;
        if caller != admin {
            return Err(TicketError::Unauthorized);
        }

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
    let key = DataKey::NextTicketId;
    let id: u64 = env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(1);
    env.storage()
        .persistent()
        .extend_ttl(&key, storage::TTL_THRESHOLD, storage::TTL_BUMP);
    id
}

fn write_next_ticket_id(env: &Env, next_id: u64) {
    let key = DataKey::NextTicketId;
    env.storage()
        .persistent()
        .set(&key, &next_id);
    env.storage()
        .persistent()
        .extend_ttl(&key, storage::TTL_THRESHOLD, storage::TTL_BUMP);
}
