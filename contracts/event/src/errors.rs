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
    /// The requested refund-choice window is shorter than the mandatory minimum
    /// (`MIN_POSTPONEMENT_CHOICE_WINDOW_LEDGERS`).
    PostponementWindowTooShort = 30,
    /// The proposed new event date is not strictly after the close of the
    /// refund-choice window, or is in the past.
    InvalidPostponementDate = 31,
    /// The event has already been postponed the maximum number of times
    /// (`MAX_POSTPONEMENTS`); the organizer must run or cancel it instead.
    MaxPostponementsReached = 32,
    /// `finalize_postponement` was called while the refund-choice window is still open.
    PostponementWindowOpen = 33,
    /// The operation requires the event to be in the `Postponed` state.
    EventNotPostponed = 34,
}
