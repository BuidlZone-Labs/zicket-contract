#![no_std]
use soroban_sdk::{contract, contractimpl, token, Address, BytesN, Env, Symbol};

mod errors;
mod events;
mod storage;
mod types;

#[cfg(test)]
mod migration_test;

pub use errors::*;
pub use events::*;
pub use storage::*;
pub use types::*;

#[derive(Clone)]
struct PaymentParams {
    nonce: u64,
    payer: Address,
    event_id: Symbol,
    amount: i128,
    token_address: Address,
    is_anonymous: bool,
    is_verified: bool,
    privacy_level: PaymentPrivacy,
    email_hash: Option<BytesN<32>>,
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

fn require_not_paused(env: &Env) -> Result<(), PaymentError> {
    if storage::is_paused(env) {
        return Err(PaymentError::ContractPaused);
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn create_payment(env: Env, params: PaymentParams) -> Result<u64, PaymentError> {
    params.payer.require_auth();
    require_not_paused(&env)?;

    if params.nonce == 0 {
        return Err(PaymentError::NonceRequired);
    }

    if storage::has_nonce(&env, &params.payer, params.nonce) {
        return Err(PaymentError::DuplicateRequest);
    }

    if params.amount <= 0 {
        return Err(PaymentError::InvalidAmount);
    }

    validate_payment_privacy(
        &env,
        &params.event_id,
        params.is_anonymous,
        params.is_verified,
    )?;

    if let Some(config) = storage::get_event_config(&env, &params.event_id) {
        if config.max_supply > 0 && config.sold_count >= config.max_supply {
            return Err(PaymentError::EventSoldOut);
        }

        if config.max_tickets_per_user > 0 {
            let current_tickets =
                storage::get_user_event_tickets(&env, &params.event_id, &params.payer);
            if current_tickets >= config.max_tickets_per_user {
                return Err(PaymentError::MaxTicketsReached);
            }
        }
    }

    if let Some(status) = storage::get_event_status(&env, &params.event_id) {
        if matches!(status, EventStatus::Completed | EventStatus::Cancelled) {
            return Err(PaymentError::EventNotActive);
        }
    }

    let contract_address = env.current_contract_address();

    let token_client = token::Client::new(&env, &params.token_address);
    token_client.transfer(&params.payer, &contract_address, &params.amount);

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
        refunded_amount: 0,
    };

    storage::save_payment(&env, &payment)?;
    storage::add_event_payment(&env, &params.event_id, payment_id);
    storage::add_payer_payment(&env, &params.payer, payment_id);
    storage::set_nonce(&env, &params.payer, params.nonce);
    storage::add_event_revenue(&env, &params.event_id, params.amount);

    // Track token-specific revenue
    storage::add_event_token_revenue(&env, &params.event_id, &params.token_address, params.amount);
    storage::add_event_token(&env, &params.event_id, &params.token_address);

    let privacy = storage::get_emission_privacy(&env, &params.event_id);

    events::emit_payment_received(
        &env,
        payment_id,
        params.event_id.clone(),
        params.payer.clone(),
        params.amount,
        params.token_address.clone(),
        paid_at,
        &privacy,
    );

    if let Some(hash) = params.email_hash {
        events::emit_payment_receipt_requested(
            &env,
            payment_id,
            params.event_id.clone(),
            Some(hash),
        );
    }

    let ticket_id = storage::get_next_ticket_id(&env);
    let ticket = Ticket {
        ticket_id,
        event_id: payment.event_id.clone(),
        owner: payment.payer.clone(),
        payment_id,
    };
    storage::save_ticket(&env, &ticket)?;
    storage::add_owner_ticket(&env, &payment.payer, ticket_id);
    storage::increment_user_event_tickets(&env, &params.event_id, &params.payer);
    if storage::get_event_config(&env, &params.event_id).is_some() {
        storage::increment_event_sold_count(&env, &params.event_id)?;
    }
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

fn collect_held_payments_for_token(
    env: &Env,
    event_id: &Symbol,
    token_address: &Address,
) -> Result<(i128, soroban_sdk::Vec<PaymentRecord>), PaymentError> {
    let payment_ids = storage::get_event_payments(env, event_id);
    let mut total = 0i128;
    let mut payments = soroban_sdk::Vec::new(env);

    for index in 0..payment_ids.len() {
        let payment_id = payment_ids
            .get(index)
            .ok_or(PaymentError::PaymentNotFound)?;
        let payment = storage::get_payment(env, payment_id)?;
        if payment.status == PaymentStatus::Held && payment.token == *token_address {
            total += payment.amount;
            payments.push_back(payment);
        }
    }

    Ok((total, payments))
}

#[contractimpl]
impl PaymentsContract {
    /// Initialize the contract with an admin address, accepted token address,
    /// platform fee (in basis points, 0-10000), and platform wallet address.
    /// This can only be called once. If already initialized, this is a no-op.
    pub fn initialize(
        env: Env,
        admin: Address,
        token: Address,
        platform_fee_bps: u32,
        platform_wallet: Address,
        event_contract: Address,
    ) -> Result<(), PaymentError> {
        if storage::is_initialized(&env) {
            return Ok(());
        }

        if platform_fee_bps > 10_000 {
            return Err(PaymentError::InvalidFeeBps);
        }

        storage::set_admin(&env, &admin);
        storage::set_accepted_token(&env, &token);
        storage::set_platform_fee_bps(&env, platform_fee_bps);
        storage::set_platform_wallet(&env, &platform_wallet);
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

    pub fn is_paused(env: Env) -> bool {
        storage::is_paused(&env)
    }

    pub fn set_paused(env: Env, admin: Address, paused: bool) -> Result<(), PaymentError> {
        let stored_admin = storage::get_admin(&env)?;
        if admin != stored_admin {
            return Err(PaymentError::Unauthorized);
        }
        admin.require_auth();

        storage::set_paused(&env, paused);
        Ok(())
    }

    /// Set the current lifecycle status for an event.
    pub fn set_event_status(
        env: Env,
        admin: Address,
        event_id: Symbol,
        status: EventStatus,
    ) -> Result<(), PaymentError> {
        require_not_paused(&env)?;
        let stored_admin = storage::get_admin(&env)?;
        if admin != stored_admin {
            return Err(PaymentError::Unauthorized);
        }
        admin.require_auth();
        storage::set_event_status(&env, &event_id, &status);
        Ok(())
    }

    /// Pay for a ticket with a specific token. Transfers tokens from payer to contract escrow.
    #[allow(clippy::too_many_arguments)]
    pub fn pay_for_ticket(
        env: Env,
        nonce: u64,
        payer: Address,
        event_id: Symbol,
        amount: i128,
        email_hash: Option<BytesN<32>>,
        token_address: Address,
        privacy_level: PaymentPrivacy,
    ) -> Result<u64, PaymentError> {
        create_payment(
            env,
            PaymentParams {
                nonce,
                payer,
                event_id,
                amount,
                token_address,
                is_anonymous: false,
                is_verified: false,
                privacy_level,
                email_hash,
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pay_for_ticket_with_options(
        env: Env,
        nonce: u64,
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
                nonce,
                payer,
                event_id,
                amount,
                token_address,
                is_anonymous,
                is_verified,
                privacy_level: PaymentPrivacy::Standard,
                email_hash: None,
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
        require_not_paused(&env)?;
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

    #[allow(clippy::too_many_arguments)]
    pub fn sync_event_config(
        env: Env,
        event_contract: Address,
        event_id: Symbol,
        organizer: Address,
        payout_token: Address,
        allow_anonymous: bool,
        requires_verification: bool,
        max_tickets_per_user: u32,
        max_supply: u32,
    ) -> Result<(), PaymentError> {
        require_not_paused(&env)?;
        if event_contract != storage::get_event_contract(&env)? {
            return Err(PaymentError::Unauthorized);
        }
        event_contract.require_auth();

        let accepted_token = storage::get_accepted_token(&env)?;
        if payout_token != accepted_token {
            return Err(PaymentError::InvalidPayoutToken);
        }

        let existing_sold_count =
            if let Some(existing_config) = storage::get_event_config(&env, &event_id) {
                if existing_config.organizer != organizer {
                    return Err(PaymentError::InvalidOrganizer);
                }
                if existing_config.payout_token != payout_token {
                    return Err(PaymentError::InvalidPayoutToken);
                }
                existing_config.sold_count
            } else {
                0
            };

        storage::set_event_config(
            &env,
            &event_id,
            &EventConfig {
                organizer,
                payout_token,
                allow_anonymous,
                requires_verification,
                max_tickets_per_user,
                max_supply,
                sold_count: existing_sold_count,
            },
        );

        Ok(())
    }

    pub fn refund(
        env: Env,
        admin: Address,
        payment_id: u64,
        amount: Option<i128>,
    ) -> Result<(), PaymentError> {
        require_not_paused(&env)?;
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

        let remaining = payment.amount - payment.refunded_amount;
        let refund_amt = amount.unwrap_or(remaining);

        if refund_amt <= 0 || refund_amt > remaining {
            return Err(PaymentError::InvalidAmount);
        }

        let token_client = token::Client::new(&env, &payment.token);
        token_client.transfer(&env.current_contract_address(), &payment.payer, &refund_amt);

        payment.refunded_amount += refund_amt;
        if payment.refunded_amount == payment.amount {
            payment.status = PaymentStatus::Refunded;
        }

        storage::update_payment(&env, &payment)?;

        // Update both general and token-specific revenue
        let revenue = storage::get_event_revenue(&env, &payment.event_id);
        storage::set_event_revenue(&env, &payment.event_id, revenue - refund_amt);

        let token_revenue =
            storage::get_event_token_revenue(&env, &payment.event_id, &payment.token);
        storage::set_event_token_revenue(
            &env,
            &payment.event_id,
            &payment.token,
            token_revenue - refund_amt,
        );

        events::emit_payment_refunded(
            &env,
            payment_id,
            payment.event_id.clone(),
            payment.payer,
            refund_amt,
            payment.token.clone(),
            &storage::get_emission_privacy(&env, &payment.event_id),
        );

        Ok(())
    }

    pub fn withdraw(env: Env, organizer: Address, event_id: Symbol) -> Result<(), PaymentError> {
        require_not_paused(&env)?;
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
        let revenue = storage::get_event_token_revenue(&env, &event_id, &payout_token);
        if revenue <= 0 {
            return Err(PaymentError::NoRevenue);
        }

        let (total, payments_to_release) =
            collect_held_payments_for_token(&env, &event_id, &payout_token)?;

        if total <= 0 {
            return Err(PaymentError::NoRevenue);
        }

        let token_client = token::Client::new(&env, &payout_token);

        // Calculate platform fee
        let fee_bps = storage::get_platform_fee_bps(&env) as i128;
        let fee_amount = total * fee_bps / 10_000;
        let organizer_amount = total - fee_amount;

        // Transfer organizer share
        token_client.transfer(
            &env.current_contract_address(),
            &stored_organizer,
            &organizer_amount,
        );

        // Accumulate platform revenue if there is a fee
        if fee_amount > 0 {
            storage::add_platform_revenue(&env, &event_id, fee_amount);
            events::emit_platform_fee_collected(
                &env,
                event_id.clone(),
                fee_amount,
                organizer_amount,
                payout_token.clone(),
            );
        }

        for i in 0..payments_to_release.len() {
            let mut payment = payments_to_release
                .get(i)
                .ok_or(PaymentError::PaymentNotFound)?;
            payment.status = PaymentStatus::Released;
            storage::update_payment(&env, &payment)?;
        }

        storage::set_event_token_revenue(&env, &event_id, &payout_token, 0);

        events::emit_revenue_withdrawn(
            &env,
            event_id.clone(),
            stored_organizer.clone(),
            organizer_amount,
            payout_token,
            stored_organizer,
            &storage::get_emission_privacy(&env, &event_id),
        );

        Ok(())
    }

    pub fn get_event_payments(env: Env, event_id: Symbol) -> soroban_sdk::Vec<u64> {
        storage::get_event_payments(&env, &event_id)
    }

    pub fn get_payments_by_event(env: Env, event_id: Symbol) -> soroban_sdk::Vec<PaymentRecord> {
        let payment_ids = storage::get_event_payments(&env, &event_id);
        let mut payments = soroban_sdk::Vec::new(&env);
        for id in payment_ids {
            if let Ok(payment) = storage::get_payment(&env, id) {
                payments.push_back(payment);
            }
        }
        payments
    }

    pub fn get_payments_by_user(env: Env, user: Address) -> soroban_sdk::Vec<PaymentRecord> {
        let payment_ids = storage::get_payer_payments(&env, &user);
        let mut payments = soroban_sdk::Vec::new(&env);
        for id in payment_ids {
            if let Ok(payment) = storage::get_payment(&env, id) {
                payments.push_back(payment);
            }
        }
        payments
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
        require_not_paused(&env)?;
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
        require_not_paused(&env)?;
        let mut meta = storage::get_escrow_meta(&env, &event_id)?;

        if meta.auto_released {
            return Err(PaymentError::EscrowAlreadyReleased);
        }

        if env.ledger().timestamp() < meta.event_end_time {
            return Err(PaymentError::EscrowNotExpired);
        }

        let tokens = storage::get_event_tokens(&env, &event_id);
        let mut total = 0i128;

        for i in 0..tokens.len() {
            if let Some(token_address) = tokens.get(i) {
                let (token_total, to_release) =
                    collect_held_payments_for_token(&env, &event_id, &token_address)?;
                if token_total > 0 {
                    let token_client = token::Client::new(&env, &token_address);
                    token_client.transfer(
                        &env.current_contract_address(),
                        &meta.organizer,
                        &token_total,
                    );

                    for j in 0..to_release.len() {
                        if let Some(mut payment) = to_release.get(j) {
                            payment.status = PaymentStatus::Released;
                            storage::update_payment(&env, &payment)?;
                        }
                    }

                    storage::set_event_token_revenue(&env, &event_id, &token_address, 0);
                    total += token_total;
                }
            }
        }

        meta.auto_released = true;
        storage::set_escrow_meta(&env, &event_id, &meta);

        events::emit_escrow_auto_released(&env, event_id, meta.organizer, total);

        Ok(())
    }

    /// Withdraw revenue for an event. Deducts platform fee and sends the rest
    /// to the specified address. Platform fees are accumulated for later
    /// withdrawal by the admin.
    pub fn withdraw_revenue(env: Env, event_id: Symbol, to: Address) -> Result<(), PaymentError> {
        require_not_paused(&env)?;
        let admin = storage::get_admin(&env)?;
        admin.require_auth();

        let token_address = storage::get_accepted_token(&env)?;
        let revenue = storage::get_event_token_revenue(&env, &event_id, &token_address);
        if revenue <= 0 {
            return Err(PaymentError::InvalidAmount);
        }

        // Calculate platform fee
        let fee_bps = storage::get_platform_fee_bps(&env) as i128;
        let fee_amount = revenue * fee_bps / 10_000;
        let organizer_amount = revenue - fee_amount;

        let token_client = token::Client::new(&env, &token_address);

        // Transfer organizer share
        token_client.transfer(&env.current_contract_address(), &to, &organizer_amount);

        // Accumulate platform revenue if there is a fee
        if fee_amount > 0 {
            storage::add_platform_revenue(&env, &event_id, fee_amount);
            events::emit_platform_fee_collected(
                &env,
                event_id.clone(),
                fee_amount,
                organizer_amount,
                token_address.clone(),
            );
        }

        // Release payments
        let payment_ids = storage::get_event_payments(&env, &event_id);
        for i in 0..payment_ids.len() {
            let pid = payment_ids.get(i).ok_or(PaymentError::PaymentNotFound)?;
            let mut payment = storage::get_payment(&env, pid)?;
            if payment.status == PaymentStatus::Held && payment.token == token_address {
                payment.status = PaymentStatus::Released;
                storage::update_payment(&env, &payment)?;
            }
        }

        // Update revenue tracking
        storage::set_event_token_revenue(&env, &event_id, &token_address, 0);

        // Record withdrawal history
        let record = WithdrawalRecord {
            amount: organizer_amount,
            timestamp: env.ledger().timestamp(),
            organizer: to.clone(),
        };
        storage::add_withdrawal_record(&env, &event_id, &record);

        events::emit_revenue_withdrawn(
            &env,
            event_id.clone(),
            to.clone(),
            organizer_amount,
            token_address,
            to,
            &storage::get_emission_privacy(&env, &event_id),
        );

        Ok(())
    }

    /// Get all withdrawal history for an event.
    pub fn get_withdrawal_history(
        env: Env,
        event_id: Symbol,
    ) -> soroban_sdk::Vec<WithdrawalRecord> {
        storage::get_withdrawal_history(&env, &event_id)
    }

    /// Update the platform fee (admin only). Fee is in basis points (0-10000).
    pub fn set_platform_fee(env: Env, fee_bps: u32, wallet: Address) -> Result<(), PaymentError> {
        require_not_paused(&env)?;
        let admin = storage::get_admin(&env)?;
        admin.require_auth();

        if fee_bps > 10_000 {
            return Err(PaymentError::InvalidFeeBps);
        }

        let old_bps = storage::get_platform_fee_bps(&env);
        storage::set_platform_fee_bps(&env, fee_bps);
        storage::set_platform_wallet(&env, &wallet);

        events::emit_platform_fee_updated(&env, admin, old_bps, fee_bps);

        Ok(())
    }

    /// Get the current platform fee in basis points.
    pub fn get_platform_fee_bps(env: Env) -> u32 {
        storage::get_platform_fee_bps(&env)
    }

    /// Get the accumulated platform revenue for an event.
    pub fn get_platform_revenue(env: Env, event_id: Symbol) -> i128 {
        storage::get_platform_revenue(&env, &event_id)
    }

    /// Withdraw accumulated platform fees for an event (admin only).
    /// Sends fees to the configured platform wallet.
    pub fn withdraw_platform_revenue(env: Env, event_id: Symbol) -> Result<(), PaymentError> {
        require_not_paused(&env)?;
        let admin = storage::get_admin(&env)?;
        admin.require_auth();

        let platform_revenue = storage::get_platform_revenue(&env, &event_id);
        if platform_revenue <= 0 {
            return Err(PaymentError::NoPlatformRevenue);
        }

        let platform_wallet = storage::get_platform_wallet(&env)?;
        let token_address = storage::get_accepted_token(&env)?;
        let token_client = token::Client::new(&env, &token_address);

        token_client.transfer(
            &env.current_contract_address(),
            &platform_wallet,
            &platform_revenue,
        );

        storage::reset_platform_revenue(&env, &event_id);

        events::emit_platform_revenue_withdrawn(
            &env,
            event_id,
            platform_revenue,
            token_address,
            platform_wallet,
        );

        Ok(())
    }

    /// Set the privacy level for event emissions. Admin only.
    pub fn set_event_privacy(
        env: Env,
        admin: Address,
        event_id: Symbol,
        level: PrivacyLevel,
    ) -> Result<(), PaymentError> {
        require_not_paused(&env)?;
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

    /// Get the current contract version.
    pub fn contract_version(env: Env) -> u32 {
        storage::get_contract_version(&env)
    }

    /// Migrate the contract to a new version. Only admin can call this.
    pub fn migrate(env: Env, admin: Address) -> Result<u32, PaymentError> {
        require_not_paused(&env)?;
        admin.require_auth();

        let current_admin = storage::get_admin(&env)?;
        if current_admin != admin {
            return Err(PaymentError::Unauthorized);
        }

        let current_version = storage::get_contract_version(&env);
        let new_version = current_version + 1;

        // Perform any necessary migrations based on version transitions
        match current_version {
            0 => {
                // First migration: initialize version tracking
                storage::set_contract_version(&env, 1);
            }
            1 => {
                // Future migrations can be added here
                storage::set_contract_version(&env, 2);
            }
            2 => {
                // v2 -> v3 migration
                storage::set_contract_version(&env, 3);
            }
            _ => {
                return Err(PaymentError::UnsupportedVersion);
            }
        }

        Ok(new_version)
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
        require_not_paused(&env)?;
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
            organizer.clone(),
            total,
            token_address,
            organizer,
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
        require_not_paused(&env)?;
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
                        total += payment.amount - payment.refunded_amount;
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
                        token_address.clone(),
                        organizer.clone(),
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
