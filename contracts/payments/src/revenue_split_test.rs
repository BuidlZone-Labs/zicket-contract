//! Tests for multi-organizer revenue splits and co-host wallet management (#122).
//!
//! These exercise the payments-contract surface directly, which is where the
//! escrowed funds live and where each recipient's share is actually paid out.

use super::*;
use mock_event_contract::MockEventContract;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, token, vec, Address, Env, Symbol, Vec};

/// Standard split-event setup: payments contract + asset, a bound event whose
/// organizer is the split index-0 recipient, with revenue already paid in.
struct SplitFixture<'a> {
    env: &'a Env,
    admin: Address,
    token: Address,
    client: PaymentsContractClient<'a>,
    contract_id: Address,
    organizer: Address,
    event_id: Symbol,
}

fn make_payments(
    env: &Env,
    fee_bps: u32,
) -> (
    Address,
    Address,
    PaymentsContractClient<'_>,
    Address,
    Address,
) {
    let contract_id = env.register(PaymentsContract, ());
    let client = PaymentsContractClient::new(env, &contract_id);
    let event_contract = env.register(MockEventContract, ());

    let admin = Address::generate(env);
    let platform_wallet = Address::generate(env);
    let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
    let token = token_contract.address();
    client.initialize(&admin, &token, &fee_bps, &platform_wallet, &event_contract);
    (admin, token, client, contract_id, event_contract)
}

fn bind(
    client: &PaymentsContractClient,
    event_contract: &Address,
    event_id: &Symbol,
    organizer: &Address,
    token: &Address,
) {
    // event_start_ledger=0, event_end_ledger=1000, withdrawal_delay_ledgers=17280
    // => unlock at ledger 18280.
    client.sync_event_config(
        event_contract,
        event_id,
        organizer,
        token,
        &true,
        &false,
        &0,
        &0,
        &0,
        &1000,
        &17280,
    );
}

/// Pay `amount` into the event escrow from a freshly funded payer.
fn pay(fx: &SplitFixture, nonce: u64, amount: i128) {
    let payer = Address::generate(fx.env);
    let asset = token::StellarAssetClient::new(fx.env, &fx.token);
    asset.mint(&payer, &amount);
    fx.client.pay_for_ticket(
        &nonce,
        &payer,
        &fx.event_id,
        &amount,
        &None,
        &fx.token,
        &PaymentPrivacy::Standard,
    );
}

fn balance(fx: &SplitFixture, who: &Address) -> i128 {
    token::Client::new(fx.env, &fx.token).balance(who)
}

/// Build a completed split event with two recipients (primary 60% / co-host 40%)
/// and `total` revenue already escrowed. Ledger is advanced past the unlock.
fn completed_two_way(env: &Env, fee_bps: u32, total: i128) -> (SplitFixture<'_>, Address) {
    let (admin, token, client, contract_id, event_contract) = make_payments(env, fee_bps);
    let organizer = Address::generate(env);
    let cohost = Address::generate(env);
    let event_id = symbol_short!("SPLITEV");

    bind(&client, &event_contract, &event_id, &organizer, &token);
    let splits: Vec<(Address, u32)> =
        vec![env, (organizer.clone(), 6000u32), (cohost.clone(), 4000u32)];
    client.sync_revenue_splits(&event_contract, &event_id, &splits);

    let _ = event_contract;
    let fx = SplitFixture {
        env,
        admin,
        token,
        client,
        contract_id,
        organizer,
        event_id,
    };

    pay(&fx, 1, total);
    fx.client
        .set_event_status(&fx.admin, &fx.event_id, &EventStatus::Completed);
    env.ledger().with_mut(|li| li.sequence_number = 20_000);

    (fx, cohost)
}

