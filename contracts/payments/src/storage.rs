use crate::errors::PaymentError;
use crate::types::{EscrowMetadata, EventStatus, PaymentRecord, PrivacyLevel, Ticket};
use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

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
    EventPayments(Symbol),
    EventRevenue(Symbol),
    EventTokenRevenue(Symbol, Address), // Revenue per token per event
    EventTokens(Symbol),                // List of tokens used for an event
    EventStatus(Symbol),
    OwnerTickets(Address),
    WithdrawalHistory(Symbol),
    NextPaymentId,
    NextTicketId,
    EmissionPrivacy(Symbol),
    EscrowMeta(Symbol),
}

pub fn set_event_status(env: &Env, event_id: &Symbol, status: &EventStatus) {
    let key = DataKey::EventStatus(event_id.clone());
    env.storage().persistent().set(&key, status);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
}

pub fn get_event_status(env: &Env, event_id: &Symbol) -> Option<EventStatus> {
    env.storage()
        .persistent()
        .get(&DataKey::EventStatus(event_id.clone()))
}

/// Get the admin address from storage.
pub fn get_admin(env: &Env) -> Result<soroban_sdk::Address, PaymentError> {
    env.storage()
        .persistent()
        .get(&DataKey::Admin)
        .ok_or(PaymentError::NotInitialized)
}

pub fn set_admin(env: &Env, admin: &soroban_sdk::Address) {
    env.storage().persistent().set(&DataKey::Admin, admin);
    env.storage().persistent().extend_ttl(
        &DataKey::Admin,
        60 * 60 * 24 * 30,
        60 * 60 * 24 * 30 * 2,
    );
}

/// Get the accepted token address from storage.
pub fn get_accepted_token(env: &Env) -> Result<soroban_sdk::Address, PaymentError> {
    env.storage()
        .persistent()
        .get(&DataKey::AcceptedToken)
        .ok_or(PaymentError::NotInitialized)
}

/// Set the accepted token address in storage.
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
    env.storage()
        .persistent()
        .get(&DataKey::EventContract)
        .ok_or(PaymentError::NotInitialized)
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
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
}

pub fn get_event_config(env: &Env, event_id: &Symbol) -> Option<EventConfig> {
    env.storage()
        .persistent()
        .get(&DataKey::EventConfig(event_id.clone()))
}

