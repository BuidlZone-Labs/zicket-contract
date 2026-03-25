#![no_std]
//! # Payments Contract
//! 
//! This contract ensures that payments only happen when wallet interaction is explicitly triggered.
//! 
//! ## Wallet Authentication Requirements:
//! - `pay_for_ticket`: Requires payer wallet authentication via `payer.require_auth()`
//! - `refund`: Requires admin wallet authentication via `caller.require_auth()`
//! 
//! ## Security Features:
//! - No implicit/automatic payments allowed
//! - All payment operations require explicit wallet signatures
//! - Only authenticated admin can process refunds
//! - Comprehensive test coverage for authentication failures

use soroban_sdk::{contract, contractimpl, token, Address, Env, Symbol};

mod errors;
mod events;
mod storage;
mod types;

pub use errors::*;
pub use events::*;
pub use storage::*;
pub use types::*;

#[contract]
pub struct PaymentsContract;

#[contractimpl]
impl PaymentsContract {
    /// Initialize the contract with an admin address and accepted token address.
    /// This can only be called once. If already initialized, this is a no-op.
    pub fn initialize(env: Env, admin: Address, token: Address) -> Result<(), PaymentError> {
        if storage::is_initialized(&env) {
            return Ok(());
        }

        storage::set_admin(&env, &admin);
        storage::set_accepted_token(&env, &token);

        Ok(())
    }

    /// Get a payment record by payment ID.
    pub fn get_payment(env: Env, payment_id: u64) -> Result<PaymentRecord, PaymentError> {
        storage::get_payment(&env, payment_id)
    }

    /// Get the total revenue for an event.
    pub fn get_event_revenue(env: Env, event_id: Symbol) -> i128 {
        storage::get_event_revenue(&env, &event_id)
    }

    /// Get a ticket record by ticket ID.
    pub fn get_ticket(env: Env, ticket_id: u64) -> Result<Ticket, PaymentError> {
        storage::get_ticket(&env, ticket_id)
    }

    /// Get all ticket IDs owned by a wallet.
    pub fn get_owner_tickets(env: Env, owner: Address) -> soroban_sdk::Vec<u64> {
        storage::get_owner_tickets(&env, &owner)
    }

    /// Pay for a ticket. Transfers tokens from payer to contract escrow.
    pub fn pay_for_ticket(
        env: Env,
        payer: Address,
        event_id: Symbol,
        amount: i128,
    ) -> Result<u64, PaymentError> {
        payer.require_auth();

        if amount <= 0 {
            return Err(PaymentError::InvalidAmount);
        }

        let token_address = storage::get_accepted_token(&env)?;
        let contract_address = env.current_contract_address();

        let token_client = token::Client::new(&env, &token_address);
        token_client.transfer(&payer, &contract_address, &amount);

        let payment_id = storage::get_next_payment_id(&env);
        let paid_at = env.ledger().timestamp();

        let payment = PaymentRecord {
            payment_id,
            event_id: event_id.clone(),
            payer: payer.clone(),
            amount,
            token: token_address.clone(),
            status: PaymentStatus::Held,
            paid_at,
        };

        storage::save_payment(&env, &payment);
        storage::add_event_payment(&env, &event_id, payment_id);
        storage::add_event_revenue(&env, &event_id, amount);

        events::emit_payment_received(&env, payment_id, event_id, payer, amount);

        let ticket_id = storage::get_next_ticket_id(&env);
        let ticket = Ticket {
            ticket_id,
            event_id: payment.event_id.clone(),
            owner: payment.payer.clone(),
            payment_id,
        };
        storage::save_ticket(&env, &ticket);
        storage::add_owner_ticket(&env, &payment.payer, ticket_id);
        events::emit_ticket_issued(&env, ticket_id, payment.event_id, payment.payer);

        Ok(payment_id)
    }

    /// Refund a payment. Transfers tokens from contract back to payer.
    /// Only the admin can initiate refunds.
    pub fn refund(env: Env, caller: Address, payment_id: u64) -> Result<(), PaymentError> {
        caller.require_auth();
        
        // Only admin can refund payments
        let admin = storage::get_admin(&env)?;
        if caller != admin {
            return Err(PaymentError::Unauthorized);
        }

        // Retrieve the payment record
        let mut payment = storage::get_payment(&env, payment_id)?;

        // Verify status is Held
        if payment.status == PaymentStatus::Refunded {
            return Err(PaymentError::PaymentAlreadyRefunded);
        }
        if payment.status != PaymentStatus::Held {
            return Err(PaymentError::PaymentAlreadyProcessed);
        }

        let token_client = token::Client::new(&env, &payment.token);
        let contract_address = env.current_contract_address();

        // Transfer tokens back to payer
        token_client.transfer(&contract_address, &payment.payer, &payment.amount);

        // Update status and save
        payment.status = PaymentStatus::Refunded;
        storage::update_payment(&env, &payment)?;

        // Update event revenue
        let mut revenue = storage::get_event_revenue(&env, &payment.event_id);
        revenue -= payment.amount;
        let key = storage::DataKey::EventRevenue(payment.event_id.clone());
        env.storage().persistent().set(&key, &revenue);

        events::emit_payment_refunded(
            &env,
            payment_id,
            payment.event_id,
            payment.payer,
            payment.amount,
        );

        Ok(())
    }

    /// Get all payment IDs for an event.
    pub fn get_event_payments(env: Env, event_id: Symbol) -> soroban_sdk::Vec<u64> {
        storage::get_event_payments(&env, &event_id)
    }
}

#[cfg(test)]
mod test;
