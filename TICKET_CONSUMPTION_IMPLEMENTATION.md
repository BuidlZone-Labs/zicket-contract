# Ticket Consumption Implementation (Check-in/Entry Validation)

## Overview

This document describes the implementation of ticket consumption functionality that allows tickets to be marked as "used" when entering an event.

## Implementation Details

### Location
- **Main Implementation**: `contracts/ticket/src/lib.rs` (lines 151-191)
- **Type Definitions**: `contracts/ticket/src/types.rs` (lines 5-9, 13-21)
- **Error Handling**: `contracts/ticket/src/errors.rs` (line 19)
- **Events**: `contracts/ticket/src/events.rs` (lines 12-18, 54-62)
- **Tests**: `contracts/ticket/src/test.rs` (lines 232-343)

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
   
4. **Status Validation**: Ensures ticket is in `Valid` status
   - Returns `TicketAlreadyUsed` if already used
   - Returns `EventNotActive` if cancelled

5. **Status Update**: Changes ticket status from `Valid` to `Used`
   - Persists the updated ticket to storage

6. **Event Emission**: Emits `TicketUsed` event with timestamp

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
   - Verifies status changes to `Used`

2. **Double Check-in Prevention** (`test_use_ticket_double_checkin`)
   - Attempts to use already used ticket
   - Expects `TicketAlreadyUsed` error

3. **Unauthorized Usage** (`test_use_ticket_unauthorized`)
   - Non-organizer attempts to use ticket
   - Expects `Unauthorized` error

4. **Cancelled Ticket Usage** (`test_use_ticket_cancelled`)
   - Attempts to use cancelled ticket
   - Expects `EventNotActive` error

## Acceptance Criteria Met

✅ **Tickets cannot be reused**
- Implemented via `TicketAlreadyUsed` error
- Status validation prevents double consumption

✅ **Organizer-only validation**
- Implemented via `organizer.require_auth()`
- Additional organizer verification check

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

## Security Considerations

- Organizer authorization prevents unauthorized check-ins
- Status validation prevents ticket reuse
- Immutable status transitions prevent fraud
- Event emission provides audit trail

## Future Enhancements

Potential improvements for future iterations:
- Time-based validation (event date/time windows)
- Batch check-in functionality
- QR code integration support
- Analytics dashboard integration
