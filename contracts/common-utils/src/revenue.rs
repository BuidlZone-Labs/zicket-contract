//! Revenue calculation and distribution utilities.

use crate::validation::{calculate_recipient_share, TOTAL_BASIS_POINTS};
use soroban_sdk::Address;

/// Calculate platform fee from gross amount.
///
/// # Arguments
/// * `gross_amount` - The gross revenue amount
/// * `platform_fee_bps` - Platform fee in basis points
///
/// # Returns
/// * Platform fee amount (floor division)
pub fn calculate_platform_fee(gross_amount: i128, platform_fee_bps: u32) -> i128 {
    gross_amount * (platform_fee_bps as i128) / (TOTAL_BASIS_POINTS as i128)
}

/// Calculate net amount after deducting platform fee.
///
/// # Arguments
/// * `gross_amount` - The gross revenue amount
/// * `platform_fee_bps` - Platform fee in basis points
///
/// # Returns
/// * Net amount after fee deduction
pub fn calculate_net_amount(gross_amount: i128, platform_fee_bps: u32) -> i128 {
    let fee = calculate_platform_fee(gross_amount, platform_fee_bps);
    gross_amount - fee
}

/// Calculate all recipient shares from a revenue split.
///
/// For empty splits (legacy single-organizer mode), returns a single entry
/// with the organizer receiving the full net_amount.
///
/// # Arguments
/// * `splits` - Vector of (Address, basis_points) tuples
/// * `organizer` - The primary organizer address
/// * `net_amount` - The net amount to distribute (after platform fees)
///
/// # Returns
/// * Vector of (Address, amount) tuples
pub fn calculate_all_shares(
    splits: &soroban_sdk::Vec<(Address, u32)>,
    organizer: &Address,
    net_amount: i128,
) -> soroban_sdk::Vec<(Address, i128)> {
    let env = splits.env();
    let mut result = soroban_sdk::Vec::new(env);
    
    if splits.is_empty() {
        // Empty splits: single entry for organizer
        result.push_back((organizer.clone(), net_amount));
        return result;
    }
    
    for i in 0..splits.len() {
        if let Some((recipient, _)) = splits.get(i) {
            let share = calculate_recipient_share(splits, &recipient, organizer, net_amount);
            result.push_back((recipient, share));
        }
    }
    
    result
}

/// Verify that calculated shares sum to the net amount (no dust leakage).
///
/// # Arguments
/// * `shares` - Vector of (Address, amount) tuples
/// * `expected_total` - Expected sum
///
/// # Returns
/// * `true` if shares sum to expected total, `false` otherwise
pub fn verify_shares_sum(
    shares: &soroban_sdk::Vec<(Address, i128)>,
    expected_total: i128,
) -> bool {
    let mut sum: i128 = 0;
    for i in 0..shares.len() {
        if let Some((_, amount)) = shares.get(i) {
            sum += amount;
        }
    }
    sum == expected_total
}
