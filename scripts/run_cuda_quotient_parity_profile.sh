#!/usr/bin/env bash
set -euo pipefail

cat >&2 <<'EOF'
run_cuda_quotient_parity_profile.sh was retired in M22.

Reason:
- the old-stwo maintenance workspace was deleted
- this script existed only for that retired legacy validation lane

Current supported CUDA validation lives under:
- scripts/run_remote_cuda_validation.sh
- scripts/run_gcloud_cuda_validation.sh

See:
- docs/milestones/m22-close-note.md
EOF
exit 1
