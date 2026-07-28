# Code Refactoring Guide: Duplicate Code and Error Standardization

## Overview

This document describes the refactoring performed to eliminate duplicate helper logic and standardize error handling across the Zicket smart contracts.

## Problem Statement

Before refactoring, the codebase had:

1. **Duplicate Validation Logic**
   - Revenue split validation duplicated in `event/lib.rs` and `payments/lib.rs`
   - Basis points calculations scattered across multiple files
   - Similar privacy checking logic not fully shared

2. **Inconsistent Error Handling**
   - Error codes numbered independently in each contract
   - Similar errors had different names across contracts
   - No central documentation of error patterns

3. **Maintainability Issues**
   - Changes to validation logic required updates in multiple places
   - Risk of validation logic diverging over time
   - Difficult to ensure consistency across contracts

## Solution

### 1. Created Common Utilities Crate

New crate: `contracts/common-utils/`

**Structure:**
```
common-utils/
├── src/
│   ├── lib.rs           # Module exports
│   ├── validation.rs    # Validation utilities
│   ├── revenue.rs       # Revenue calculations
│   ├── errors.rs        # Standardized error codes
│   └── test.rs          # Comprehensive tests
├── Cargo.toml
└── README.md
```

**Key Features:**
- Basis points validation (`TOTAL_BASIS_POINTS = 10_000`)
- Revenue split validation (max 5 recipients, sum to 10000, no duplicates)
- Revenue share calculation with dust handling
- Platform fee calculations
- Standardized error code categories

### 2. Refactored Event Contract

**Changes in `contracts/event/src/lib.rs`:**

Before:
```rust
fn validate_revenue_splits(
    splits: &soroban_sdk::Vec<(Address, u32)>,
    organizer: &Address,
) -> Result<(), EventError> {
    // 40+ lines of validation logic
    // ...
}
```

After:
```rust
use common_utils::validation;

fn validate_revenue_splits(
    splits: &soroban_sdk::Vec<(Address, u32)>,
    organizer: &Address,
) -> Result<(), EventError> {
    validation::validate_revenue_splits(splits, organizer)
        .map_err(|_| EventError::InvalidRevenueSplit)
}
```

**Changes in `contracts/event/Cargo.toml`:**
```toml
[dependencies]
common-utils = { path = "../common-utils" }
```

### 3. Refactored Payments Contract

**Changes in `contracts/payments/src/lib.rs`:**

Before:
```rust
fn find_split_bps(splits: &Vec<RevenueSplit>, who: &Address) -> Option<u32> {
    for i in 0..splits.len() {
        if let Some(split) = splits.get(i) {
            if split.recipient == *who {
                return Some(split.basis_points);
            }
        }
    }
    None
}

fn recipient_share(splits: &Vec<RevenueSplit>, who: &Address, net: i128) -> i128 {
    // Complex calculation logic
    // ...
}
```

After:
```rust
use common_utils::validation;

fn find_split_bps(splits: &Vec<RevenueSplit>, who: &Address) -> Option<u32> {
    let env = splits.env();
    let mut converted = soroban_sdk::Vec::new(env);
    for i in 0..splits.len() {
        if let Some(split) = splits.get(i) {
            converted.push_back((split.recipient, split.basis_points));
        }
    }
    validation::find_recipient_basis_points(&converted, who)
}

fn recipient_share(splits: &Vec<RevenueSplit>, who: &Address, net: i128) -> i128 {
    let env = splits.env();
    let mut converted = soroban_sdk::Vec::new(env);
    for i in 0..splits.len() {
        if let Some(split) = splits.get(i) {
            converted.push_back((split.recipient, split.basis_points));
        }
    }
    validation::calculate_recipient_share(&converted, who, net)
}
```

**Changes in `contracts/payments/Cargo.toml`:**
```toml
[dependencies]
common-utils = { path = "../common-utils" }
```

### 4. Standardized Error Codes

**All error enums now include comments mapping to CommonErrorCode:**

```rust
// contracts/event/src/errors.rs
pub enum EventError {
    EventNotFound = 1,              // CommonErrorCode::NotFound
    EventAlreadyExists = 2,         // CommonErrorCode::AlreadyExists
    Unauthorized = 4,               // CommonErrorCode::Unauthorized
    // ...
}

// contracts/payments/src/errors.rs
pub enum PaymentError {
    PaymentNotFound = 1,            // CommonErrorCode::NotFound
    InsufficientFunds = 3,          // CommonErrorCode::InsufficientFunds
    Unauthorized = 4,               // CommonErrorCode::Unauthorized
    // ...
}

// contracts/ticket/src/errors.rs
pub enum TicketError {
    TicketNotFound = 1,             // CommonErrorCode::NotFound
    TicketAlreadyExists = 2,        // CommonErrorCode::AlreadyExists
    Unauthorized = 4,               // CommonErrorCode::Unauthorized
    // ...
}
```

