use soroban_sdk::contracterror;

/// Event contract error codes.
///
/// These errors follow common patterns defined in `common_utils::errors::CommonErrorCode`
/// where applicable, but are numbered to avoid conflicts and maintain backward compatibility.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EventError {
    EventNotFound = 1,           // CommonErrorCode::NotFound
    EventAlreadyExists = 2,      // CommonErrorCode::AlreadyExists
    InvalidStatusTransition = 3, // CommonErrorCode::InvalidStatusTransition
    Unauthorized = 4,            // CommonErrorCode::Unauthorized
    InvalidInput = 5,            // CommonErrorCode::InvalidInput
    EventNotActive = 6,          // CommonErrorCode::NotActive
    InvalidEventDate = 7,
    InvalidTicketCount = 8,
    InvalidPrice = 9, // CommonErrorCode::InvalidAmount
    EventNotUpdatable = 10,
    EventSoldOut = 11,      // CommonErrorCode::SoldOut
    AlreadyRegistered = 12, // CommonErrorCode::AlreadyProcessed
    TierNotFound = 13,
    TierSoldOut = 14,                // CommonErrorCode::SoldOut
    ContractLinksNotConfigured = 15, // CommonErrorCode::NotConfigured
    RefundFailed = 16,
    ReservationNotFound = 17,
    ReservationExpired = 18,
    InvalidOrganizer = 19,
    InvalidPayoutToken = 20,
    MigrationFailed = 21,           // CommonErrorCode::MigrationFailed
    UnsupportedVersion = 22,        // CommonErrorCode::UnsupportedVersion
    UnauthorizedPrivateAccess = 23, // CommonErrorCode::Unauthorized
    PrivacyViolation = 24,
    ClaimLimitExceeded = 25, // CommonErrorCode::MaxLimitReached
    ClaimCooldownActive = 26,
    AnonCommitmentReused = 27,
    AnonClaimWindowFull = 28, // CommonErrorCode::MaxLimitReached
    AnonymousClaimsNotEnabled = 29,
    /// The requested refund-choice window is shorter than the mandatory minimum
    /// (`MIN_POSTPONEMENT_CHOICE_WINDOW_LEDGERS`).
    PostponementWindowTooShort = 30,
    /// The proposed new event date is not strictly after the close of the
    /// refund-choice window, or is in the past.
    InvalidPostponementDate = 31,
    /// The event has already been postponed the maximum number of times
    /// (`MAX_POSTPONEMENTS`); the organizer must run or cancel it instead.
    MaxPostponementsReached = 32, // CommonErrorCode::MaxLimitReached
    /// `finalize_postponement` was called while the refund-choice window is still open.
    PostponementWindowOpen = 33,
    /// The operation requires the event to be in the `Postponed` state.
    EventNotPostponed = 34,
    /// The caller holds no revocable (valid, unused) ticket for the event, so a
    /// postponement refund cannot be issued (e.g. the ticket was already used or
    /// transferred away).
    NoRefundableTicket = 35,
    /// Revenue split is malformed: wrong basis-point sum, too many recipients,
    /// a zero/duplicate recipient, or index 0 is not the primary organizer.
    InvalidRevenueSplit = 36, // CommonErrorCode::InvalidInput
    // -- zkPassport errors ----------------------------------------------------
    /// The proof's `expiry_ledger` is less than the current ledger sequence.
    ZkProofExpired = 37,
    /// This nullifier has already been recorded for this event -- proof reuse
    /// is not allowed.
    ZkNullifierReused = 38,
    /// The event `requires_verification` is `true` but the ZkVerificationConfig
    /// has not been enabled by the organizer.
    ZkVerificationRequired = 39,
    /// Reserved for future on-chain verifier integration. Currently signals that
    /// the provided proof bytes are structurally invalid.
    ZkProofInvalid = 40,
    /// The submitted `ZkPassportClaim.claim_type` does not match the type
    /// required by the event's `ZkVerificationConfig`.
    ZkClaimTypeMismatch = 41,
    /// The event is configured for Private/Anonymous payment privacy, which the
    /// cross-contract `register_for_event` path cannot settle because it has no
    /// client-generated privacy material (stealth key / nullifier commitment).
    /// Use the payments contract's privacy-aware entry point directly.
    PaymentPrivacyUnsupported = 42,
}
