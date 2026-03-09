# stwo-metal

`stwo-metal` is an isolated staging workspace for porting the `stwo-cuda`
companion backend toward Apple Silicon and Metal.

This repository started as a direct copy of `stwo-cuda`. The first cleanup pass
has been completed with these invariants:

- the nested git history was removed so this tree is no longer coupled to the
  copied repository
- the Cargo workspace and package surface now use `stwo-metal` and
  `stwo-metal-sys`
- the implementation is intentionally still CUDA-backed internally; this cleanup
  does not claim that a Metal backend exists yet

## Current truth

- Public crate surface:
  - [`crates/stwo-metal`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/crates/stwo-metal)
  - [`crates/stwo-metal-sys`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/crates/stwo-metal-sys)
- Upstream dependency snapshot:
  - [`vendor/stwo-upstream-dev-62b228e`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/vendor/stwo-upstream-dev-62b228e)
- Active process docs:
  - [`docs/README.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/README.md)
  - [`docs/controller.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/controller.md)
  - [`docs/program-plan.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/program-plan.md)

What still carries CUDA semantics on purpose:

- internal Rust module names such as `CudaBackend`
- the native build crate layout under
  [`crates/stwo-metal-sys/cuda`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/crates/stwo-metal-sys/cuda)
- the native link target and build-script cfg gate `stwo_cuda_link`
- most historical docs, scripts, and CI files copied from `stwo-cuda`

Those remaining CUDA names are now explicit migration debt, not part of the new
repository identity.

## How to use this repo right now

- Treat it as a planning and refactoring baseline for the Metal port.
- Prefer host-safe validation only unless a slice explicitly works on the
  inherited CUDA backend.
- Use [`docs/controller.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/controller.md)
  and [`docs/program-plan.md`](/Users/theodorepender/Coding/gpu-acc/stwo-metal/docs/program-plan.md)
  as the source of truth for execution and sequencing.

## Non-goals of this cleanup

- no backend algorithm changes
- no CUDA-to-Metal translation yet
- no claim that inherited benchmark scripts or CUDA validation lanes are valid
  for Apple Silicon
