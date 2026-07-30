# Zicket Contract Threat Model

## Trust Assumptions

### Event Organizers
- Organizers are trusted to provide accurate event details (name, date, venue, tier pricing).
- Organizers **cannot** withdraw revenue before the configured withdrawal delay.
- Organizers **cannot** modify event details or tiers after tickets have been sold for certain fields (e.g., resale royalty).
- Organizers **can** cancel or postpone events, triggering refund logic.
- Malicious organizers could create events with unrealistic dates or pricing, but cannot steal funds from attendees beyond the agreed-upon ticket price.

### Contract Admins
- Admin keys control contract initialization, pause/unpause, and WASM upgrades.
- Admins **cannot** mint tickets, register for events, or modify individual event storage directly.
- Admins **can** extend withdrawal delays and set platform fees.
- Admin compromise would allow contract upgrade to malicious code — mitigated by upgrade timelock mechanisms (48h delay).
- Admin can pause the payments contract in emergencies.

### Relayers / Transaction Submitters
- Relayers are **not trusted** with funds — all token transfers require payer signature.
- Relayers only submit pre-signed transactions; they cannot modify parameters.
- Private/Anonymous payment privacy applies at the contract storage level, not the transaction layer (the submitter's address is visible in the Soroban transaction envelope).

### External Dependencies
- **Token contracts**: The accepted token (XLM/USDC) is assumed to be well-behaved. A malicious token contract could break accounting invariants.
- **Anonymous claim verifier**: Configured once and immutable. A malicious verifier would allow forged anonymous claims.
- **Oracle contracts**: Used for price feeds in multi-token scenarios; oracle manipulation could affect payment valuations.

## Threat Scenarios

### T1: Storage Expiration (Critical)
| Property | Detail |
|----------|--------|
| **Threat** | Soroban persistent storage entries expire if TTL is not periodically extended. Key entries like `Admin`, `EventConfig`, `Ticket`, and `PaymentRecord` become inaccessible, bricking contract reads and user payouts. |
| **Impact** | Complete loss of access to event data, ticket ownership records, and held funds. |
| **Mitigation** | All storage read/write operations call `extend_ttl` with a 30-day threshold and 60-day bump. Critical entries are extended on every access, not just writes. |
| **Residual Risk** | If no one interacts with a contract for 60+ days, entries could expire. Admin monitoring should ensure periodic touch transactions. |

### T2: Reentrancy / Cross-Contract Calls
| Property | Detail |
|----------|--------|
| **Threat** | Malicious token contracts could attempt reentrancy during `transfer` calls in `pay_for_ticket`. |
| **Impact** | Double-spending of tickets or incorrect accounting. |
| **Mitigation** | Soroban's serial execution model prevents reentrancy. The CEI (Checks-Effects-Interactions) pattern is followed: state mutations occur before external token transfers in the create_payment flow. |
| **Residual Risk** | Low — Soroban's architecture inherently prevents reentrant attacks. |

### T3: Privacy Leakage
| Property | Detail |
|----------|--------|
| **Threat** | Private/Anonymous events store no raw addresses, but the transaction envelope exposes the submitter's wallet to the network. |
| **Impact** | Attendee identity could be inferred by monitoring the Soroban transaction stream. |
| **Mitigation** | Private payments store only `sha256(address + stealth_key)`. Anonymous payments store only a nullifier commitment. Relayer networks can provide transaction-level anonymity. |
| **Residual Risk** | Transaction-layer privacy requires a relayer/metatransaction model — this is a known limitation documented in the code. |

### T4: Revenue Split Manipulation
| Property | Detail |
|----------|--------|
| **Threat** | A co-host (revenue split recipient) attempts to withdraw more than their allocated share. |
| **Impact** | Financial loss for other recipients. |
| **Mitigation** | Split settlement freezes the net-distributable amount once. Each recipient can only withdraw their basis-point share. The primary organizer can flag compromised co-host wallets, freezing their share. Integer-division rounding ensures the organizer (index 0) receives the remainder, preventing stranded dust. |
| **Residual Risk** | Low — enforced at the protocol level with invariant checks. |

### T5: Anonymous Claim Double-Spend
| Property | Detail |
|----------|--------|
| **Threat** | A user claims multiple anonymous tickets using the same nullifier. |
| **Impact** | Free ticket acquisition beyond entitlement. |
| **Mitigation** | Nullifier uniqueness is enforced on-chain: `get_anonymous_ticket_commitment` checks for existing commitments. Reused nullifiers are rejected. Per-window rate limiting via `AnonWindowState` limits claim velocity. |
| **Residual Risk** | Low — combined nullifier uniqueness + rate limiting. |

### T6: Free Claim Abuse
| Property | Detail |
|----------|--------|
| **Threat** | User claims more free tickets than the configured `max_free_claims` limit. |
| **Impact** | Event capacity consumed by free riders. |
| **Mitigation** | Free claim count is tracked per-user per-event. Cooldown timers prevent rapid successive claims. Both limits are checked in `register_for_event` and `batch_register_for_event`. |
| **Residual Risk** | Low — enforced at the contract level. |

### T7: Event Cancellation Front-Running
| Property | Detail |
|----------|--------|
| **Threat** | Organizer cancels an event immediately after ticket sales close to avoid paying refunds (or to minimize organizer loss). |
| **Impact** | Attendees are refunded pro-rata based on the cancellation ledger. The organizer receives a pro-rata share based on elapsed time. |
| **Mitigation** | The `withdrawable_ratio_bps` is computed as `elapsed / total * 10000`, ensuring fair distribution. Zero ratio applies if cancelled before event start (organizer gets nothing). Refund ratio = `10000 - withdrawable_ratio_bps`. |
| **Residual Risk** | Medium — organizer could cancel right before the event ends, gaining most revenue while attendees are refunded a small fraction. The dispute window and attendee claim refund mechanism mitigate this partially. |

### T8: Upgrade Timelock Bypass
| Property | Detail |
|----------|--------|
| **Threat** | Admin bypasses the 48-hour timelock to upgrade contracts with malicious code. |
| **Impact** | Complete compromise of contract funds and data. |
| **Mitigation** | Two-step upgrade pattern: `propose_upgrade` → 48h wait → `execute_upgrade`. The timelock is enforced at the contract level. |
| **Residual Risk** | Low — enforced in contract code, not off-chain. |

### T9: Denial of Service via Storage Exhaustion
| Property | Detail |
|----------|--------|
| **Threat** | Attacker creates many events, registrations, or tickets to consume contract storage and drive up fees. |
| **Impact** | Increased operational costs for legitimate users. |
| **Mitigation** | Event creation requires organizer authentication. Ticket capacity limits (max 100K per tier). Pagination limits (100 entries per query). Fee market on Stellar naturally limits spam. |
| **Residual Risk** | Low — economic barriers (transaction fees) and authentication requirements limit DoS potential. |

### T10: Dispute Resolution Timeout
| Property | Detail |
|----------|--------|
| **Threat** | A dispute remains unresolved past the timeout window, auto-releasing funds to the organizer. |
| **Impact** | Attendee loses ability to contest a ticket. |
| **Mitigation** | `DISPUTE_TIMEOUT_LEDGERS` (14 days) provides adequate time for resolution. Attendees can initiate disputes on-chain. The `process_timed_out_disputes` function auto-resolves expired disputes. |
| **Residual Risk** | Low — the timeout is generous and automated. |

## Security Controls Summary

| Control | Location | Description |
|---------|----------|-------------|
| Authentication | All contracts | `require_auth()` on Address operations prevents unauthorized state changes. |
| M-of-N Admin | Payments contract | Multi-signature admin threshold for sensitive operations. |
| TTL Extension | Storage layers | All persistent storage operations extend TTL to prevent data loss. |
| Privacy Isolation | Payments contract | Privacy-level enforcement prevents cross-level data leakage. |
| Nullifier Uniqueness | Event contract | Prevents double-spending of anonymous claims. |
| Rate Limiting | Event contract | Per-window rate limits for anonymous and free claims. |
| Revenue Invariant | Payments contract | `validate_revenue_invariant` ensures accounting consistency. |
| Upgrade Timelock | Payments contract | 48-hour delay on contract upgrades. |
| Pause Mechanism | Payments contract | Emergency pause by admin. |
| Swap-remove Arrays | Ticket storage | O(1) removal from indexed collections prevents gas griefing. |
