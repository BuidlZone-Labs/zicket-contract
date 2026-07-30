# Zicket Contract Architecture

## Overview

Zicket is a decentralized event ticketing platform built on Stellar Soroban smart contracts. The system consists of four core contracts that work together to provide event creation, ticket minting, payment processing, and contract deployment.

## Contract Architecture

```
┌─────────────────────┐
│   FactoryContract   │
│  (contract factory) │
└──────┬──────────────┘
       │ deploys
       ▼
┌─────────────────────┐     ┌─────────────────────┐     ┌─────────────────────┐
│   EventContract     │◄───►│   PaymentsContract  │◄───►│   TicketContract    │
│  (event lifecycle)  │     │  (payment escrow)   │     │  (ticket registry)  │
└─────────────────────┘     └─────────────────────┘     └─────────────────────┘
```

### 1. EventContract

Manages the full lifecycle of events:

- **Creation**: Organizers create events with tiered ticket pricing, capacity limits, revenue splits, and privacy settings.
- **Registration**: Attendees register for events via `register_for_event` or `batch_register_for_event` (multi-ticket purchases).
- **Anonymous claims**: Zero-knowledge proof-based anonymous ticket claims via `claim_anonymous_ticket`.
- **ZK passport verification**: Identity-verified attendance via `verify_and_attend` using ZK passport claims.
- **Lifecycle management**: Events transition through states: `Upcoming → Active → Completed` (or `Cancelled` / `Postponed`).
- **Reservation system**: Time-limited ticket reservations with 15-minute expiry windows.
- **Revenue withdrawal**: Organizers withdraw event revenue after the configured withdrawal delay.

**Trust boundary**: The event organizer is trusted to manage event details and tiers. The admin (set during initialization) can configure global settings like the anonymous claim verifier.

### 2. PaymentsContract

Handles all financial operations:

- **Payment processing**: Accepts token transfers from attendees, holds funds in escrow until the event completes.
- **Revenue splits**: Distributes revenue among multiple recipients according to configured basis-point allocations.
- **Refunds**: Processes refunds for cancelled/postponed events with pro-rata calculations.
- **Dispute resolution**: Manages attendee disputes with configurable timeout windows.
- **Platform fees**: Deducts configurable platform fees from organizer payouts.
- **Privacy levels**: Supports Standard (raw address), Private (hashed wallet), and Anonymous (nullifier commitment) payment privacy.

**Trust boundary**: The payment contract admin controls pause/unpause, fee configuration, and administrative refunds. The event organizer is trusted to manage their event's financial config.

### 3. TicketContract

Manages ticket minting and ownership:

- **Minting**: Issues tickets with unique IDs, linked to event and owner.
- **Batch minting**: `batch_mint_ticket` issues multiple tickets in a single atomic transaction.
- **Transfer**: Supports direct transfers, admin-mediated transfers, and recovery-key-based transfers.
- **Lifecycle**: Tickets transition through states: `Valid → Used` or `Valid → Cancelled`.
- **Indexing**: Map-based storage for efficient owner-to-ticket and event-to-ticket lookups.

**Trust boundary**: The payments contract is authorized to mint tickets. Ticket owners control transfers and cancellations. Recovery keys enable off-chain ownership recovery.

### 4. FactoryContract

Deploys and tracks event contracts:

- **Deployment**: Creates new EventContract instances with pre-configured linked contracts.
- **Registry**: Maintains a registry of all deployed events, indexed by event ID and organizer.

**Trust boundary**: The factory admin controls the WASM hash used for deployments and the linked contract addresses.

## Interaction Flow: Ticket Purchase

```
Attendee              EventContract          PaymentsContract       TicketContract
   │                       │                       │                     │
   │  register_for_event   │                       │                     │
   │──────────────────────►│                       │                     │
   │                       │  pay_for_ticket        │                     │
   │                       │──────────────────────►│                     │
   │                       │                       │   Transfer tokens   │
   │                       │                       │     (XLM/USDC)      │
   │                       │                       │◄───────────────────►│
   │                       │                       │                     │
   │                       │  mint_ticket           │                     │
   │                       │────────────────────────────────────────────►│
   │                       │                       │                     │
   │                       │  ◄──── ticket_id ──────│                     │
   │◄─────── success ──────│                       │                     │
```

## Interaction Flow: Batch Purchase

```
Attendee              EventContract          PaymentsContract       TicketContract
   │                       │                       │                     │
   │  batch_register(5)    │                       │                     │
   │──────────────────────►│                       │                     │
   │                       │  pay_for_ticket        │                     │
   │                       │  (5 × price)          │                     │
   │                       │──────────────────────►│                     │
   │                       │                       │                     │
   │                       │  batch_mint(5)         │                     │
   │                       │────────────────────────────────────────────►│
   │                       │                       │                     │
   │                       │  ◄─── [5 ticket_ids] ──│                     │
   │◄─────── success ──────│                       │                     │
```

## Interaction Flow: Revenue Split Settlement

```
Organizer              EventContract          PaymentsContract
   │                       │                       │
   │  withdraw_revenue     │                       │
   │──────────────────────►│                       │
   │                       │  withdraw              │
   │                       │──────────────────────►│
   │                       │                       │
   │                       │  Validate event status │
   │                       │  Check withdrawal delay│
   │                       │  Deduct platform fee   │
   │                       │  Freeze settlement     │
   │                       │                       │
   │                       │  Transfer organizer    │
   │                       │  share                 │
   │                       │◄──────────────────────│
   │◄─────── success ──────│                       │
```

## Storage Architecture

All contracts use Soroban's persistent storage with TTL (Time-To-Live) management:

- **Instance storage** (`env.storage().instance()`): Not used - all data uses persistent storage.
- **Persistent storage** (`env.storage().persistent()`): All contract data with TTL extension.
- **TTL strategy**: Every read and write extends the TTL by 60 days (threshold + bump), ensuring frequently accessed data remains available.
- **Key design**: `DataKey` enums structure storage keys with appropriate parameters (event IDs, addresses, ticket IDs).

## Privacy Model

| Level | Storage Identity | Refundable | Indexed |
|-------|-----------------|------------|---------|
| Standard | Raw Address | Yes | Yes |
| Private | Hashed Wallet + Stealth Key | No (off-chain) | No |
| Anonymous | Nullifier Commitment | No (off-chain) | No |