#[test]
fn test_sync_revenue_splits_stores_and_reads_back() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, token, client, _cid, event_contract) = make_payments(&env, 0);
    let organizer = Address::generate(&env);
    let cohost = Address::generate(&env);
    let event_id = symbol_short!("EV");
    bind(&client, &event_contract, &event_id, &organizer, &token);

    let splits: Vec<(Address, u32)> = vec![
        &env,
        (organizer.clone(), 7000u32),
        (cohost.clone(), 3000u32),
    ];
    client.sync_revenue_splits(&event_contract, &event_id, &splits);

    let read = client.get_revenue_splits(&event_id);
    assert_eq!(read.len(), 2);
    assert_eq!(read.get(0).unwrap(), (organizer, 7000u32));
    assert_eq!(read.get(1).unwrap(), (cohost, 3000u32));
}

#[test]
fn test_sync_revenue_splits_rejects_bad_sum() {
    let env = Env::default();
    env.mock_all_auths();
    let (_a, token, client, _c, event_contract) = make_payments(&env, 0);
    let organizer = Address::generate(&env);
    let cohost = Address::generate(&env);
    let event_id = symbol_short!("EV");
    bind(&client, &event_contract, &event_id, &organizer, &token);

    let bad: Vec<(Address, u32)> = vec![&env, (organizer, 6000u32), (cohost, 3000u32)]; // 9000
    let res = client.try_sync_revenue_splits(&event_contract, &event_id, &bad);
    assert_eq!(res.err(), Some(Ok(PaymentError::InvalidSplitConfig)));
}

#[test]
fn test_sync_revenue_splits_rejects_more_than_five() {
    let env = Env::default();
    env.mock_all_auths();
    let (_a, token, client, _c, event_contract) = make_payments(&env, 0);
    let organizer = Address::generate(&env);
    let event_id = symbol_short!("EV");
    bind(&client, &event_contract, &event_id, &organizer, &token);

    // 6 recipients summing to 10000.
    let six: Vec<(Address, u32)> = vec![
        &env,
        (organizer, 5000u32),
        (Address::generate(&env), 1000u32),
        (Address::generate(&env), 1000u32),
        (Address::generate(&env), 1000u32),
        (Address::generate(&env), 1000u32),
        (Address::generate(&env), 1000u32),
    ];
    let res = client.try_sync_revenue_splits(&event_contract, &event_id, &six);
    assert_eq!(res.err(), Some(Ok(PaymentError::InvalidSplitConfig)));
}

#[test]
fn test_sync_revenue_splits_rejects_duplicate_and_zero() {
    let env = Env::default();
    env.mock_all_auths();
    let (_a, token, client, _c, event_contract) = make_payments(&env, 0);
    let organizer = Address::generate(&env);
    let dup = Address::generate(&env);
    let event_id = symbol_short!("EV");
    bind(&client, &event_contract, &event_id, &organizer, &token);

    let duplicate: Vec<(Address, u32)> = vec![
        &env,
        (organizer.clone(), 5000u32),
        (dup.clone(), 2500u32),
        (dup, 2500u32),
    ];
    assert_eq!(
        client
            .try_sync_revenue_splits(&event_contract, &event_id, &duplicate)
            .err(),
        Some(Ok(PaymentError::InvalidSplitConfig))
    );

    let zero: Vec<(Address, u32)> =
        vec![&env, (organizer, 10000u32), (Address::generate(&env), 0u32)];
    assert_eq!(
        client
            .try_sync_revenue_splits(&event_contract, &event_id, &zero)
            .err(),
        Some(Ok(PaymentError::InvalidSplitConfig))
    );
}

#[test]
fn test_revenue_splits_are_immutable_once_set() {
    let env = Env::default();
    env.mock_all_auths();
    let (_a, token, client, _c, event_contract) = make_payments(&env, 0);
    let organizer = Address::generate(&env);
    let cohost = Address::generate(&env);
    let event_id = symbol_short!("EV");
    bind(&client, &event_contract, &event_id, &organizer, &token);

    let splits: Vec<(Address, u32)> = vec![
        &env,
        (organizer.clone(), 6000u32),
        (cohost.clone(), 4000u32),
    ];
    client.sync_revenue_splits(&event_contract, &event_id, &splits);

    // A second attempt (even with a different valid config) must be rejected.
    let changed: Vec<(Address, u32)> = vec![&env, (organizer, 8000u32), (cohost, 2000u32)];
    let res = client.try_sync_revenue_splits(&event_contract, &event_id, &changed);
    assert_eq!(res.err(), Some(Ok(PaymentError::InvalidSplitConfig)));
}