pub fn set_event_config(env: &Env, event_id: &Symbol, config: &EventConfig) {
    let key = DataKey::EventConfig(event_id.clone());
    env.storage().persistent().set(&key, config);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
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

/// Check if contract is initialized.
pub fn is_initialized(env: &Env) -> bool {
    env.storage().persistent().has(&DataKey::Admin)
        && env.storage().persistent().has(&DataKey::AcceptedToken)
        && env.storage().persistent().has(&DataKey::EventContract)
}

/// Get the next payment ID and increment it.
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

/// Get the next ticket ID and increment it.
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

/// Save a payment record to storage
pub fn save_payment(env: &Env, payment: &PaymentRecord) {
    let key = DataKey::Payment(payment.payment_id);
    env.storage().persistent().set(&key, payment);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
}

/// Get a payment record by ID
pub fn get_payment(env: &Env, payment_id: u64) -> Result<PaymentRecord, PaymentError> {
    env.storage()
        .persistent()
        .get(&DataKey::Payment(payment_id))
        .ok_or(PaymentError::PaymentNotFound)
}

/// Save a ticket record to storage.
pub fn save_ticket(env: &Env, ticket: &Ticket) {
    let key = DataKey::Ticket(ticket.ticket_id);
    env.storage().persistent().set(&key, ticket);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
}

/// Get a ticket record by ID.
pub fn get_ticket(env: &Env, ticket_id: u64) -> Result<Ticket, PaymentError> {
    env.storage()
        .persistent()
        .get(&DataKey::Ticket(ticket_id))
        .ok_or(PaymentError::TicketNotFound)
}

/// Add a ticket ID to the list of tickets for an owner.
pub fn add_owner_ticket(env: &Env, owner: &Address, ticket_id: u64) {
    let key = DataKey::OwnerTickets(owner.clone());
    let mut tickets: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    tickets.push_back(ticket_id);
    env.storage().persistent().set(&key, &tickets);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
}

/// Get all ticket IDs for an owner.
pub fn get_owner_tickets(env: &Env, owner: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::OwnerTickets(owner.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// add a payment id to the list of payments for an event
pub fn add_event_payment(env: &Env, event_id: &Symbol, payment_id: u64) {
    let key = DataKey::EventPayments(event_id.clone());
    let mut payments: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    payments.push_back(payment_id);
    env.storage().persistent().set(&key, &payments);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
}

/// Get all payment IDs for an event.
pub fn get_event_payments(env: &Env, event_id: &Symbol) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::EventPayments(event_id.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Get the total revenue for an event.
pub fn get_event_revenue(env: &Env, event_id: &Symbol) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::EventRevenue(event_id.clone()))
        .unwrap_or(0)
}

/// Add to the total revenue for an event.
pub fn add_event_revenue(env: &Env, event_id: &Symbol, amount: i128) {
    let current_revenue = get_event_revenue(env, event_id);
    let new_revenue = current_revenue + amount;
    let key = DataKey::EventRevenue(event_id.clone());
    env.storage().persistent().set(&key, &new_revenue);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
}

pub fn set_event_revenue(env: &Env, event_id: &Symbol, amount: i128) {
    let key = DataKey::EventRevenue(event_id.clone());
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
}

/// Update a payment record in storage.
pub fn update_payment(env: &Env, payment: &PaymentRecord) -> Result<(), PaymentError> {
    if !env
        .storage()
        .persistent()
        .has(&DataKey::Payment(payment.payment_id))
    {
        return Err(PaymentError::PaymentNotFound);
    }
    save_payment(env, payment);
    Ok(())
}

pub fn add_withdrawal_record(
    env: &Env,
    event_id: &Symbol,
    record: &crate::types::WithdrawalRecord,
) {
    let key = DataKey::WithdrawalHistory(event_id.clone());
    let mut history: Vec<crate::types::WithdrawalRecord> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    history.push_back(record.clone());
    env.storage().persistent().set(&key, &history);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
}

pub fn get_withdrawal_history(env: &Env, event_id: &Symbol) -> Vec<crate::types::WithdrawalRecord> {
    let key = DataKey::WithdrawalHistory(event_id.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn reset_event_revenue(env: &Env, event_id: &Symbol) {
    let key = DataKey::EventRevenue(event_id.clone());
    env.storage().persistent().set(&key, &0i128);
}

pub fn set_emission_privacy(env: &Env, event_id: &Symbol, level: &PrivacyLevel) {
    let key = DataKey::EmissionPrivacy(event_id.clone());
    env.storage().persistent().set(&key, level);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
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
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
}

pub fn get_escrow_meta(env: &Env, event_id: &Symbol) -> Result<EscrowMetadata, PaymentError> {
    env.storage()
        .persistent()
        .get(&DataKey::EscrowMeta(event_id.clone()))
        .ok_or(PaymentError::EscrowNotConfigured)
}
/// Get the total revenue for an event and specific token.
pub fn get_event_token_revenue(env: &Env, event_id: &Symbol, token_address: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::EventTokenRevenue(
            event_id.clone(),
            token_address.clone(),
        ))
        .unwrap_or(0)
}

/// Add to the total revenue for an event and specific token.
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
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
}

/// Set the total revenue for an event and specific token.
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
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
}

/// Add a token to the list of tokens used for an event.
pub fn add_event_token(env: &Env, event_id: &Symbol, token_address: &Address) {
    let key = DataKey::EventTokens(event_id.clone());
    let mut tokens: Vec<Address> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    // Only add if not already present
    for i in 0..tokens.len() {
        if let Some(existing_token) = tokens.get(i) {
            if existing_token == *token_address {
                return;
            }
        }
    }

    tokens.push_back(token_address.clone());
    env.storage().persistent().set(&key, &tokens);
    env.storage()
        .persistent()
        .extend_ttl(&key, 60 * 60 * 24 * 30, 60 * 60 * 24 * 30 * 2);
}

/// Get all tokens used for an event.
pub fn get_event_tokens(env: &Env, event_id: &Symbol) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::EventTokens(event_id.clone()))
        .unwrap_or_else(|| Vec::new(env))
}
