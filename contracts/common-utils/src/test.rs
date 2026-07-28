#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_basis_points_validation() {
    assert!(validation::is_valid_basis_points(0));
    assert!(validation::is_valid_basis_points(5000));
    assert!(validation::is_valid_basis_points(10_000));
    assert!(!validation::is_valid_basis_points(10_001));
    assert!(!validation::is_valid_basis_points(u32::MAX));
}

#[test]
fn test_basis_points_sum() {
    let values = [2500u32, 2500, 2500, 2500];
    let sum = validation::validate_basis_points_sum(values.iter().copied());
    assert_eq!(sum, Some(10_000));
    
    let overflow_values = [u32::MAX, 1u32];
    let overflow = validation::validate_basis_points_sum(overflow_values.iter().copied());
    assert_eq!(overflow, None);
}

#[test]
fn test_revenue_split_validation_empty() {
    let env = Env::default();
    let organizer = Address::generate(&env);
    let splits = soroban_sdk::Vec::new(&env);
    
    let result = validation::validate_revenue_splits(&splits, &organizer);
    assert!(result.is_ok());
}

#[test]
fn test_empty_split_organizer_gets_full_amount() {
    let env = Env::default();
    let organizer = Address::generate(&env);
    let splits = soroban_sdk::Vec::new(&env);
    
    let share = validation::calculate_recipient_share(&splits, &organizer, &organizer, 1000);
    assert_eq!(share, 1000);
}

#[test]
fn test_empty_split_non_organizer_gets_zero() {
    let env = Env::default();
    let organizer = Address::generate(&env);
    let non_organizer = Address::generate(&env);
    let splits = soroban_sdk::Vec::new(&env);
    
    let share = validation::calculate_recipient_share(&splits, &non_organizer, &organizer, 1000);
    assert_eq!(share, 0);
}

#[test]
fn test_empty_split_all_shares_single_entry() {
    let env = Env::default();
    let organizer = Address::generate(&env);
    let splits = soroban_sdk::Vec::new(&env);
    
    let shares = revenue::calculate_all_shares(&splits, &organizer, 1000);
    assert_eq!(shares.len(), 1);
    
    let (recipient, amount) = shares.get(0).unwrap();
    assert_eq!(recipient, organizer);
    assert_eq!(amount, 1000);
}

#[test]
fn test_revenue_split_validation_valid() {
    let env = Env::default();
    let organizer = Address::generate(&env);
    let recipient2 = Address::generate(&env);
    let recipient3 = Address::generate(&env);
    
    let mut splits = soroban_sdk::Vec::new(&env);
    splits.push_back((organizer.clone(), 5000));
    splits.push_back((recipient2, 3000));
    splits.push_back((recipient3, 2000));
    
    let result = validation::validate_revenue_splits(&splits, &organizer);
    assert!(result.is_ok());
}

#[test]
fn test_revenue_split_validation_wrong_organizer() {
    let env = Env::default();
    let organizer = Address::generate(&env);
    let wrong_first = Address::generate(&env);
    
    let mut splits = soroban_sdk::Vec::new(&env);
    splits.push_back((wrong_first, 10_000));
    
    let result = validation::validate_revenue_splits(&splits, &organizer);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "First recipient must be the primary organizer");
}

#[test]
fn test_revenue_split_validation_wrong_sum() {
    let env = Env::default();
    let organizer = Address::generate(&env);
    let recipient2 = Address::generate(&env);
    
    let mut splits = soroban_sdk::Vec::new(&env);
    splits.push_back((organizer.clone(), 5000));
    splits.push_back((recipient2, 4000)); // Sum = 9000, not 10000
    
    let result = validation::validate_revenue_splits(&splits, &organizer);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Basis points must sum to 10000");
}

#[test]
fn test_revenue_split_validation_duplicate() {
    let env = Env::default();
    let organizer = Address::generate(&env);
    
    let mut splits = soroban_sdk::Vec::new(&env);
    splits.push_back((organizer.clone(), 5000));
    splits.push_back((organizer.clone(), 5000)); // Duplicate
    
    let result = validation::validate_revenue_splits(&splits, &organizer);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Duplicate recipient in split");
}

#[test]
fn test_revenue_split_validation_zero_bps() {
    let env = Env::default();
    let organizer = Address::generate(&env);
    let recipient2 = Address::generate(&env);
    
    let mut splits = soroban_sdk::Vec::new(&env);
    splits.push_back((organizer.clone(), 10_000));
    splits.push_back((recipient2, 0)); // Zero basis points
    
    let result = validation::validate_revenue_splits(&splits, &organizer);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Basis points cannot be zero");
}

#[test]
fn test_revenue_split_validation_too_many() {
    let env = Env::default();
    let organizer = Address::generate(&env);
    
    let mut splits = soroban_sdk::Vec::new(&env);
    splits.push_back((organizer.clone(), 2000));
    splits.push_back((Address::generate(&env), 2000));
    splits.push_back((Address::generate(&env), 2000));
    splits.push_back((Address::generate(&env), 2000));
    splits.push_back((Address::generate(&env), 2000));
    splits.push_back((Address::generate(&env), 2000)); // 6th recipient
    
    let result = validation::validate_revenue_splits(&splits, &organizer);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Too many split recipients");
}

