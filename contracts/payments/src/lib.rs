#![no_std]
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

        Ok(payment_id)
    }

    pub fn refund(env: Env, admin: Address, payment_id: u64) -> Result<(), PaymentError> {
        admin.require_auth();

        let stored_admin = storage::get_admin(&env)?;
        if admin != stored_admin {
            return Err(PaymentError::Unauthorized);
        }

        let mut payment = storage::get_payment(&env, payment_id)?;

        if payment.status != PaymentStatus::Held {
            return Err(PaymentError::PaymentAlreadyProcessed);
        }

        let token_client = token::Client::new(&env, &payment.token);
        token_client.transfer(
            &env.current_contract_address(),
            &payment.payer,
            &payment.amount,
        );

        payment.status = PaymentStatus::Refunded;
        storage::update_payment(&env, &payment)?;
        storage::subtract_event_revenue(&env, &payment.event_id, payment.amount);

        events::emit_payment_refunded(
            &env,
            payment_id,
            payment.event_id.clone(),
            payment.payer.clone(),
            payment.amount,
        );

        Ok(())
    }

    pub fn withdraw_revenue(
        env: Env,
        organizer: Address,
        event_id: Symbol,
    ) -> Result<i128, PaymentError> {
        organizer.require_auth();

        let revenue = storage::get_event_revenue(&env, &event_id);
        if revenue <= 0 {
            return Err(PaymentError::InvalidAmount);
        }

        let payment_ids = storage::get_event_payments(&env, &event_id);
        let mut total: i128 = 0;

        for i in 0..payment_ids.len() {
            let pid = payment_ids.get(i).unwrap();
            let mut payment = storage::get_payment(&env, pid).unwrap();

            if payment.status == PaymentStatus::Held {
                total += payment.amount;
                payment.status = PaymentStatus::Released;
                storage::update_payment(&env, &payment).unwrap();
            }
        }

        if total > 0 {
            let token_address = storage::get_accepted_token(&env)?;
            let token_client = token::Client::new(&env, &token_address);
            token_client.transfer(&env.current_contract_address(), &organizer, &total);
        }

        storage::set_event_revenue(&env, &event_id, 0);

        events::emit_revenue_withdrawn(&env, event_id, organizer, total);

        Ok(total)
    }
}

#[cfg(test)]
mod test;
