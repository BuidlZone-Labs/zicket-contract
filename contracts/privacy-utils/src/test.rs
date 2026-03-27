use super::{mask_address, MaskedAddress, PrivacyLevel};
use soroban_sdk::{testutils::Address as _, xdr::ToXdr, Address, Bytes, BytesN, Env};

fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

// ---------------------------------------------------------------------------
// Standard mode
// ---------------------------------------------------------------------------

#[test]
fn test_standard_returns_full_address() {
    let env = setup_env();
    let address = Address::generate(&env);

    let masked = mask_address(&env, &address, PrivacyLevel::Standard);

    match masked {
        MaskedAddress::Full(addr) => assert_eq!(addr, address),
        _ => panic!("Expected MaskedAddress::Full for Standard privacy level"),
    }
}

#[test]
fn test_standard_two_different_addresses_remain_distinct() {
    let env = setup_env();
    let addr1 = Address::generate(&env);
    let addr2 = Address::generate(&env);

    let m1 = mask_address(&env, &addr1, PrivacyLevel::Standard);
    let m2 = mask_address(&env, &addr2, PrivacyLevel::Standard);

    match (m1, m2) {
        (MaskedAddress::Full(a), MaskedAddress::Full(b)) => assert_ne!(a, b),
        _ => panic!("Expected MaskedAddress::Full for both"),
    }
}

// ---------------------------------------------------------------------------
// Private mode
// ---------------------------------------------------------------------------

#[test]
fn test_private_returns_partial_bytes() {
    let env = setup_env();
    let address = Address::generate(&env);

    let masked = mask_address(&env, &address, PrivacyLevel::Private);

    match masked {
        MaskedAddress::Partial(bytes) => {
            // Must be non-empty and at most 8 bytes
            assert!(!bytes.is_empty(), "Partial bytes must be non-empty");
            assert!(bytes.len() <= 8, "Partial bytes must not exceed 8 bytes");
        }
        _ => panic!("Expected MaskedAddress::Partial for Private privacy level"),
    }
}

#[test]
fn test_private_does_not_expose_full_address() {
    let env = setup_env();
    let address = Address::generate(&env);

    // Full XDR is longer than 8 bytes; partial must be a strict subset.
    let full_xdr = address.clone().to_xdr(&env);
    let masked = mask_address(&env, &address, PrivacyLevel::Private);

    match masked {
        MaskedAddress::Partial(bytes) => {
            assert!(
                bytes.len() < full_xdr.len(),
                "Partial bytes must be shorter than the full XDR representation"
            );
        }
        _ => panic!("Expected MaskedAddress::Partial"),
    }
}

#[test]
fn test_private_same_address_deterministic() {
    let env = setup_env();
    let address = Address::generate(&env);

    let m1 = mask_address(&env, &address, PrivacyLevel::Private);
    let m2 = mask_address(&env, &address, PrivacyLevel::Private);

    match (m1, m2) {
        (MaskedAddress::Partial(b1), MaskedAddress::Partial(b2)) => {
            assert_eq!(b1, b2, "Same address must produce identical partial bytes");
        }
        _ => panic!("Expected MaskedAddress::Partial for both"),
    }
}

// ---------------------------------------------------------------------------
// Anonymous mode
// ---------------------------------------------------------------------------

#[test]
fn test_anonymous_returns_hash() {
    let env = setup_env();
    let address = Address::generate(&env);

    let masked = mask_address(&env, &address, PrivacyLevel::Anonymous);

    match masked {
        MaskedAddress::Hashed(hash) => {
            // SHA-256 is always 32 bytes
            let _: BytesN<32> = hash;
        }
        _ => panic!("Expected MaskedAddress::Hashed for Anonymous privacy level"),
    }
}

#[test]
fn test_anonymous_hash_differs_from_raw_xdr() {
    let env = setup_env();
    let address = Address::generate(&env);

    let full_xdr: Bytes = address.clone().to_xdr(&env);
    let masked = mask_address(&env, &address, PrivacyLevel::Anonymous);

    match masked {
        MaskedAddress::Hashed(hash) => {
            // The hash must not equal the raw XDR bytes (different lengths alone
            // make equality impossible, but we assert it explicitly).
            let hash_bytes: Bytes = hash.into();
            assert_ne!(
                full_xdr, hash_bytes,
                "Anonymous hash must not equal the raw XDR bytes"
            );
        }
        _ => panic!("Expected MaskedAddress::Hashed"),
    }
}

#[test]
fn test_anonymous_same_address_deterministic() {
    let env = setup_env();
    let address = Address::generate(&env);

    let m1 = mask_address(&env, &address, PrivacyLevel::Anonymous);
    let m2 = mask_address(&env, &address, PrivacyLevel::Anonymous);

    match (m1, m2) {
        (MaskedAddress::Hashed(h1), MaskedAddress::Hashed(h2)) => {
            assert_eq!(h1, h2, "Same address must produce identical hashes");
        }
        _ => panic!("Expected MaskedAddress::Hashed for both"),
    }
}

#[test]
fn test_anonymous_different_addresses_produce_different_hashes() {
    let env = setup_env();
    let addr1 = Address::generate(&env);
    let addr2 = Address::generate(&env);

    let m1 = mask_address(&env, &addr1, PrivacyLevel::Anonymous);
    let m2 = mask_address(&env, &addr2, PrivacyLevel::Anonymous);

    match (m1, m2) {
        (MaskedAddress::Hashed(h1), MaskedAddress::Hashed(h2)) => {
            assert_ne!(h1, h2, "Different addresses must produce different hashes");
        }
        _ => panic!("Expected MaskedAddress::Hashed for both"),
    }
}

// ---------------------------------------------------------------------------
// Cross-mode: address not leaked in non-Standard modes
// ---------------------------------------------------------------------------

#[test]
fn test_no_raw_address_in_anonymous_mode() {
    let env = setup_env();
    let address = Address::generate(&env);

    let masked = mask_address(&env, &address, PrivacyLevel::Anonymous);

    // Must NOT be a Full variant (would expose the raw address).
    assert!(
        !matches!(masked, MaskedAddress::Full(_)),
        "Anonymous mode must not return the raw address"
    );
}

#[test]
fn test_no_raw_address_in_private_mode() {
    let env = setup_env();
    let address = Address::generate(&env);

    let masked = mask_address(&env, &address, PrivacyLevel::Private);

    assert!(
        !matches!(masked, MaskedAddress::Full(_)),
        "Private mode must not return the raw address"
    );
}
