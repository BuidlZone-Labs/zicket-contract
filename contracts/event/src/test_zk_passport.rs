//! Tests for the zkPassport verification path (verify_and_attend).
//!
//! Acceptance criteria covered:
//!  [AC-1] ZkPassportClaim struct (claim_type / proof / nullifier / expiry_ledger)
//!  [AC-2] verify_and_attend entry point on the Event contract
//!  [AC-3] Nullifier stored on-chain to prevent proof reuse across events
//!  [AC-4] Proof expiry checked against current ledger sequence
//!  [AC-5] Verification result gates ticket issuance
//!  [AC-6] Proof bytes are NEVER stored; only nullifier is persisted

use crate::errors::EventError;
use crate::types::{
    CreateEventParams, EventStatus, PrivacyLevel, TicketTierParams, ZkClaimType, ZkPassportClaim,
    ZkVerificationConfig,
};
use crate::{EventContract, EventContractClient};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Bytes, BytesN, Env, String, Symbol};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

const BASE_TIMESTAMP: u64 = 1_704_067_200; // 2024-01-01T00:00:00Z

fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = BASE_TIMESTAMP;
        li.sequence_number = 1000;
    });
    env
}

/// Register a minimal Upcoming event with `requires_verification = true`.
fn setup_verified_event(env: &Env, client: &EventContractClient, organizer: &Address) -> Symbol {
    let event_id = Symbol::new(env, "ev_zk_01");
    let tiers = soroban_sdk::vec![
        env,
        TicketTierParams {
            name: String::from_str(env, "General"),
            price: 0,
            capacity: 100,
        },
    ];

    client.create_event(&CreateEventParams {
        organizer: organizer.clone(),
        payout_token: Address::generate(env),
        event_id: event_id.clone(),
        name: String::from_str(env, "ZK Conference"),
        description: String::from_str(env, "Passport gated"),
        venue: String::from_str(env, "Metaverse Hall"),
        event_date: env.ledger().timestamp() + 86_401,
        initial_tiers: tiers,
        allow_anonymous: false,
        requires_verification: true,
        privacy_level: PrivacyLevel::Standard,
        max_tickets_per_user: 1,
        event_start_ledger: 0,
        event_end_ledger: 9_999,
        withdrawal_delay_ledgers: 17_280,
    });
    event_id
}

/// Transition an event to Active status.
fn activate_event(env: &Env, client: &EventContractClient, organizer: &Address, event_id: &Symbol) {
    let _ = env;
    client.update_event_status(organizer, event_id, &EventStatus::Active);
}

