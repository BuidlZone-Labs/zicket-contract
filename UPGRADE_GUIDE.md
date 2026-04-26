# Contract Storage Upgradability Guide

## Overview

This document describes the upgrade-safety mechanisms implemented across all contracts in the Zicket system. These mechanisms ensure contracts can be upgraded without breaking existing storage or disrupting operations.

## Implementation Details

### 1. Contract Versioning

Each contract now tracks its version via the `ContractVersion` storage key:

**Event Contract** - `storage::get_contract_version()` → Current version
**Factory Contract** - `storage::get_contract_version()` → Current version
**Ticket Contract** - `storage::get_contract_version()` → Current version
**Payments Contract** - `storage::get_contract_version()` → Current version

#### Initial Version
- Version defaults to 1 if not explicitly set
- Version is set during initialization and updated during migrations

### 2. Migration Entry Points

Each contract implements a `migrate()` function that handles contract upgrades:

```rust
// Event Contract
pub fn migrate(env: Env, admin: Address) -> Result<u32, EventError>

// Factory Contract
pub fn migrate(env: Env, admin: Address) -> Result<u32, FactoryError>

// Ticket Contract
pub fn migrate(env: Env, caller: Address) -> Result<u32, TicketError>

// Payments Contract
pub fn migrate(env: Env, admin: Address) -> Result<u32, PaymentError>
```

#### Migration Process

1. **Authorization**: Only authorized users (admin) can trigger migrations
2. **Version Increment**: Current version is automatically incremented
3. **Data Transformation**: Version-specific migrations handle data layout changes
4. **Backward Compatibility**: Old data formats are validated and converted if needed

### 3. Storage Compatibility

#### DataKey Structure
All contracts use `contracttype` enums for storage keys, ensuring:
- **Type Safety**: Soroban enforces type checking at compile time
- **Layout Preservation**: Adding new keys doesn't invalidate existing data
- **Selective Updates**: Only keys that need migration are updated

#### Current DataKeys

**Event Contract:**
- Event(Symbol) - Event data by ID
- Registration(Symbol, Address) - Registration records
- EventAttendees(Symbol) - List of attendees
- Reservation(Symbol, Address) - Ticket reservations
- Admin - Admin address
- TicketContract - Ticket contract address
- PaymentsContract - Payments contract address
- EventPrivacy(Symbol) - Privacy levels
- **ContractVersion** - Contract version (NEW)

**Factory Contract:**
- Admin - Admin address
- EventWasm - Event contract WASM hash
- DeployedEvent(Symbol) - Deployed event records
- AllEvents - List of all events
- OrganizerEvents(Address) - Events by organizer
- TicketContract - Ticket contract address
- PaymentsContract - Payments contract address
- **ContractVersion** - Contract version (NEW)

**Ticket Contract:**
- Ticket(u64) - Ticket data by ID
- OwnerTickets(Address) - Tickets owned by address
- EventTickets(Symbol) - Tickets for event
- NextTicketId - Counter for ticket IDs
- **ContractVersion** - Contract version (NEW)

**Payments Contract:**
- Admin - Admin address
- AcceptedToken - Accepted payment token
- EventContract - Event contract address
- EventPrivacy(Symbol) - Privacy settings
- EventConfig(Symbol) - Event configuration
- Payment(u64) - Payment records
- Ticket(u64) - Ticket records
- EventPayments(Symbol) - Payments for event
- EventRevenue(Symbol) - Revenue for event
- EventStatus(Symbol) - Event status
- OwnerTickets(Address) - Tickets by owner
- WithdrawalHistory(Symbol) - Withdrawal records
- NextPaymentId - Counter for payment IDs
- NextTicketId - Counter for ticket IDs
- EmissionPrivacy(Symbol) - Emission privacy levels
- EscrowMeta(Symbol) - Escrow metadata
- **ContractVersion** - Contract version (NEW)

## Usage

### Getting Contract Version

```rust
// Get the current version of the contract
let version = EventContract::contract_version(env);
```

### Performing a Migration

```rust
let admin = Address::from_contract_id(&env, &contract_id);

// Trigger migration
let new_version = EventContract::migrate(env, admin)?;
// Returns: new version number
```

### Checking Version Compatibility

```rust
// Verify the contract version is compatible
storage::verify_version(&env)?;  // Returns Ok if version <= CURRENT_VERSION
```

## Version Transition Matrix

### Event Contract
| From | To | Action |
|------|----|----|
| 0 | 1 | Initialize version tracking |
| 1 | 2 | Reserved for future migrations |

### Factory Contract
| From | To | Action |
|------|----|----|
| 0 | 1 | Initialize version tracking |
| 1 | 2 | Reserved for future migrations |

### Ticket Contract
| From | To | Action |
|------|----|----|
| 0 | 1 | Initialize version tracking |
| 1 | 2 | Reserved for future migrations |

### Payments Contract
| From | To | Action |
|------|----|----|
| 0 | 1 | Initialize version tracking |
| 1 | 2 | Reserved for future migrations |

## Error Handling

### New Error Types

**EventError:**
- `MigrationFailed` (21) - Migration process failed
- `UnsupportedVersion` (22) - Contract version exceeds supported version

**FactoryError:**
- `MigrationFailed` (5) - Migration process failed
- `UnsupportedVersion` (6) - Contract version exceeds supported version

**TicketError:**
- `MigrationFailed` (15) - Migration process failed
- `UnsupportedVersion` (16) - Contract version exceeds supported version

**PaymentError:**
- `MigrationFailed` (22) - Migration process failed
- `UnsupportedVersion` (23) - Contract version exceeds supported version

## Testing

All contracts include comprehensive migration tests in `migration_test.rs`:

### Tests Included

1. **Version Initialization Test**
   - Verifies initial contract version is set correctly
   
2. **Migration v1→v2 Test**
   - Tests the migration process
   - Verifies version increments
   
3. **Authorization Test**
   - Ensures only authorized users can migrate
   
4. **Storage Compatibility Test**
   - Verifies existing data remains accessible after migration
   
5. **Multiple Migrations Test**
   - Tests sequential migrations
   
6. **Version Compatibility Check Test**
   - Validates version verification logic

## Storage Preservation Strategy

### Non-Breaking Changes (No Migration Needed)

1. **Adding new optional fields** - Use a new DataKey
2. **Adding new mapping keys** - Use new variants in DataKey enum
3. **Extending functionality** - Implement as new functions

### Breaking Changes (Migration Required)

1. **Changing field types** - Requires data transformation
2. **Removing fields** - Data must be archived
3. **Restructuring data layout** - Version migration handles conversion

### Standard TTL Configuration

All storage entries use:
- **TTL Threshold**: 60 × 60 × 24 × 30 = 2,592,000 seconds (~30 days)
- **TTL Bump**: 60 × 60 × 24 × 30 × 2 = 5,184,000 seconds (~60 days)

## Best Practices

1. **Always increment version** when making breaking changes
2. **Write migration logic** for each version transition
3. **Test migrations thoroughly** with real data
4. **Preserve backward compatibility** when possible
5. **Document data transformations** in version handlers
6. **Use TTL wisely** to prevent excessive storage costs

## Future Enhancements

Potential improvements for future versions:

1. **State snapshots** - Save contract state before migration
2. **Rollback capability** - Ability to revert to previous version
3. **Migration hooks** - Custom logic between versions
4. **Atomic migrations** - Ensure all-or-nothing updates
5. **Event emissions** - Track migration history on-chain
