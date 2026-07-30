use crate::errors::PaymentError;
use crate::types::{
    EscrowMetadata, EventStatus, PaymentRecord, PrivacyLevel, RevenueSplit, SplitSettlement, Ticket,
};
use soroban_sdk::{contracttype, Address, BytesN, Env, Symbol, Vec};

const TTL_THRESHOLD: u32 = 60 * 60 * 24 * 30;
const TTL_BUMP: u32 = 60 * 60 * 24 * 30 * 2;
const CURRENT_VERSION: u32 = 1;

#[contracttype]
#[derive(Clone)]
pub struct EventPrivacyConfig {
    pub allow_anonymous: bool,
    pub requires_verification: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventConfig {
    pub organizer: Address,
    pub payout_token: Address,
    pub allow_anonymous: bool,
    pub requires_verification: bool,
    pub max_tickets_per_user: u32,
    pub max_supply: u32,
    pub sold_count: u32,
    pub event_start_ledger: u32,
    pub event_end_ledger: u32,
    pub withdrawal_delay_ledgers: u32,
    pub admin_delay_extension_ledgers: u32,
    pub cancel_ledger: Option<u32>,
    pub withdrawable_ratio_bps: Option<u32>,
    pub organizer_withdrawn: bool,
    pub resale_royalty_bps: u32,
    pub max_resale_price: Option<i128>,
    pub allow_free_ticket_transfer: bool,
}

#[contracttype]
pub enum DataKey {
    Admin,
    AcceptedToken,
    EventContract,
    EventPrivacy(Symbol),
    EventConfig(Symbol),
    Payment(u64),
    Ticket(u64),
    EventRevenue(Symbol),
    EventTokenRevenue(Symbol, Address),
    EventStatus(Symbol),
    /// Map-based: Individual event-payment relationship
    EventPayment(Symbol, u64),
    /// Map-based: Individual owner-ticket relationship
    OwnerTicket(Address, u64),
    /// Map-based: Individual payer-payment relationship
    PayerPayment(Address, u64),
    /// Map-based: Individual withdrawal record by event and index
    WithdrawalRecord(Symbol, u32),
    /// Map-based: Count of withdrawals for an event
    WithdrawalCount(Symbol),
    /// Map-based: Individual event-token relationship
    EventToken(Symbol, Address),
    /// Indexed storage for event tokens
    EventTokenIndex(Symbol, u32),
    EventTokenCount(Symbol),
    NextPaymentId,
    NextTicketId,
    PlatformFeeBps,
    PlatformWallet,
    PlatformRevenue(Symbol),
    EmissionPrivacy(Symbol),
    EscrowMeta(Symbol),
    ProcessedNonce(Address, u64),
    ContractVersion,
    UserEventTickets(Symbol, Address),
    Paused,
    /// Refund-choice deadline ledger for a postponed event.
    PostponeDeadline(Symbol),
    /// Configured revenue split recipients for an event (immutable once set).
    EventSplits(Symbol),
    /// Frozen net-distributable snapshot taken at first split settlement.
    SplitSettlement(Symbol),
    /// Amount already paid out to a given split recipient for an event.
    SplitWithdrawn(Symbol, Address),
    /// Whether a split recipient's share is frozen pending dispute resolution.
    SplitFlagged(Symbol, Address),
    ResaleListing(u64),
    TicketContract,
    /// Nonce replay-protection keyed by a privacy-preserving wallet hash
    /// (used for Private payments instead of the raw payer address).
    ProcessedNonceHash(BytesN<32>, u64),
    /// Per-user ticket counter keyed by a privacy-preserving wallet hash.
    UserEventTicketsHash(Symbol, BytesN<32>),
    /// Records a spent nullifier commitment to guarantee Anonymous-payment uniqueness.
    SpentNullifier(BytesN<32>),
    /// Dispute record keyed by ticket id.
    Dispute(u64),
    /// List of disputed ticket ids for an event.
    EventDisputes(Symbol),
    TotalPayments(Symbol),
    TotalRefunds(Symbol),
    TotalWithdrawn(Symbol),
    EventPaymentIndex(Symbol, u64),
    EventPaymentsCount(Symbol),
    PayerPaymentIndex(Address, u64),
    PayerPaymentsCount(Address),
    EventTokenVolume(Symbol, Address),
    /// Indexed storage for owner tickets
    OwnerTicketIndex(Address, u64),
    OwnerTicketsCount(Address),
    /// Legacy: Vector storage pattern (kept for migration compatibility only)
    /// @deprecated Only for migration - use EventPayment instead
    EventPayments(Symbol),
}

pub fn set_event_status(env: &Env, event_id: &Symbol, status: &EventStatus) {
    let key = DataKey::EventStatus(event_id.clone());
    env.storage().persistent().set(&key, status);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_event_status(env: &Env, event_id: &Symbol) -> Option<EventStatus> {
    let key = DataKey::EventStatus(event_id.clone());
    let status = env.storage()
        .persistent()
        .get(&key);
    if status.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    }
    status
}
pub fn set_postpone_deadline(env: &Env, event_id: &Symbol, deadline_ledger: u32) {
    let key = DataKey::PostponeDeadline(event_id.clone());
    env.storage().persistent().set(&key, &deadline_ledger);
    let current = env.ledger().sequence();
    let window = deadline_ledger.saturating_sub(current);
    let extend_to = window.saturating_add(TTL_THRESHOLD);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, extend_to.max(TTL_BUMP));
}
pub fn get_postpone_deadline(env: &Env, event_id: &Symbol) -> Option<u32> {
    env.storage()
        .persistent()
        .get(&DataKey::PostponeDeadline(event_id.clone()))
}
pub fn remove_postpone_deadline(env: &Env, event_id: &Symbol) {
    env.storage()
        .persistent()
        .remove(&DataKey::PostponeDeadline(event_id.clone()));
}
pub fn get_admin(env: &Env) -> Result<soroban_sdk::Address, PaymentError> {
    let key = DataKey::Admin;
    let admin = env.storage()
        .persistent()
        .get(&key)
        .ok_or(PaymentError::NotInitialized)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    Ok(admin)
}

