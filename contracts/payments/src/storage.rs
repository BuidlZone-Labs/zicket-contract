use crate::errors::PaymentError;
use crate::types::PaymentRecord;
use soroban_sdk::{contracttype, Env, Symbol, Vec};

#[contracttype]
pub enum DataKey {
    Admin,
    AcceptedToken,
    Payment(u64),
    EventPayments(Symbol),
    EventRevenue(Symbol),
    NextPaymentId,
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
    env.storage().persistent().set(&DataKey::AcceptedToken, token);
    env.storage().persistent().extend_ttl(
        &DataKey::AcceptedToken,
        60 * 60 * 24 * 30,
        60 * 60 * 24 * 30 * 2,
    );
}

///check ifcontract isinitialized.
pub fn is_initialized(env: &Env) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Admin)
        && env.storage().persistent().has(&DataKey::AcceptedToken)
}

/// Get the next payment ID and increment it.
pub fn get_next_payment_id(env: &Env) -> u64 {
    let current_id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::NextPaymentId)
        .unwrap_or(0);
    let next_id = current_id + 1;
    env.storage().persistent().set(&DataKey::NextPaymentId, &next_id);
    env.storage().persistent().extend_ttl(
        &DataKey::NextPaymentId,
        60 * 60 * 24 * 30,
        60 * 60 * 24 * 30 * 2,
    );
    next_id
}

/// Save a payment record to storage
pub fn save_payment(env: &Env, payment: &PaymentRecord) {
    let key = DataKey::Payment(payment.payment_id);
    env.storage().persistent().set(&key, payment);
    env.storage().persistent().extend_ttl(
        &key,
        60 * 60 * 24 * 30,
        60 * 60 * 24 * 30 * 2,
    );
}

/// Get a payment record by ID
pub fn get_payment(env: &Env, payment_id: u64) -> Result<PaymentRecord, PaymentError> {
    env.storage()
        .persistent()
        .get(&DataKey::Payment(payment_id))
        .ok_or(PaymentError::PaymentNotFound)
}

/// add a payment id to the list of payments for an event
pub fn add_event_payment(env: &Env, event_id: &Symbol, payment_id: u64) {
    let key = DataKey::EventPayments(event_id.clone());
    let mut payments: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(&env));
    payments.push_back(payment_id);
    env.storage().persistent().set(&key, &payments);
    env.storage().persistent().extend_ttl(
        &key,
        60 * 60 * 24 * 30,
        60 * 60 * 24 * 30 * 2,
    );
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
    env.storage().persistent().extend_ttl(
        &key,
        60 * 60 * 24 * 30,
        60 * 60 * 24 * 30 * 2,
    );
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
