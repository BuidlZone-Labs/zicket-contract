#[cfg(test)]
mod tests {
    use crate::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup_test() -> (Env, EventContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(EventContract, ());
        let client = EventContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        (env, client, admin)
    }

    #[test]
    fn test_contract_version_initialization() {
        let (env, client, admin) = setup_test();

        client.initialize(&admin, &Address::generate(&env), &Address::generate(&env));

        let version = client.contract_version();
        assert_eq!(version, 1);
    }

    #[test]
    fn test_migration_v1_to_v2() {
        let (env, client, admin) = setup_test();

        client.initialize(&admin, &Address::generate(&env), &Address::generate(&env));

        let current_version = client.contract_version();
        assert_eq!(current_version, 1);

        let new_version = client.migrate(&admin);
        assert_eq!(new_version, 2);

        let updated_version = client.contract_version();
        assert_eq!(updated_version, 2);
    }

    #[test]
    fn test_migration_unauthorized() {
        let (env, client, admin) = setup_test();
        let unauthorized = Address::generate(&env);

        client.initialize(&admin, &Address::generate(&env), &Address::generate(&env));

        let result = client.try_migrate(&unauthorized);
        assert!(result.is_err());
    }

    #[test]
    fn test_storage_compatibility_after_migration() {
        let (env, client, admin) = setup_test();
        let ticket_contract = Address::generate(&env);
        let payments_contract = Address::generate(&env);
        let contract_id = client.address.clone();

        client.initialize(&admin, &ticket_contract, &payments_contract);

        client.migrate(&admin);

        env.as_contract(&contract_id, || {
            let admin_after = storage::get_admin(&env).unwrap();
            assert_eq!(admin_after, admin);

            let ticket_contract_after = storage::get_ticket_contract(&env).unwrap();
            assert_eq!(ticket_contract_after, ticket_contract);

            let payments_contract_after = storage::get_payments_contract(&env).unwrap();
            assert_eq!(payments_contract_after, payments_contract);
        });
    }

    #[test]
    fn test_multiple_migrations() {
        let (env, client, admin) = setup_test();

        client.initialize(&admin, &Address::generate(&env), &Address::generate(&env));

        let v2 = client.migrate(&admin);
        assert_eq!(v2, 2);

        let v3 = client.migrate(&admin);
        assert_eq!(v3, 3);

        let final_version = client.contract_version();
        assert_eq!(final_version, 3);
    }

    #[test]
    fn test_version_compatibility_check() {
        let (env, client, admin) = setup_test();
        let contract_id = client.address.clone();

        client.initialize(&admin, &Address::generate(&env), &Address::generate(&env));

        env.as_contract(&contract_id, || {
            let result = storage::verify_version(&env);
            assert!(result.is_ok());
        });
    }
}