pub fn set_admin(env: &Env, admin: &soroban_sdk::Address) {
    env.storage().persistent().set(&DataKey::Admin, admin);
    env.storage().persistent().extend_ttl(
        &DataKey::Admin,
        60 * 60 * 24 * 30,
        60 * 60 * 24 * 30 * 2,
    );
}

pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

pub fn set_paused(env: &Env, paused: bool) {
    env.storage().persistent().set(&DataKey::Paused, &paused);
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::Paused, TTL_THRESHOLD, TTL_BUMP);
}
pub fn get_accepted_token(env: &Env) -> Result<soroban_sdk::Address, PaymentError> {
    let key = DataKey::AcceptedToken;
    let token = env.storage()
        .persistent()
        .get(&key)
        .ok_or(PaymentError::NotInitialized)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    Ok(token)
}
pub fn set_accepted_token(env: &Env, token: &soroban_sdk::Address) {
    env.storage()
        .persistent()
        .set(&DataKey::AcceptedToken, token);
    env.storage().persistent().extend_ttl(
        &DataKey::AcceptedToken,
        60 * 60 * 24 * 30,
        60 * 60 * 24 * 30 * 2,
    );
}

pub fn get_event_contract(env: &Env) -> Result<soroban_sdk::Address, PaymentError> {
    let key = DataKey::EventContract;
    let contract = env.storage()
        .persistent()
        .get(&key)
        .ok_or(PaymentError::NotInitialized)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    Ok(contract)
}

pub fn set_event_contract(env: &Env, event_contract: &soroban_sdk::Address) {
    env.storage()
        .persistent()
        .set(&DataKey::EventContract, event_contract);
    env.storage().persistent().extend_ttl(
        &DataKey::EventContract,
        60 * 60 * 24 * 30,
        60 * 60 * 24 * 30 * 2,
    );
}

pub fn get_ticket_contract(env: &Env) -> Result<soroban_sdk::Address, PaymentError> {
    let key = DataKey::TicketContract;
    let contract = env.storage()
        .persistent()
        .get(&key)
        .ok_or(PaymentError::NotInitialized)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    Ok(contract)
}

pub fn set_ticket_contract(env: &Env, ticket_contract: &soroban_sdk::Address) {
    env.storage()
        .persistent()
        .set(&DataKey::TicketContract, ticket_contract);
    env.storage().persistent().extend_ttl(
        &DataKey::TicketContract,
        60 * 60 * 24 * 30,
        60 * 60 * 24 * 30 * 2,
    );
}

pub fn get_event_privacy(env: &Env, event_id: &Symbol) -> EventPrivacyConfig {
    env.storage()
        .persistent()
        .get(&DataKey::EventPrivacy(event_id.clone()))
        .unwrap_or(EventPrivacyConfig {
            allow_anonymous: true,
            requires_verification: false,
        })
}

pub fn set_event_privacy(env: &Env, event_id: &Symbol, privacy: &EventPrivacyConfig) {
    let key = DataKey::EventPrivacy(event_id.clone());
    env.storage().persistent().set(&key, privacy);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_event_config(env: &Env, event_id: &Symbol) -> Option<EventConfig> {
    let key = DataKey::EventConfig(event_id.clone());
    let config = env.storage()
        .persistent()
        .get(&key);
    if config.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    }
    config
}

