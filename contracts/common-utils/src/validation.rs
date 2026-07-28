//! Common validation utilities for contract inputs and business rules.

use soroban_sdk::Address;

/// Maximum number of revenue split recipients allowed.
pub const MAX_REVENUE_SPLIT_RECIPIENTS: u32 = 5;

/// Total basis points (100%).
pub const TOTAL_BASIS_POINTS: u32 = 10_000;

/// Validate that basis points are within valid range (0-10000).
///
/// # Arguments
/// * `bps` - The basis points value to validate
///
/// # Returns
/// * `true` if valid (0 <= bps <= 10000), `false` otherwise
pub fn is_valid_basis_points(bps: u32) -> bool {
    bps <= TOTAL_BASIS_POINTS
}

/// Validate that multiple basis point values sum to exactly 10000.
///
/// # Arguments
/// * `values` - Iterator of basis point values
///
/// # Returns
/// * `Some(total)` if the sum is valid (no overflow), `None` on overflow
/// * Caller should check if total == TOTAL_BASIS_POINTS
pub fn validate_basis_points_sum<I>(values: I) -> Option<u32>
where
    I: Iterator<Item = u32>,
{
    let mut total: u32 = 0;
    for bps in values {
        total = total.checked_add(bps)?;
    }
    Some(total)
}

/// Validate revenue split configuration.
///
/// Rules:
/// - Empty splits are allowed (legacy single-organizer mode)
/// - 1-5 recipients maximum
/// - Basis points must sum to exactly 10000
/// - No zero allocations
/// - No duplicate recipients
/// - Index 0 must be the primary organizer
///
/// # Arguments
/// * `splits` - Vector of (Address, basis_points) tuples
/// * `organizer` - The primary organizer address
///
/// # Returns
/// * `Ok(())` if valid, `Err(&'static str)` with error description
pub fn validate_revenue_splits(
    splits: &soroban_sdk::Vec<(Address, u32)>,
    organizer: &Address,
) -> Result<(), &'static str> {
    let len = splits.len();
    
    // Empty splits are allowed (single organizer)
    if len == 0 {
        return Ok(());
    }
    
    // Check maximum recipients
    if len > MAX_REVENUE_SPLIT_RECIPIENTS {
        return Err("Too many split recipients");
    }
    
    // First recipient must be the primary organizer
    let (first, _) = splits.get(0).ok_or("Split configuration is empty")?;
    if first != *organizer {
        return Err("First recipient must be the primary organizer");
    }
    
    let mut total: u32 = 0;
    
    for i in 0..len {
        let (recipient, bps) = splits.get(i).ok_or("Failed to get split entry")?;
        
        // No zero allocations
        if bps == 0 {
            return Err("Basis points cannot be zero");
        }
        
        // Sum with overflow check
        total = total
            .checked_add(bps)
            .ok_or("Basis points sum overflow")?;
        
        // Check for duplicates
        for j in 0..i {
            let (other, _) = splits.get(j).ok_or("Failed to get split entry")?;
            if other == recipient {
                return Err("Duplicate recipient in split");
            }
        }
    }
    
    // Must sum to exactly 10000 (100%)
    if total != TOTAL_BASIS_POINTS {
        return Err("Basis points must sum to 10000");
    }
    
    Ok(())
}

/// Calculate the share for a specific recipient based on basis points.
///
/// Uses floor division for non-primary recipients. The primary organizer
/// (index 0) receives the remainder to ensure all dust goes to them and
/// the sum of shares equals the net amount.
///
/// For empty splits (legacy single-organizer mode), only the organizer receives
/// the full net_amount; other addresses receive 0.
///
/// # Arguments
/// * `splits` - Vector of (Address, basis_points) tuples
/// * `recipient` - The recipient to calculate share for
/// * `organizer` - The primary organizer (used for empty split validation)
/// * `net_amount` - The total net amount to distribute
///
/// # Returns
/// * The calculated share as i128
pub fn calculate_recipient_share(
    splits: &soroban_sdk::Vec<(Address, u32)>,
    recipient: &Address,
    organizer: &Address,
    net_amount: i128,
) -> i128 {
    if splits.is_empty() {
        // Empty splits: only organizer receives the full amount
        if recipient == organizer {
            return net_amount;
        } else {
            return 0;
        }
    }
    
    let (primary, _) = match splits.get(0) {
        Some(entry) => entry,
        None => return 0,
    };
    
    // Primary organizer gets the remainder (to capture dust)
    if *recipient == primary {
        let mut others_total: i128 = 0;
        for i in 1..splits.len() {
            if let Some((_, bps)) = splits.get(i) {
                others_total += net_amount * (bps as i128) / (TOTAL_BASIS_POINTS as i128);
            }
        }
        net_amount - others_total
    } else {
        // Other recipients get floor division
        find_recipient_basis_points(splits, recipient)
            .map(|bps| net_amount * (bps as i128) / (TOTAL_BASIS_POINTS as i128))
            .unwrap_or(0)
    }
}

/// Find the basis points allocation for a specific recipient.
///
/// # Arguments
/// * `splits` - Vector of (Address, basis_points) tuples
/// * `recipient` - The recipient to find
///
/// # Returns
/// * `Some(bps)` if recipient is found, `None` otherwise
pub fn find_recipient_basis_points(
    splits: &soroban_sdk::Vec<(Address, u32)>,
    recipient: &Address,
) -> Option<u32> {
    for i in 0..splits.len() {
        if let Some((addr, bps)) = splits.get(i) {
            if addr == *recipient {
                return Some(bps);
            }
        }
    }
    None
}

/// Check if an address is in the revenue split configuration.
///
/// # Arguments
/// * `splits` - Vector of (Address, basis_points) tuples
/// * `address` - The address to check
///
/// # Returns
/// * `true` if address is a recipient, `false` otherwise
pub fn is_split_recipient(
    splits: &soroban_sdk::Vec<(Address, u32)>,
    address: &Address,
) -> bool {
    find_recipient_basis_points(splits, address).is_some()
}
