use dep::poseidon::poseidon2::Poseidon2;

fn main(
    secret: Field,
    event_scope: pub Field,
    tier_id: pub u32,
    expiry_ledger: pub u32,
) -> pub (Field, Field) {
    assert(expiry_ledger > 0);
    (
        Poseidon2::hash([secret, event_scope], 2),
        Poseidon2::hash([secret, event_scope, tier_id as Field], 3),
    )
}
