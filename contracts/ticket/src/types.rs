use soroban_sdk::{contracttype, Address, Symbol};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum TicketStatus {
    Valid,
    Used,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Ticket {
    pub ticket_id: u64,
    pub event_id: Symbol,
    pub organizer: Address,
    pub owner: Address,
    pub issued_at: u64,
    pub status: TicketStatus,
    pub is_transferable: bool,
    pub is_used: bool,
}

/// Maximum number of tickets that can be minted in a single batch operation.
///
/// Each mint operation performs multiple persistent storage write operations:
/// - `DataKey::Ticket(ticket_id)`
/// - `DataKey::OwnerTicket(owner, ticket_id)`
/// - `DataKey::OwnerTicketIndex(owner, count)`
/// - `DataKey::EventTicket(event_id, ticket_id)`
/// - `DataKey::EventTicketIndex(event_id, count)`
///
/// Shared batch storage updates:
/// - `DataKey::OwnerTicketsCount(owner)`
/// - `DataKey::EventTicketsCount(event_id)`
/// - `DataKey::NextTicketId`
///
/// Total persistent storage writes for `N` tickets equal `(5 * N) + 3`.
/// Capping the batch at 8 keeps total persistent storage writes (43 writes) and total
/// footprint entries strictly within Soroban's invocation limits (50 write entries, 100 footprint entries).
pub const MAX_BATCH_TICKET_MINT: u32 = 8;
