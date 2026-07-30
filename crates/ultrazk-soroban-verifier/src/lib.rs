#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(all(test, not(feature = "std")))]
extern crate std;

pub mod debug;
pub mod ec;
pub mod field;
pub mod hash;
pub mod relations;
pub mod shplemini;
pub mod sumcheck;
pub mod transcript;
pub mod types;
pub mod utils;
pub mod verifier;

pub const PROOF_FIELDS: usize = 507;
pub const PROOF_BYTES: usize = PROOF_FIELDS * 32;

pub use verifier::{UltraHonkVerifier, VkLoadError};