#[test]
fn test_sync_revenue_splits_rejects_foreign_caller() {
    let env = Env::default();
    env.mock_all_auths();
    let (_a, token, client, _c, event_contract) = make_payments(&env, 0);
    let organizer = Address::generate(&env);
    let cohost = Address::generate(&env);
    let event_id = symbol_short!("EV");
    bind(&client, &event_contract, &event_id, &organizer, &token);

    let not_event_contract = Address::generate(&env);
    let splits: Vec<(Address, u32)> = vec![&env, (organizer, 6000u32), (cohost, 4000u32)];
    let res = client.try_sync_revenue_splits(&not_event_contract, &event_id, &splits);
    assert_eq!(res.err(), Some(Ok(PaymentError::Unauthorized)));
}

#[test]
fn test_platform_fee_deducted_before_split_and_independent_withdrawals() {
    let env = Env::default();
    env.mock_all_auths();
    // 2.5% platform fee, 200M revenue, 60/40 split.
    let (fx, cohost) = completed_two_way(&env, 250, 200_000_000);

    // Co-host withdraws independently first.
    fx.client.withdraw_split(&cohost, &fx.event_id);
    // fee = 5_000_000; net = 195_000_000; co-host 40% = 78_000_000.
    assert_eq!(balance(&fx, &cohost), 78_000_000);
    // Primary has not withdrawn yet.
    assert_eq!(balance(&fx, &fx.organizer), 0);

    // Primary withdraws independently.
    fx.client.withdraw_split(&fx.organizer, &fx.event_id);
    assert_eq!(balance(&fx, &fx.organizer), 117_000_000); // 60% of net

    // Platform fee (5M) remains in the contract until the admin sweeps it.
    assert_eq!(balance(&fx, &fx.contract_id), 5_000_000);
    assert_eq!(fx.client.get_platform_revenue(&fx.event_id), 5_000_000);
}

#[test]
fn test_withdraw_split_rejects_double_withdraw_and_non_recipient() {
    let env = Env::default();
    env.mock_all_auths();
    let (fx, cohost) = completed_two_way(&env, 0, 100_000_000);

    fx.client.withdraw_split(&cohost, &fx.event_id);
    assert_eq!(balance(&fx, &cohost), 40_000_000);

    // Second withdrawal by same recipient is rejected.
    assert_eq!(
        fx.client.try_withdraw_split(&cohost, &fx.event_id).err(),
        Some(Ok(PaymentError::SplitAlreadyWithdrawn))
    );

    // A stranger is not a recipient.
    let stranger = Address::generate(&env);
    assert_eq!(
        fx.client.try_withdraw_split(&stranger, &fx.event_id).err(),
        Some(Ok(PaymentError::NotASplitRecipient))
    );
}

#[test]
fn test_withdraw_split_respects_withdrawal_delay() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, _cid, event_contract) = make_payments(&env, 0);
    let organizer = Address::generate(&env);
    let cohost = Address::generate(&env);
    let event_id = symbol_short!("SPLITEV");
    bind(&client, &event_contract, &event_id, &organizer, &token);
    let splits: Vec<(Address, u32)> = vec![
        &env,
        (organizer.clone(), 6000u32),
        (cohost.clone(), 4000u32),
    ];
    client.sync_revenue_splits(&event_contract, &event_id, &splits);

    let payer = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token).mint(&payer, &100_000_000);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &100_000_000,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    client.set_event_status(&admin, &event_id, &EventStatus::Completed);

    // Before the unlock ledger (18280) settlement must fail.
    env.ledger().with_mut(|li| li.sequence_number = 5000);
    assert_eq!(
        client.try_withdraw_split(&cohost, &event_id).err(),
        Some(Ok(PaymentError::EscrowNotExpired))
    );

    // After the unlock it succeeds.
    env.ledger().with_mut(|li| li.sequence_number = 20_000);
    client.withdraw_split(&cohost, &event_id);
    assert_eq!(
        token::Client::new(&env, &token).balance(&cohost),
        40_000_000
    );
}

