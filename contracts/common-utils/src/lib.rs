#![no_std]

//! Common utilities shared across Zicket smart contracts.
//!
//! This crate provides shared validation, calculation, and helper functions
//! to reduce code duplication and ensure consistency across all contracts.

pub mod validation;
pub mod revenue;
pub mod errors;

pub use validation::*;
pub use revenue::*;

#[cfg(test)]
mod test;
