# 🎫 Zicket — Smart Contract Platform

A decentralized event ticketing platform built on [Stellar Soroban](https://soroban.stellar.org/). Zicket enables organizers to create events, sell tickets, and manage payments — all on-chain.

## Architecture

The platform is composed of four Soroban smart contracts:

| Contract | Description |
|----------|-------------|
| **`event`** | Create, update, cancel events and manage event lifecycle (`Upcoming → Active → Completed`) |
| **`ticket`** | Mint, transfer, validate, and cancel tickets tied to events |
| **`payments`** | Handle ticket payments via Stellar tokens, escrow funds, process refunds and revenue withdrawal |
| **`factory`** | Deploy new event contract instances and maintain a platform-wide event registry |

## Project Structure

```
zicket-contract/
├── Cargo.toml              # Workspace configuration
├── contracts/
│   ├── event/              # Event management contract
│   │   └── src/
│   │       ├── lib.rs      # Contract entry points
│   │       ├── types.rs    # Event, EventStatus types
│   │       ├── storage.rs  # Persistent storage helpers
│   │       ├── errors.rs   # Custom error types
│   │       ├── events.rs   # Soroban event emitters
│   │       └── test.rs     # Unit tests
│   ├── ticket/             # Ticket minting & management
│   ├── payments/           # Payment processing & escrow
│   └── factory/            # Contract deployment factory
└── issues/                 # GitHub issue descriptions
```

## Tech Stack

- **Language:** Rust (`no_std`)
- **SDK:** `soroban-sdk 22.0.0`
- **Platform:** Stellar Soroban
- **Build profile:** Optimized release with `opt-level = "z"`, LTO, and symbol stripping

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Soroban CLI](https://soroban.stellar.org/docs/getting-started/setup)
- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli) (optional, for deployment)

## Getting Started

### Build

```bash
cargo build --release
```

### Run Tests

```bash
# All contracts
cargo test

# Specific contract
cargo test -p event
cargo test -p ticket
cargo test -p payments
cargo test -p factory
```

### Build with Logs (Debug)

```bash
cargo build --profile release-with-logs
```

## Event Contract — Current Features

The `event` contract is the most developed and supports:

