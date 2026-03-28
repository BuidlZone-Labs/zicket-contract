#[cfg(test)]
mod tests {
    use crate::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

    fn setup_test() -> (
        Env,
        PaymentsContractClient<'static>,
        Address,
        Address,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PaymentsContract, ());
        let client = PaymentsContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let token = Address::generate(&env);
        let event_contract = Address::generate(&env);
        (env, client, admin, token, event_contract)
    }

    #[test]
    fn test_contract_version_initialization() {
        let (_env, client, admin, token, event_contract) = setup_test();

        client.initialize(&admin, &token, &event_contract);

        let version = client.contract_version();
        assert_eq!(version, 1);
    }

    #[test]
    fn test_migration_v1_to_v2() {
        let (_env, client, admin, token, event_contract) = setup_test();

        client.initialize(&admin, &token, &event_contract);

        let current_version = client.contract_version();
        assert_eq!(current_version, 1);

        let new_version = client.migrate(&admin);
        assert_eq!(new_version, 2);

        let updated_version = client.contract_version();
        assert_eq!(updated_version, 2);
    }

    #[test]
    fn test_migration_unauthorized() {
        let (_env, client, admin, token, event_contract) = setup_test();
        let unauthorized = Address::generate(&_env);

        client.initialize(&admin, &token, &event_contract);

        let result = client.try_migrate(&unauthorized);
        assert!(result.is_err());
    }

    #[test]
    fn test_storage_compatibility_after_migration() {
        let (env, client, admin, token, event_contract) = setup_test();
        let contract_id = client.address.clone();

        client.initialize(&admin, &token, &event_contract);

        client.migrate(&admin);

        env.as_contract(&contract_id, || {
            let admin_after = storage::get_admin(&env).unwrap();
            assert_eq!(admin_after, admin);

            let token_after = storage::get_accepted_token(&env).unwrap();
            assert_eq!(token_after, token);

            let event_contract_after = storage::get_event_contract(&env).unwrap();
            assert_eq!(event_contract_after, event_contract);
        });
    }

    #[test]
    fn test_multiple_migrations() {
        let (_env, client, admin, token, event_contract) = setup_test();

        client.initialize(&admin, &token, &event_contract);

        let v2 = client.migrate(&admin);
        assert_eq!(v2, 2);

        let v3 = client.migrate(&admin);
        assert_eq!(v3, 3);

        let final_version = client.contract_version();
        assert_eq!(final_version, 3);
    }

    #[test]
    fn test_version_compatibility_check() {
        let (env, client, admin, token, event_contract) = setup_test();
        let contract_id = client.address.clone();

        client.initialize(&admin, &token, &event_contract);

        env.as_contract(&contract_id, || {
            let result = storage::verify_version(&env);
            assert!(result.is_ok());
        });
    }

    #[test]
    fn test_payment_operations_after_migration() {
        let (_env, client, admin, token, event_contract) = setup_test();

        client.initialize(&admin, &token, &event_contract);

        client.migrate(&admin);

        let event_id = Symbol::new(&_env, "test_event");
        let payments = client.get_event_payments(&event_id);
        assert_eq!(payments.len(), 0);

        let revenue = client.get_event_revenue(&event_id);
        assert_eq!(revenue, 0);
    }
}