pub fn set_event_config(env: &Env, event_id: &Symbol, config: &EventConfig) {
    let key = DataKey::EventConfig(event_id.clone());
    env.storage().persistent().set(&key, config);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_event_organizer(env: &Env, event_id: &Symbol) -> Result<Address, PaymentError> {
    get_event_config(env, event_id)
        .map(|config| config.organizer)
        .ok_or(PaymentError::InvalidOrganizer)
}

pub fn get_event_payout_token(env: &Env, event_id: &Symbol) -> Result<Address, PaymentError> {
    get_event_config(env, event_id)
        .map(|config| config.payout_token)
        .ok_or(PaymentError::InvalidPayoutToken)
}
pub fn is_initialized(env: &Env) -> bool {
    env.storage().persistent().has(&DataKey::Admin)
        && env.storage().persistent().has(&DataKey::AcceptedToken)
        && env.storage().persistent().has(&DataKey::EventContract)
}
pub fn get_next_payment_id(env: &Env) -> u64 {
    let current_id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::NextPaymentId)
        .unwrap_or(0);
    let next_id = current_id + 1;
    env.storage()
        .persistent()
        .set(&DataKey::NextPaymentId, &next_id);
    env.storage().persistent().extend_ttl(
        &DataKey::NextPaymentId,
        60 * 60 * 24 * 30,
        60 * 60 * 24 * 30 * 2,
    );
    next_id
}
pub fn get_next_ticket_id(env: &Env) -> u64 {
    let current_id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::NextTicketId)
        .unwrap_or(0);
    let next_id = current_id + 1;
    env.storage()
        .persistent()
        .set(&DataKey::NextTicketId, &next_id);
    env.storage().persistent().extend_ttl(
        &DataKey::NextTicketId,
        60 * 60 * 24 * 30,
        60 * 60 * 24 * 30 * 2,
    );
    next_id
}
pub fn save_payment(env: &Env, payment: &PaymentRecord) -> Result<(), PaymentError> {
    let key = DataKey::Payment(payment.payment_id);
    if env.storage().persistent().has(&key) {
        return Err(PaymentError::PaymentAlreadyProcessed);
    }
    env.storage().persistent().set(&key, payment);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    Ok(())
}
pub fn get_payment(env: &Env, payment_id: u64) -> Result<PaymentRecord, PaymentError> {
    let key = DataKey::Payment(payment_id);
    let payment = env.storage()
        .persistent()
        .get(&key)
        .ok_or(PaymentError::PaymentNotFound)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    Ok(payment)
}
pub fn save_ticket(env: &Env, ticket: &Ticket) -> Result<(), PaymentError> {
    let key = DataKey::Ticket(ticket.ticket_id);
    if env.storage().persistent().has(&key) {
        return Err(PaymentError::DuplicateRequest);
    }
    env.storage().persistent().set(&key, ticket);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    Ok(())
}
pub fn get_ticket(env: &Env, ticket_id: u64) -> Result<Ticket, PaymentError> {
    let key = DataKey::Ticket(ticket_id);
    let ticket = env.storage()
        .persistent()
        .get(&key)
        .ok_or(PaymentError::TicketNotFound)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    Ok(ticket)
}
/// Map-based: Add an owner-ticket relationship
pub fn add_owner_ticket_map(env: &Env, owner: &Address, ticket_id: u64) {
    // Get the current count which will be the index for this ticket
    let count = get_owner_tickets_count(env, owner);

    // Store the membership with the index as the value
    let key = DataKey::OwnerTicket(owner.clone(), ticket_id);
    env.storage().persistent().set(&key, &count);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);

    // Add to indexed list for retrieval
    let idx_key = DataKey::OwnerTicketIndex(owner.clone(), count);
    env.storage().persistent().set(&idx_key, &ticket_id);
    env.storage()
        .persistent()
        .extend_ttl(&idx_key, TTL_THRESHOLD, TTL_BUMP);

    let count_key = DataKey::OwnerTicketsCount(owner.clone());
    env.storage().persistent().set(&count_key, &(count + 1));
    env.storage()
        .persistent()
        .extend_ttl(&count_key, TTL_THRESHOLD, TTL_BUMP);
}

