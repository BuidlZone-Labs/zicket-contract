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

    /// Get a ticket record by ticket ID.
    pub fn get_ticket(env: Env, ticket_id: u64) -> Result<Ticket, PaymentError> {
        storage::get_ticket(&env, ticket_id)
    }

    /// Get all ticket IDs owned by a wallet.
    pub fn get_owner_tickets(env: Env, owner: Address) -> soroban_sdk::Vec<u64> {
        storage::get_owner_tickets(&env, &owner)
    }

    /// Set the current lifecycle status for an event.
    pub fn set_event_status(
        env: Env,
        admin: Address,
        event_id: Symbol,
        status: EventStatus,
    ) -> Result<(), PaymentError> {
        let stored_admin = storage::get_admin(&env)?;
        if admin != stored_admin {
            return Err(PaymentError::Unauthorized);
        }
        admin.require_auth();
        storage::set_event_status(&env, &event_id, &status);
        Ok(())
    }

    /// Pay for a ticket with a specific token. Transfers tokens from payer to contract escrow.
    pub fn pay_for_ticket(
        env: Env,
        payer: Address,
        event_id: Symbol,
        amount: i128,
        token_address: Address,
    ) -> Result<u64, PaymentError> {
        payer.require_auth();

        if amount <= 0 {
            return Err(PaymentError::InvalidAmount);
        }

        if let Some(status) = storage::get_event_status(&env, &event_id) {
            if matches!(status, EventStatus::Completed | EventStatus::Cancelled) {
                return Err(PaymentError::EventNotActive);
            }
        }

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

        // Track token-specific revenue
        storage::add_event_token_revenue(&env, &event_id, &token_address, amount);
        storage::add_event_token(&env, &event_id, &token_address);

        events::emit_payment_received(
            &env,
            payment_id,
            event_id,
            payer,
            amount,
            token_address,
            paid_at,
        );

        let ticket_id = storage::get_next_ticket_id(&env);
        let ticket = Ticket {
            ticket_id,
            event_id: payment.event_id.clone(),
            owner: payment.payer.clone(),
            payment_id,
        };
        storage::save_ticket(&env, &ticket);
        storage::add_owner_ticket(&env, &payment.payer, ticket_id);
        events::emit_ticket_issued(&env, ticket_id, payment.event_id, payment.payer, payment_id);

        Ok(payment_id)
    }

    pub fn refund(env: Env, admin: Address, payment_id: u64) -> Result<(), PaymentError> {
        let stored_admin = storage::get_admin(&env)?;
        if admin != stored_admin {
            return Err(PaymentError::Unauthorized);
        }
        admin.require_auth();

        let mut payment = storage::get_payment(&env, payment_id)?;

        if payment.status == PaymentStatus::Refunded {
            return Err(PaymentError::PaymentAlreadyRefunded);
        }
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

        // Update both general and token-specific revenue
        let revenue = storage::get_event_revenue(&env, &payment.event_id);
        storage::set_event_revenue(&env, &payment.event_id, revenue - payment.amount);

        let token_revenue =
            storage::get_event_token_revenue(&env, &payment.event_id, &payment.token);
        storage::set_event_token_revenue(
            &env,
            &payment.event_id,
            &payment.token,
            token_revenue - payment.amount,
        );

        events::emit_payment_refunded(
            &env,
            payment_id,
            payment.event_id,
            payment.payer,
            payment.amount,
        );

        Ok(())
    }

    pub fn withdraw(env: Env, organizer: Address, event_id: Symbol) -> Result<(), PaymentError> {
        organizer.require_auth();

        match storage::get_event_status(&env, &event_id) {
            Some(EventStatus::Completed) => {}
            _ => return Err(PaymentError::EventNotCompleted),
        }

        let revenue = storage::get_event_revenue(&env, &event_id);
        if revenue <= 0 {
            return Err(PaymentError::NoRevenue);
        }

        let token_address = storage::get_accepted_token(&env)?;
        let token_client = token::Client::new(&env, &token_address);
        let payment_ids = storage::get_event_payments(&env, &event_id);

        let mut total: i128 = 0;
        let mut payments_to_release: soroban_sdk::Vec<PaymentRecord> = soroban_sdk::Vec::new(&env);

        for i in 0..payment_ids.len() {
            let pid = payment_ids.get(i).ok_or(PaymentError::PaymentNotFound)?;
            let payment = storage::get_payment(&env, pid)?;
            if payment.status == PaymentStatus::Held {
                total += payment.amount;
                payments_to_release.push_back(payment);
            }
        }

        if total <= 0 {
            return Err(PaymentError::NoRevenue);
        }

        token_client.transfer(&env.current_contract_address(), &organizer, &total);

        for i in 0..payments_to_release.len() {
            let mut payment = payments_to_release
                .get(i)
                .ok_or(PaymentError::PaymentNotFound)?;
            payment.status = PaymentStatus::Released;
            storage::update_payment(&env, &payment)?;
        }

        storage::set_event_revenue(&env, &event_id, 0);

        events::emit_revenue_withdrawn(&env, event_id, organizer, total);

        Ok(())
    }

    pub fn get_event_payments(env: Env, event_id: Symbol) -> soroban_sdk::Vec<u64> {
        storage::get_event_payments(&env, &event_id)
    }

    /// Withdraw revenue for an event.
    pub fn withdraw_revenue(env: Env, event_id: Symbol, to: Address) -> Result<(), PaymentError> {
        let admin = storage::get_admin(&env)?;
        admin.require_auth();

        let revenue = storage::get_event_revenue(&env, &event_id);
        if revenue <= 0 {
            return Err(PaymentError::InvalidAmount);
        }

        let token_address = storage::get_accepted_token(&env)?;
        let token_client = token::Client::new(&env, &token_address);
        token_client.transfer(&env.current_contract_address(), &to, &revenue);

        // Update revenue tracking
        storage::reset_event_revenue(&env, &event_id);

        // Record withdrawal history
        let record = WithdrawalRecord {
            amount: revenue,
            timestamp: env.ledger().timestamp(),
            organizer: to.clone(),
        };
        storage::add_withdrawal_record(&env, &event_id, &record);

        Ok(())
    }

    /// Get all withdrawal history for an event.
    pub fn get_withdrawal_history(
        env: Env,
        event_id: Symbol,
    ) -> soroban_sdk::Vec<WithdrawalRecord> {
        storage::get_withdrawal_history(&env, &event_id)
    }

    /// Get the total revenue for an event and specific token.
    pub fn get_event_token_revenue(env: Env, event_id: Symbol, token_address: Address) -> i128 {
        storage::get_event_token_revenue(&env, &event_id, &token_address)
    }

    /// Get all tokens used for an event.
    pub fn get_event_tokens(env: Env, event_id: Symbol) -> soroban_sdk::Vec<Address> {
        storage::get_event_tokens(&env, &event_id)
    }

    /// Get the accepted token address (for backward compatibility).
    pub fn get_accepted_token(env: Env) -> Result<Address, PaymentError> {
        storage::get_accepted_token(&env)
    }

    /// Withdraw revenue for a specific token from an event.
    pub fn withdraw_token(
        env: Env,
        organizer: Address,
        event_id: Symbol,
        token_address: Address,
    ) -> Result<(), PaymentError> {
        organizer.require_auth();

        match storage::get_event_status(&env, &event_id) {
            Some(EventStatus::Completed) => {}
            _ => return Err(PaymentError::EventNotCompleted),
        }

        let revenue = storage::get_event_token_revenue(&env, &event_id, &token_address);
        if revenue <= 0 {
            return Err(PaymentError::NoRevenue);
        }

        let token_client = token::Client::new(&env, &token_address);
        let payment_ids = storage::get_event_payments(&env, &event_id);

        let mut total: i128 = 0;
        let mut payments_to_release: soroban_sdk::Vec<PaymentRecord> = soroban_sdk::Vec::new(&env);

        for i in 0..payment_ids.len() {
            let pid = payment_ids.get(i).ok_or(PaymentError::PaymentNotFound)?;
            let payment = storage::get_payment(&env, pid)?;
            if payment.status == PaymentStatus::Held && payment.token == token_address {
                total += payment.amount;
                payments_to_release.push_back(payment);
            }
        }

        if total <= 0 {
            return Err(PaymentError::NoRevenue);
        }

        token_client.transfer(&env.current_contract_address(), &organizer, &total);

        for i in 0..payments_to_release.len() {
            let mut payment = payments_to_release
                .get(i)
                .ok_or(PaymentError::PaymentNotFound)?;
            payment.status = PaymentStatus::Released;
            storage::update_payment(&env, &payment)?;
        }

        storage::set_event_token_revenue(&env, &event_id, &token_address, 0);

        events::emit_revenue_withdrawn(&env, event_id, organizer, total);

        Ok(())
    }

    /// Withdraw all tokens for an event.
    pub fn withdraw_all_tokens(
        env: Env,
        organizer: Address,
        event_id: Symbol,
    ) -> Result<(), PaymentError> {
        organizer.require_auth();

        match storage::get_event_status(&env, &event_id) {
            Some(EventStatus::Completed) => {}
            _ => return Err(PaymentError::EventNotCompleted),
        }

        let tokens = storage::get_event_tokens(&env, &event_id);
        if tokens.is_empty() {
            return Err(PaymentError::NoRevenue);
        }

        for i in 0..tokens.len() {
            let token_address = tokens.get(i).ok_or(PaymentError::PaymentNotFound)?;
            let revenue = storage::get_event_token_revenue(&env, &event_id, &token_address);

            if revenue > 0 {
                let token_client = token::Client::new(&env, &token_address);
                let payment_ids = storage::get_event_payments(&env, &event_id);

                let mut total: i128 = 0;
                let mut payments_to_release: soroban_sdk::Vec<PaymentRecord> =
                    soroban_sdk::Vec::new(&env);

                for j in 0..payment_ids.len() {
                    let pid = payment_ids.get(j).ok_or(PaymentError::PaymentNotFound)?;
                    let payment = storage::get_payment(&env, pid)?;
                    if payment.status == PaymentStatus::Held && payment.token == token_address {
                        total += payment.amount;
                        payments_to_release.push_back(payment);
                    }
                }

                if total > 0 {
                    token_client.transfer(&env.current_contract_address(), &organizer, &total);

                    for k in 0..payments_to_release.len() {
                        let mut payment = payments_to_release
                            .get(k)
                            .ok_or(PaymentError::PaymentNotFound)?;
                        payment.status = PaymentStatus::Released;
                        storage::update_payment(&env, &payment)?;
                    }

                    storage::set_event_token_revenue(&env, &event_id, &token_address, 0);
                    events::emit_revenue_withdrawn(
                        &env,
                        event_id.clone(),
                        organizer.clone(),
                        total,
                    );
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[cfg(test)]
mod test;
