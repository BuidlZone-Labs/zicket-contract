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

#[derive(Clone)]
struct PaymentParams {
    payer: Address,
    event_id: Symbol,
    amount: i128,
    token_address: Address,
    is_anonymous: bool,
    is_verified: bool,
    privacy_level: PaymentPrivacy,
}

#[contract]
pub struct PaymentsContract;

fn validate_payment_privacy(
    env: &Env,
    event_id: &Symbol,
    is_anonymous: bool,
    is_verified: bool,
) -> Result<(), PaymentError> {
    let privacy = storage::get_event_privacy(env, event_id);

    if is_anonymous && !privacy.allow_anonymous {
        return Err(PaymentError::AnonymousPaymentsDisabled);
    }

    if privacy.requires_verification && !is_verified {
        return Err(PaymentError::VerificationRequired);
    }

    Ok(())
}

fn create_payment(env: Env, params: PaymentParams) -> Result<u64, PaymentError> {
    params.payer.require_auth();

    if params.amount <= 0 {
        return Err(PaymentError::InvalidAmount);
    }

    validate_payment_privacy(
        &env,
        &params.event_id,
        params.is_anonymous,
        params.is_verified,
    )?;

    if let Some(status) = storage::get_event_status(&env, &params.event_id) {
        if matches!(status, EventStatus::Completed | EventStatus::Cancelled) {
            return Err(PaymentError::EventNotActive);
        }
    }

    let contract_address = env.current_contract_address();

    let token_client = token::Client::new(&env, &params.token_address);
    token_client
        .try_transfer(&params.payer, &contract_address, &params.amount)
        .map_err(|_| PaymentError::TransferFailed)?;

    let payment_id = storage::get_next_payment_id(&env);
    let paid_at = env.ledger().timestamp();

    let payment = PaymentRecord {
        payment_id,
        event_id: params.event_id.clone(),
        payer: params.payer.clone(),
        amount: params.amount,
        token: params.token_address.clone(),
        status: PaymentStatus::Held,
        paid_at,
        privacy_level: params.privacy_level.clone(),
    };

    storage::save_payment(&env, &payment);
    storage::add_event_payment(&env, &params.event_id, payment_id);
    storage::add_event_revenue(&env, &params.event_id, params.amount);

    // Track token-specific revenue
    storage::add_event_token_revenue(&env, &params.event_id, &params.token_address, params.amount);
    storage::add_event_token(&env, &params.event_id, &params.token_address);

    let privacy = storage::get_emission_privacy(&env, &params.event_id);

    events::emit_payment_received(
        &env,
        payment_id,
        params.event_id,
        params.payer,
        params.amount,
        params.token_address.clone(),
        paid_at,
        &privacy,
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
    events::emit_ticket_issued(
        &env,
        ticket_id,
        payment.event_id,
        payment.payer,
        payment_id,
        &privacy,
    );

    Ok(payment_id)
}

#[contractimpl]
impl PaymentsContract {
    /// Initialize the contract with an admin address and accepted token address.
    /// This can only be called once. If already initialized, this is a no-op.
    pub fn initialize(
        env: Env,
        admin: Address,
        token: Address,
        event_contract: Address,
    ) -> Result<(), PaymentError> {
        if storage::is_initialized(&env) {
            return Ok(());
        }

        storage::set_admin(&env, &admin);
        storage::set_accepted_token(&env, &token);
        storage::set_event_contract(&env, &event_contract);

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

    pub fn get_accepted_token(env: Env) -> Result<Address, PaymentError> {
        storage::get_accepted_token(&env)
    }

    pub fn get_event_config(env: Env, event_id: Symbol) -> Result<EventConfig, PaymentError> {
        storage::get_event_config(&env, &event_id).ok_or(PaymentError::InvalidOrganizer)
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
        privacy_level: PaymentPrivacy,
    ) -> Result<u64, PaymentError> {
        create_payment(
            env,
            PaymentParams {
                payer,
                event_id,
                amount,
                token_address,
                is_anonymous: false,
                is_verified: false,
                privacy_level,
            },
        )
    }

    pub fn pay_for_ticket_with_options(
        env: Env,
        payer: Address,
        event_id: Symbol,
        amount: i128,
        token_address: Address,
        is_anonymous: bool,
        is_verified: bool,
    ) -> Result<u64, PaymentError> {
        create_payment(
            env,
            PaymentParams {
                payer,
                event_id,
                amount,
                token_address,
                is_anonymous,
                is_verified,
                privacy_level: PaymentPrivacy::Standard,
            },
        )
    }

    pub fn sync_event_privacy(
        env: Env,
        event_contract: Address,
        event_id: Symbol,
        allow_anonymous: bool,
        requires_verification: bool,
    ) -> Result<(), PaymentError> {
        if event_contract != storage::get_event_contract(&env)? {
            return Err(PaymentError::Unauthorized);
        }
        event_contract.require_auth();

        let privacy = EventPrivacyConfig {
            allow_anonymous,
            requires_verification,
        };
        storage::set_event_privacy(&env, &event_id, &privacy);

        Ok(())
    }

    pub fn sync_event_config(
        env: Env,
        event_contract: Address,
        event_id: Symbol,
        organizer: Address,
        payout_token: Address,
        allow_anonymous: bool,
        requires_verification: bool,
    ) -> Result<(), PaymentError> {
        if event_contract != storage::get_event_contract(&env)? {
            return Err(PaymentError::Unauthorized);
        }
        event_contract.require_auth();

        let accepted_token = storage::get_accepted_token(&env)?;
        if payout_token != accepted_token {
            return Err(PaymentError::InvalidPayoutToken);
        }

        if let Some(existing_config) = storage::get_event_config(&env, &event_id) {
            if existing_config.organizer != organizer {
                return Err(PaymentError::InvalidOrganizer);
            }
            if existing_config.payout_token != payout_token {
                return Err(PaymentError::InvalidPayoutToken);
            }
        }

        storage::set_event_config(
            &env,
            &event_id,
            &EventConfig {
                organizer,
                payout_token,
                allow_anonymous,
                requires_verification,
            },
        );

        Ok(())
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
            payment.event_id.clone(),
            payment.payer,
            payment.amount,
            &storage::get_emission_privacy(&env, &payment.event_id),
        );

        Ok(())
    }

    pub fn withdraw(env: Env, organizer: Address, event_id: Symbol) -> Result<(), PaymentError> {
        organizer.require_auth();

        let stored_organizer = storage::get_event_organizer(&env, &event_id)?;
        if organizer != stored_organizer {
            return Err(PaymentError::UnauthorizedWithdrawal);
        }

        match storage::get_event_status(&env, &event_id) {
            Some(EventStatus::Completed) => {}
            _ => return Err(PaymentError::EventNotCompleted),
        }

        let payout_token = storage::get_event_payout_token(&env, &event_id)?;
        let revenue = storage::get_event_revenue(&env, &event_id);
        if revenue <= 0 {
            return Err(PaymentError::NoRevenue);
        }

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

        let token_client = token::Client::new(&env, &payout_token);
        token_client.transfer(&env.current_contract_address(), &stored_organizer, &total);

        for i in 0..payments_to_release.len() {
            let mut payment = payments_to_release
                .get(i)
                .ok_or(PaymentError::PaymentNotFound)?;
            payment.status = PaymentStatus::Released;
            storage::update_payment(&env, &payment)?;
        }

        storage::set_event_revenue(&env, &event_id, 0);

        events::emit_revenue_withdrawn(
            &env,
            event_id.clone(),
            stored_organizer,
            total,
            &storage::get_emission_privacy(&env, &event_id),
        );

        Ok(())
    }

    pub fn get_event_payments(env: Env, event_id: Symbol) -> soroban_sdk::Vec<u64> {
        storage::get_event_payments(&env, &event_id)
    }

    /// Register escrow metadata for an event. Admin only.
    /// Must be called before release_if_expired can be used.
    pub fn set_event_end_time(
        env: Env,
        admin: Address,
        event_id: Symbol,
        organizer: Address,
        event_end_time: u64,
    ) -> Result<(), PaymentError> {
        let stored_admin = storage::get_admin(&env)?;
        if admin != stored_admin {
            return Err(PaymentError::Unauthorized);
        }
        admin.require_auth();

        let meta = EscrowMetadata {
            organizer,
            event_end_time,
            auto_released: false,
        };
        storage::set_escrow_meta(&env, &event_id, &meta);
        Ok(())
    }

    /// Release escrowed funds to the organizer if the event end time has passed.
    /// Permissionless: anyone can trigger this after expiry.
    /// Idempotent: calling after already released returns EscrowAlreadyReleased.
    pub fn release_if_expired(env: Env, event_id: Symbol) -> Result<(), PaymentError> {
        let mut meta = storage::get_escrow_meta(&env, &event_id)?;

        if meta.auto_released {
            return Err(PaymentError::EscrowAlreadyReleased);
        }

        if env.ledger().timestamp() < meta.event_end_time {
            return Err(PaymentError::EscrowNotExpired);
        }

        let token_address = storage::get_accepted_token(&env)?;
        let token_client = token::Client::new(&env, &token_address);
        let payment_ids = storage::get_event_payments(&env, &event_id);

        let mut total: i128 = 0;
        let mut to_release: soroban_sdk::Vec<PaymentRecord> = soroban_sdk::Vec::new(&env);

        for i in 0..payment_ids.len() {
            let pid = payment_ids.get(i).unwrap();
            let payment = storage::get_payment(&env, pid)?;
            if payment.status == PaymentStatus::Held {
                total += payment.amount;
                to_release.push_back(payment);
            }
        }

        if total > 0 {
            token_client.transfer(&env.current_contract_address(), &meta.organizer, &total);

            for i in 0..to_release.len() {
                let mut payment = to_release.get(i).unwrap();
                payment.status = PaymentStatus::Released;
                storage::update_payment(&env, &payment)?;
            }

            storage::set_event_revenue(&env, &event_id, 0);
        }

        meta.auto_released = true;
        storage::set_escrow_meta(&env, &event_id, &meta);

        events::emit_escrow_auto_released(&env, event_id, meta.organizer, total);

        Ok(())
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

    /// Set the privacy level for event emissions. Admin only.
    pub fn set_event_privacy(
        env: Env,
        admin: Address,
        event_id: Symbol,
        level: PrivacyLevel,
    ) -> Result<(), PaymentError> {
        let stored_admin = storage::get_admin(&env)?;
        if admin != stored_admin {
            return Err(PaymentError::Unauthorized);
        }
        admin.require_auth();
        storage::set_emission_privacy(&env, &event_id, &level);
        Ok(())
    }

    /// Get the privacy level for event emissions.
    pub fn get_event_privacy(env: Env, event_id: Symbol) -> PrivacyLevel {
        storage::get_emission_privacy(&env, &event_id)
    }

    /// Get the total revenue for an event and specific token.
    pub fn get_event_token_revenue(env: Env, event_id: Symbol, token_address: Address) -> i128 {
        storage::get_event_token_revenue(&env, &event_id, &token_address)
    }

    /// Get all tokens used for an event.
    pub fn get_event_tokens(env: Env, event_id: Symbol) -> soroban_sdk::Vec<Address> {
        storage::get_event_tokens(&env, &event_id)
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

        events::emit_revenue_withdrawn(
            &env,
            event_id.clone(),
            organizer,
            total,
            &storage::get_emission_privacy(&env, &event_id),
        );

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
                        &storage::get_emission_privacy(&env, &event_id),
                    );
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod multi_token_test;
#[cfg(test)]
mod test;