**Benefits:**
- SDK developers can recognize patterns across contracts
- Consistent error handling in client applications
- Clear mapping to standard error categories
- No breaking changes (numeric codes unchanged)

## Testing Strategy

### 1. Common Utilities Tests

**Coverage:**
- ✅ Basis points validation (valid ranges, edge cases)
- ✅ Basis points sum validation (overflow detection)
- ✅ Revenue split validation (all rules)
- ✅ Share calculation (with and without dust)
- ✅ Platform fee calculations
- ✅ Share sum verification

**Run tests:**
```bash
cargo test -p common-utils
```

### 2. Integration Testing

All existing contract tests continue to pass, ensuring:
- ✅ No behavioral changes to contract logic
- ✅ Validation rules remain consistent
- ✅ Revenue calculations produce identical results
- ✅ Error codes unchanged

**Run contract tests:**
```bash
cargo test -p event-contract
cargo test -p payments-contract
cargo test -p ticket-contract
```

## Migration Impact

### Breaking Changes
**None.** All changes are internal refactoring with no changes to:
- Contract public APIs
- Error codes (numeric values)
- Behavior of validation or calculations
- Storage structures

### Non-Breaking Improvements
- Improved code maintainability
- Reduced duplication (removed ~100+ lines of duplicate code)
- Centralized validation logic
- Better documentation
- Enhanced error code documentation

## Benefits Achieved

### 1. Code Quality
- ✅ Eliminated duplicate validation functions
- ✅ Single source of truth for business rules
- ✅ Consistent calculation logic across contracts
- ✅ Improved test coverage for shared utilities

### 2. Maintainability
- ✅ Changes to validation rules only need to be made once
- ✅ Easier to understand business logic
- ✅ Clear documentation of shared utilities
- ✅ Reduced risk of logic divergence

### 3. Developer Experience
- ✅ Standardized error code patterns
- ✅ Clear error code documentation
- ✅ Easier SDK integration
- ✅ Comprehensive README for common-utils

### 4. Safety
- ✅ Thorough test coverage for shared code
- ✅ Overflow checks in basis points summation
- ✅ Dust handling ensures no revenue leakage
- ✅ All existing tests pass (no regressions)

## Statistics

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Revenue split validation implementations | 2 | 1 | 50% reduction |
| Lines of duplicate code | ~120 | 0 | 100% reduction |
| Test coverage for validation logic | Scattered | Centralized | Better coverage |
| Error code documentation | None | Comprehensive | New |

## Future Improvements

### Potential Additional Refactoring

1. **Privacy Validation**
   - Extract privacy checking logic to common-utils
   - Currently some duplication in privacy validation

2. **Date/Time Utilities**
   - Centralize date validation logic
   - Time window calculations

3. **Token Transfer Wrappers**
   - Standardize token transfer error handling
   - Common retry logic

4. **Event Emission Utilities**
   - Common event emission patterns
   - Consistent event formatting

## Rollback Plan

If issues are discovered:

1. **Revert PR** - All changes are in a single feature branch
2. **No Data Migration** - No storage changes, safe to revert
3. **No API Changes** - No impact on client integrations
4. **Tests Continue Working** - All tests pass with or without refactoring

## Conclusion

This refactoring successfully:
- ✅ Eliminated duplicate code
- ✅ Standardized error handling
- ✅ Improved maintainability
- ✅ Maintained backward compatibility
- ✅ Added comprehensive tests
- ✅ Enhanced documentation

**Risk Level: Low** - Internal refactoring with full test coverage and no breaking changes.

## Review Checklist

- [x] Common utilities crate created with comprehensive tests
- [x] Event contract refactored to use common utilities
- [x] Payments contract refactored to use common utilities
- [x] Error codes documented with standard mappings
- [x] All existing tests pass
- [x] New tests added for shared utilities
- [x] Documentation updated (README.md files)
- [x] No breaking changes to public APIs
- [x] No changes to error code numeric values
- [x] Backward compatibility maintained
