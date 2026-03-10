// Mirror scaffold for cuda/fields.cu.
//
// Inputs:
// - base-field and secure-field vector operations required by the native hot path
//
// Outputs:
// - native Metal kernels for reusable field storage and arithmetic primitives
//
// Invariants:
// - file presence alone does not imply compile-active support
// - implementation must validate against the vendored CPU oracle before activation
//
// Failure modes:
// - until implemented, this file is a structural mirror only
