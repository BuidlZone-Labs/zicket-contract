use soroban_sdk::contracterror;

/// Ticket contract error codes.
///
/// These errors follow common patterns defined in `common_utils::errors::CommonErrorCode`
/// where applicable, but are numbered to avoid conflicts and maintain backward compatibility.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TicketError {
    TicketNotFound = 1,             // CommonErrorCode::NotFound
    TicketAlreadyExists = 2,        // CommonErrorCode::AlreadyExists
    InvalidStatusTransition = 3,    // CommonErrorCode::InvalidStatusTransition
    Unauthorized = 4,               // CommonErrorCode::Unauthorized
    InvalidInput = 5,               // CommonErrorCode::InvalidInput
    TicketNotActive = 6,            // CommonErrorCode::NotActive
    InvalidTicketDate = 7,
    InvalidTicketCount = 8,
    InvalidPrice = 9,               // CommonErrorCode::InvalidAmount
    TicketNotUpdatable = 10,
    TicketNotTransferable = 11,
    TransferToSelf = 12,
    TicketAlreadyUsed = 13,         // CommonErrorCode::AlreadyProcessed
    EventNotActive = 14,            // CommonErrorCode::NotActive
    MigrationFailed = 15,           // CommonErrorCode::MigrationFailed
    UnsupportedVersion = 16,        // CommonErrorCode::UnsupportedVersion
    RecoveryKeyNotFound = 17,       // CommonErrorCode::NotFound
    InvalidRecoverySignature = 18,  // CommonErrorCode::InvalidInput
}
