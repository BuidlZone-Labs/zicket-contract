use super::{AnonymousClaimVerifier, AnonymousClaimVerifierClient};
use soroban_sdk::{Bytes, Env};

fn fixture(env: &Env, name: &str) -> Bytes {
    let bytes: &[u8] = match name {
        "proof" => include_bytes!("../fixtures/proof"),
        "public_inputs" => include_bytes!("../fixtures/public_inputs"),
        _ => unreachable!(),
    };
    Bytes::from_slice(env, bytes)
}

#[test]
fn verifies_real_ultrahonk_zk_proof() {
    let env = Env::default();
    let contract_id = env.register(AnonymousClaimVerifier, ());
    let client = AnonymousClaimVerifierClient::new(&env, &contract_id);

    assert!(client.verify(&fixture(&env, "proof"), &fixture(&env, "public_inputs")));
}

#[test]
fn rejects_mutated_public_input() {
    let env = Env::default();
    let contract_id = env.register(AnonymousClaimVerifier, ());
    let client = AnonymousClaimVerifierClient::new(&env, &contract_id);
    let proof = fixture(&env, "proof");
    let mut public_inputs = fixture(&env, "public_inputs");
    public_inputs.set(31, public_inputs.get(31).unwrap() ^ 1);

    assert!(!client.verify(&proof, &public_inputs));
}

#[test]
fn rejects_non_canonical_public_input() {
    const NULLIFIER_PLUS_MODULUS: [u8; 32] = [
        0x32, 0x12, 0x39, 0x99, 0x15, 0x4f, 0x3c, 0x45, 0x29, 0x90, 0x41, 0xa0, 0x31, 0xb8, 0x2e,
        0x0d, 0x6e, 0x33, 0xd2, 0x31, 0xba, 0x48, 0xde, 0xc2, 0x08, 0xac, 0x1c, 0x5c, 0xb3, 0x73,
        0x32, 0x52,
    ];
    let env = Env::default();
    let contract_id = env.register(AnonymousClaimVerifier, ());
    let client = AnonymousClaimVerifierClient::new(&env, &contract_id);
    let proof = fixture(&env, "proof");
    let mut public_inputs = fixture(&env, "public_inputs");
    for (index, byte) in NULLIFIER_PLUS_MODULUS.iter().enumerate() {
        public_inputs.set(96 + index as u32, *byte);
    }

    assert!(!client.verify(&proof, &public_inputs));
}

#[test]
fn rejects_mutated_proof() {
    let env = Env::default();
    let contract_id = env.register(AnonymousClaimVerifier, ());
    let client = AnonymousClaimVerifierClient::new(&env, &contract_id);
    let mut proof = fixture(&env, "proof");
    proof.set(0, proof.get(0).unwrap() ^ 1);

    assert!(!client.verify(&proof, &fixture(&env, "public_inputs")));
}

#[test]
fn rejects_wrong_proof_length() {
    let env = Env::default();
    let contract_id = env.register(AnonymousClaimVerifier, ());
    let client = AnonymousClaimVerifierClient::new(&env, &contract_id);

    assert!(!client.verify(&Bytes::new(&env), &fixture(&env, "public_inputs")));
}
