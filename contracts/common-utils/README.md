# Common Utilities

This crate provides shared validation, calculation, and helper functions used across all Zicket smart contracts. It was created to eliminate code duplication and ensure consistency in business logic validation and error handling.

## Modules

### `validation`

Common validation utilities for contract inputs and business rules:

- **Basis Points Validation**
  - `is_valid_basis_points(bps)` - Validates basis points are in range 0-10000
  - `validate_basis_points_sum(values)` - Sums values with overflow detection; returns `Some(total)` on success or `None` on overflow. Callers must verify the result equals `TOTAL_BASIS_POINTS` (10,000) for complete allocation validation.
  - Constants: `TOTAL_BASIS_POINTS = 10_000`, `MAX_REVENUE_SPLIT_RECIPIENTS = 5`

- **Revenue Split Validation**
  - `validate_revenue_splits(splits, organizer)` - Comprehensive validation of revenue splits
    - Empty splits allowed (single organizer mode)
    - Maximum 5 recipients
    - Basis points must sum to exactly 10000
    - No zero allocations
    - No duplicate recipients
    - Index 0 must be the primary organizer

- **Revenue Share Calculation**
  - `calculate_recipient_share(splits, recipient, net_amount)` - Calculates individual share
    - Uses floor division for non-primary recipients
    - Primary organizer (index 0) receives remainder (captures dust)
    - Ensures sum of shares equals net amount (no dust leakage)
  - `find_recipient_basis_points(splits, recipient)` - Finds allocation for a recipient
  - `is_split_recipient(splits, address)` - Checks if address is a recipient

### `revenue`

Revenue calculation and distribution utilities:

- **Fee Calculations**
  - `calculate_platform_fee(gross_amount, platform_fee_bps)` - Calculates platform fee
  - `calculate_net_amount(gross_amount, platform_fee_bps)` - Net after fee deduction

- **Share Distribution**
  - `calculate_all_shares(splits, net_amount)` - Calculates all recipient shares
  - `verify_shares_sum(shares, expected_total)` - Verifies no dust leakage

### `errors`

Standardized error codes and utilities for error handling:

- **CommonErrorCode** - Standard error categories across all contracts
  - Resource errors (1-10): NotFound, AlreadyExists
  - Authorization errors (11-20): Unauthorized
  - Validation errors (21-40): InvalidInput, InvalidAmount, InvalidStatusTransition, InvalidFeeBps
  - State errors (41-60): NotActive, NotCompleted, AlreadyProcessed
  - Configuration errors (61-80): NotInitialized, NotConfigured
  - Business logic errors (81-100): InsufficientFunds, MaxLimitReached, SoldOut
  - System errors (101-120): ContractPaused, TransferFailed, AccountingMismatch
  - Migration errors (121-130): MigrationFailed, UnsupportedVersion

- **error_message(code)** - Provides human-readable messages for standard error codes

## Usage

### Adding to Your Contract

1. Add dependency to `Cargo.toml`:
```toml
[dependencies]
common-utils = { path = "../common-utils" }
```

2. Import in your contract:
```rust
use common_utils::validation;
use common_utils::revenue;
```

### Example: Validating Revenue Splits

```rust
use common_utils::validation;

fn validate_splits(
    splits: &soroban_sdk::Vec<(Address, u32)>,
    organizer: &Address,
) -> Result<(), ContractError> {
    validation::validate_revenue_splits(splits, organizer)
        .map_err(|_| ContractError::InvalidSplitConfig)
}
```

### Example: Calculating Revenue Shares

```rust
use common_utils::validation;

fn calculate_shares(
    splits: &soroban_sdk::Vec<(Address, u32)>,
    recipient: &Address,
    net_revenue: i128,
) -> i128 {
    validation::calculate_recipient_share(splits, recipient, net_revenue)
}
```

## Benefits

1. **Code Reuse** - Eliminates duplicate validation logic across contracts
2. **Consistency** - Ensures all contracts use the same validation rules
3. **Maintainability** - Single source of truth for common business logic
4. **Testing** - Comprehensive test coverage for shared utilities
5. **Documentation** - Clear documentation of validation rules and calculations

## Design Decisions

### Dust Handling

The revenue share calculation uses a specific strategy to handle integer division dust:
- Non-primary recipients receive `floor(net * bps / 10000)`
- Primary organizer (index 0) receives the remainder
- This ensures the sum of all shares always equals the net amount
- No revenue is ever stranded due to rounding

### Error Code Standardization

Contract-specific error enums retain their existing numeric codes for backward compatibility, but now include comments mapping to `CommonErrorCode` categories. This provides:
- Consistent error patterns for SDK integration
- Human-readable error messages
- No breaking changes to existing contracts

## Testing

Run tests with:
```bash
cargo test -p common-utils
```

The test suite includes:
- Basis points validation edge cases
- Revenue split validation (empty, valid, invalid configurations)
- Share calculation with and without dust
- Platform fee calculations
- Share sum verification

## Future Enhancements

Potential additions to this crate:
- Privacy validation utilities (currently scattered across contracts)
- Date/time validation helpers
- Token transfer wrappers with standard error handling
- Event emission utilities
