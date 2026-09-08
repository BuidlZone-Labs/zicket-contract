Listed directory zicket-contract
Listed directory contracts
Searched for ".*"
Viewed lib.rs:1-800
Viewed lib.rs:801-1367
Viewed lib.rs:1-800
Viewed lib.rs:801-1600
Viewed lib.rs:1601-2306
Viewed lib.rs:1-396

# Smart Contract Security & Architecture Review: Zicket (Stellar / Soroban)

---

# GitHub Issues Backlog

## Issue 1: Unprotected Public Minting in `TicketContract::mint_ticket` Allows Unauthorized Ticket Creation

### Category
Security

### Severity
Critical

### Description
In `TicketContract::mint_ticket` ([`lib.rs`](contracts/ticket/src/lib.rs#L23-L77)), the contract accepts `event_id`, `organizer`, and `owner` parameters to mint a new NFT ticket. However, the function does **not** enforce authorization checks on either the `organizer` or a registered caller contract address (e.g., `event` or `payments` contract).

```rust
pub fn mint_ticket(
    env: Env,
    event_id: Symbol,
    organizer: Address,
    owner: Address,
) -> Result<u64, TicketError> {
    let ticket_id = read_next_ticket_id(&env);
    // ... mints ticket to `owner` without checking caller or requiring organizer.require_auth()
```

Any external actor can call `mint_ticket` directly for any active or sold-out event, minting valid tickets for arbitrary addresses without making any payment.

### Proposed Solution
1. Store an authorized `event_contract` address in `TicketContract` storage during initialization or via an admin function.
2. Restrict `mint_ticket` so that it can only be invoked by the authorized `event_contract` or `payments_contract`, or require `organizer.require_auth()` if direct organizer minting is intentional.

### Acceptance Criteria
- [ ] Root cause identified and access controls added to `mint_ticket`.
- [ ] Invoking `mint_ticket` from unauthorized caller addresses fails with `TicketError::Unauthorized`.
- [ ] Unit and integration tests updated to verify restricted access.
- [ ] Documentation updated.
- [ ] No regressions introduced.

### Estimated Complexity
S

### Labels
security, soroban, stellar, contract, ticket-contract

### Files
- [lib.rs](contracts/ticket/src/lib.rs#L23-L77)
- [storage.rs](contracts/ticket/src/storage.rs)

### Notes
- Dependencies: None.
- Affects core protocol ticketing logic.

---

## Issue 2: Privilege Escalation and Admin Takeover in `TicketContract::set_payments_contract`

### Category
Security

### Severity
Critical

### Description
In `TicketContract::set_payments_contract` ([`lib.rs`](contracts/ticket/src/lib.rs#L291-L306)), if the admin has not yet been initialized in storage, any unauthenticated caller can pass their own address as `admin`. The contract will record this caller as the official admin and set the `payments_contract` address to any contract of their choosing.

```rust
pub fn set_payments_contract(
    env: Env,
    admin: Address,
    payments_contract: Address,
) -> Result<(), TicketError> {
    if let Ok(stored_admin) = storage::get_admin(&env) {
        if admin != stored_admin {
            return Err(TicketError::Unauthorized);
        }
    } else {
        storage::set_admin(&env, &admin); // First caller claims admin!
    }
    admin.require_auth();
    storage::set_payments_contract(&env, &payments_contract);
    Ok(())
}
```

Since `payments_contract` has privileged access to execute `admin_transfer_ticket` ([`lib.rs`](contracts/ticket/src/lib.rs#L308-L354)), an attacker can claim initial admin privileges, point `payments_contract` to an attacker-controlled contract, and force-transfer any ticket from any user.

### Proposed Solution
1. Add a explicit `initialize(env: Env, admin: Address, payments_contract: Address)` method called once upon contract deployment.
2. Prevent `set_payments_contract` from setting the initial admin lazily.

### Acceptance Criteria
- [ ] Dedicated `initialize` method implemented for `TicketContract`.
- [ ] Uninitialized admin state cannot be claimed via `set_payments_contract`.
- [ ] Test cases written for contract deployment and initialization protection.
- [ ] Documentation updated.
- [ ] No regressions introduced.

### Estimated Complexity
S

### Labels
security, soroban, stellar, contract, ticket-contract

### Files
- [lib.rs](contracts/ticket/src/lib.rs#L291-L306)

### Notes
- Dependencies: Requires matching migration updates in deployment scripts.

---

## Issue 3: Missing Admin Authorization Check in `TicketContract::migrate`

### Category
Security

### Severity
High

### Description
In `TicketContract::migrate` ([`lib.rs`](contracts/ticket/src/lib.rs#L360-L381)), the migration method requires authentication from the provided `caller` parameter (`caller.require_auth()`), but fails to verify whether `caller` matches the stored contract `admin`.

```rust
pub fn migrate(env: Env, caller: Address) -> Result<u32, TicketError> {
    caller.require_auth();

    let current_version = storage::get_contract_version(&env);
    let new_version = current_version + 1;
    // ... increments version without validating caller == admin
```

In contrast, both `EventContract::migrate` ([`lib.rs`](contracts/event/src/lib.rs#L1191-L1217)) and `PaymentsContract::migrate` ([`lib.rs`](contracts/payments/src/lib.rs#L1626-L1653)) correctly check `current_admin != admin`. Any arbitrary Stellar account can invoke `TicketContract::migrate` and exhaust allowable contract version upgrades.

### Proposed Solution
Fetch `storage::get_admin(&env)` inside `TicketContract::migrate` and return `TicketError::Unauthorized` if `caller != admin`.

### Acceptance Criteria
- [ ] `caller == admin` check added to `TicketContract::migrate`.
- [ ] Unauthorized caller attempts to migrate fail.
- [ ] Unit tests for migration authority added.
- [ ] Documentation updated.
- [ ] No regressions introduced.

### Estimated Complexity
XS

### Labels
security, soroban, ticket-contract

### Files
- [lib.rs](contracts/ticket/src/lib.rs#L360-L381)

### Notes
- Aligns contract migration safety across all protocol contracts.

---

## Issue 4: Double-Withdrawal Vulnerability in `PaymentsContract::withdraw_revenue` Admin Function

### Category
Security

### Severity
High

### Description
`PaymentsContract::withdraw_revenue` ([`lib.rs`](contracts/payments/src/lib.rs#L1481-L1543)) allows contract admins to bypass normal timing checks and withdraw event revenue. However, unlike `PaymentsContract::withdraw` ([`lib.rs`](contracts/payments/src/lib.rs#L982-L1126)), `withdraw_revenue` does **not** check or update `config.organizer_withdrawn`.

If an admin calls `withdraw_revenue` and transfers funds to `to`, the standard organizer `withdraw` function can still be called afterwards (or vice versa), allowing double-withdrawal of contract funds and draining escrow balance intended for other events.

### Proposed Solution
1. Enforce `config.organizer_withdrawn == false` in `withdraw_revenue`.
2. Update `config.organizer_withdrawn = true` and save `EventConfig` upon completion.
3. Ensure platform fee calculation and revenue splits are honored consistently.

### Acceptance Criteria
- [ ] Root cause identified and `organizer_withdrawn` state check added to `withdraw_revenue`.
- [ ] Double withdrawal attempts return `PaymentError::NoRevenue` or `PaymentError::PaymentAlreadyProcessed`.
- [ ] Automated regression tests added in `test.rs`.
- [ ] Documentation updated.
- [ ] No regressions introduced.

### Estimated Complexity
S

### Labels
security, soroban, payments-contract, accounting

### Files
- [lib.rs](contracts/payments/src/lib.rs#L1481-L1543)

### Notes
- Critical accounting fix for administrative overrides.

---

## Issue 5: Denial of Service Risk via Unbounded Vector Iteration in Settlement and Invariant Verification

### Category
Performance

### Severity
High

### Description
Multiple core routines in `PaymentsContract` and `EventContract` iterate over unbounded vectors stored in persistent storage:
- `PaymentsContract::validate_revenue_invariant` ([`lib.rs`](contracts/payments/src/lib.rs#L176-L244)) iterates through all `payment_ids` and `withdrawal_history` entries for an event on **every withdrawal**.
- `PaymentsContract::collect_held_payments_for_token` ([`lib.rs`](contracts/payments/src/lib.rs#L423-L444)) loops over all event payment IDs.
- `EventContract::get_attendees` ([`lib.rs`](contracts/event/src/lib.rs#L899-L910)) fetches and returns full attendee vectors.

As ticket sales grow (e.g., thousands of attendees), the CPU and memory budget of Soroban ledger invocations will be exceeded. This will cause transactions like `withdraw` or `withdraw_split` to revert permanently due to host resource limits.

### Proposed Solution
1. Maintain aggregate counters for held revenue, refunded amounts, and payment tallies in storage key values rather than re-scanning payment vectors.
2. Replace unbounded payment ID lists with paginated storage mappings or map-based indices (`(event_id, index) -> payment_id`).

### Acceptance Criteria
- [ ] O(N) payment scans removed from revenue withdrawal and settlement paths.
- [ ] Revenue invariants validated using running accumulator counters.
- [ ] Benchmarks or execution cost tests confirm gas/CPU stability under high payment volumes.
- [ ] Documentation updated.
- [ ] No regressions introduced.

### Estimated Complexity
L

### Labels
performance, security, soroban, gas-optimization, architecture

### Files
- [lib.rs](contracts/payments/src/lib.rs#L176-L244)
- [lib.rs](contracts/payments/src/lib.rs#L423-L444)
- [storage.rs](contracts/payments/src/storage.rs)

### Notes
- Essential for production readiness and scaling to high-capacity events.

---

## Issue 6: Missing Soroban Persistent Storage TTL Extension Management

### Category
Bug

### Severity
High

### Description
Contracts across the codebase (`event`, `payments`, `ticket`, `factory`) write data to Soroban persistent storage (`env.storage().persistent().set(...)`), but never invoke `env.storage().persistent().extend_ttl(...)` on stored keys.

In Soroban, persistent entries have a maximum time-to-live (TTL). If entries are not periodically extended, they become archived/expired. Key entries like `Admin`, `EventConfig`, `Ticket`, and `PaymentRecord` will become inaccessible over time, bricking contract reads and user payouts.

### Proposed Solution
1. Implement a helper function in `storage.rs` across contracts to automatically call `extend_ttl` when reading or writing critical persistent storage keys.
2. Establish standard TTL extension thresholds (e.g., extend by 535,680 ledgers ~30 days whenever remaining TTL drops below 100,000 ledgers).

### Acceptance Criteria
- [ ] `extend_ttl` implemented for all storage key accessors.
- [ ] Expiration simulation tests added using Soroban SDK test environment.
- [ ] Documentation updated.
- [ ] No regressions introduced.

### Estimated Complexity
M

### Labels
bug, soroban, storage, stellar

### Files
- [storage.rs](contracts/event/src/storage.rs)
- [storage.rs](contracts/payments/src/storage.rs)
- [lib.rs](contracts/ticket/src/lib.rs)

### Notes
- Critical for long-term state persistence on Stellar Mainnet/Testnet.

---

## Issue 7: Front-Running and Capacity Exhaustion in Unauthenticated `claim_anonymous_ticket`

### Category
Security

### Severity
Medium

### Description
`EventContract::claim_anonymous_ticket` ([`lib.rs`](contracts/event/src/lib.rs#L997-L1059)) does not require any signature or proof of authorization for the `commitment` parameter.

```rust
pub fn claim_anonymous_ticket(
    env: Env,
    event_id: Symbol,
    tier_id: u32,
    commitment: BytesN<32>,
) -> Result<(), EventError>
```

Because commitment claims do not require transaction signers or ZK proofs of commitment knowledge, bot operators can monitor the mempool, copy published commitments, front-run user transactions, or submit batch dummy commitments to exhaust event ticket capacity and flood rate-limit windows (`AnonClaimSettings`).

### Proposed Solution
1. Integrate ZK proof verification or nullifier verification into `claim_anonymous_ticket` to ensure the caller possesses the private key corresponding to the commitment.
2. Add rate-limiting per transaction origin or require relayer authentication where appropriate.

### Acceptance Criteria
- [ ] Commitment claim path requires verifiable proof or signature payload.
- [ ] Front-running test scenario fails to steal commitments.
- [ ] Unit tests for anonymous claims updated.
- [ ] Documentation updated.
- [ ] No regressions introduced.

### Estimated Complexity
M

### Labels
security, privacy, soroban, event-contract

### Files
- [lib.rs](contracts/event/src/lib.rs#L997-L1059)
- [test_anon_claims.rs](contracts/event/src/test_anon_claims.rs)

### Notes
- Touches privacy protocol flow.

---

## Issue 8: Incomplete Authorization Verification in `TicketContract::use_ticket`

### Category
Security

### Severity
Medium

### Description
In `TicketContract::use_ticket` ([`lib.rs`](contracts/ticket/src/lib.rs#L152-L184)), the contract requires authorization from `organizer` (`organizer.require_auth()`) and verifies `ticket.organizer == organizer`. However, it does not verify whether the ticket owner has authorized the check-in or presented a valid signature/token for attendance.

While organizers control event check-in, an organizer could unilaterally mark tickets as `is_used = true` without the ticket holder's presence or consent, preventing attendees from transferring or claiming refunds for unused tickets.

### Proposed Solution
Allow ticket consumption to optionally accept a signed ticket check-in message from `ticket.owner` or a valid ZK nullifier proof before updating `is_used`.

### Acceptance Criteria
- [ ] Owner presentation/signature check option added to `use_ticket`.
- [ ] Unit tests covering valid and unauthorized check-ins added.
- [ ] Documentation updated.
- [ ] No regressions introduced.

### Estimated Complexity
M

### Labels
security, ticket-contract, developer-experience

### Files
- [lib.rs](contracts/ticket/src/lib.rs#L152-L184)

### Notes
- Improves overall protocol trust model between organizers and attendees.

---

## Issue 9: Expand Test Suite Coverage for Escrow Splits, Postponement Edge Cases, and Multi-Token Settlements

### Category
Testing

### Severity
Medium

### Description
While unit tests exist for basic payments and event creation, several high-risk edge cases currently lack sufficient automated test coverage:
1. Multi-token revenue splits where co-hosts are flagged mid-event.
2. Postponement refund choices made exactly on the `choice_deadline_ledger` boundary.
3. Resale ticket purchasing when royalties exceed total proceeds.
4. Escrow release after multiple delay extensions.

### Proposed Solution
Add comprehensive test suites in `contracts/payments/src/revenue_split_test.rs` and `contracts/event/src/integration_tests.rs` targeting these complex state paths.

### Acceptance Criteria
- [ ] Unit/integration tests added covering co-host flagging during active splits.
- [ ] Boundary tests added for `choice_deadline_ledger` in postponement refunds.
- [ ] Code coverage increased across all contract modules.
- [ ] Documentation updated.
- [ ] No regressions introduced.

### Estimated Complexity
M

### Labels
testing, soroban, quality-assurance

### Files
- [revenue_split_test.rs](contracts/payments/src/revenue_split_test.rs)
- [integration_tests.rs](contracts/event/src/integration_tests.rs)

### Notes
- Essential for verifying protocol correctness before mainnet launch.

---

## Issue 10: Complete Comprehensive Architecture and Security Threat Model Documentation

### Category
Documentation

### Severity
Informational

### Description
The repository contains README and implementation guides, but lacks a complete formal threat model, architecture diagram, and explicit explanation of trust boundaries between `EventContract`, `PaymentsContract`, and `TicketContract`.

### Proposed Solution
1. Create `docs/ARCHITECTURE.md` with visual sequence diagrams for ticket purchasing, revenue split settlement, and postponement refunds.
2. Create `docs/THREAT_MODEL.md` outlining trust assumptions regarding event organizers, admins, and relayer networks.

### Acceptance Criteria
- [ ] Architecture document with sequence diagrams added to repository.
- [ ] Threat model document completed.
- [ ] References in root `README.md` updated.
- [ ] No regressions introduced.

### Estimated Complexity
S

### Labels
documentation, architecture, stellar

### Files
- [README.md](README.md)

### Notes
- Recommended for external security auditors and integration partners.

---

## Issue 11: Implement Batch Ticket Minting and Purchasing Optimizations

### Category
Enhancement

### Severity
Low

### Description
Currently, `register_for_event` and `mint_ticket` process single ticket purchases per invocation. Users purchasing multiple tickets for family or group bookings must submit multiple distinct transactions, resulting in excessive network fees and poor UX.

### Proposed Solution
Add `batch_register_for_event(env: Env, count: u32, ...)` and `batch_mint_ticket` functions to allow multi-ticket purchases in a single atomic transaction while respecting `max_tickets_per_user`.

### Acceptance Criteria
- [ ] Batch registration functions implemented and tested.
- [ ] Single atomic transaction handles multi-ticket minting correctly.
- [ ] User ticket limits enforced accurately in batch mode.
- [ ] Documentation updated.
- [ ] No regressions introduced.

### Estimated Complexity
M

### Labels
enhancement, soroban, developer-experience

### Files
- [lib.rs](contracts/event/src/lib.rs)
- [lib.rs](contracts/ticket/src/lib.rs)

### Notes
- Improves overall platform user experience.

---

## Issue 12: Refactor Duplicate Code and Standardize Error Handling Across Contracts

### Category
Refactor

### Severity
Low

### Description
There is duplicated helper logic across `contracts/event/src/lib.rs` and `contracts/payments/src/lib.rs` (e.g., validating Basis Points, privacy checking logic, revenue conversion). Furthermore, error types across modules could be unified to simplify client-side SDK integration.

### Proposed Solution
1. Extract shared validation utilities into a common helper crate or utility module.
2. Standardize error codes across `EventError`, `PaymentError`, and `TicketError`.

### Acceptance Criteria
- [ ] Duplicate helper functions refactored into shared utility module.
- [ ] All existing contract tests pass cleanly.
- [ ] Code cleanliness and maintainability improved.
- [ ] Documentation updated.
- [ ] No regressions introduced.

### Estimated Complexity
S

### Labels
refactor, clean-code, soroban

### Files
- [lib.rs](contracts/event/src/lib.rs)
- [lib.rs](contracts/payments/src/lib.rs)

### Notes
- Low risk refactoring task.

---

## Issue 13: Deprecate Legacy Monolithic Vector Storage in Favor of Map-Based Indexing

### Category
Architecture

### Severity
Low

### Description
Legacy vector storage patterns (e.g., `DataKey::OwnerTickets(Address)` storing `Vec<u64>`) perform full read-modify-write cycles on every ticket mint or transfer. This pattern is inefficient for accounts owning many tickets.

### Proposed Solution
Transition from `Vec<u64>` values under a single address key to individual boolean or status keys (e.g., `DataKey::OwnerTicket(Address, TicketId)`), avoiding vector serialization overhead.

### Acceptance Criteria
- [ ] Storage keys redesigned for map-style single key lookup.
- [ ] Storage migration test verified.
- [ ] Gas costs benchmarked showing reduction in serialization cost.
- [ ] Documentation updated.
- [ ] No regressions introduced.

### Estimated Complexity
L

### Labels
architecture, soroban, storage, performance

### Files
- [storage.rs](contracts/ticket/src/storage.rs)
- [storage.rs](contracts/payments/src/storage.rs)

### Notes
- Part of long-term technical debt reduction.

---

# Audit Milestones

Below is the organized milestone roadmap for resolving all identified audit findings:

### Milestone 1: Critical Security
- **Issue 1**: Unprotected Public Minting in `TicketContract::mint_ticket` Allows Unauthorized Ticket Creation
- **Issue 2**: Privilege Escalation and Admin Takeover in `TicketContract::set_payments_contract`

### Milestone 2: Protocol Correctness
- **Issue 3**: Missing Admin Authorization Check in `TicketContract::migrate`
- **Issue 4**: Double-Withdrawal Vulnerability in `PaymentsContract::withdraw_revenue` Admin Function

### Milestone 3: Stability
- **Issue 6**: Missing Soroban Persistent Storage TTL Extension Management
- **Issue 7**: Front-Running and Capacity Exhaustion in Unauthenticated `claim_anonymous_ticket`
- **Issue 8**: Incomplete Authorization Verification in `TicketContract::use_ticket`

### Milestone 4: Performance
- **Issue 5**: Denial of Service Risk via Unbounded Vector Iteration in Settlement and Invariant Verification

### Milestone 5: Refactoring
- **Issue 12**: Refactor Duplicate Code and Standardize Error Handling Across Contracts

### Milestone 6: Testing
- **Issue 9**: Expand Test Suite Coverage for Escrow Splits, Postponement Edge Cases, and Multi-Token Settlements

### Milestone 7: Documentation
- **Issue 10**: Complete Comprehensive Architecture and Security Threat Model Documentation

### Milestone 8: New Features
- **Issue 11**: Implement Batch Ticket Minting and Purchasing Optimizations

### Milestone 9: Long-term Technical Debt
- **Issue 13**: Deprecate Legacy Monolithic Vector Storage in Favor of Map-Based Indexing


---

# CodeRabbit Review that were unattended to

## Issue 1: Extend the owner-list read TTL to OwnerTicket(owner, ticket_id)

### Category 
Data Integrity & Integration

### Severity 
Major

### Description
get_tickets_by_owner refreshes the list count and index before reading each ticket_id, but not the membership entry that 
remove_owner_ticket later looks up in transfer_ticket, recovery, and admin transfers. If OwnerTicket(owner, ticket_id) expires, 
the index and count can stay alive and cleanup will skip removing OwnerTicketIndex(owner, idx), leaving the old owner with stale 
ownership metadata.

### Files Affected
`contracts/ticket/src/storage.rs`

### Proposed path
In `@contracts/ticket/src/storage.rs` around lines 191 - 209, Update
get_tickets_by_owner so each OwnerTicket(owner, ticket_id) membership entry has
its persistent TTL extended while reading the owner’s tickets, alongside the
existing count and index refreshes. Use the same TTL_THRESHOLD and TTL_BUMP
values and ensure the membership key remains available for later cleanup in
remove_owner_ticket.



## Issue 2: Add ledger-TTL simulation coverage for replay-protection storage

### Category 
🎯 Functional Correctness

### Severity 
Minor

### Description
The current ledger advances cover dispute timeouts, not the storage TTLs. Add Soroban simulation cases that advance ledgers before 
and after TTL_THRESHOLD, TTL_BUMP, NONCE_TTL_THRESHOLD, NONCE_TTL_BUMP, NULLIFIER_TTL_THRESHOLD, and NULLIFIER_TTL_BUMP. 
Include existing entries, renewal before expiry, and replay after expiry.

### Files Affected
`contracts/payments/src/storage.rs`

### Proposed path
In `@contracts/payments/src/storage.rs` around lines 7 - 19, Add Soroban
simulation test cases that verify ledger-TTL behavior for the replay-protection
storage constants now defined in the file. Create test scenarios that advance
the simulated ledger before and after each threshold value (TTL_THRESHOLD,
TTL_BUMP, NONCE_TTL_THRESHOLD, NONCE_TTL_BUMP, NULLIFIER_TTL_THRESHOLD,
NULLIFIER_TTL_BUMP). For each constant, include test paths that verify existing
entries remain valid, entries can be renewed before expiry, and entries
correctly fail verification after expiry to enforce replay protection. Ensure
the simulation ledger advancement covers the full lifecycle of each TTL
constant.


## Issue 3: Add TTL expiry simulations for ticket storage keys

### Category 
🎯 Functional Correctness

### Severity 
Minor

### Description
contracts/ticket/src/test.rs does not advance ledger_sequence() past TTL_THRESHOLD. Add coverage that reads/writes refresh Ticket, 
OwnerTicket, OwnerTicketIndex, OwnerTicketsCount, and RecoveryKey so the expiry objective is satisfied before merge.

### Files Affected
`contracts/ticket/src/storage.rs`

### Proposed path
In `@contracts/ticket/src/storage.rs` around lines 6 - 10, Add TTL expiry
simulations in the ticket storage tests around the existing ledger_sequence and
storage-key test helpers. Advance ledger_sequence() beyond TTL_THRESHOLD, verify
each key type—Ticket, OwnerTicket, OwnerTicketIndex, OwnerTicketsCount, and
RecoveryKey—expires as expected, then perform reads/writes to confirm the access
refreshes its TTL and prevents expiry; cover all listed key types before
completion.


## Issue 4: The batch path ignores existing reservations, so `tier.reserved` leaks

### Category 
🗄️ Data Integrity & Integration

### Severity 
Major - 🏗️ Heavy lift

### Description
`register_for_event` lines 855-925 handle a caller who holds a reservation. It validates the expiry and tier, decrements `tier.reserved`, 
and calls `storage::remove_reservation`. `batch_register_for_event` performs none of these steps.

If an attendee reserves a ticket and then calls `batch_register_for_event`, the reservation record stays in storage and `tier.reserved` 
stays incremented. That reserved slot is never released, so the tier permanently loses sellable capacity. `release_expired_reservation` 
is the only remaining path, and it requires an explicit external call.

Decide the intended behavior and implement it. Two options are consistent:

- Consume the reservation: validate it, decrement tier.reserved by one, and call storage::remove_reservation.
- Reject the call: return EventError::InvalidInput when storage::has_reservation returns true, and require the attendee to 
use `register_for_event`.

### Files Affected
`contracts/event/src/lib.rs`

### Proposed path
In `@contracts/event/src/lib.rs` around lines 961 - 980, The batch registration
flow around batch_register_for_event must explicitly handle an attendee’s
existing reservation instead of leaving tier.reserved incremented. Choose the
intended behavior: either validate and consume the reservation by decrementing
the matched tier’s reserved count and calling storage::remove_reservation, or
reject the batch call with EventError::InvalidInput when
storage::has_reservation is true; preserve normal batch registration for callers
without reservations.


## Issue 5: Enforce `max_tickets_per_user` against the attendee’s owned tickets.

### Category 
🎯 Functional Correctness

### Severity 
Major

### Description
`batch_register_for_event` rejects `count > event.max_tickets_per_user`, but repeated calls with `count == event.max_tickets_per_user` bypass 
the limit. Use a per-event owner total, including existing tickets, so one attendee cannot exceed the configured per-user cap.

### Files Affected
`contracts/event/src/lib.rs`

### Proposed path
In `@contracts/event/src/lib.rs` around lines 982 - 984, Update
batch_register_for_event’s max_tickets_per_user validation to compare the
attendee’s existing owned ticket total for this event plus the requested count
against the configured cap. Preserve the unlimited behavior when
max_tickets_per_user is zero and return EventError::InvalidInput whenever the
combined total exceeds the limit.



## Issue 6: Keep `batch_mint_ticket` under Soroban write limits.

### Category 
🚀 Performance & Scalability

### Severity 
Major - 🏗️ Heavy lift

### Description
Each loop iteration writes a ticket and two index/count entries. A count of 100 therefore touches ~300 ledger entries in one transaction, 
while Soroban caps write operations at 200. This can make the advertised maximum unusable because the batch reverts. 
Reduce the count cap or split batching across transactions, and test the upper bound with the contract simulator before merge.

### Files Affected
`contracts/ticket/src/lib.rs`

### Proposed path
In `@contracts/ticket/src/lib.rs` around lines 90 - 125, Update batch_mint_ticket
so its maximum accepted count stays within Soroban’s 200-write limit, accounting
for the ticket and owner/event index writes performed per iteration. Reduce the
advertised count cap to a safe upper bound, and add simulator coverage
confirming the maximum succeeds while exceeding it is rejected.