/// Map-based: Remove an owner-ticket relationship
pub fn remove_owner_ticket_map(env: &Env, owner: &Address, ticket_id: u64) {
    // Get the stored index from the membership entry
    let membership_key = DataKey::OwnerTicket(owner.clone(), ticket_id);
    let stored_index: Option<u64> = env.storage().persistent().get(&membership_key);

    // Remove the membership entry
    env.storage().persistent().remove(&membership_key);

    // Only proceed with index removal if the membership existed
    if let Some(idx) = stored_index {
        let count = get_owner_tickets_count(env, owner);
        if count == 0 {
            return;
        }

        // Swap with last element
        let last_idx = count - 1;
        if idx < last_idx {
            let last_key = DataKey::OwnerTicketIndex(owner.clone(), last_idx);
            if let Some(last_ticket_id) = env.storage().persistent().get::<DataKey, u64>(&last_key)
            {
                // Update the index entry
                let current_key = DataKey::OwnerTicketIndex(owner.clone(), idx);
                env.storage()
                    .persistent()
                    .set(&current_key, &last_ticket_id);
                env.storage()
                    .persistent()
                    .extend_ttl(&current_key, TTL_THRESHOLD, TTL_BUMP);

                // Update the membership entry of the moved ticket to reflect its new index
                let moved_membership_key = DataKey::OwnerTicket(owner.clone(), last_ticket_id);
                env.storage().persistent().set(&moved_membership_key, &idx);
                env.storage().persistent().extend_ttl(
                    &moved_membership_key,
                    TTL_THRESHOLD,
                    TTL_BUMP,
                );
            }
        }

        // Remove the last element
        let last_key = DataKey::OwnerTicketIndex(owner.clone(), last_idx);
        env.storage().persistent().remove(&last_key);

        // Decrement count
        let count_key = DataKey::OwnerTicketsCount(owner.clone());
        env.storage().persistent().set(&count_key, &last_idx);
        env.storage()
            .persistent()
            .extend_ttl(&count_key, TTL_THRESHOLD, TTL_BUMP);
    }
}

/// Get the count of tickets owned by an address
pub fn get_owner_tickets_count(env: &Env, owner: &Address) -> u64 {
    let key = DataKey::OwnerTicketsCount(owner.clone());
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Get owner tickets using indexed storage
pub fn get_owner_tickets(env: &Env, owner: &Address) -> Vec<u64> {
    let count = get_owner_tickets_count(env, owner);
    let mut tickets = Vec::new(env);

    for i in 0..count {
        let idx_key = DataKey::OwnerTicketIndex(owner.clone(), i);
        if let Some(ticket_id) = env.storage().persistent().get(&idx_key) {
            tickets.push_back(ticket_id);
        }
    }

    tickets
}

/// Map-based: Add an event-payment relationship
pub fn add_event_payment_map(env: &Env, event_id: &Symbol, payment_id: u64) {
    let key = DataKey::EventPayment(event_id.clone(), payment_id);
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);

    // Also add to indexed list for retrieval
    let count = get_event_payments_count(env, event_id);
    let idx_key = DataKey::EventPaymentIndex(event_id.clone(), count);
    env.storage().persistent().set(&idx_key, &payment_id);
    env.storage()
        .persistent()
        .extend_ttl(&idx_key, TTL_THRESHOLD, TTL_BUMP);

    let count_key = DataKey::EventPaymentsCount(event_id.clone());
    env.storage().persistent().set(&count_key, &(count + 1));
    env.storage()
        .persistent()
        .extend_ttl(&count_key, TTL_THRESHOLD, TTL_BUMP);
}

/// Get the count of payments for an event
pub fn get_event_payments_count(env: &Env, event_id: &Symbol) -> u64 {
    let key = DataKey::EventPaymentsCount(event_id.clone());
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Add event payment using map-based approach
pub fn add_event_payment(env: &Env, event_id: &Symbol, payment_id: u64) {
    add_event_payment_map(env, event_id, payment_id);
}

/// Get event payments using indexed storage
pub fn get_event_payments(env: &Env, event_id: &Symbol) -> Vec<u64> {
    let count = get_event_payments_count(env, event_id);
    let mut payments = Vec::new(env);

    for i in 0..count {
        let idx_key = DataKey::EventPaymentIndex(event_id.clone(), i);
        if let Some(payment_id) = env.storage().persistent().get(&idx_key) {
            payments.push_back(payment_id);
        }
    }

    payments
}

/// Map-based: Add a payer-payment relationship
pub fn add_payer_payment_map(env: &Env, payer: &Address, payment_id: u64) {
    let key = DataKey::PayerPayment(payer.clone(), payment_id);
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);

    // Also add to indexed list for retrieval
    let count = get_payer_payments_count(env, payer);
    let idx_key = DataKey::PayerPaymentIndex(payer.clone(), count);
    env.storage().persistent().set(&idx_key, &payment_id);
    env.storage()
        .persistent()
        .extend_ttl(&idx_key, TTL_THRESHOLD, TTL_BUMP);

    let count_key = DataKey::PayerPaymentsCount(payer.clone());
    env.storage().persistent().set(&count_key, &(count + 1));
    env.storage()
        .persistent()
        .extend_ttl(&count_key, TTL_THRESHOLD, TTL_BUMP);
}

