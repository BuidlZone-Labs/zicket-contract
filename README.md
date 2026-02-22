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

### Event Lifecycle

```
Upcoming ──→ Active ──→ Completed
    │           │
    └───────────┴──→ Cancelled
```

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

## License

This project is under active development.
