use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EventError {
    EventNotFound = 1,
    EventAlreadyExists = 2,
    InvalidStatusTransition = 3,
    Unauthorized = 4,
    InvalidInput = 5,
    EventNotActive = 6,
    InvalidEventDate = 7,
    InvalidTicketCount = 8,
    InvalidPrice = 9,
    EventNotUpdatable = 10,
    EventSoldOut = 11,
    AlreadyRegistered = 12,
    TierNotFound = 13,
    TierSoldOut = 14,
    ContractLinksNotConfigured = 15,
    RefundFailed = 16,
    ReservationNotFound = 17,
    ReservationExpired = 18,
    InvalidOrganizer = 19,
    InvalidPayoutToken = 20,
    MigrationFailed = 21,
    UnsupportedVersion = 22,
    UnauthorizedPrivateAccess = 23,
    PrivacyViolation = 24,
    ClaimLimitExceeded = 25,
    ClaimCooldownActive = 26,
    AnonCommitmentReused = 27,
    AnonClaimWindowFull = 28,
    AnonymousClaimsNotEnabled = 29,
    ///
    ///
    PostponementWindowTooShort = 30,
    ///
    ///
    InvalidPostponementDate = 31,
    ///
    ///
    MaxPostponementsReached = 32,
    ///
    PostponementWindowOpen = 33,
    ///
    EventNotPostponed = 34,
    ///
    ///
    ///
    NoRefundableTicket = 35,
    ///
    ZkProofExpired = 36,
    ///
    ///
    ZkNullifierReused = 37,
    ///
    ///
    ZkVerificationRequired = 38,
    ///
    ///
    ZkProofInvalid = 39,
    ///
    ///
    ZkClaimTypeMismatch = 40,
}
