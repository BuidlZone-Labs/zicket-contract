//! Shplemini batch-opening verifier (Gemini + Shplonk + KZG) for BN254.
//!
//! Verifies the polynomial commitment scheme opening by constructing a single
//! MSM that accumulates unshifted/shifted claims, Gemini fold evaluations, and
//! the constant term, then performs a BN254 pairing check.
//!
//! BB reference (v0.87.0):
//!   - `commitment_schemes/shplonk/shplemini.hpp::ShpleminiVerifier_::compute_batch_opening_claim`
//!   - `commitment_schemes/kzg/kzg.hpp::KZG::reduce_verify_batch_opening_claim`

use crate::ec::{g1_msm, pairing_check};
use crate::field::{batch_inverse, Fr};
use crate::trace;
use crate::types::{
    G1Point, Proof, Transcript, VerificationKey, BATCHED_RELATION_PARTIAL_LENGTH,
    CONST_PROOF_SIZE_LOG_N, NUMBER_OF_ENTITIES, NUMBER_UNSHIFTED,
};
use core::array::repeat;
use core::ops::Neg;
use soroban_sdk::Env;

const SUBGROUP_SIZE: usize = 256;
const SUBGROUP_GENERATOR_BYTES: [u8; 32] = [
    0x07, 0xb0, 0xc5, 0x61, 0xa6, 0x14, 0x84, 0x04, 0xf0, 0x86, 0x20, 0x4a, 0x9f, 0x36, 0xff, 0xb0,
    0x61, 0x79, 0x42, 0x54, 0x67, 0x50, 0xf2, 0x30, 0xc8, 0x93, 0x61, 0x91, 0x74, 0xa5, 0x7a, 0x76,
];
const SUBGROUP_GENERATOR_INVERSE_BYTES: [u8; 32] = [
    0x20, 0x4b, 0xd3, 0x27, 0x74, 0x22, 0xfa, 0xd3, 0x64, 0x75, 0x1a, 0xd9, 0x38, 0xe2, 0xb5, 0xe6,
    0xa5, 0x4c, 0xf8, 0xc6, 0x87, 0x12, 0x84, 0x8a, 0x69, 0x2c, 0x55, 0x3d, 0x03, 0x29, 0xf5, 0xd6,
];

fn check_libra_evaluations_consistency(
    env: &Env,
    proof: &Proof,
    tp: &Transcript,
) -> Result<(), &'static str> {
    let zero = Fr::zero(env);
    let one = Fr::one(env);
    let subgroup_generator_inverse = Fr::from_array(env, &SUBGROUP_GENERATOR_INVERSE_BYTES);
    let vanishing_evaluation = tp.gemini_r.pow(SUBGROUP_SIZE as u64) - &one;
    if vanishing_evaluation.is_zero() {
        return Err("gemini challenge is in the Libra subgroup");
    }

    let mut challenge_lagrange = Fr::zero_array::<SUBGROUP_SIZE>(env);
    challenge_lagrange[0] = one.clone();
    for (round, challenge) in tp.sumcheck_u_challenges.iter().enumerate() {
        let current = 1 + BATCHED_RELATION_PARTIAL_LENGTH * round;
        challenge_lagrange[current] = one.clone();
        for index in (current + 1)..(current + BATCHED_RELATION_PARTIAL_LENGTH) {
            challenge_lagrange[index] = &challenge_lagrange[index - 1] * challenge;
        }
    }

    let mut denominators = Fr::zero_array::<SUBGROUP_SIZE>(env);
    let mut denominator_inverses = Fr::zero_array::<SUBGROUP_SIZE>(env);
    let mut root_power = one.clone();
    for denominator in denominators.iter_mut() {
        *denominator = &root_power * &tp.gemini_r - &one;
        root_power = root_power * &subgroup_generator_inverse;
    }
    batch_inverse(&denominators, &mut denominator_inverses)
        .map_err(|_| "Libra denominator is zero")?;

    let mut challenge_evaluation = zero;
    for (lagrange, denominator) in challenge_lagrange.iter().zip(denominator_inverses.iter()) {
        challenge_evaluation = challenge_evaluation + lagrange * denominator;
    }

    let numerator =
        vanishing_evaluation.clone() * Fr::from_u64(env, SUBGROUP_SIZE as u64).inverse();
    challenge_evaluation = challenge_evaluation * &numerator;
    let lagrange_first = &denominator_inverses[0] * &numerator;
    let lagrange_last = &denominator_inverses[SUBGROUP_SIZE - 1] * &numerator;
    let evaluations = &proof.libra_polynomial_evaluations;

    let mut difference = &lagrange_first * &evaluations[2];
    difference = difference
        + (&tp.gemini_r - &subgroup_generator_inverse)
            * (&evaluations[1] - &evaluations[2] - &evaluations[0] * challenge_evaluation);
    difference = difference + lagrange_last * (&evaluations[2] - &proof.libra_claimed_evaluation)
        - vanishing_evaluation * &evaluations[3];

    if difference.is_zero() {
        Ok(())
    } else {
        Err("Libra evaluations are inconsistent")
    }
}