/// Get the count of payments for a payer
pub fn get_payer_payments_count(env: &Env, payer: &Address) -> u64 {
    let key = DataKey::PayerPaymentsCount(payer.clone());
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Add payer payment using map-based approach
pub fn add_payer_payment(env: &Env, payer: &Address, payment_id: u64) {
    add_payer_payment_map(env, payer, payment_id);
}

/// Get payer payments using indexed storage
pub fn get_payer_payments(env: &Env, payer: &Address) -> Vec<u64> {
    let count = get_payer_payments_count(env, payer);
    let mut payments = Vec::new(env);

    for i in 0..count {
        let idx_key = DataKey::PayerPaymentIndex(payer.clone(), i);
        if let Some(payment_id) = env.storage().persistent().get(&idx_key) {
            payments.push_back(payment_id);
        }
    }

    payments
}
pub fn get_event_revenue(env: &Env, event_id: &Symbol) -> i128 {
    let tokens = get_event_tokens(env, event_id);
    let mut total = 0i128;

    for index in 0..tokens.len() {
        if let Some(token_address) = tokens.get(index) {
            total += get_event_token_revenue(env, event_id, &token_address);
        }
    }

    total
}
pub fn add_event_revenue(env: &Env, event_id: &Symbol, amount: i128) {
    let current_revenue = get_event_revenue(env, event_id);
    let new_revenue = current_revenue + amount;
    let key = DataKey::EventRevenue(event_id.clone());
    env.storage().persistent().set(&key, &new_revenue);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn set_event_revenue(env: &Env, event_id: &Symbol, amount: i128) {
    let key = DataKey::EventRevenue(event_id.clone());
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}
pub fn update_payment(env: &Env, payment: &PaymentRecord) -> Result<(), PaymentError> {
    let key = DataKey::Payment(payment.payment_id);
    if !env.storage().persistent().has(&key) {
        return Err(PaymentError::PaymentNotFound);
    }
    env.storage().persistent().set(&key, payment);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    Ok(())
}

/// Map-based: Add a withdrawal record
pub fn add_withdrawal_record_map(
    env: &Env,
    event_id: &Symbol,
    record: &crate::types::WithdrawalRecord,
) {
    // Get current count
    let count = get_withdrawal_count(env, event_id);

    // Store record at index
    let key = DataKey::WithdrawalRecord(event_id.clone(), count);
    env.storage().persistent().set(&key, record);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);

    // Increment count
    let count_key = DataKey::WithdrawalCount(event_id.clone());
    env.storage().persistent().set(&count_key, &(count + 1));
    env.storage()
        .persistent()
        .extend_ttl(&count_key, TTL_THRESHOLD, TTL_BUMP);
}

/// Get the count of withdrawals for an event
pub fn get_withdrawal_count(env: &Env, event_id: &Symbol) -> u32 {
    let key = DataKey::WithdrawalCount(event_id.clone());
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Get a specific withdrawal record by index
pub fn get_withdrawal_record_at(
    env: &Env,
    event_id: &Symbol,
    index: u32,
) -> Option<crate::types::WithdrawalRecord> {
    let key = DataKey::WithdrawalRecord(event_id.clone(), index);
    env.storage().persistent().get(&key)
}

/// Add withdrawal record using map-based approach
pub fn add_withdrawal_record(
    env: &Env,
    event_id: &Symbol,
    record: &crate::types::WithdrawalRecord,
) {
    add_withdrawal_record_map(env, event_id, record);
}

/// Get withdrawal history by iterating through indexed records
pub fn get_withdrawal_history(env: &Env, event_id: &Symbol) -> Vec<crate::types::WithdrawalRecord> {
    let count = get_withdrawal_count(env, event_id);
    let mut history = Vec::new(env);

    for i in 0..count {
        if let Some(record) = get_withdrawal_record_at(env, event_id, i) {
            history.push_back(record);
        }
    }

    history
}

pub fn reset_event_revenue(env: &Env, event_id: &Symbol) {
    let key = DataKey::EventRevenue(event_id.clone());
    env.storage().persistent().set(&key, &0i128);

    let tokens = get_event_tokens(env, event_id);
    for index in 0..tokens.len() {
        if let Some(token_address) = tokens.get(index) {
            set_event_token_revenue(env, event_id, &token_address, 0);
        }
    }
}
pub fn get_platform_fee_bps(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::PlatformFeeBps)
        .unwrap_or(0)
}
pub fn set_platform_fee_bps(env: &Env, bps: u32) {
    env.storage()
        .persistent()
        .set(&DataKey::PlatformFeeBps, &bps);
    env.storage().persistent().extend_ttl(
        &DataKey::PlatformFeeBps,
        60 * 60 * 24 * 30,
        60 * 60 * 24 * 30 * 2,
    );
}
pub fn get_platform_wallet(env: &Env) -> Result<Address, PaymentError> {
    let key = DataKey::PlatformWallet;
    let wallet = env.storage()
        .persistent()
        .get(&key)
        .ok_or(PaymentError::NotInitialized)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    Ok(wallet)
}
pub fn set_platform_wallet(env: &Env, wallet: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::PlatformWallet, wallet);
    env.storage().persistent().extend_ttl(
        &DataKey::PlatformWallet,
        60 * 60 * 24 * 30,
        60 * 60 * 24 * 30 * 2,
    );
}
pub fn get_platform_revenue(env: &Env, event_id: &Symbol) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::PlatformRevenue(event_id.clone()))
        .unwrap_or(0)
}
pub fn add_platform_revenue(env: &Env, event_id: &Symbol, amount: i128) {
    let current = get_platform_revenue(env, event_id);
    let key = DataKey::PlatformRevenue(event_id.clone());
    env.storage().persistent().set(&key, &(current + amount));
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn set_emission_privacy(env: &Env, event_id: &Symbol, level: &PrivacyLevel) {
    let key = DataKey::EmissionPrivacy(event_id.clone());
    env.storage().persistent().set(&key, level);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}
pub fn reset_platform_revenue(env: &Env, event_id: &Symbol) {
    let key = DataKey::PlatformRevenue(event_id.clone());
    env.storage().persistent().set(&key, &0i128);
}

pub fn get_emission_privacy(env: &Env, event_id: &Symbol) -> PrivacyLevel {
    env.storage()
        .persistent()
        .get(&DataKey::EmissionPrivacy(event_id.clone()))
        .unwrap_or(PrivacyLevel::Standard)
}

pub fn set_escrow_meta(env: &Env, event_id: &Symbol, meta: &EscrowMetadata) {
    let key = DataKey::EscrowMeta(event_id.clone());
    env.storage().persistent().set(&key, meta);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_escrow_meta(env: &Env, event_id: &Symbol) -> Result<EscrowMetadata, PaymentError> {
    env.storage()
        .persistent()
        .get(&DataKey::EscrowMeta(event_id.clone()))
        .ok_or(PaymentError::EscrowNotConfigured)
}

pub fn has_nonce(env: &Env, address: &Address, nonce: u64) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::ProcessedNonce(address.clone(), nonce))
        .unwrap_or(false)
}