#[test]
fn test_rounding_dust_accrues_to_primary_organizer() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client, contract_id, event_contract) = make_payments(&env, 0);
    let organizer = Address::generate(&env);
    let r2 = Address::generate(&env);
    let r3 = Address::generate(&env);
    let event_id = symbol_short!("DUST");
    bind(&client, &event_contract, &event_id, &organizer, &token);
    // 3334 / 3333 / 3333 over a net of 100 leaves 1 unit of dust.
    let splits: Vec<(Address, u32)> = vec![
        &env,
        (organizer.clone(), 3334u32),
        (r2.clone(), 3333u32),
        (r3.clone(), 3333u32),
    ];
    client.sync_revenue_splits(&event_contract, &event_id, &splits);

    let payer = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token).mint(&payer, &100);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &100,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );
    client.set_event_status(&admin, &event_id, &EventStatus::Completed);
    env.ledger().with_mut(|li| li.sequence_number = 20_000);

    let tc = token::Client::new(&env, &token);
    client.withdraw_split(&r2, &event_id);
    client.withdraw_split(&r3, &event_id);
    client.withdraw_split(&organizer, &event_id);

    assert_eq!(tc.balance(&r2), 33);
    assert_eq!(tc.balance(&r3), 33);
    assert_eq!(tc.balance(&organizer), 34); // 33 + 1 dust
                                            // Whole net distributed: nothing left (zero fee).
    assert_eq!(tc.balance(&contract_id), 0);
}

#[test]
fn test_only_primary_can_flag_and_cannot_flag_self() {
    let env = Env::default();
    env.mock_all_auths();
    let (fx, cohost) = completed_two_way(&env, 0, 100_000_000);

    // A non-primary caller cannot flag.
    assert_eq!(
        fx.client
            .try_flag_cohost(&cohost, &fx.event_id, &cohost)
            .err(),
        Some(Ok(PaymentError::Unauthorized))
    );

    // Primary cannot flag itself.
    assert_eq!(
        fx.client
            .try_flag_cohost(&fx.organizer, &fx.event_id, &fx.organizer)
            .err(),
        Some(Ok(PaymentError::Unauthorized))
    );

    // Primary flags the co-host successfully.
    fx.client.flag_cohost(&fx.organizer, &fx.event_id, &cohost);
    assert!(fx.client.is_recipient_flagged(&fx.event_id, &cohost));
}

#[test]
fn test_flagged_cohost_share_held_in_escrow() {
    let env = Env::default();
    env.mock_all_auths();
    let (fx, cohost) = completed_two_way(&env, 0, 100_000_000);

    fx.client.flag_cohost(&fx.organizer, &fx.event_id, &cohost);

    // Flagged co-host cannot withdraw.
    assert_eq!(
        fx.client.try_withdraw_split(&cohost, &fx.event_id).err(),
        Some(Ok(PaymentError::RecipientFlagged))
    );

    // Primary can still withdraw its own share independently.
    fx.client.withdraw_split(&fx.organizer, &fx.event_id);
    assert_eq!(balance(&fx, &fx.organizer), 60_000_000);

    // The co-host's 40M remains escrowed in the contract.
    assert_eq!(balance(&fx, &cohost), 0);
    assert_eq!(balance(&fx, &fx.contract_id), 40_000_000);
}

#[test]
fn test_resolve_flag_release_to_recipient_allows_withdrawal() {
    let env = Env::default();
    env.mock_all_auths();
    let (fx, cohost) = completed_two_way(&env, 0, 100_000_000);

    fx.client.flag_cohost(&fx.organizer, &fx.event_id, &cohost);
    fx.client
        .resolve_flagged_share(&fx.event_id, &cohost, &FlagResolution::ReleaseToRecipient);

    assert!(!fx.client.is_recipient_flagged(&fx.event_id, &cohost));
    fx.client.withdraw_split(&cohost, &fx.event_id);
    assert_eq!(balance(&fx, &cohost), 40_000_000);
}