/// Verify the Shplemini batch-opening claim.
///
/// High-level flow (matching BB):
/// 1. Compute powers of Gemini evaluation challenge `r^{2^i}`.
/// 2. Batch-invert all Shplonk/Gemini denominators (`z ± r^{2^j}`, fold-round
///    denominators, and `r` itself).
/// 3. Compute Shplonk scalar weights for unshifted and shifted polynomial batches.
/// 4. Accumulate batched multilinear evaluation `∑ ρⁱ·evalᵢ`.
/// 5. Load VK + proof commitments into MSM arrays (shifted scalars merged into
///    unshifted counterparts to match BB's `remove_repeated_commitments`).
/// 6. Reconstruct positive Gemini fold evaluations `Aⱼ(r^{2^j})`.
/// 7. Accumulate constant term and fold-round MSM scalars.
/// 8. Add generator (with constant-term scalar) and KZG quotient (with scalar `z`).
/// 9. Single MSM + pairing check.
///
/// BB: `commitment_schemes/shplonk/shplemini.hpp::ShpleminiVerifier_::compute_batch_opening_claim`
pub fn verify_shplemini(
    env: &Env,
    proof: &Proof,
    vk: &VerificationKey,
    tp: &Transcript,
) -> Result<(), &'static str> {
    let log_n = vk.log_circuit_size as usize;
    if log_n == 0 || log_n > CONST_PROOF_SIZE_LOG_N {
        return Err("shplemini: log_circuit_size out of range");
    }

    // 1) r^{2^i}
    let one = Fr::one(env);
    let two = Fr::from_u64(env, 2);
    let mut r_pows = Fr::zero_array::<CONST_PROOF_SIZE_LOG_N>(env);
    r_pows[0] = tp.gemini_r.clone();
    for i in 1..log_n {
        r_pows[i] = &r_pows[i - 1] * &r_pows[i - 1];
    }

    // We need the following inversions:
    //   - (z - r^0), (z + r^0)          for shplonk weights (pos0, neg0)
    //   - gemini_r                       for shifted weight
    //   - (r^j*(1-u_j) + u_j)           for j in 1..=log_n  (fold round denoms)
    //   - (z - r^j), (z + r^j)          for j in 1..log_n   (further folding)
    //
    // Total: 2 + 1 + log_n + 2*(log_n - 1) = 3*log_n + 1 values.

    // Collect all values to invert into a flat array.
    // Layout:
    //   [0]           = z - r^0
    //   [1]           = z + r^0
    //   [2]           = gemini_r
    //   [3 .. 3+log_n)  = fold round denominators (j = log_n down to 1)
    //   [3+log_n .. 3+log_n + 2*(log_n-1))  = pairs (z - r^j, z + r^j) for j=1..log_n
    // Max batch size: 3*CONST_PROOF_SIZE_LOG_N + 1 (upper bound when log_n == CONST_PROOF_SIZE_LOG_N)
    const MAX_BATCH: usize = 3 * CONST_PROOF_SIZE_LOG_N + 1;
    let batch_size = 3 + log_n + 2 * (log_n - 1);
    let mut to_invert = Fr::zero_array::<MAX_BATCH>(env);
    let mut inverted = Fr::zero_array::<MAX_BATCH>(env);

    to_invert[0] = &tp.shplonk_z - &r_pows[0];
    to_invert[1] = &tp.shplonk_z + &r_pows[0];
    to_invert[2] = tp.gemini_r.clone();

    // fold round denominators: r^j * (1 - u_j) + u_j, for j = log_n down to 1
    for j in (1..=log_n).rev() {
        let u = &tp.sumcheck_u_challenges[j - 1];
        to_invert[3 + (log_n - j)] = &r_pows[j - 1] * &(&one - u) + u;
    }

    // further folding denominators: (z - r^j) and (z + r^j) for j = 1..log_n
    let further_base = 3 + log_n;
    for j in 1..log_n {
        to_invert[further_base + 2 * (j - 1)] = &tp.shplonk_z - &r_pows[j];
        to_invert[further_base + 2 * (j - 1) + 1] = &tp.shplonk_z + &r_pows[j];
    }

    batch_inverse(&to_invert[..batch_size], &mut inverted[..batch_size]).map_err(|_| {
        "shplemini: batch inversion failed (zero denominator in shplonk/gemini/fold)"
    })?;

    // Defense-in-depth: ensure no inverted result is zero before use.
    if inverted[..batch_size.min(inverted.len())]
        .iter()
        .any(|x| x.is_zero())
    {
        return Err("shplemini: batch inversion produced zero result");
    }

    // Unpack results
    let pos0 = inverted[0].clone();
    let neg0 = inverted[1].clone();
    let gemini_r_inv = inverted[2].clone();

    // 2) allocate arrays
    // Deduplicated layout: shifted commitments merged into unshifted counterparts.
    // Layout:
    const TOTAL: usize = NUMBER_OF_ENTITIES + CONST_PROOF_SIZE_LOG_N + 3 + 3;
    trace!("total = {}", TOTAL);
    let mut scalars = Fr::zero_array::<TOTAL>(env);
    let mut coms = repeat::<G1Point, TOTAL>(G1Point::infinity(env));

    // 3) compute shplonk weights
    let unshifted = &tp.shplonk_nu * &neg0 + &pos0;
    let shifted = gemini_r_inv * (&pos0 - &(&tp.shplonk_nu * &neg0));
    let neg_unshifted = -&unshifted;
    let neg_shifted = -&shifted;
    // 4) shplonk_Q
    scalars[0] = one.clone();
    coms[0] = proof.shplonk_q.clone();
    scalars[1] = neg_unshifted.clone();
    coms[1] = proof.hiding_polynomial_commitment.clone();

    // 5) weight sumcheck evals
    let mut rho_pow = tp.rho.clone();
    let mut eval_acc = proof.hiding_polynomial_evaluation.clone();
    let mut eval_scalars = Fr::zero_array::<NUMBER_OF_ENTITIES>(env);
    for (idx, eval) in proof
        .sumcheck_evaluations
        .iter()
        .take(NUMBER_OF_ENTITIES)
        .enumerate()
    {
        let scalar = if idx < NUMBER_UNSHIFTED {
            neg_unshifted.clone()
        } else {
            neg_shifted.clone()
        } * &rho_pow;
        eval_scalars[idx] = scalar;
        eval_acc = eval_acc + &(eval * &rho_pow);
        rho_pow = rho_pow * &tp.rho;
    }

    // 6) load VK & proof
    {
        let mut j = 2;
        macro_rules! push_vk {
            ($($field:ident),+ $(,)?) => {
                $(
                    coms[j] = vk.$field.clone();
                    scalars[j] = eval_scalars[j - 2].clone();
                    j += 1;
                )+
            };
        }
        push_vk![
            qm,
            qc,
            ql,
            qr,
            qo,
            q4,
            q_lookup,
            q_arith,
            q_delta_range,
            q_elliptic,
            q_aux,
            q_poseidon2_external,
            q_poseidon2_internal,
            s1,
            s2,
            s3,
            s4,
            id1,
            id2,
            id3,
            id4,
            t1,
            t2,
            t3,
            t4,
            lagrange_first,
            lagrange_last
        ];

        coms[j] = proof.w1.clone();
        scalars[j] = eval_scalars[27].clone();
        j += 1;
        coms[j] = proof.w2.clone();
        scalars[j] = eval_scalars[28].clone();
        j += 1;
        coms[j] = proof.w3.clone();
        scalars[j] = eval_scalars[29].clone();
        j += 1;
        coms[j] = proof.w4.clone();
        scalars[j] = eval_scalars[30].clone();
        j += 1;
        coms[j] = proof.z_perm.clone();
        scalars[j] = eval_scalars[31].clone();
        j += 1;
        coms[j] = proof.lookup_inverses.clone();
        scalars[j] = eval_scalars[32].clone();
        j += 1;
        coms[j] = proof.lookup_read_counts.clone();
        scalars[j] = eval_scalars[33].clone();
        j += 1;
        coms[j] = proof.lookup_read_tags.clone();
        scalars[j] = eval_scalars[34].clone();
        j += 1;

        for (commitment, scalar_index) in [
            (&proof.w1, 35),
            (&proof.w2, 36),
            (&proof.w3, 37),
            (&proof.w4, 38),
            (&proof.z_perm, 39),
        ] {
            coms[j] = commitment.clone();
            scalars[j] = eval_scalars[scalar_index].clone();
            j += 1;
        }
        let _ = j;
    }

    // 7) folding rounds — use batch-inverted denominators
    let mut fold_pos = Fr::zero_array::<CONST_PROOF_SIZE_LOG_N>(env);
    let mut cur = eval_acc;
    for j in (1..=log_n).rev() {
        let r2 = &r_pows[j - 1];
        let u = &tp.sumcheck_u_challenges[j - 1];
        let fold_lin = r2 * &(&one - u) - u;
        let num = r2 * &cur * &two - &(&proof.gemini_a_evaluations[j - 1] * &fold_lin);
        let den_inv = inverted[3 + (log_n - j)].clone();
        cur = num * &den_inv;
        fold_pos[j - 1] = cur.clone();
    }
    // 8) accumulate constant term
    let nu_sq = &tp.shplonk_nu * &tp.shplonk_nu;
    let mut const_acc =
        &fold_pos[0] * &pos0 + &(&proof.gemini_a_evaluations[0] * &tp.shplonk_nu * &neg0);
    let mut v_pow = nu_sq.clone();
    // 9) further folding + commit — use batch-inverted denominators
    // Base index where fold commitments start
    let base = NUMBER_OF_ENTITIES + 2;
    for j in 1..log_n {
        let pos_inv = inverted[further_base + 2 * (j - 1)].clone();
        let neg_inv = inverted[further_base + 2 * (j - 1) + 1].clone();
        let sp = &v_pow * &pos_inv;
        let sn = &v_pow * &tp.shplonk_nu * &neg_inv;

        scalars[base + j - 1] = -(&sp + &sn);
        const_acc = const_acc + &(&proof.gemini_a_evaluations[j] * &sn) + &(&fold_pos[j] * &sp);

        v_pow = v_pow * &nu_sq;

        coms[base + j - 1] = proof.gemini_fold_comms[j - 1].clone();
    }

    // Fill remaining (dummy) fold commitments so MSM layout matches Solidity (total 27 entries)
    coms[((log_n - 1) + base)..((CONST_PROOF_SIZE_LOG_N - 1) + base)]
        .clone_from_slice(&proof.gemini_fold_comms[(log_n - 1)..(CONST_PROOF_SIZE_LOG_N - 1)]);

    let libra_base = base + (CONST_PROOF_SIZE_LOG_N - 1);
    let subgroup_generator = Fr::from_array(env, &SUBGROUP_GENERATOR_BYTES);
    let libra_denominators = [
        pos0.clone(),
        (&tp.shplonk_z - &subgroup_generator * &tp.gemini_r).inverse(),
        pos0.clone(),
        pos0.clone(),
    ];
    let mut libra_power = tp.shplonk_nu.pow((2 * CONST_PROOF_SIZE_LOG_N + 2) as u64);
    let mut libra_scalars = Fr::zero_array::<4>(env);
    for index in 0..4 {
        let scaling = &libra_denominators[index] * &libra_power;
        libra_scalars[index] = -&scaling;
        const_acc = const_acc + scaling * &proof.libra_polynomial_evaluations[index];
        libra_power = libra_power * &tp.shplonk_nu;
    }
    coms[libra_base] = proof.libra_concatenation_commitment.clone();
    scalars[libra_base] = libra_scalars[0].clone();
    coms[libra_base + 1] = proof.libra_grand_sum_commitment.clone();
    scalars[libra_base + 1] = &libra_scalars[1] + &libra_scalars[2];
    coms[libra_base + 2] = proof.libra_quotient_commitment.clone();
    scalars[libra_base + 2] = libra_scalars[3].clone();

    check_libra_evaluations_consistency(env, proof, tp)?;

    // 10) add generator
    let one_idx = libra_base + 3;
    trace!("one_idx = {}", one_idx);
    coms[one_idx] = G1Point::generator(env);
    scalars[one_idx] = const_acc;

    // 11) add quotient
    let q_idx = one_idx + 1;
    trace!("q_idx = {}", q_idx);
    coms[q_idx] = proof.kzg_quotient.clone();
    scalars[q_idx] = tp.shplonk_z.clone();

    // 12) MSM + pairing
    let p0 = g1_msm(env, &coms, &scalars)?;
    let p1 = proof.kzg_quotient.0.clone().neg();
    if pairing_check(env, &p0, &p1) {
        Ok(())
    } else {
        Err("Shplonk pairing check failed")
    }
}
