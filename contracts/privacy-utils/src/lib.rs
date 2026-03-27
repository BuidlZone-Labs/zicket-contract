#![no_std]

use soroban_sdk::{contracttype, xdr::ToXdr, Address, Bytes, BytesN, Env};

/// Controls how an address is represented when emitted in events or logs.
///
/// - `Standard`  – Full address, no masking (default for trusted/admin contexts).
/// - `Private`   – First 8 bytes of the address XDR (partial reveal; cannot be reversed).
/// - `Anonymous` – SHA-256 hash of the address XDR (fully opaque; cannot be reversed).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrivacyLevel {
    Standard = 0,
    Private = 1,
    Anonymous = 2,
}

/// The result of masking an address according to a `PrivacyLevel`.
///
/// Variants:
/// - `Full(Address)`      – The original address, unchanged.
/// - `Partial(Bytes)`     – First 8 bytes of the address XDR.
/// - `Hashed(BytesN<32>)` – SHA-256 hash of the address XDR.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MaskedAddress {
    Full(Address),
    Partial(Bytes),
    Hashed(BytesN<32>),
}

/// Mask `address` according to `privacy_level`.
///
/// # Rules
/// | Level       | Output                                |
/// |-------------|---------------------------------------|
/// | Standard    | Full address (identity)               |
/// | Private     | First 8 bytes of XDR representation  |
/// | Anonymous   | SHA-256 hash of XDR representation   |
///
/// The `Anonymous` and `Private` variants cannot be reversed to recover the
/// original address, satisfying the "no raw address leakage" requirement.
pub fn mask_address(env: &Env, address: &Address, privacy_level: PrivacyLevel) -> MaskedAddress {
    match privacy_level {
        PrivacyLevel::Standard => MaskedAddress::Full(address.clone()),

        PrivacyLevel::Private => {
            let xdr = address.clone().to_xdr(env);
            let limit = 8_u32.min(xdr.len());
            // Build a Bytes of at most 8 bytes from the XDR representation.
            let mut partial = Bytes::new(env);
            let mut i = 0u32;
            while i < limit {
                partial.push_back(xdr.get(i).unwrap());
                i += 1;
            }
            MaskedAddress::Partial(partial)
        }

        PrivacyLevel::Anonymous => {
            let xdr = address.clone().to_xdr(env);
            let hash = env.crypto().sha256(&xdr);
            MaskedAddress::Hashed(hash.into())
        }
    }
}

#[cfg(test)]
mod test;