pub fn set_nonce(env: &Env, address: &Address, nonce: u64) {
    let key = DataKey::ProcessedNonce(address.clone(), nonce);
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60 * 24 * 7, 60 * 60 * 24 * 14);
}

pub fn has_nonce_hash(env: &Env, hash: &BytesN<32>, nonce: u64) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::ProcessedNonceHash(hash.clone(), nonce))
        .unwrap_or(false)
}

pub fn set_nonce_hash(env: &Env, hash: &BytesN<32>, nonce: u64) {
    let key = DataKey::ProcessedNonceHash(hash.clone(), nonce);
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60 * 24 * 7, 60 * 60 * 24 * 14);
}

pub fn has_nullifier(env: &Env, commitment: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::SpentNullifier(commitment.clone()))
        .unwrap_or(false)
}

pub fn mark_nullifier_spent(env: &Env, commitment: &BytesN<32>) {
    let key = DataKey::SpentNullifier(commitment.clone());
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60 * 24 * 365, 60 * 60 * 24 * 365 * 2);
}

/// Get the current contract version from storage.
pub fn get_contract_version(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::ContractVersion)
        .unwrap_or(1)
}
pub fn set_contract_version(env: &Env, version: u32) {
    env.storage()
        .persistent()
        .set(&DataKey::ContractVersion, &version);
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::ContractVersion, TTL_THRESHOLD, TTL_BUMP);
}
pub fn verify_version(env: &Env) -> Result<(), PaymentError> {
    let version = get_contract_version(env);
    if version > CURRENT_VERSION {
        return Err(PaymentError::UnsupportedVersion);
    }
    Ok(())
}
pub fn get_event_token_revenue(env: &Env, event_id: &Symbol, token_address: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::EventTokenRevenue(
            event_id.clone(),
            token_address.clone(),
        ))
        .unwrap_or(0)
}
pub fn add_event_token_revenue(
    env: &Env,
    event_id: &Symbol,
    token_address: &Address,
    amount: i128,
) {
    let current_revenue = get_event_token_revenue(env, event_id, token_address);
    let new_revenue = current_revenue + amount;
    let key = DataKey::EventTokenRevenue(event_id.clone(), token_address.clone());
    env.storage().persistent().set(&key, &new_revenue);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}
pub fn set_event_token_revenue(
    env: &Env,
    event_id: &Symbol,
    token_address: &Address,
    amount: i128,
) {
    let key = DataKey::EventTokenRevenue(event_id.clone(), token_address.clone());
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}
/// Map-based: Check if an event has a specific token
pub fn has_event_token(env: &Env, event_id: &Symbol, token_address: &Address) -> bool {
    env.storage().persistent().has(&DataKey::EventToken(
        event_id.clone(),
        token_address.clone(),
    ))
}

