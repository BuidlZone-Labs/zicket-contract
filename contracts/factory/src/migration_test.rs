#[cfg(test)]
mod tests {
    use crate::*;
    use soroban_sdk::{testutils::Address as _, Address, BytesN, Env};

    fn setup_test() -> (Env, FactoryContractClient<'static>, Address, BytesN<32>) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(FactoryContract, ());
        let client = FactoryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let wasm = BytesN::from_array(&env, &[0u8; 32]);
        (env, client, admin, wasm)
    }

    #[test]
    fn test_contract_version_initialization() {
        let (_env, client, admin, wasm) = setup_test();

        client.initialize(
            &admin,
            &wasm,
            &Address::generate(&_env),
            &Address::generate(&_env),
        );

        let version = client.contract_version();
        assert_eq!(version, 1);
    }

    #[test]
    fn test_migration_v1_to_v2() {
        let (_env, client, admin, wasm) = setup_test();

        client.initialize(
            &admin,
            &wasm,
            &Address::generate(&_env),
            &Address::generate(&_env),
        );

        let current_version = client.contract_version();
        assert_eq!(current_version, 1);

        let new_version = client.migrate(&admin);
        assert_eq!(new_version, 2);

        let updated_version = client.contract_version();
        assert_eq!(updated_version, 2);
    }

    #[test]
    fn test_migration_unauthorized() {
        let (_env, client, admin, wasm) = setup_test();
        let unauthorized = Address::generate(&_env);

        client.initialize(
            &admin,
            &wasm,
            &Address::generate(&_env),
            &Address::generate(&_env),
        );

        let result = client.try_migrate(&unauthorized);
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_migrations() {
        let (_env, client, admin, wasm) = setup_test();

        client.initialize(
            &admin,
            &wasm,
            &Address::generate(&_env),
            &Address::generate(&_env),
        );

        let v2 = client.migrate(&admin);
        assert_eq!(v2, 2);

        let v3 = client.migrate(&admin);
        assert_eq!(v3, 3);

        let final_version = client.contract_version();
        assert_eq!(final_version, 3);
    }

    #[test]
    fn test_event_deployment_after_migration() {
        let (_env, client, admin, wasm) = setup_test();

        client.initialize(
            &admin,
            &wasm,
            &Address::generate(&_env),
            &Address::generate(&_env),
        );

        client.migrate(&admin);

        let all_events = client.get_all_events();
        assert_eq!(all_events.len(), 0);

        let organizer = Address::generate(&_env);
        let organizer_events = client.get_organizer_events(&organizer);
        assert_eq!(organizer_events.len(), 0);
    }
}