#[test]
fn test_resolve_flag_reassign_to_primary_pays_primary_and_blocks_recipient() {
    let env = Env::default();
    env.mock_all_auths();
    let (fx, cohost) = completed_two_way(&env, 0, 100_000_000);

    fx.client.flag_cohost(&fx.organizer, &fx.event_id, &cohost);
    fx.client
        .resolve_flagged_share(&fx.event_id, &cohost, &FlagResolution::ReassignToPrimary);

    // The escrowed 40M went to the primary organizer.
    assert_eq!(balance(&fx, &fx.organizer), 40_000_000);

    // The co-host can no longer claim it.
    assert_eq!(
        fx.client.try_withdraw_split(&cohost, &fx.event_id).err(),
        Some(Ok(PaymentError::SplitAlreadyWithdrawn))
    );

    // Primary still withdraws its own 60M share.
    fx.client.withdraw_split(&fx.organizer, &fx.event_id);
    assert_eq!(balance(&fx, &fx.organizer), 100_000_000);
}

#[test]
fn test_resolve_requires_flagged_recipient() {
    let env = Env::default();
    env.mock_all_auths();
    let (fx, cohost) = completed_two_way(&env, 0, 100_000_000);

    let res = fx.client.try_resolve_flagged_share(
        &fx.event_id,
        &cohost,
        &FlagResolution::ReleaseToRecipient,
    );
    assert_eq!(res.err(), Some(Ok(PaymentError::RecipientNotFlagged)));
}

#[test]
fn test_legacy_withdraw_paths_rejected_for_split_events() {
    let env = Env::default();
    env.mock_all_auths();
    let (fx, _cohost) = completed_two_way(&env, 0, 100_000_000);

    assert_eq!(
        fx.client.try_withdraw(&fx.organizer, &fx.event_id).err(),
        Some(Ok(PaymentError::InvalidSplitConfig))
    );
    assert_eq!(
        fx.client
            .try_withdraw_token(&fx.organizer, &fx.event_id, &fx.token)
            .err(),
        Some(Ok(PaymentError::InvalidSplitConfig))
    );
    assert_eq!(
        fx.client
            .try_withdraw_all_tokens(&fx.organizer, &fx.event_id)
            .err(),
        Some(Ok(PaymentError::InvalidSplitConfig))
    );
    assert_eq!(
        fx.client
            .try_withdraw_revenue(&fx.event_id, &fx.organizer)
            .err(),
        Some(Ok(PaymentError::InvalidSplitConfig))
    );
}

#[test]
fn test_cancelled_split_event_distributes_only_withdrawable_ratio() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, token, client, contract_id, event_contract) = make_payments(&env, 0);
    let organizer = Address::generate(&env);
    let cohost = Address::generate(&env);
    let event_id = symbol_short!("CANCEL");
    bind(&client, &event_contract, &event_id, &organizer, &token);
    let splits: Vec<(Address, u32)> = vec![
        &env,
        (organizer.clone(), 6000u32),
        (cohost.clone(), 4000u32),
    ];
    client.sync_revenue_splits(&event_contract, &event_id, &splits);

    let payer = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token).mint(&payer, &200_000_000);
    client.pay_for_ticket(
        &1,
        &payer,
        &event_id,
        &200_000_000,
        &None,
        &token,
        &PaymentPrivacy::Standard,
    );

    // Cancel halfway through the event window (start=0, end=1000) => 50% ratio.
    env.ledger().with_mut(|li| li.sequence_number = 500);
    client.cancel_event(&event_id, &organizer);

    // Past the dispute window (cancel_ledger 500 + 100).
    env.ledger().with_mut(|li| li.sequence_number = 700);

    let tc = token::Client::new(&env, &token);
    client.withdraw_split(&cohost, &event_id);
    client.withdraw_split(&organizer, &event_id);

    // Only 50% (100M) is distributable: co-host 40M, primary 60M.
    assert_eq!(tc.balance(&cohost), 40_000_000);
    assert_eq!(tc.balance(&organizer), 60_000_000);
    // Remaining 100M stays escrowed for attendee refunds.
    assert_eq!(tc.balance(&contract_id), 100_000_000);
}
