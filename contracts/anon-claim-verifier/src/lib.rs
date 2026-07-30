#![no_std]

use soroban_sdk::{contract, contractimpl, Bytes, Env};
use ultrazk_soroban_verifier::{UltraHonkVerifier, PROOF_BYTES};

const PUBLIC_INPUT_BYTES: u32 = 5 * 32;
const VERIFICATION_KEY: &[u8] = include_bytes!("../fixtures/vk");

#[contract]
pub struct AnonymousClaimVerifier;

#[contractimpl]
impl AnonymousClaimVerifier {
    pub fn verify(env: Env, proof: Bytes, public_inputs: Bytes) -> bool {
        if proof.len() != PROOF_BYTES as u32 || public_inputs.len() != PUBLIC_INPUT_BYTES {
            return false;
        }

        let vk = Bytes::from_slice(&env, VERIFICATION_KEY);
        let Ok(verifier) = UltraHonkVerifier::new(&env, &vk) else {
            return false;
        };

        verifier.verify(&env, &proof, &public_inputs).is_ok()
    }
}

#[cfg(test)]
mod test;