- **Create Event** — with validation for name, venue, date (≥24h future), ticket count (1–99,999), and price
- **Get Event / Status** — query event details or status by ID
- **Update Status** — organizer-controlled transitions: `Upcoming → Active → Completed`
- **Cancel Event** — organizer can cancel any non-completed event
- **Postpone Event** — organizer can reschedule an `Active` event instead of cancelling it. Postponement opens a refund-choice window (≥72h) in which holders may opt out for a **full** refund via `request_postponement_refund` (which also revokes their ticket so they can't both refund and attend); holders who do nothing keep their ticket for the new date. Revenue withdrawal is frozen while `Postponed`. After the window closes, the organizer calls `finalize_postponement` to return the event to `Active` on its new schedule. A `Postponed` event can still be cancelled outright (full-refund path). Bounded by `MAX_POSTPONEMENTS` to prevent indefinite postponement.

### Event Lifecycle

```text
Upcoming ─────→ Active ───────────────→ Completed
    │            │   ▲
    │            │   │ finalize_postponement
    │            ▼   │ (after choice window)
    │          Postponed
    │            │
    │            │ cancel_event
    ▼            ▼
 Cancelled ◀─────┘
```

Cancellation is reachable from `Upcoming`, `Active`, **and** `Postponed` (any non-`Completed` state).

## Ticket Contract — Current Features

The `ticket` contract handles the ticket lifecycle and ownership:

- **Transfer Ticket** — allow owners to transfer their Valid tickets to other addresses
- **Check-in / Use Ticket** — organizers can validate tickets at the door, transitioning them from `Valid` to `Used`
- **Ticket Status Management** — protects against double-entry and unauthorized use of cancelled tickets
- **Owner Tracking** — query all tickets owned by a specific address

### Ticket Statuses

- **Valid** — ticket is active and can be used for entry or transferred
- **Used** — ticket has been validated at the door and cannot be reused or transferred
- **Cancelled** — ticket has been invalidated (e.g., due to event cancellation) and cannot be used or transferred

## Payments Contract — Revenue Withdrawal

Several public functions can transfer escrowed event revenue. They differ in who may call them, whether the platform fee is deducted, and which double-payout latch (if any) they respect:

| Path | Caller | Platform fee | Latch |
|---|---|---|---|
| `withdraw` | the event's stored organizer | deducted | `EventConfig::organizer_withdrawn` |
| `withdraw_revenue` | contract admin | deducted | `EventConfig::organizer_withdrawn` |
| `withdraw_split` | a configured split recipient | deducted once, at settlement | per-recipient `split_withdrawn` |
| `withdraw_token` | caller-supplied address (see note) | **not** deducted | none |
| `withdraw_all_tokens` | caller-supplied address (see note) | **not** deducted | none |
| `release_if_expired` | permissionless | **not** deducted | `EscrowMetadata::auto_released` |

- **`withdraw`** — organizer path. Honours the event status and timing rules: a `Completed` event unlocks after `withdrawal_delay_ledgers` (plus any admin extension), a `Cancelled` event unlocks after the minimum dispute window and pays out only the time-based `withdrawable_ratio_bps` share, leaving the remainder escrowed for attendee refunds via `claim_refund`. Pays the event's stored organizer.
- **`withdraw_revenue`** — admin path. Settles an event to an arbitrary recipient without the status/timing rules, for support and recovery cases.
- **`withdraw_split`** — the only path for events configured with a revenue split; each recipient claims its own share once. Every other path here rejects split events via `ensure_no_splits`.
- **`withdraw_token` / `withdraw_all_tokens`** — multi-token payout for `Completed` events, one token or every token the event accepted. They pay the full token balance with no fee deduction. **Note:** both authorize the caller-supplied `organizer` argument with `require_auth()` but do not check it against the event's stored organizer, so any caller can direct a completed event's escrow to an address they control.
- **`release_if_expired`** — permissionless auto-release once the escrow deadline and the event's end ledger have both passed. Pays `EscrowMetadata::organizer` the full balance of every event token, with no fee deduction.

**`withdraw` and `withdraw_revenue` are one-shot per event.** They draw on the same escrow balance, so they share the `EventConfig::organizer_withdrawn` latch: whichever runs first marks the event settled and the other is rejected — `NoRevenue` from `withdraw`, `PaymentAlreadyProcessed` from `withdraw_revenue`. This holds even if later ticket sales re-fund the escrow. Without the shared latch an admin withdrawal followed by (or following) an organizer withdrawal would pay the same event out twice, draining balances held for refunds and for other events.

The remaining paths do **not** participate in that latch. They zero the event's token revenue as they pay out, so a latched path that runs afterwards finds nothing to withdraw and returns `NoRevenue` — but the reverse ordering is not blocked, and none of them mark the event settled.

Legacy events that have never been synced from the `event` contract have no `EventConfig` and therefore no `organizer_withdrawn` latch; `withdraw` is unreachable for them, so only the admin path applies.

Where a platform fee applies it is taken at `platform_fee_bps` and accrued to the event's platform revenue, claimable separately by the admin via `withdraw_platform_revenue`. Every path appends to the event's withdrawal history (`get_withdrawal_history`) and to its `total_withdrawn` accounting total. Withdrawals of every kind are frozen while an event is `Postponed` or the contract is paused.

## Roadmap

See the [`issues/`](./issues/) directory for detailed GitHub-ready issue descriptions covering upcoming work:

- Register for events
- Ticket minting
- Payment escrow & refunds
- Factory contract deployment
- Event detail updates
- And more

## Contribution

- git clone `your fork`
- follow `#getting started`
- create a branch for your task e.g `feat/new-feature`, `fix/your-bugs`, `chore/update-dependencies`, `test/test-module` .etc
- ensure brief commit msg e.g `feat: add new event func`
- before pushing changes, run `cargo build`, `cargo test`, `cargo fmt`, `cargo clippy`.
- push your changes & create your PR, ensure to link your issue using the `github closing keyword`.

## Community

Join us here for discussions, suggestions & improvements: https://t.me/+nlYw80w3FF1jNGY0

## License

This project is under active development with the MIT License.
