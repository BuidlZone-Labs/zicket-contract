# Pull Request — Zicket Smart Contract

## Linked issue
Closes #<!-- issue number -->

> **Rule:** Every PR must close exactly one issue. If your changes span multiple issues, split the PR.

---

## What this PR does
<!-- One paragraph. Describe the change at the contract behaviour level — not the file level.
     Bad:  "Added privacy_level field to PaymentRecord and updated event emission."
     Good: "Payments now carry a cryptographic privacy level that determines how payer identity
            is handled in both on-chain storage and emitted events. Anonymous payments replace
            the payer address with a nullifier commitment; refunds preserve the original level." -->

---

## Change type
<!-- Check all that apply -->
- [ ] New contract entrypoint
- [ ] Struct / storage change
- [ ] Event / emission change
- [ ] Cross-contract interface change
- [ ] Bug fix
- [ ] Refactor / deprecation
- [ ] Test-only change
- [ ] Documentation change

---

## Storage impact
<!-- This section is MANDATORY for any PR touching a struct, map, or storage key. -->

| Field | Before | After | Notes |
|-------|--------|-------|-------|
| <!-- e.g. payer: Address --> | <!-- old type or "N/A" --> | <!-- new type --> | <!-- why --> |

**Is this a breaking storage change?**
- [ ] No — new optional field with a default
- [ ] No — additive only, existing records unaffected
- [ ] Yes — existing stored data must be migrated (attach migration plan below)

---

## On-chain vs. off-chain behaviour
<!--
This is the most common source of false "done" PRs in this repo.
For EVERY claim in your summary, explicitly state whether the guarantee
holds at the storage level, the event/emission level, or both.

Example for a privacy feature:
  ✅ Storage: payer address replaced with BytesN<32> commitment — not recoverable from state
  ✅ Events:  PaymentReceivedAnonymous emits no wallet-derivable fields
  ❌ Refund:  refund path not yet updated — tracked in #___

If a guarantee only holds at the event level, say so explicitly.
Hiding data from indexers is NOT the same as hiding it from chain state.
-->

| Claim | Storage level | Event level | Notes / gaps |
|-------|:---:|:---:|------|
| <!-- e.g. Payer identity not exposed (Anonymous) --> | ✅ / ❌ / ⚠️ | ✅ / ❌ / ⚠️ | |
| | | | |

---

## Cross-contract impact
<!-- Did you change any function signature that other contracts call?
     If yes, list every caller and confirm it was updated. -->

- [ ] No cross-contract interface was changed
- [ ] Yes — changed: `<!-- fn name -->` in `<!-- contract -->`
  - Callers updated: <!-- list contracts -->
  - Default arguments introduced: <!-- list any hardcoded defaults and justify them -->

> ⚠️ Hardcoded defaults in cross-contract calls must be justified. A default of `Standard`
> on a privacy parameter means every cross-contract purchase is Standard regardless of
> what the user selected. That is a product bug, not a safe default.

---

## Privacy checklist
<!-- Complete this for any PR touching payment, ticket, attendance, or identity flows. -->

- [ ] Raw wallet addresses are not stored where a commitment or nullifier is appropriate
- [ ] Emitted events contain no fields that can be cross-referenced to derive identity
  (e.g. ticket_id + block_time + amount is often enough to de-anonymise)
- [ ] The refund / reversal path preserves the privacy level of the original action
- [ ] zkPassport nullifiers (if used) are stored to prevent proof reuse across events
- [ ] zkEmail commitments (if used) store only the hash, never the raw address
- [ ] N/A — this PR does not touch any privacy-sensitive flow

---

## Security checklist

- [ ] No new entrypoint is callable without appropriate auth (`require_auth` / admin check)
- [ ] Integer arithmetic uses checked ops or Soroban's safe primitives — no silent overflow
- [ ] Any new capacity or supply check cannot be bypassed by batching calls in one tx
- [ ] Escrow / withdrawal logic enforces correct state transitions
  (e.g. withdrawal only after `event_end_ledger + WITHDRAWAL_DELAY_LEDGERS`)
- [ ] No free-event attendance path can be drained by a single caller without a commitment scheme
- [ ] N/A — this PR does not touch auth, arithmetic, capacity, or escrow

---

## Test coverage

**New tests added:**
| Test name | What it actually proves |
|-----------|------------------------|
| `test_` | |

> Tests must prove the *guarantee*, not just the *label*.
> 
> ❌ Weak: `test_anonymous_event_does_not_expose_payer` — verifies event struct shape only  
> ✅ Strong: `test_anonymous_payer_not_recoverable_from_storage` — reads raw contract state
>    and asserts no wallet address is present

**Edge cases covered:**
- [ ] The "happy path" for each new entrypoint
- [ ] Rejection of invalid state transitions
- [ ] Boundary values (zero price, max capacity, min/max ledger windows)
- [ ] The specific attack or misuse scenario described in the linked issue

**Test count:** <!-- e.g. "6 new, 12 updated, 74 total" -->

---

## Acceptance criteria sign-off
<!-- Copy the acceptance criteria from the linked issue and check each one.
     For each item, state HOW it is satisfied — a test name, a line reference, or a brief explanation.
     Do not just tick boxes. -->

- [ ] **AC:** <!-- paste criterion -->
  - Satisfied by: <!-- test name or explanation -->
- [ ] **AC:** <!-- paste criterion -->
  - Satisfied by:
- [ ] **AC:** <!-- paste criterion -->
  - Satisfied by:

---

## What this PR deliberately does NOT cover
<!-- Be explicit about scope boundaries. If a related concern is out of scope,
     name it and link the issue that will handle it. This prevents reviewers
     from approving under the assumption that a gap will be addressed "later"
     with no paper trail. -->

- <!-- e.g. Refund path privacy — tracked in #___ -->
- <!-- e.g. Cross-contract default privacy level — tracked in #___ -->

---

## Reviewer focus areas
<!-- Tell the reviewer where to spend their time. Be specific. -->

1. <!-- e.g. "Check PaymentRecord in storage — does the payer field still exist for Anonymous payments?" -->
2. <!-- e.g. "Verify the cross-contract call in event/lib.rs doesn't hardcode a privacy level" -->
3. <!-- e.g. "Confirm nullifier is stored, not just checked at call time" -->

## Checklist

<!-- Mark completed items with an "x" -->

- [ ] My code follows the project's style guidelines
- [ ] I have run `cargo fmt` and `cargo clippy`
- [ ] I have performed a self-review of my code
- [ ] I have commented my code, particularly in hard-to-understand areas
- [ ] I have made corresponding changes to the documentation
- [ ] My changes generate no new warnings
- [ ] I have added tests that prove my fix is effective or that my feature works
- [ ] New and existing unit tests pass locally with my changes
- [ ] Any dependent changes have been merged and published

## Additional Notes

<!-- Add any additional context, screenshots, or information about the PR here -->