#[test]
fn test_calculate_recipient_share() {
    let env = Env::default();
    let primary = Address::generate(&env);
    let recipient2 = Address::generate(&env);
    let recipient3 = Address::generate(&env);
    
    let mut splits = soroban_sdk::Vec::new(&env);
    splits.push_back((primary.clone(), 5000)); // 50%
    splits.push_back((recipient2.clone(), 3000)); // 30%
    splits.push_back((recipient3.clone(), 2000)); // 20%
    
    let net = 1000_i128;
    
    let share2 = validation::calculate_recipient_share(&splits, &recipient2, &primary, net);
    assert_eq!(share2, 300); // 30%
    
    let share3 = validation::calculate_recipient_share(&splits, &recipient3, &primary, net);
    assert_eq!(share3, 200); // 20%
    
    // Primary gets remainder (includes dust)
    let share1 = validation::calculate_recipient_share(&splits, &primary, &primary, net);
    assert_eq!(share1, 500); // 50%
    
    // Verify total
    assert_eq!(share1 + share2 + share3, net);
}

#[test]
fn test_calculate_recipient_share_with_dust() {
    let env = Env::default();
    let primary = Address::generate(&env);
    let recipient2 = Address::generate(&env);
    let recipient3 = Address::generate(&env);
    
    let mut splits = soroban_sdk::Vec::new(&env);
    splits.push_back((primary.clone(), 3334)); // 33.34%
    splits.push_back((recipient2.clone(), 3333)); // 33.33%
    splits.push_back((recipient3.clone(), 3333)); // 33.33%
    
    let net = 1000_i128;
    
    let share2 = validation::calculate_recipient_share(&splits, &recipient2, &primary, net);
    let share3 = validation::calculate_recipient_share(&splits, &recipient3, &primary, net);
    let share1 = validation::calculate_recipient_share(&splits, &primary, &primary, net);
    
    // Verify total equals net (no dust leakage)
    assert_eq!(share1 + share2 + share3, net);
    
    // Primary should get slightly more due to remainder
    assert!(share1 >= share2);
    assert!(share1 >= share3);
}

#[test]
fn test_platform_fee_calculation() {
    assert_eq!(revenue::calculate_platform_fee(10_000, 250), 250); // 2.5%
    assert_eq!(revenue::calculate_platform_fee(10_000, 1000), 1000); // 10%
    assert_eq!(revenue::calculate_platform_fee(10_000, 0), 0); // 0%
    assert_eq!(revenue::calculate_platform_fee(10_000, 10_000), 10_000); // 100%
}

#[test]
fn test_net_amount_calculation() {
    assert_eq!(revenue::calculate_net_amount(10_000, 250), 9750); // After 2.5% fee
    assert_eq!(revenue::calculate_net_amount(10_000, 1000), 9000); // After 10% fee
    assert_eq!(revenue::calculate_net_amount(10_000, 0), 10_000); // No fee
}

#[test]
fn test_calculate_all_shares() {
    let env = Env::default();
    let primary = Address::generate(&env);
    let recipient2 = Address::generate(&env);
    
    let mut splits = soroban_sdk::Vec::new(&env);
    splits.push_back((primary.clone(), 7000));
    splits.push_back((recipient2.clone(), 3000));
    
    let shares = revenue::calculate_all_shares(&splits, &primary, 10_000);
    
    assert_eq!(shares.len(), 2);
    
    // Verify shares sum correctly
    assert!(revenue::verify_shares_sum(&shares, 10_000));
}

#[test]
fn test_find_recipient_basis_points() {
    let env = Env::default();
    let primary = Address::generate(&env);
    let recipient2 = Address::generate(&env);
    let not_in_list = Address::generate(&env);
    
    let mut splits = soroban_sdk::Vec::new(&env);
    splits.push_back((primary.clone(), 7000));
    splits.push_back((recipient2.clone(), 3000));
    
    assert_eq!(validation::find_recipient_basis_points(&splits, &primary), Some(7000));
    assert_eq!(validation::find_recipient_basis_points(&splits, &recipient2), Some(3000));
    assert_eq!(validation::find_recipient_basis_points(&splits, &not_in_list), None);
}

#[test]
fn test_is_split_recipient() {
    let env = Env::default();
    let primary = Address::generate(&env);
    let recipient2 = Address::generate(&env);
    let not_in_list = Address::generate(&env);
    
    let mut splits = soroban_sdk::Vec::new(&env);
    splits.push_back((primary.clone(), 7000));
    splits.push_back((recipient2.clone(), 3000));
    
    assert!(validation::is_split_recipient(&splits, &primary));
    assert!(validation::is_split_recipient(&splits, &recipient2));
    assert!(!validation::is_split_recipient(&splits, &not_in_list));
}
