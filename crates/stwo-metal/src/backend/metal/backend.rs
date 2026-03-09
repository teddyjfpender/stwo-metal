use serde::{Deserialize, Serialize};
use stwo::prover::backend::Backend;

/// Marker type for the staged native-Metal port.
///
/// The first execution slice intentionally does not implement Stwo's full `Backend`
/// contract yet. We only claim the bounded `ColumnOps<BaseField>` surface that is
/// migrated and validated in this crate.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct MetalBackend;

impl Backend for MetalBackend {}