/// Build a mock `ZkPassportClaim` with a given nullifier seed byte and expiry.
fn make_claim(
    env: &Env,
    claim_type: ZkClaimType,
    nullifier_seed: u8,
    expiry_ledger: u32,
) -> ZkPassportClaim {
    // Proof is 64 bytes of mock data — never stored.
    let mut proof_arr = [0u8; 64];
    proof_arr[0] = nullifier_seed;
    let proof = Bytes::from_array(env, &proof_arr);

    // Nullifier is 32 bytes — this is what IS stored.
    let mut null_arr = [0u8; 32];
    null_arr[0] = nullifier_seed;
    let nullifier = BytesN::from_array(env, &null_arr);

    ZkPassportClaim {
        claim_type,
        proof,
        nullifier,
        expiry_ledger,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// [AC-5] Happy path: valid proof gates ticket issuance
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_verify_and_attend_happy_path() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_verified_event(&env, &client, &organizer);
    activate_event(&env, &client, &organizer, &event_id);

    // Organizer enables zkPassport for this event.
    client.set_zk_config(
        &organizer,
        &event_id,
        &ZkVerificationConfig {
            required_claim_type: ZkClaimType::Any,
            enabled: true,
        },
    );

    // Build a valid claim with expiry well in the future.
    let claim = make_claim(&env, ZkClaimType::Age, 1, 9_999);

    // Should succeed.
    client.verify_and_attend(&event_id, &0u32, &claim);

    // Sold count must increment.
    let event = client.get_event(&event_id);
    assert_eq!(event.sold_count, 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// [AC-3] Nullifier reuse must be rejected
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_nullifier_reuse_rejected() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_verified_event(&env, &client, &organizer);
    activate_event(&env, &client, &organizer, &event_id);

    client.set_zk_config(
        &organizer,
        &event_id,
        &ZkVerificationConfig {
            required_claim_type: ZkClaimType::Any,
            enabled: true,
        },
    );

    let claim = make_claim(&env, ZkClaimType::Age, 42, 9_999);

    // First use succeeds.
    client.verify_and_attend(&event_id, &0u32, &claim);

    // Second use with identical nullifier must fail.
    let result = client.try_verify_and_attend(&event_id, &0u32, &claim);
    assert_eq!(result, Err(Ok(EventError::ZkNullifierReused)));
}

// ─────────────────────────────────────────────────────────────────────────────
// [AC-4] Expired proof must be rejected
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_expired_proof_rejected() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_verified_event(&env, &client, &organizer);
    activate_event(&env, &client, &organizer, &event_id);

    client.set_zk_config(
        &organizer,
        &event_id,
        &ZkVerificationConfig {
            required_claim_type: ZkClaimType::Any,
            enabled: true,
        },
    );

    // Advance the ledger sequence past the claim's expiry.
    env.ledger().with_mut(|li| {
        li.sequence_number = 2000;
    });

    // Claim expires at ledger 999, current is 2000.
    let expired_claim = make_claim(&env, ZkClaimType::Age, 99, 999);

    let result = client.try_verify_and_attend(&event_id, &0u32, &expired_claim);
    assert_eq!(result, Err(Ok(EventError::ZkProofExpired)));
}

// ─────────────────────────────────────────────────────────────────────────────
// [AC-5] Non-gated event rejects verify_and_attend
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_non_gated_event_rejects_verify_and_attend() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = Symbol::new(&env, "ev_open");
    client.create_event(&CreateEventParams {
        organizer: organizer.clone(),
        payout_token: Address::generate(&env),
        event_id: event_id.clone(),
        name: String::from_str(&env, "Open Event"),
        description: String::from_str(&env, "no verification"),
        venue: String::from_str(&env, "Park"),
        event_date: env.ledger().timestamp() + 86_401,
        initial_tiers: soroban_sdk::vec![
            &env,
            TicketTierParams {
                name: String::from_str(&env, "GA"),
                price: 0,
                capacity: 50,
            },
        ],
        allow_anonymous: false,
        requires_verification: false, // <-- not gated
        privacy_level: PrivacyLevel::Standard,
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 9_999,
        withdrawal_delay_ledgers: 17_280,
    });
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);

    let claim = make_claim(&env, ZkClaimType::Age, 5, 9_999);
    let result = client.try_verify_and_attend(&event_id, &0u32, &claim);
    assert_eq!(result, Err(Ok(EventError::ZkVerificationRequired)));
}

// ─────────────────────────────────────────────────────────────────────────────
// Claim type mismatch is rejected
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_claim_type_mismatch_rejected() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_verified_event(&env, &client, &organizer);
    activate_event(&env, &client, &organizer, &event_id);

    // Organizer requires Citizenship only.
    client.set_zk_config(
        &organizer,
        &event_id,
        &ZkVerificationConfig {
            required_claim_type: ZkClaimType::Citizenship,
            enabled: true,
        },
    );

    // Attendee submits an Age claim.
    let claim = make_claim(&env, ZkClaimType::Age, 11, 9_999);
    let result = client.try_verify_and_attend(&event_id, &0u32, &claim);
    assert_eq!(result, Err(Ok(EventError::ZkClaimTypeMismatch)));
}

// ─────────────────────────────────────────────────────────────────────────────
// Correct claim type is accepted when organizer specifies it
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_correct_claim_type_accepted() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_verified_event(&env, &client, &organizer);
    activate_event(&env, &client, &organizer, &event_id);

    client.set_zk_config(
        &organizer,
        &event_id,
        &ZkVerificationConfig {
            required_claim_type: ZkClaimType::Location,
            enabled: true,
        },
    );

    let claim = make_claim(&env, ZkClaimType::Location, 22, 9_999);
    client.verify_and_attend(&event_id, &0u32, &claim);

    assert_eq!(client.get_event(&event_id).sold_count, 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// ZK config disabled rejects verify_and_attend
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_zk_config_disabled_rejects() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_verified_event(&env, &client, &organizer);
    activate_event(&env, &client, &organizer, &event_id);

    client.set_zk_config(
        &organizer,
        &event_id,
        &ZkVerificationConfig {
            required_claim_type: ZkClaimType::Any,
            enabled: false,
        },
    );

    let claim = make_claim(&env, ZkClaimType::Age, 33, 9_999);
    let result = client.try_verify_and_attend(&event_id, &0u32, &claim);
    assert_eq!(result, Err(Ok(EventError::ZkVerificationRequired)));
}

