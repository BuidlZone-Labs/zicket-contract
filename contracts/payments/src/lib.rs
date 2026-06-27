#![no_std]
#[cfg(test)]
extern crate std;
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

// Minimum dispute window (in ledgers) that must pass after cancellation before organizer can withdraw
const MIN_DISPUTE_WINDOW_LEDGERS: u32 = 100;

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
    zk_email_commitment: Option<BytesN<32>>,
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

fn validate_revenue_invariant(env: &Env, event_id: &Symbol) -> Result<(), PaymentError> {
    if let Some(EventStatus::Cancelled) = storage::get_event_status(env, event_id) {
        return Ok(());
    }

    let payment_ids = storage::get_event_payments(env, event_id);
    let mut total_payments: i128 = 0;
    let mut total_refunds: i128 = 0;

    for i in 0..payment_ids.len() {
        if let Some(pid) = payment_ids.get(i) {
            if let Ok(payment) = storage::get_payment(env, pid) {
                total_payments += payment.amount;
                if payment.status == PaymentStatus::Refunded {
                    total_refunds += payment.amount;
                }
            }
        }
    }

    let current_revenue = storage::get_event_revenue(env, event_id);
    let withdrawal_history = storage::get_withdrawal_history(env, event_id);
    let mut total_withdrawn: i128 = 0;
    for i in 0..withdrawal_history.len() {
        if let Some(record) = withdrawal_history.get(i) {
            total_withdrawn += record.amount;
        }
    }

    let platform_revenue = storage::get_platform_revenue(env, event_id);

    if total_payments != current_revenue + total_refunds + total_withdrawn + platform_revenue {
        return Err(PaymentError::AccountingMismatch);
    }

    // Verify token revenue sum matches Held items
    let tokens = storage::get_event_tokens(env, event_id);
    for i in 0..tokens.len() {
        if let Some(token) = tokens.get(i) {
            let mut expected_token_revenue: i128 = 0;
            for j in 0..payment_ids.len() {
                if let Some(pid) = payment_ids.get(j) {
                    if let Ok(payment) = storage::get_payment(env, pid) {
                        if payment.token == token && payment.status == PaymentStatus::Held {
                            expected_token_revenue += payment.amount;
                        }
                    }
                }
            }
            if storage::get_event_token_revenue(env, event_id, &token) != expected_token_revenue {
                return Err(PaymentError::AccountingMismatch);
            }
        }
    }

    let mut expected_event_revenue: i128 = 0;
    for i in 0..payment_ids.len() {
        if let Some(pid) = payment_ids.get(i) {
            if let Ok(payment) = storage::get_payment(env, pid) {
                if payment.status == PaymentStatus::Held {
                    expected_event_revenue += payment.amount;
                }
            }
        }
    }
    if storage::get_event_revenue(env, event_id) != expected_event_revenue {
        return Err(PaymentError::AccountingMismatch);
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
        if matches!(
            status,
            EventStatus::Completed | EventStatus::Cancelled | EventStatus::Postponed
        ) {
            return Err(PaymentError::EventNotActive);
        }
    }

    let contract_address = env.current_contract_address();

    let token_client = token::Client::new(&env, &params.token_address);
    token_client
        .try_transfer(&params.payer, &contract_address, &params.amount)
        .map_err(|_| PaymentError::TransferFailed)?
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
        refunded_amount: 0,
        // Stored, never emitted. Binds the payment to an off-chain email target
        // without revealing the address.
        zk_email_commitment: params.zk_email_commitment.clone(),
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

/// Reject legacy single-organizer withdrawal paths for events that carry a
/// revenue split. Split events must settle through `withdraw_split` so that the
/// platform fee is deducted once and each recipient is paid exactly their share.
fn ensure_no_splits(env: &Env, event_id: &Symbol) -> Result<(), PaymentError> {
    if storage::has_splits(env, event_id) {
        return Err(PaymentError::InvalidSplitConfig);
    }
    Ok(())
}

/// Look up a recipient's basis-point allocation within a split configuration.
fn find_split_bps(splits: &soroban_sdk::Vec<RevenueSplit>, who: &Address) -> Option<u32> {
    for i in 0..splits.len() {
        if let Some(split) = splits.get(i) {
            if split.recipient == *who {
                return Some(split.basis_points);
            }
        }
    }
    None
}

/// Compute a recipient's payout from the frozen net-distributable amount.
///
/// Non-primary recipients receive `floor(net * bps / 10000)`. The primary
/// organizer (index 0) receives the remainder, so integer-division dust is never
/// stranded and the sum of all shares always equals `net`.
fn recipient_share(splits: &soroban_sdk::Vec<RevenueSplit>, who: &Address, net: i128) -> i128 {
    let primary = match splits.get(0) {
        Some(s) => s.recipient,
        None => return 0,
    };

    if *who == primary {
        let mut others_total: i128 = 0;
        for i in 1..splits.len() {
            if let Some(split) = splits.get(i) {
                others_total += net * (split.basis_points as i128) / 10_000;
            }
        }
        net - others_total
    } else {
        match find_split_bps(splits, who) {
            Some(bps) => net * (bps as i128) / 10_000,
            None => 0,
        }
    }
}

/// Settle a split event exactly once, returning the frozen net-distributable
/// snapshot. Mirrors the status/timing rules of [`PaymentsContract::withdraw`]:
/// completed events honour the withdrawal delay, cancelled events honour the
/// dispute window and the time-based withdrawable ratio. The platform fee is
/// deducted here, before any recipient share is computed.
fn ensure_split_settled(env: &Env, event_id: &Symbol) -> Result<SplitSettlement, PaymentError> {
    if let Some(settlement) = storage::get_split_settlement(env, event_id) {
        return Ok(settlement);
    }

    let config = storage::get_event_config(env, event_id).ok_or(PaymentError::InvalidOrganizer)?;
    let mut withdrawable_ratio_bps = 10_000u32;
    let current_ledger = env.ledger().sequence();

    match storage::get_event_status(env, event_id) {
        Some(EventStatus::Completed) => {
            let unlock_ledger = config.event_end_ledger
                + config.withdrawal_delay_ledgers
                + config.admin_delay_extension_ledgers;
            if current_ledger < unlock_ledger {
                return Err(PaymentError::EscrowNotExpired);
            }
        }
        Some(EventStatus::Cancelled) => {
            if let Some(cancel_ledger) = config.cancel_ledger {
                if current_ledger < cancel_ledger + MIN_DISPUTE_WINDOW_LEDGERS {
                    return Err(PaymentError::EscrowNotExpired);
                }
            } else {
                return Err(PaymentError::EventNotCompleted);
            }

            match config.withdrawable_ratio_bps {
                Some(0) => return Err(PaymentError::NoRevenue),
                Some(ratio) => withdrawable_ratio_bps = ratio,
                None => return Err(PaymentError::EventNotCompleted),
            }
        }
        _ => return Err(PaymentError::EventNotCompleted),
    }

    validate_revenue_invariant(env, event_id)?;

    let payout_token = storage::get_event_payout_token(env, event_id)?;
    let (total, payments_to_release) =
        collect_held_payments_for_token(env, event_id, &payout_token)?;
    if total <= 0 {
        return Err(PaymentError::NoRevenue);
    }

    let total_to_withdraw = total * (withdrawable_ratio_bps as i128) / 10_000;
    if total_to_withdraw <= 0 {
        return Err(PaymentError::NoRevenue);
    }

    let fee_bps = storage::get_platform_fee_bps(env) as i128;
    let fee_amount = total_to_withdraw * fee_bps / 10_000;
    let net = total_to_withdraw - fee_amount;
    if net <= 0 {
        return Err(PaymentError::NoRevenue);
    }

    // Move the distributable funds out of the held-payment accounting. For a full
    // (non-cancelled) settlement we release every held payment; for a partial
    // (cancelled) settlement we only reduce the revenue counters and leave the
    // remainder Held so attendees can still claim their pro-rata refunds.
    if withdrawable_ratio_bps == 10_000 {
        for i in 0..payments_to_release.len() {
            let mut payment = payments_to_release
                .get(i)
                .ok_or(PaymentError::PaymentNotFound)?;
            payment.status = PaymentStatus::Released;
            storage::update_payment(env, &payment)?;
        }
        storage::set_event_token_revenue(env, event_id, &payout_token, 0);
    } else {
        let current_token_rev = storage::get_event_token_revenue(env, event_id, &payout_token);
        storage::set_event_token_revenue(
            env,
            event_id,
            &payout_token,
            current_token_rev - total_to_withdraw,
        );
        let current_rev = storage::get_event_revenue(env, event_id);
        storage::set_event_revenue(env, event_id, current_rev - total_to_withdraw);
    }

    if fee_amount > 0 {
        storage::add_platform_revenue(env, event_id, fee_amount);
        events::emit_platform_fee_collected(
            env,
            event_id.clone(),
            fee_amount,
            net,
            payout_token.clone(),
        );
    }

    let settlement = SplitSettlement {
        token: payout_token,
        net_distributable: net,
    };
    storage::set_split_settlement(env, event_id, &settlement);

    Ok(settlement)
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
                zk_email_commitment: None,
            },
        )
    }

    /// Pay for a ticket and bind a zkEmail receipt commitment in the same call.
    ///
    /// `zk_email_commitment` is an optional salted hash of the buyer's email
    /// (e.g. `H(email || ticket_id)`) computed off-chain. It is stored on the
    /// payment record and never emitted; the raw email never touches the chain.
    /// Pass `None` to behave exactly like [`PaymentsContract::pay_for_ticket`]
    /// (fully anonymous attendees).
    #[allow(clippy::too_many_arguments)]
    pub fn pay_for_ticket_with_commitment(
        env: Env,
        nonce: u64,
        payer: Address,
        event_id: Symbol,
        amount: i128,
        email_hash: Option<BytesN<32>>,
        token_address: Address,
        privacy_level: PaymentPrivacy,
        zk_email_commitment: Option<BytesN<32>>,
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
                zk_email_commitment,
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
                zk_email_commitment: None,
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
        event_start_ledger: u32,
        event_end_ledger: u32,
        withdrawal_delay_ledgers: u32,
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

        let (
            existing_sold,
            existing_admin_delay,
            existing_cancel,
            existing_ratio,
            existing_withdrawn,
        ) = if let Some(existing_config) = storage::get_event_config(&env, &event_id) {
            if existing_config.organizer != organizer {
                return Err(PaymentError::InvalidOrganizer);
            }
            if existing_config.payout_token != payout_token {
                return Err(PaymentError::InvalidPayoutToken);
            }
            (
                existing_config.sold_count,
                existing_config.admin_delay_extension_ledgers,
                existing_config.cancel_ledger,
                existing_config.withdrawable_ratio_bps,
                existing_config.organizer_withdrawn,
            )
        } else {
            (0, 0, None, None, false)
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
                sold_count: existing_sold,
                event_start_ledger,
                event_end_ledger,
                withdrawal_delay_ledgers,
                admin_delay_extension_ledgers: existing_admin_delay,
                cancel_ledger: existing_cancel,
                withdrawable_ratio_bps: existing_ratio,
                organizer_withdrawn: existing_withdrawn,
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
        ensure_no_splits(&env, &event_id)?;

        let stored_organizer = storage::get_event_organizer(&env, &event_id)?;
        if organizer != stored_organizer {
            return Err(PaymentError::UnauthorizedWithdrawal);
        }

        let mut config =
            storage::get_event_config(&env, &event_id).ok_or(PaymentError::InvalidOrganizer)?;
        if config.organizer_withdrawn {
            return Err(PaymentError::NoRevenue);
        }

        let mut withdrawable_ratio_bps = 10000u32;
        let current_ledger = env.ledger().sequence();

        match storage::get_event_status(&env, &event_id) {
            Some(EventStatus::Completed) => {
                let unlock_ledger = config.event_end_ledger
                    + config.withdrawal_delay_ledgers
                    + config.admin_delay_extension_ledgers;
                if current_ledger < unlock_ledger {
                    return Err(PaymentError::EscrowNotExpired); // EscrowNotExpired makes sense here
                }
            }
            Some(EventStatus::Cancelled) => {
                // Check dispute window: must wait at least MIN_DISPUTE_WINDOW_LEDGERS after cancellation
                if let Some(cancel_ledger) = config.cancel_ledger {
                    let min_dispute_unlock = cancel_ledger + MIN_DISPUTE_WINDOW_LEDGERS;
                    if current_ledger < min_dispute_unlock {
                        return Err(PaymentError::EscrowNotExpired);
                    }
                } else {
                    // Should never happen if event is Cancelled, but be defensive
                    return Err(PaymentError::EventNotCompleted);
                }

                if let Some(ratio) = config.withdrawable_ratio_bps {
                    if ratio == 0 {
                        return Err(PaymentError::NoRevenue);
                    }
                    withdrawable_ratio_bps = ratio;
                } else {
                    return Err(PaymentError::EventNotCompleted);
                }
            }
            _ => return Err(PaymentError::EventNotCompleted),
        }

        validate_revenue_invariant(&env, &event_id)?;

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

        let total_to_withdraw = total * (withdrawable_ratio_bps as i128) / 10000;
        if total_to_withdraw <= 0 {
            return Err(PaymentError::NoRevenue);
        }

        let token_client = token::Client::new(&env, &payout_token);

        let fee_bps = storage::get_platform_fee_bps(&env) as i128;
        let fee_amount = total_to_withdraw * fee_bps / 10_000;
        let organizer_amount = total_to_withdraw - fee_amount;

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

        if withdrawable_ratio_bps == 10000 {
            for i in 0..payments_to_release.len() {
                let mut payment = payments_to_release
                    .get(i)
                    .ok_or(PaymentError::PaymentNotFound)?;
                payment.status = PaymentStatus::Released;
                storage::update_payment(&env, &payment)?;
                let current_token_rev =
                    storage::get_event_token_revenue(&env, &event_id, &payment.token);
                storage::set_event_token_revenue(
                    &env,
                    &event_id,
                    &payment.token,
                    current_token_rev - payment.amount,
                );
            }
            storage::set_event_token_revenue(&env, &event_id, &payout_token, 0);
        } else {
            let current_token_rev =
                storage::get_event_token_revenue(&env, &event_id, &payout_token);
            storage::set_event_token_revenue(
                &env,
                &event_id,
                &payout_token,
                current_token_rev - total_to_withdraw,
            );

            let current_rev = storage::get_event_revenue(&env, &event_id);
            storage::set_event_revenue(&env, &event_id, current_rev - total_to_withdraw);
        }

        config.organizer_withdrawn = true;
        storage::set_event_config(&env, &event_id, &config);

        let record = WithdrawalRecord {
            amount: total,
            timestamp: env.ledger().timestamp(),
            organizer: stored_organizer.clone(),
        };
        storage::add_withdrawal_record(&env, &event_id, &record);

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

    /// Admin can extend the withdrawal delay.
    pub fn extend_withdrawal_delay(
        env: Env,
        admin: Address,
        event_id: Symbol,
        additional_ledgers: u32,
    ) -> Result<(), PaymentError> {
        let stored_admin = storage::get_admin(&env)?;
        if admin != stored_admin {
            return Err(PaymentError::Unauthorized);
        }
        admin.require_auth();

        let mut config =
            storage::get_event_config(&env, &event_id).ok_or(PaymentError::InvalidOrganizer)?;
        config.admin_delay_extension_ledgers += additional_ledgers;
        storage::set_event_config(&env, &event_id, &config);
        Ok(())
    }

    /// Handle event cancellation from the event contract.
    pub fn cancel_event(
        env: Env,
        event_id: Symbol,
        organizer: Address,
    ) -> Result<(), PaymentError> {
        let event_contract = storage::get_event_contract(&env)?;
        event_contract.require_auth();

        let mut config =
            storage::get_event_config(&env, &event_id).ok_or(PaymentError::InvalidOrganizer)?;
        if config.organizer != organizer {
            return Err(PaymentError::Unauthorized);
        }

        let current_ledger = env.ledger().sequence();
        config.cancel_ledger = Some(current_ledger);

        if current_ledger < config.event_start_ledger {
            config.withdrawable_ratio_bps = Some(0);
        } else if current_ledger >= config.event_end_ledger {
            config.withdrawable_ratio_bps = Some(10000);
        } else {
            let elapsed = current_ledger - config.event_start_ledger;
            let total = config.event_end_ledger - config.event_start_ledger;
            if total == 0 {
                config.withdrawable_ratio_bps = Some(10000);
            } else {
                let ratio = (elapsed as u64 * 10000 / total as u64) as u32;
                config.withdrawable_ratio_bps = Some(ratio);
            }
        }

        storage::set_event_config(&env, &event_id, &config);
        storage::set_event_status(&env, &event_id, &EventStatus::Cancelled);
        Ok(())
    }

    /// Claim refund for a cancelled event.
    pub fn claim_refund(env: Env, payer: Address, payment_id: u64) -> Result<(), PaymentError> {
        payer.require_auth();

        let mut payment = storage::get_payment(&env, payment_id)?;
        if payment.payer != payer {
            return Err(PaymentError::Unauthorized);
        }
        if payment.status != PaymentStatus::Held {
            return Err(PaymentError::PaymentAlreadyProcessed);
        }

        let status = storage::get_event_status(&env, &payment.event_id);
        if status != Some(EventStatus::Cancelled) {
            return Err(PaymentError::EventNotActive); // Or appropriate error
        }

        let config = storage::get_event_config(&env, &payment.event_id)
            .ok_or(PaymentError::InvalidOrganizer)?;
        let withdrawable_ratio_bps = config.withdrawable_ratio_bps.unwrap_or(0);
        let refund_ratio_bps = 10000 - withdrawable_ratio_bps;
        if refund_ratio_bps == 0 {
            return Err(PaymentError::NoRevenue);
        }

        let max_refund = payment.amount * (refund_ratio_bps as i128) / 10000;
        let remaining = max_refund - payment.refunded_amount;

        if remaining <= 0 {
            return Err(PaymentError::InvalidAmount);
        }

        let token_client = token::Client::new(&env, &payment.token);
        token_client.transfer(&env.current_contract_address(), &payment.payer, &remaining);

        payment.refunded_amount += remaining;
        payment.status = PaymentStatus::Refunded;
        storage::update_payment(&env, &payment)?;

        let revenue = storage::get_event_revenue(&env, &payment.event_id);
        storage::set_event_revenue(&env, &payment.event_id, revenue - remaining);

        let token_revenue =
            storage::get_event_token_revenue(&env, &payment.event_id, &payment.token);
        storage::set_event_token_revenue(
            &env,
            &payment.event_id,
            &payment.token,
            token_revenue - remaining,
        );

        events::emit_payment_refunded(
            &env,
            payment_id,
            payment.event_id.clone(),
            payment.payer,
            remaining,
            payment.token.clone(),
            &storage::get_emission_privacy(&env, &payment.event_id),
        );

        Ok(())
    }

    /// Freeze escrow for a postponed event and open the refund-choice window.
    ///
    /// Called by the event contract as part of `event::postpone_event`. Sets the
    /// payments-side status to `Postponed` (which blocks every withdrawal path —
    /// `withdraw`, `withdraw_token`, `withdraw_all_tokens` all reject any non-
    /// `Completed`/`Cancelled` status) and records the choice-window deadline used
    /// by `request_postponement_refund`.
    pub fn postpone_event(
        env: Env,
        event_id: Symbol,
        organizer: Address,
        choice_deadline_ledger: u32,
    ) -> Result<(), PaymentError> {
        require_not_paused(&env)?;
        let event_contract = storage::get_event_contract(&env)?;
        event_contract.require_auth();

        let config =
            storage::get_event_config(&env, &event_id).ok_or(PaymentError::InvalidOrganizer)?;
        if config.organizer != organizer {
            return Err(PaymentError::Unauthorized);
        }

        storage::set_event_status(&env, &event_id, &EventStatus::Postponed);
        storage::set_postpone_deadline(&env, &event_id, choice_deadline_ledger);
        Ok(())
    }

    /// Resume a postponed event back to `Active` once its choice window has closed.
    ///
    /// Called by the event contract as part of `event::finalize_postponement`.
    /// Clears the choice-window deadline so the refund path is no longer open.
    pub fn resume_event(
        env: Env,
        event_id: Symbol,
        organizer: Address,
    ) -> Result<(), PaymentError> {
        require_not_paused(&env)?;
        let event_contract = storage::get_event_contract(&env)?;
        event_contract.require_auth();

        let config =
            storage::get_event_config(&env, &event_id).ok_or(PaymentError::InvalidOrganizer)?;
        if config.organizer != organizer {
            return Err(PaymentError::Unauthorized);
        }

        if storage::get_event_status(&env, &event_id) != Some(EventStatus::Postponed) {
            return Err(PaymentError::EventNotPostponed);
        }

        storage::set_event_status(&env, &event_id, &EventStatus::Active);
        storage::remove_postpone_deadline(&env, &event_id);
        Ok(())
    }

    /// Opt out of a postponed event in exchange for a full refund.
    ///
    /// Callable only by the linked event contract, which orchestrates the full
    /// opt-out (refund here, then registration + ticket revocation on its side) so
    /// a refunded holder cannot also attend the resumed event. `caller` is the
    /// ticket owner on whose behalf the event contract is acting; ownership is
    /// verified here. Refunds 100% of the held amount.
    pub fn request_postponement_refund(
        env: Env,
        caller: Address,
        ticket_id: u64,
    ) -> Result<(), PaymentError> {
        require_not_paused(&env)?;
        let event_contract = storage::get_event_contract(&env)?;
        event_contract.require_auth();

        let ticket = storage::get_ticket(&env, ticket_id)?;
        if ticket.owner != caller {
            return Err(PaymentError::Unauthorized);
        }

        let mut payment = storage::get_payment(&env, ticket.payment_id)?;
        if payment.status == PaymentStatus::Refunded {
            return Err(PaymentError::PaymentAlreadyRefunded);
        }
        if payment.status != PaymentStatus::Held {
            return Err(PaymentError::PaymentAlreadyProcessed);
        }

        if storage::get_event_status(&env, &payment.event_id) != Some(EventStatus::Postponed) {
            return Err(PaymentError::EventNotPostponed);
        }

        let deadline = storage::get_postpone_deadline(&env, &payment.event_id)
            .ok_or(PaymentError::EventNotPostponed)?;
        if env.ledger().sequence() > deadline {
            return Err(PaymentError::PostponementWindowClosed);
        }

        let refund_amt = payment.amount - payment.refunded_amount;
        if refund_amt <= 0 {
            return Err(PaymentError::InvalidAmount);
        }

        let token_client = token::Client::new(&env, &payment.token);
        token_client.transfer(&env.current_contract_address(), &payment.payer, &refund_amt);

        payment.refunded_amount += refund_amt;
        payment.status = PaymentStatus::Refunded;
        storage::update_payment(&env, &payment)?;

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
            payment.payment_id,
            payment.event_id.clone(),
            payment.payer.clone(),
            refund_amt,
            payment.token.clone(),
            &storage::get_emission_privacy(&env, &payment.event_id),
        );

        Ok(())
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
        ensure_no_splits(&env, &event_id)?;
        let mut meta = storage::get_escrow_meta(&env, &event_id)?;

        if meta.auto_released {
            return Err(PaymentError::EscrowAlreadyReleased);
        }

        // Escrow is frozen while the event is postponed (refund-choice window open).
        if storage::get_event_status(&env, &event_id) == Some(EventStatus::Postponed) {
            return Err(PaymentError::EventNotActive);
        }

        if env.ledger().timestamp() < meta.event_end_time {
            return Err(PaymentError::EscrowNotExpired);
        }

        // When the event has a ledger schedule (set/rescheduled via the event
        // contract), the timestamp-based escrow metadata is not authoritative on its
        // own: a postponed-then-resumed event carries a stale `event_end_time`. Also
        // require the (possibly rescheduled) ledger end to have passed so escrow
        // cannot auto-release before the rescheduled event actually ends. Events that
        // only use the legacy timestamp escrow (no config) are unaffected.
        if let Some(config) = storage::get_event_config(&env, &event_id) {
            if env.ledger().sequence() < config.event_end_ledger {
                return Err(PaymentError::EscrowNotExpired);
            }
        }

        validate_revenue_invariant(&env, &event_id)?;

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

                    let current_event_revenue = storage::get_event_revenue(&env, &event_id);
                    storage::set_event_revenue(
                        &env,
                        &event_id,
                        current_event_revenue - token_total,
                    );

                    let record = WithdrawalRecord {
                        amount: token_total,
                        timestamp: env.ledger().timestamp(),
                        organizer: meta.organizer.clone(),
                    };
                    storage::add_withdrawal_record(&env, &event_id, &record);

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
        ensure_no_splits(&env, &event_id)?;

        // Escrow is frozen while the event is postponed (refund-choice window open).
        if storage::get_event_status(&env, &event_id) == Some(EventStatus::Postponed) {
            return Err(PaymentError::EventNotActive);
        }

        validate_revenue_invariant(&env, &event_id)?;

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

    /// Get the number of tickets a user has purchased for a specific event.
    ///
    /// This is an on-chain query that can be used to verify per-user purchase
    /// limits without relying on any off-chain data source or frontend logic.
    /// Returns 0 if the user has not purchased any tickets for the event.
    pub fn get_user_tickets(env: Env, event_id: Symbol, user: Address) -> u32 {
        storage::get_user_event_tickets(&env, &event_id, &user)
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
        ensure_no_splits(&env, &event_id)?;

        match storage::get_event_status(&env, &event_id) {
            Some(EventStatus::Completed) => {}
            _ => return Err(PaymentError::EventNotCompleted),
        }

        validate_revenue_invariant(&env, &event_id)?;

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
            let current_token_rev =
                storage::get_event_token_revenue(&env, &event_id, &payment.token);
            storage::set_event_token_revenue(
                &env,
                &event_id,
                &payment.token,
                current_token_rev - payment.amount,
            );
        }

        storage::set_event_token_revenue(&env, &event_id, &token_address, 0);

        let current_event_revenue = storage::get_event_revenue(&env, &event_id);
        storage::set_event_revenue(&env, &event_id, current_event_revenue - total);

        let record = WithdrawalRecord {
            amount: total,
            timestamp: env.ledger().timestamp(),
            organizer: organizer.clone(),
        };
        storage::add_withdrawal_record(&env, &event_id, &record);

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
        ensure_no_splits(&env, &event_id)?;

        match storage::get_event_status(&env, &event_id) {
            Some(EventStatus::Completed) => {}
            _ => return Err(PaymentError::EventNotCompleted),
        }

        validate_revenue_invariant(&env, &event_id)?;

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

                    let current_event_revenue = storage::get_event_revenue(&env, &event_id);
                    storage::set_event_revenue(&env, &event_id, current_event_revenue - total);

                    let record = WithdrawalRecord {
                        amount: total,
                        timestamp: env.ledger().timestamp(),
                        organizer: organizer.clone(),
                    };
                    storage::add_withdrawal_record(&env, &event_id, &record);
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

    // ── Revenue splits & co-host wallet management ────────────────────────────

    /// Register the revenue split for an event. Callable only by the linked event
    /// contract, and only once — splits are immutable for the life of the event.
    ///
    /// `splits` is `Vec<(Address, u32)>` where the `u32` is basis points. The
    /// basis points must sum to exactly 10000, there must be between 1 and 5
    /// recipients, no zero allocations, and no duplicate recipients. Index 0 is
    /// the primary organizer.
    pub fn sync_revenue_splits(
        env: Env,
        event_contract: Address,
        event_id: Symbol,
        splits: soroban_sdk::Vec<(Address, u32)>,
    ) -> Result<(), PaymentError> {
        require_not_paused(&env)?;
        if event_contract != storage::get_event_contract(&env)? {
            return Err(PaymentError::Unauthorized);
        }
        event_contract.require_auth();

        // Immutable: never overwrite an existing configuration.
        if storage::has_splits(&env, &event_id) {
            return Err(PaymentError::InvalidSplitConfig);
        }

        let len = splits.len();
        if len == 0 || len > 5 {
            return Err(PaymentError::InvalidSplitConfig);
        }

        let mut normalized: soroban_sdk::Vec<RevenueSplit> = soroban_sdk::Vec::new(&env);
        let mut total: u32 = 0;
        for i in 0..len {
            let (recipient, bps) = splits.get(i).ok_or(PaymentError::InvalidSplitConfig)?;
            if bps == 0 {
                return Err(PaymentError::InvalidSplitConfig);
            }
            total = total
                .checked_add(bps)
                .ok_or(PaymentError::InvalidSplitConfig)?;
            for j in 0..i {
                let (other, _) = splits.get(j).ok_or(PaymentError::InvalidSplitConfig)?;
                if other == recipient {
                    return Err(PaymentError::InvalidSplitConfig);
                }
            }
            normalized.push_back(RevenueSplit {
                recipient,
                basis_points: bps,
            });
        }
        if total != 10_000 {
            return Err(PaymentError::InvalidSplitConfig);
        }

        storage::set_splits(&env, &event_id, &normalized);
        Ok(())
    }

    /// Get the configured revenue split for an event as `Vec<(Address, u32)>`.
    pub fn get_revenue_splits(env: Env, event_id: Symbol) -> soroban_sdk::Vec<(Address, u32)> {
        let splits = storage::get_splits(&env, &event_id);
        let mut out: soroban_sdk::Vec<(Address, u32)> = soroban_sdk::Vec::new(&env);
        for i in 0..splits.len() {
            if let Some(split) = splits.get(i) {
                out.push_back((split.recipient, split.basis_points));
            }
        }
        out
    }

    /// Withdraw the caller's allocated share of a split event's revenue.
    ///
    /// Any configured recipient may call this independently. The first call
    /// settles the event (deducting the platform fee and freezing the
    /// net-distributable amount); subsequent calls simply pay out each
    /// recipient's frozen share. A flagged recipient cannot withdraw.
    pub fn withdraw_split(
        env: Env,
        recipient: Address,
        event_id: Symbol,
    ) -> Result<(), PaymentError> {
        require_not_paused(&env)?;
        recipient.require_auth();

        let splits = storage::get_splits(&env, &event_id);
        if splits.is_empty() {
            return Err(PaymentError::SplitsNotConfigured);
        }
        if find_split_bps(&splits, &recipient).is_none() {
            return Err(PaymentError::NotASplitRecipient);
        }
        if storage::is_split_flagged(&env, &event_id, &recipient) {
            return Err(PaymentError::RecipientFlagged);
        }
        if storage::get_split_withdrawn(&env, &event_id, &recipient) > 0 {
            return Err(PaymentError::SplitAlreadyWithdrawn);
        }

        let settlement = ensure_split_settled(&env, &event_id)?;
        let share = recipient_share(&splits, &recipient, settlement.net_distributable);
        if share <= 0 {
            return Err(PaymentError::NoRevenue);
        }

        let token_client = token::Client::new(&env, &settlement.token);
        token_client.transfer(&env.current_contract_address(), &recipient, &share);

        storage::set_split_withdrawn(&env, &event_id, &recipient, share);

        let record = WithdrawalRecord {
            amount: share,
            timestamp: env.ledger().timestamp(),
            organizer: recipient.clone(),
        };
        storage::add_withdrawal_record(&env, &event_id, &record);

        events::emit_revenue_withdrawn(
            &env,
            event_id.clone(),
            recipient.clone(),
            share,
            settlement.token,
            recipient,
            &storage::get_emission_privacy(&env, &event_id),
        );

        Ok(())
    }

    /// Flag a co-host wallet as compromised. Only the primary organizer (split
    /// index 0) may call this. The flagged recipient's share is frozen in escrow
    /// and cannot be withdrawn until the dispute is resolved. The primary
    /// organizer cannot flag itself, and an already-paid recipient cannot be
    /// flagged.
    pub fn flag_cohost(
        env: Env,
        primary_organizer: Address,
        event_id: Symbol,
        recipient: Address,
    ) -> Result<(), PaymentError> {
        require_not_paused(&env)?;

        let splits = storage::get_splits(&env, &event_id);
        if splits.is_empty() {
            return Err(PaymentError::SplitsNotConfigured);
        }
        let primary = splits
            .get(0)
            .ok_or(PaymentError::SplitsNotConfigured)?
            .recipient;
        if primary_organizer != primary {
            return Err(PaymentError::Unauthorized);
        }
        primary_organizer.require_auth();

        if recipient == primary {
            return Err(PaymentError::Unauthorized);
        }
        if find_split_bps(&splits, &recipient).is_none() {
            return Err(PaymentError::NotASplitRecipient);
        }
        if storage::get_split_withdrawn(&env, &event_id, &recipient) > 0 {
            return Err(PaymentError::SplitAlreadyWithdrawn);
        }

        storage::set_split_flagged(&env, &event_id, &recipient, true);
        events::emit_cohost_flagged(&env, event_id, recipient, primary_organizer);
        Ok(())
    }

    /// Resolve a flagged co-host's escrowed share (admin only).
    ///
    /// - `ReleaseToRecipient`: clears the flag so the recipient can withdraw.
    /// - `ReassignToPrimary`: settles the event if needed and transfers the
    ///   escrowed share to the primary organizer, marking the recipient as paid.
    pub fn resolve_flagged_share(
        env: Env,
        event_id: Symbol,
        recipient: Address,
        resolution: FlagResolution,
    ) -> Result<(), PaymentError> {
        require_not_paused(&env)?;
        let admin = storage::get_admin(&env)?;
        admin.require_auth();

        let splits = storage::get_splits(&env, &event_id);
        if splits.is_empty() {
            return Err(PaymentError::SplitsNotConfigured);
        }
        if find_split_bps(&splits, &recipient).is_none() {
            return Err(PaymentError::NotASplitRecipient);
        }
        if !storage::is_split_flagged(&env, &event_id, &recipient) {
            return Err(PaymentError::RecipientNotFlagged);
        }

        match resolution {
            FlagResolution::ReleaseToRecipient => {
                storage::set_split_flagged(&env, &event_id, &recipient, false);
                events::emit_flagged_share_resolved(&env, event_id, recipient, true, 0);
            }
            FlagResolution::ReassignToPrimary => {
                if storage::get_split_withdrawn(&env, &event_id, &recipient) > 0 {
                    return Err(PaymentError::SplitAlreadyWithdrawn);
                }
                let settlement = ensure_split_settled(&env, &event_id)?;
                let primary = splits
                    .get(0)
                    .ok_or(PaymentError::SplitsNotConfigured)?
                    .recipient;
                let share = recipient_share(&splits, &recipient, settlement.net_distributable);
                if share <= 0 {
                    return Err(PaymentError::NoRevenue);
                }

                let token_client = token::Client::new(&env, &settlement.token);
                token_client.transfer(&env.current_contract_address(), &primary, &share);

                // Mark the flagged recipient as paid so the funds cannot be
                // double-spent, and clear the flag.
                storage::set_split_withdrawn(&env, &event_id, &recipient, share);
                storage::set_split_flagged(&env, &event_id, &recipient, false);

                let record = WithdrawalRecord {
                    amount: share,
                    timestamp: env.ledger().timestamp(),
                    organizer: primary,
                };
                storage::add_withdrawal_record(&env, &event_id, &record);

                events::emit_flagged_share_resolved(&env, event_id, recipient, false, share);
            }
        }

        Ok(())
    }

    /// Whether a split recipient is currently flagged (share frozen in escrow).
    pub fn is_recipient_flagged(env: Env, event_id: Symbol, recipient: Address) -> bool {
        storage::is_split_flagged(&env, &event_id, &recipient)
    }


    /// Amount already paid out to a given split recipient for an event.
    pub fn get_split_withdrawn(env: Env, event_id: Symbol, recipient: Address) -> i128 {
        storage::get_split_withdrawn(&env, &event_id, &recipient)
    }

    // ── zkEmail receipt commitments ───────────────────────────────────────────

    /// Bind a zkEmail receipt commitment to an existing payment.
    ///
    /// This is the canonical path when the commitment is salted with the
    /// `ticket_id` (which is only known after the payment is created). The payer
    /// computes `commitment = H(email || ticket_id)` off-chain and binds it here.
    ///
    /// Rules:
    /// - Only the original payer may bind, and must authorize the call.
    /// - Commitments are write-once: a payment that already has one is rejected.
    /// - A refunded payment can no longer accept a commitment.
    /// - Only the salted hash is stored; the raw email never touches the chain
    ///   and the commitment value is never emitted.
    pub fn bind_email_commitment(
        env: Env,
        payer: Address,
        payment_id: u64,
        commitment: BytesN<32>,
    ) -> Result<(), PaymentError> {
        require_not_paused(&env)?;
        payer.require_auth();

        let mut payment = storage::get_payment(&env, payment_id)?;
        if payment.payer != payer {
            return Err(PaymentError::Unauthorized);
        }
        if payment.status == PaymentStatus::Refunded {
            return Err(PaymentError::CommitmentNotAllowed);
        }
        if payment.zk_email_commitment.is_some() {
            return Err(PaymentError::CommitmentAlreadySet);
        }

        payment.zk_email_commitment = Some(commitment);
        storage::update_payment(&env, &payment)?;

        events::emit_receipt_commitment_bound(&env, payment_id, payment.event_id);
        Ok(())
    }

    /// Read the zkEmail commitment stored for a payment, if any.
    ///
    /// Returns `Ok(None)` when the payer opted out. Returns
    /// `Err(PaymentError::PaymentNotFound)` for an invalid `payment_id`. An
    /// off-chain relayer reads this to learn the commitment, then recomputes
    /// `H(email || ticket_id)` from a claimed email to confirm delivery
    /// eligibility — without the email ever being exposed on-chain.
    pub fn get_payment_commitment(
        env: Env,
        payment_id: u64,
    ) -> Result<Option<BytesN<32>>, PaymentError> {
        let payment = storage::get_payment(&env, payment_id)?;
        Ok(payment.zk_email_commitment)
    }

    /// Verify a candidate commitment against the one stored for a payment.
    ///
    /// Lets a relayer prove delivery eligibility on-chain without revealing the
    /// email: it supplies a freshly recomputed commitment and gets a boolean.
    /// Returns `false` if the payment has no stored commitment.
    pub fn verify_email_commitment(
        env: Env,
        payment_id: u64,
        candidate: BytesN<32>,
    ) -> Result<bool, PaymentError> {
        let payment = storage::get_payment(&env, payment_id)?;
        Ok(payment.zk_email_commitment == Some(candidate))
    }
}

#[cfg(test)]
mod multi_token_test;
#[cfg(test)]
mod revenue_split_test;
#[cfg(test)]
mod receipt_commitment_test;
#[cfg(test)]
mod test;
