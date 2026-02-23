use soroban_sdk::{Address, BytesN, Env};

pub fn deploy_event_contract(
    env: &Env,
    salt: &BytesN<32>,
    wasm_hash: &BytesN<32>,
) -> Address {
    env.deployer()
        .with_current_contract(salt.clone())
        .deploy_v2(wasm_hash.clone(), ())
}