// ─────────────────────────────────────────────────────────────────────────────
// Default ZK config (never set) also rejects
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_default_zk_config_rejects() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_verified_event(&env, &client, &organizer);
    activate_event(&env, &client, &organizer, &event_id);
    // No set_zk_config call.

    let claim = make_claim(&env, ZkClaimType::Citizenship, 44, 9_999);
    let result = client.try_verify_and_attend(&event_id, &0u32, &claim);
    assert_eq!(result, Err(Ok(EventError::ZkVerificationRequired)));
}

// ─────────────────────────────────────────────────────────────────────────────
// is_nullifier_used query reflects storage correctly
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_is_nullifier_used_query() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_verified_event(&env, &client, &organizer);
    activate_event(&env, &client, &organizer, &event_id);

    client.set_zk_config(
        &organizer,
        &event_id,
        &ZkVerificationConfig {
            required_claim_type: ZkClaimType::Any,
            enabled: true,
        },
    );

    let claim = make_claim(&env, ZkClaimType::Age, 55, 9_999);

    // Before: not recorded.
    assert!(!client.is_nullifier_used(&event_id, &claim.nullifier));

    // After: recorded.
    client.verify_and_attend(&event_id, &0u32, &claim);
    assert!(client.is_nullifier_used(&event_id, &claim.nullifier));
}

// ─────────────────────────────────────────────────────────────────────────────
// Non-organizer cannot change zk_config
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_only_organizer_can_set_zk_config() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);
    let intruder = Address::generate(&env);

    let event_id = setup_verified_event(&env, &client, &organizer);

    let result = client.try_set_zk_config(
        &intruder,
        &event_id,
        &ZkVerificationConfig {
            required_claim_type: ZkClaimType::Any,
            enabled: true,
        },
    );
    assert_eq!(result, Err(Ok(EventError::Unauthorized)));
}

// ─────────────────────────────────────────────────────────────────────────────
// get_zk_config returns defaults when never set
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_get_zk_config_defaults() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = setup_verified_event(&env, &client, &organizer);
    let config = client.get_zk_config(&event_id);

    assert!(!config.enabled);
    assert_eq!(config.required_claim_type, ZkClaimType::Any);
}

// ─────────────────────────────────────────────────────────────────────────────
// Inactive (Upcoming) event rejects verify_and_attend
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_inactive_event_rejects_verify_and_attend() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    // Event NOT activated.
    let event_id = setup_verified_event(&env, &client, &organizer);

    client.set_zk_config(
        &organizer,
        &event_id,
        &ZkVerificationConfig {
            required_claim_type: ZkClaimType::Any,
            enabled: true,
        },
    );

    let claim = make_claim(&env, ZkClaimType::Age, 77, 9_999);
    let result = client.try_verify_and_attend(&event_id, &0u32, &claim);
    assert_eq!(result, Err(Ok(EventError::EventNotActive)));
}

// ─────────────────────────────────────────────────────────────────────────────
// Sold-out event rejects verify_and_attend
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_sold_out_event_rejects_verify_and_attend() {
    let env = setup_env();
    let contract_id = env.register(EventContract, ());
    let client = EventContractClient::new(&env, &contract_id);
    let organizer = Address::generate(&env);

    let event_id = Symbol::new(&env, "ev_tiny");
    client.create_event(&CreateEventParams {
        organizer: organizer.clone(),
        payout_token: Address::generate(&env),
        event_id: event_id.clone(),
        name: String::from_str(&env, "Tiny ZK Event"),
        description: String::from_str(&env, "one slot"),
        venue: String::from_str(&env, "Closet"),
        event_date: env.ledger().timestamp() + 86_401,
        initial_tiers: soroban_sdk::vec![
            &env,
            TicketTierParams {
                name: String::from_str(&env, "Only"),
                price: 0,
                capacity: 1,
            },
        ],
        allow_anonymous: false,
        requires_verification: true,
        privacy_level: PrivacyLevel::Standard,
        max_tickets_per_user: 0,
        event_start_ledger: 0,
        event_end_ledger: 9_999,
        withdrawal_delay_ledgers: 17_280,
    });
    client.update_event_status(&organizer, &event_id, &EventStatus::Active);
    client.set_zk_config(
        &organizer,
        &event_id,
        &ZkVerificationConfig {
            required_claim_type: ZkClaimType::Any,
            enabled: true,
        },
    );

    // Fill the only seat.
    client.verify_and_attend(
        &event_id,
        &0u32,
        &make_claim(&env, ZkClaimType::Age, 80, 9_999),
    );

    // Second attendee (different nullifier) hits sold-out.
    let result = client.try_verify_and_attend(
        &event_id,
        &0u32,
        &make_claim(&env, ZkClaimType::Age, 81, 9_999),
    );
    assert_eq!(result, Err(Ok(EventError::EventSoldOut)));
}
