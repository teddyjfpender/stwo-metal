use stwo::core::channel::Blake2sChannel;
use stwo::core::fri::{ExtendedFriProof, FriConfig};
use stwo::core::poly::circle::CircleDomain;
use stwo::core::queries::Queries;
use stwo::core::vcs_lifted::blake2_merkle::{Blake2sMerkleChannel, Blake2sMerkleHasher};
use stwo::prover::fri::FriDecommitResult;

use super::prover::MetalFriProver;
use crate::stwo_metal::secure_field_vec::SecureFieldVec;

#[derive(Clone, Debug)]
pub struct MetalFriBlake2sSubpath {
    config: FriConfig,
}

impl MetalFriBlake2sSubpath {
    pub fn new(config: FriConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> FriConfig {
        self.config
    }

    pub fn prove(
        &self,
        column: &SecureFieldVec,
        domain: CircleDomain,
    ) -> FriDecommitResult<Blake2sMerkleHasher> {
        let mut channel = Blake2sChannel::default();
        MetalFriProver::<Blake2sMerkleChannel>::commit(&mut channel, self.config, column, domain)
            .decommit(&mut channel)
    }

    pub fn prove_on_queries(
        &self,
        column: &SecureFieldVec,
        domain: CircleDomain,
        queries: &Queries,
    ) -> ExtendedFriProof<Blake2sMerkleHasher> {
        let mut channel = Blake2sChannel::default();
        MetalFriProver::<Blake2sMerkleChannel>::commit(&mut channel, self.config, column, domain)
            .decommit_on_queries(queries)
    }
}
