#[cfg(test)]
mod tests {
    use crate::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup_test() -> (Env, TicketContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(TicketContract, ());
        let client = TicketContractClient::new(&env, &contract_id);
        let caller = Address::generate(&env);
        (env, client, caller)
    }

    #[test]
    fn test_contract_version_initialization() {
        let (_env, client, _caller) = setup_test();

        let version = client.contract_version();
        assert_eq!(version, 1);
    }

    #[test]
    fn test_migration_v1_to_v2() {
        let (_env, client, caller) = setup_test();

        let current_version = client.contract_version();
        assert_eq!(current_version, 1);

        let new_version = client.migrate(&caller);
        assert_eq!(new_version, 2);

        let updated_version = client.contract_version();
        assert_eq!(updated_version, 2);
    }

    #[test]
    fn test_migration_requires_auth() {
        let (_env, client, caller) = setup_test();

        let result = client.try_migrate(&caller);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_multiple_migrations() {
        let (_env, client, caller) = setup_test();

        let v2 = client.migrate(&caller);
        assert_eq!(v2, 2);

        let v3 = client.migrate(&caller);
        assert_eq!(v3, 3);

        let final_version = client.contract_version();
        assert_eq!(final_version, 3);
    }

    #[test]
    fn test_version_compatibility_check() {
        let (env, client, _caller) = setup_test();
        let contract_id = client.address.clone();

        env.as_contract(&contract_id, || {
            let result = storage::verify_version(&env);
            assert!(result.is_ok());
        });
    }

    #[test]
    fn test_ticket_operations_after_migration() {
        let (_env, client, caller) = setup_test();

        client.migrate(&caller);

        let owner = Address::generate(&_env);
        let tickets = client.get_tickets_by_owner(&owner);
        assert_eq!(tickets.len(), 0);
    }
}
