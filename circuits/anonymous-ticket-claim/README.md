# Anonymous ticket claim circuit

The circuit proves knowledge of a private `secret` and returns two public
values:

- `nullifier = Poseidon2(secret, event_scope)`
- `ticket_commitment = Poseidon2(secret, event_scope, tier_id)`

`event_scope` is derived on-chain from the network ID, event contract address,
and event ID. The public `tier_id` and `expiry_ledger` are also checked by the
event contract. This binds a proof to one network, contract, event, tier, and
expiry while keeping the claimant and secret private.

Pinned toolchain:

- Nargo `1.0.0-beta.9`
- Barretenberg `0.87.0`
- UltraHonk with Keccak transcript and zero knowledge enabled

Generate a proof with:

```sh
../../scripts/generate-anonymous-claim-proof.sh
```

The verifier contract must be deployed and configured once through
`set_anonymous_claim_verifier` before claims can be submitted. Clients fetch
`get_anonymous_claim_scope(event_id)`, generate a fresh secret and future expiry,
then submit the proof and derived values to `claim_anonymous_ticket`.

The verifier expects five canonical 32-byte public inputs in this order:
`event_scope`, `tier_id`, `expiry_ledger`, `nullifier`, and
`ticket_commitment`. Replaying the same claim is idempotent; reusing its
nullifier with a different ticket commitment is rejected.

The verifier accepts the 16,224-byte `--zk` proof layout only. Non-ZK
UltraHonk proofs are intentionally rejected.

`Prover.toml` is a test fixture. Production clients must replace its secret,
scope, tier, and expiry values for every claim.
