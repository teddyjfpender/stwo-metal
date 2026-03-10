# Metal Native Port Status

This directory is the native Metal mirror of the copied CUDA subsystem.

Rules:

- file presence does not imply compile-active support
- mirrored file names are used to preserve reviewable parity with `cuda/`
- each file must eventually graduate through:
  `scaffolded` -> `parity-tested` -> `benchmark-active`

Current compile-active Metal sources wired by `build.rs`:

- `fields.metal`
- `twiddles.metal`
- `poly_utils.metal`
- `rfft.metal`
- `ifft.metal`
- `quotients.metal`
- `fold_circle_into_line.metal`
- `fold_line.metal`
- `mle.metal`
- `gkr.metal`
- `prefix_sum.metal`
- `bit_reverse.metal`
- `poly_order.metal`
- `wide_fibonacci.metal`
- `runtime.m`

Current parity-tested native files:

- `fields.metal`
- `twiddles.metal`
- `poly_utils.metal`
- `rfft.metal`
- `ifft.metal`
- `quotients.metal`
- `fold_circle_into_line.metal`
- `fold_line.metal`
- `mle.metal`
- `gkr.metal`
- `prefix_sum.metal`

Current scaffold-only mirror files for the native hot path:

- none

Historical non-mirrored Metal sources retained outside the compile-active set:

- `fri.metal`
- `quotient.metal`

Declared port order:

1. `fields`
2. `twiddles`
3. `rfft`
4. `ifft`
5. `poly_utils`
6. `quotients`
7. `fold_circle_into_line`
8. `fold_line`
9. `mle`
10. `gkr`
11. `prefix_sum`
