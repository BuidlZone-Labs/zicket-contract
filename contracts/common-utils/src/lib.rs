#![no_std]

//! Common utilities shared across Zicket smart contracts.
//!
//! This crate provides shared validation, calculation, and helper functions
//! to reduce code duplication and ensure consistency across all contracts.

pub mod errors;
pub mod revenue;
pub mod validation;

pub use revenue::*;
pub use validation::*;

#[cfg(test)]
mod test;
