// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use crate::ProofOptions;
use core::cmp;
use fri::FriProof;
use math::utils::log2;
use serde::{Deserialize, Serialize};

mod commitments;
pub use commitments::Commitments;

mod queries;
pub use queries::Queries;

mod ood_frame;
pub use ood_frame::OodFrame;

// CONSTANTS
// ================================================================================================

const GRINDING_CONTRIBUTION_FLOOR: u32 = 80;

// TYPES AND INTERFACES
// ================================================================================================

#[derive(Clone, Serialize, Deserialize)]
pub struct StarkProof {
    pub context: Context,
    pub commitments: Commitments,
    pub trace_queries: Queries,
    pub constraint_queries: Queries,
    pub ood_frame: OodFrame,
    pub fri_proof: FriProof,
    pub pow_nonce: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Context {
    pub lde_domain_depth: u8,
    pub field_modulus_bytes: Vec<u8>,
    pub options: ProofOptions,
}

// STARK PROOF IMPLEMENTATION
// ================================================================================================
impl StarkProof {
    /// Returns proof options which were used to generate this proof.
    pub fn options(&self) -> &ProofOptions {
        &self.context.options
    }

    /// Returns trace length for the computation described by this proof.
    pub fn trace_length(&self) -> usize {
        self.lde_domain_size() / self.context.options.blowup_factor()
    }

    /// Returns the size of the LDE domain for the computation described by this proof.
    pub fn lde_domain_size(&self) -> usize {
        2usize.pow(self.context.lde_domain_depth as u32)
    }

    // SECURITY LEVEL
    // --------------------------------------------------------------------------------------------

    /// Returns security level of this proof (in bits). When `conjectured` is true, conjectured
    /// security level is returned; otherwise, provable security level is returned. Usually, the
    /// number of queries needed for provable security is 2x - 3x higher than the number of queries
    /// needed for conjectured security at the same security level.
    pub fn security_level(&self, conjectured: bool) -> u32 {
        let options = &self.context.options;

        let base_field_size_bits = get_num_modulus_bits(&self.context.field_modulus_bytes);

        if conjectured {
            get_conjectured_security(
                options,
                base_field_size_bits,
                self.lde_domain_size() as u64,
                self.trace_length() as u64,
            )
        } else {
            // TODO: implement proven security estimation
            unimplemented!("proven security estimation has not been implement yet")
        }
    }
}

// HELPER FUNCTIONS
// ================================================================================================

/// Returns number of bits in the provided modulus; the modulus is assumed to be encoded in
/// little-endian byte order
fn get_num_modulus_bits(modulus_bytes: &[u8]) -> u32 {
    let mut num_bits = modulus_bytes.len() as u32 * 8;
    for &byte in modulus_bytes.iter().rev() {
        if byte != 0 {
            num_bits -= byte.leading_zeros();
            return num_bits;
        }
        num_bits -= 8;
    }

    0
}

/// Computes conjectured security level for the specified proof parameters.
fn get_conjectured_security(
    options: &ProofOptions,
    base_field_size: u32, // in bits
    lde_domain_size: u64,
    trace_length: u64,
) -> u32 {
    // compute max security we can get for a given field size
    let field_size = base_field_size * options.field_extension().degree();
    let field_security = field_size - lde_domain_size.trailing_zeros();

    // compute max security we can get for a given hash function
    let hash_fn_security = options.hash_fn().collision_resistance();

    // compute security we get by executing multiple query rounds
    let one_over_rho = lde_domain_size / trace_length;
    let security_per_query = log2(one_over_rho as usize);
    let mut query_security = security_per_query * options.num_queries() as u32;

    // include grinding factor contributions only for proofs adequate security
    if query_security >= GRINDING_CONTRIBUTION_FLOOR {
        query_security += options.grinding_factor();
    }

    cmp::min(cmp::min(field_security, hash_fn_security), query_security) - 1
}
