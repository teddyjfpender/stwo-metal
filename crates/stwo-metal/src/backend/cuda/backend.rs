use serde::{Deserialize, Serialize};
use stwo::core::channel::{Blake2sChannel, Poseidon252Channel};
use stwo::core::proof_of_work::GrindOps;
#[cfg(not(feature = "vendored-upstream-bridge"))]
use stwo::core::vcs::blake2_merkle::Blake2sMerkleChannel;
#[cfg(not(feature = "vendored-upstream-bridge"))]
use stwo::core::vcs::poseidon252_merkle::Poseidon252MerkleChannel;
#[cfg(feature = "vendored-upstream-bridge")]
use stwo::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
#[cfg(feature = "vendored-upstream-bridge")]
use stwo::core::vcs_lifted::poseidon252_merkle::Poseidon252MerkleChannel;
use stwo::prover::backend::simd::SimdBackend;
use stwo::prover::backend::{Backend, BackendForChannel};

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct CudaBackend;

impl Backend for CudaBackend {}

impl GrindOps<Blake2sChannel> for CudaBackend {
    fn grind(channel: &Blake2sChannel, pow_bits: u32) -> u64 {
        SimdBackend::grind(channel, pow_bits)
    }
}

impl BackendForChannel<Blake2sMerkleChannel> for CudaBackend {}

impl GrindOps<Poseidon252Channel> for CudaBackend {
    fn grind(channel: &Poseidon252Channel, pow_bits: u32) -> u64 {
        SimdBackend::grind(channel, pow_bits)
    }
}

impl BackendForChannel<Poseidon252MerkleChannel> for CudaBackend {}
