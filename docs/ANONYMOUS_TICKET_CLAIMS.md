# Anonymous ticket claims

Anonymous free-ticket claims use a Noir UltraHonk ZK proof instead of an
address signature or a caller-supplied commitment.

## Security model

The private witness is a user-generated secret. The circuit exposes:

```text
nullifier = Poseidon2(secret, event_scope)
ticket_commitment = Poseidon2(secret, event_scope, tier_id)
```

The event contract derives `event_scope` from:

```text
"zicket:anonymous-ticket-claim:v1"
|| network_id
|| event_contract_address_xdr
|| event_id_xdr
```

It takes the low 128 bits of the SHA-256 digest so the value is always a
canonical BN254 field element. The contract also supplies `tier_id` and
`expiry_ledger` as public inputs. A proof therefore cannot be moved to another
network, event contract, event, tier, or expiry.

The nullifier is stored against the ticket commitment. If a watcher copies and
submits the complete proof first, the exact replay is idempotent: supply is
incremented once and the same secret-bound ticket commitment remains recorded.
The watcher cannot create a different valid commitment without knowing the
secret. Reusing the nullifier with a different commitment is rejected.

The verifier rejects non-canonical BN254 public-input encodings. This prevents
the same field nullifier from being represented as a different 32-byte storage
key by adding the scalar-field modulus.

This prevents proof-copying from stealing a second ticket or permanently
blocking the secret holder. It does not provide Sybil resistance for an open
free claim; that requires a separate eligibility credential or allowlist root.

## Deployment

1. Build and deploy `anon-claim-verifier`.
2. Initialize the event contract.
3. Call `set_anonymous_claim_verifier(admin, verifier_address)`.

The verifier address is admin-authorized and can only be set once. Anonymous
claims fail closed until it is configured.

## Client flow

1. Fetch `get_anonymous_claim_scope(event_id)`.
2. Generate a random private secret.
3. Set `event_scope`, `tier_id`, and a future `expiry_ledger` in
   `circuits/anonymous-ticket-claim/Prover.toml`.
4. Run `scripts/generate-anonymous-claim-proof.sh`.
5. Read the five 32-byte public inputs in this order:
   `event_scope`, `tier_id`, `expiry_ledger`, `nullifier`,
   `ticket_commitment`.
6. Submit `claim_anonymous_ticket(event_id, tier_id, claim)` with the proof,
   nullifier, ticket commitment, and expiry.

The committed `Prover.toml` and binary fixtures contain test-only values. Never
reuse that secret in production.

## Pinned verifier

- Nargo `1.0.0-beta.9`
- Barretenberg `0.87.0`
- `ultra_honk`, Keccak transcript, `--zk`
- Proof length: 16,224 bytes
- Verification key SHA-256:
  `a7b2b3a9f7044e198ae2f4dbd4f63e7f5145b03fc1a635aba8c6ae7a149395a5`

Changing either tool version requires regenerating the verification key and
fixtures and revalidating the Soroban verifier against Barretenberg's generated
reference verifier.

## Soroban resource validation

The verifier requires Soroban SDK 26 for host-native BN254 scalar arithmetic
and G1 MSM. The optimized verifier WASM was exercised with a freshly generated
real proof under the SDK 26.1.1 mainnet invocation limits:

- WASM: 23,133 bytes
- CPU: 110,752,549 instructions
- Memory: 5,383,749 bytes
- Build hash:
  `5ca0686a162d55c7ffac183d2e49009482c871c78713b5cbee29c1b3893911bc`
