//! Standardized error codes and utilities for error handling across contracts.
//!
//! This module provides a common set of error codes that can be mapped to
//! contract-specific error types, ensuring consistency in error handling
//! and simplifying SDK integration.

/// Standard error code categories used across all contracts.
/// These provide a consistent baseline that contract-specific errors can map to.
#[repr(u32)]
pub enum CommonErrorCode {
    // Resource errors (1-10)
    NotFound = 1,
    AlreadyExists = 2,

    // Authorization errors (11-20)
    Unauthorized = 11,

    // Validation errors (21-40)
    InvalidInput = 21,
    InvalidAmount = 22,
    InvalidStatusTransition = 23,
    InvalidFeeBps = 24,

    // State errors (41-60)
    NotActive = 41,
    NotCompleted = 42,
    AlreadyProcessed = 43,

    // Configuration errors (61-80)
    NotInitialized = 61,
    NotConfigured = 62,

    // Business logic errors (81-100)
    InsufficientFunds = 81,
    MaxLimitReached = 82,
    SoldOut = 83,

    // System errors (101-120)
    ContractPaused = 101,
    TransferFailed = 102,
    AccountingMismatch = 103,

    // Migration errors (121-130)
    MigrationFailed = 121,
    UnsupportedVersion = 122,
}

/// Common error messages for standardized errors.
pub const fn error_message(code: u32) -> &'static str {
    match code {
        1 => "Resource not found",
        2 => "Resource already exists",
        11 => "Unauthorized access",
        21 => "Invalid input provided",
        22 => "Invalid amount",
        23 => "Invalid status transition",
        24 => "Invalid fee basis points",
        41 => "Resource not active",
        42 => "Operation not completed",
        43 => "Already processed",
        61 => "Contract not initialized",
        62 => "Feature not configured",
        81 => "Insufficient funds",
        82 => "Maximum limit reached",
        83 => "Sold out",
        101 => "Contract is paused",
        102 => "Transfer failed",
        103 => "Accounting mismatch detected",
        121 => "Migration failed",
        122 => "Unsupported version",
        _ => "Unknown error",
    }
}