/// Map-based: Add a token to an event
pub fn add_event_token_map(env: &Env, event_id: &Symbol, token_address: &Address) {
    let key = DataKey::EventToken(event_id.clone(), token_address.clone());
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);

    // Also add to indexed list for retrieval
    let count = get_event_token_count(env, event_id);
    let idx_key = DataKey::EventTokenIndex(event_id.clone(), count);
    env.storage().persistent().set(&idx_key, token_address);
    env.storage()
        .persistent()
        .extend_ttl(&idx_key, TTL_THRESHOLD, TTL_BUMP);

    let count_key = DataKey::EventTokenCount(event_id.clone());
    env.storage().persistent().set(&count_key, &(count + 1));
    env.storage()
        .persistent()
        .extend_ttl(&count_key, TTL_THRESHOLD, TTL_BUMP);
}

/// Get the count of tokens for an event
pub fn get_event_token_count(env: &Env, event_id: &Symbol) -> u32 {
    let key = DataKey::EventTokenCount(event_id.clone());
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Add event token using map-based approach
pub fn add_event_token(env: &Env, event_id: &Symbol, token_address: &Address) {
    if !has_event_token(env, event_id, token_address) {
        add_event_token_map(env, event_id, token_address);
    }
}

/// Get event tokens using indexed storage
pub fn get_event_tokens(env: &Env, event_id: &Symbol) -> Vec<Address> {
    let count = get_event_token_count(env, event_id);
    let mut tokens = Vec::new(env);

    for i in 0..count {
        let idx_key = DataKey::EventTokenIndex(event_id.clone(), i);
        if let Some(token_address) = env.storage().persistent().get(&idx_key) {
            tokens.push_back(token_address);
        }
    }

    tokens
}

pub fn get_user_event_tickets(env: &Env, event_id: &Symbol, user: &Address) -> u32 {
    let key = DataKey::UserEventTickets(event_id.clone(), user.clone());
    env.storage().persistent().get(&key).unwrap_or(0)
}

pub fn increment_user_event_tickets(env: &Env, event_id: &Symbol, user: &Address) {
    let current = get_user_event_tickets(env, event_id, user);
    let key = DataKey::UserEventTickets(event_id.clone(), user.clone());
    env.storage().persistent().set(&key, &(current + 1));
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_user_event_tickets_hash(env: &Env, event_id: &Symbol, user_hash: &BytesN<32>) -> u32 {
    let key = DataKey::UserEventTicketsHash(event_id.clone(), user_hash.clone());
    env.storage().persistent().get(&key).unwrap_or(0)
}

pub fn increment_user_event_tickets_hash(env: &Env, event_id: &Symbol, user_hash: &BytesN<32>) {
    let current = get_user_event_tickets_hash(env, event_id, user_hash);
    let key = DataKey::UserEventTicketsHash(event_id.clone(), user_hash.clone());
    env.storage().persistent().set(&key, &(current + 1));
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn increment_event_sold_count(env: &Env, event_id: &Symbol) -> Result<(), PaymentError> {
    let mut config = get_event_config(env, event_id).ok_or(PaymentError::InvalidOrganizer)?;
    config.sold_count = config
        .sold_count
        .checked_add(1)
        .ok_or(PaymentError::EventSoldOut)?;
    set_event_config(env, event_id, &config);
    Ok(())
}

// ── Revenue split helpers ─────────────────────────────────────────────────────

/// Whether the event has any revenue split configured (more than zero recipients).
pub fn has_splits(env: &Env, event_id: &Symbol) -> bool {
    let key = DataKey::EventSplits(event_id.clone());
    let exists = env.storage().persistent().has(&key);
    if exists {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    }
    exists
}

pub fn get_splits(env: &Env, event_id: &Symbol) -> Vec<RevenueSplit> {
    let key = DataKey::EventSplits(event_id.clone());
    match env.storage().persistent().get(&key) {
        Some(splits) => {
            env.storage()
                .persistent()
                .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
            splits
        }
        None => Vec::new(env),
    }
}

pub fn set_splits(env: &Env, event_id: &Symbol, splits: &Vec<RevenueSplit>) {
    let key = DataKey::EventSplits(event_id.clone());
    env.storage().persistent().set(&key, splits);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_split_settlement(env: &Env, event_id: &Symbol) -> Option<SplitSettlement> {
    let key = DataKey::SplitSettlement(event_id.clone());
    match env.storage().persistent().get(&key) {
        Some(settlement) => {
            env.storage()
                .persistent()
                .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
            Some(settlement)
        }
        None => None,
    }
}

pub fn set_split_settlement(env: &Env, event_id: &Symbol, settlement: &SplitSettlement) {
    let key = DataKey::SplitSettlement(event_id.clone());
    env.storage().persistent().set(&key, settlement);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_split_withdrawn(env: &Env, event_id: &Symbol, recipient: &Address) -> i128 {
    let key = DataKey::SplitWithdrawn(event_id.clone(), recipient.clone());
    match env.storage().persistent().get(&key) {
        Some(amount) => {
            env.storage()
                .persistent()
                .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
            amount
        }
        None => 0,
    }
}

pub fn set_split_withdrawn(env: &Env, event_id: &Symbol, recipient: &Address, amount: i128) {
    let key = DataKey::SplitWithdrawn(event_id.clone(), recipient.clone());
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn is_split_flagged(env: &Env, event_id: &Symbol, recipient: &Address) -> bool {
    let key = DataKey::SplitFlagged(event_id.clone(), recipient.clone());
    match env.storage().persistent().get(&key) {
        Some(flagged) => {
            env.storage()
                .persistent()
                .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
            flagged
        }
        None => false,
    }
}

pub fn set_split_flagged(env: &Env, event_id: &Symbol, recipient: &Address, flagged: bool) {
    let key = DataKey::SplitFlagged(event_id.clone(), recipient.clone());
    env.storage().persistent().set(&key, &flagged);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn save_resale_listing(env: &Env, ticket_id: u64, listing: &crate::types::ResaleListing) {
    let key = DataKey::ResaleListing(ticket_id);
    env.storage().persistent().set(&key, listing);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_resale_listing(env: &Env, ticket_id: u64) -> Option<crate::types::ResaleListing> {
    env.storage()
        .persistent()
        .get(&DataKey::ResaleListing(ticket_id))
}

pub fn remove_resale_listing(env: &Env, ticket_id: u64) {
    let key = DataKey::ResaleListing(ticket_id);
    env.storage().persistent().remove(&key);
}

pub fn get_dispute(env: &Env, ticket_id: u64) -> Option<crate::types::DisputeRecord> {
    env.storage().persistent().get(&DataKey::Dispute(ticket_id))
}

pub fn set_dispute(env: &Env, ticket_id: u64, record: &crate::types::DisputeRecord) {
    let key = DataKey::Dispute(ticket_id);
    env.storage().persistent().set(&key, record);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_total_payments(env: &Env, event_id: &Symbol) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::TotalPayments(event_id.clone()))
        .unwrap_or(0)
}
pub fn add_total_payments(env: &Env, event_id: &Symbol, amount: i128) {
    let current = get_total_payments(env, event_id);
    let key = DataKey::TotalPayments(event_id.clone());
    env.storage().persistent().set(&key, &(current + amount));
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}
pub fn get_total_refunds(env: &Env, event_id: &Symbol) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::TotalRefunds(event_id.clone()))
        .unwrap_or(0)
}
pub fn add_total_refunds(env: &Env, event_id: &Symbol, amount: i128) {
    let current = get_total_refunds(env, event_id);
    let key = DataKey::TotalRefunds(event_id.clone());
    env.storage().persistent().set(&key, &(current + amount));
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}
pub fn get_total_withdrawn(env: &Env, event_id: &Symbol) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::TotalWithdrawn(event_id.clone()))
        .unwrap_or(0)
}
pub fn add_total_withdrawn(env: &Env, event_id: &Symbol, amount: i128) {
    let current = get_total_withdrawn(env, event_id);
    let key = DataKey::TotalWithdrawn(event_id.clone());
    env.storage().persistent().set(&key, &(current + amount));
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn remove_dispute(env: &Env, ticket_id: u64) {
    env.storage()
        .persistent()
        .remove(&DataKey::Dispute(ticket_id));
}

pub fn get_event_disputes(env: &Env, event_id: &Symbol) -> soroban_sdk::Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::EventDisputes(event_id.clone()))
        .unwrap_or_else(|| soroban_sdk::Vec::new(env))
}

pub fn set_event_disputes(env: &Env, event_id: &Symbol, disputes: &soroban_sdk::Vec<u64>) {
    let key = DataKey::EventDisputes(event_id.clone());
    env.storage().persistent().set(&key, disputes);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_total_token_volume(env: &Env, event_id: &Symbol, token: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::EventTokenVolume(event_id.clone(), token.clone()))
        .unwrap_or(0)
}

pub fn add_total_token_volume(env: &Env, event_id: &Symbol, token: &Address, amount: i128) {
    let current = get_total_token_volume(env, event_id, token);
    let key = DataKey::EventTokenVolume(event_id.clone(), token.clone());
    env.storage().persistent().set(&key, &(current + amount));
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
}
