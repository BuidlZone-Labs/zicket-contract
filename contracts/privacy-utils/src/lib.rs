#![no_std]

use soroban_sdk::{contracttype, xdr::ToXdr, Address, Bytes, BytesN, Env};
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrivacyLevel {
    Standard = 0,
    Private = 1,
    Anonymous = 2,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MaskedAddress {
    Full(Address),
    Partial(Bytes),
    Hashed(BytesN<32>),
}
pub fn mask_address(env: &Env, address: &Address, privacy_level: PrivacyLevel) -> MaskedAddress {
    match privacy_level {
        PrivacyLevel::Standard => MaskedAddress::Full(address.clone()),

        PrivacyLevel::Private => {
            let xdr = address.to_xdr(env);
            let limit = 8_u32.min(xdr.len());
            MaskedAddress::Partial(xdr.slice(0..limit))
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
