# Soroban UltraHonk ZK verifier

This crate verifies Barretenberg UltraHonk proofs generated with:

```text
bb 0.87.0
--scheme ultra_honk
--oracle_hash keccak
--zk
```

It is derived from
[`yugocabrio/rs-soroban-ultrahonk`](https://github.com/yugocabrio/rs-soroban-ultrahonk)
at commit `661db07200f890b1bd9a7349ed787c70a706dd12`.

The upstream verifier handled the non-ZK UltraHonk flavor. This fork adds the
UltraZK transcript, Libra sumcheck masking, hiding-polynomial opening, and
Libra consistency/opening checks needed for `bb 0.87.0 --zk` proofs.

The verifier is deliberately pinned to that proof layout. Changing Noir or
Barretenberg versions requires regenerating the verification key and fixtures,
then comparing the verifier against Barretenberg's generated reference
verifier before deployment.
