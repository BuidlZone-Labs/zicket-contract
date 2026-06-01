# Ticket Consumption Implementation (Check-in/Entry Validation)

## Overview

This document describes the complete implementation of ticket consumption functionality that allows tickets to be marked as "used" when entering an event.

## Implementation Details

### Location
- **Main Implementation**: `contracts/ticket/src/lib.rs` (lines 152-197)
- **Type Definitions**: `contracts/ticket/src/types.rs` (lines 11-22)
- **Error Handling**: `contracts/ticket/src/errors.rs` (line 19)
- **Events**: `contracts/ticket/src/events.rs` (lines 12-18, 54-62)
- **Tests**: `contracts/ticket/src/test.rs` (lines 232-485)

### Core Function: `use_ticket`

```rust
pub fn use_ticket(env: Env, organizer: Address, ticket_id: u64) -> Result<(), TicketError>
```

**Parameters:**
- `env`: Soroban environment
- `organizer`: Address of the event organizer (must be authorized)
- `ticket_id`: Unique identifier of the ticket to consume

**Return:**
- `Result<(), TicketError>` - Success or error with specific reason

### Validation Logic

1. **Organizer Authorization**: `organizer.require_auth()`
   - Only the event organizer can call this function
   
2. **Ticket Existence**: Retrieves ticket from persistent storage
   - Returns `TicketNotFound` if ticket doesn't exist
   
3. **Organizer Verification**: Verifies caller is the ticket's organizer
   - Returns `Unauthorized` if caller is not the organizer
   
4. **Usage Validation**: Checks `is_used` field and status
   - Returns `TicketAlreadyUsed` if `is_used` is true
   - Returns `EventNotActive` if status is `Cancelled`
   - Returns `TicketAlreadyUsed` if status is `Used`

5. **Status Update**: Sets both `is_used` and `status`
   - Sets `is_used = true`
   - Sets `status = TicketStatus::Used`
   - Persists the updated ticket to storage

6. **Event Emission**: Emits `TicketUsed` event with timestamp

### Ticket Struct Enhancement

The `Ticket` struct now includes an `is_used` field as requested in issue #102:

```rust
pub struct Ticket {
    pub ticket_id: u64,
    pub event_id: Symbol,
    pub organizer: Address,
    pub owner: Address,
    pub issued_at: u64,
    pub status: TicketStatus,
    pub is_transferable: bool,
    pub is_used: bool,  // NEW: Tracks ticket consumption
}
```

### Integration with Other Operations

**Transfer Ticket:**
- Now checks `is_used` field before allowing transfers
- Prevents transferring tickets that have been consumed
- Returns `TicketNotTransferable` error for used tickets

**Cancel Ticket:**
- Now checks `is_used` field before allowing cancellation
- Prevents cancelling tickets that have been consumed
- Returns `TicketAlreadyUsed` error for used tickets

### Ticket Status Flow

```
Valid → Used (via use_ticket)
Valid → Cancelled (via cancel_ticket)
Used → (no transitions allowed)
Cancelled → (no transitions allowed)
```

### Error Types

| Error Code | Error Type | Description |
|------------|------------|-------------|
| 1 | `TicketNotFound` | Ticket ID doesn't exist |
| 4 | `Unauthorized` | Caller is not the organizer |
| 11 | `TicketNotTransferable` | Ticket cannot be transferred (used or disabled) |
| 13 | `TicketAlreadyUsed` | Ticket already consumed |
| 14 | `EventNotActive` | Ticket is cancelled |

### Event Emission

**TicketUsed Event:**
```rust
pub struct TicketUsed {
    pub ticket_id: u64,
    pub event_id: Symbol,
    pub owner: Address,
    pub used_at: u64,
}
```

## Test Coverage

### Test Cases Implemented

1. **Happy Path Usage** (`test_use_ticket_happy_path`)
   - Organizer successfully uses a valid ticket
   - Verifies `is_used` changes to `true`
   - Verifies `status` changes to `Used`

2. **Double Check-in Prevention** (`test_use_ticket_double_checkin`)
   - Attempts to use already used ticket (via `is_used` field)
   - Expects `TicketAlreadyUsed` error

3. **Unauthorized Usage** (`test_use_ticket_unauthorized`)
   - Non-organizer attempts to use ticket
   - Expects `Unauthorized` error

4. **Cancelled Ticket Usage** (`test_use_ticket_cancelled`)
   - Attempts to use cancelled ticket
   - Expects `EventNotActive` error

5. **Transfer Used Ticket** (`test_transfer_used_ticket_via_is_used`)
   - Attempts to transfer a ticket with `is_used = true`
   - Expects `TicketNotTransferable` error

6. **Cancel Used Ticket** (`test_cancel_used_ticket_via_is_used`)
   - Attempts to cancel a ticket with `is_used = true`
   - Expects `TicketAlreadyUsed` error

## Acceptance Criteria Met

✅ **Extend Ticket with is_used: bool field**
- Added `is_used` field to `Ticket` struct
- Initialized to `false` in `mint_ticket`
- Set to `true` in `use_ticket`

✅ **Entry function use_ticket(env, organizer, ticket_id)**
- Function already implemented
- Enhanced with `is_used` field validation
- Maintains backward compatibility with `status` field

✅ **Validation: Only organizer can call**
- Implemented via `organizer.require_auth()`
- Additional organizer verification check

✅ **Validation: Reject already used**
- Implemented via `is_used` field check
- Returns `TicketAlreadyUsed` error
- Also checks `status` field for consistency

✅ **Tests: Use once → success**
- `test_use_ticket_happy_path` verifies successful usage
- Confirms both `is_used` and `status` are updated

✅ **Tests: Use again → fail**
- `test_use_ticket_double_checkin` verifies rejection
- Confirms `TicketAlreadyUsed` error is returned

✅ **Additional: Prevent transfer of used tickets**
- `test_transfer_used_ticket_via_is_used` verifies protection
- Ensures used tickets cannot be transferred

✅ **Additional: Prevent cancel of used tickets**
- `test_cancel_used_ticket_via_is_used` verifies protection
- Ensures used tickets cannot be cancelled

## Usage Example

```rust
// Organizer checks in a ticket
let result = ticket_contract.use_ticket(
    &env,
    &organizer_address,
    ticket_id
);

match result {
    Ok(()) => println!("Ticket successfully checked in"),
    Err(TicketError::TicketAlreadyUsed) => println!("Ticket already used"),
    Err(TicketError::Unauthorized) => println!("Not authorized"),
    // ... other error handling
}
```

## Integration Notes

- The function integrates with existing ticket storage system
- Uses the same `DataKey::Ticket` storage pattern
- Compatible with existing event management system
- Event emission allows for off-chain tracking and analytics
- `is_used` field provides explicit consumption tracking
- Maintains backward compatibility with `status` field

## Security Considerations

- Organizer authorization prevents unauthorized check-ins
- `is_used` field validation prevents ticket reuse
- Status validation provides additional protection
- Immutable status transitions prevent fraud
- Event emission provides audit trail
- Transfer and cancel operations respect `is_used` state

## Future Enhancements

Potential improvements for future iterations:
- Time-based validation (event date/time windows)
- Batch check-in functionality
- QR code integration support
- Analytics dashboard integration
- Consider removing `status` field dependency in favor of `is_used`
