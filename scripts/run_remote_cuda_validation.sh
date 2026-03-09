#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
usage: run_remote_cuda_validation.sh <host>

Environment:
  SSH_USER    Remote SSH user. Default: root
  SSH_PORT    Remote SSH port. Default: 22
  SSH_IDENTITY_FILE  Optional SSH private key path
  SSH_KNOWN_HOSTS_FILE  Optional known_hosts file path. Default: log-scoped file
  REMOTE_DIR  Remote checkout path. Default: /root/stwo-cuda
  BOOTSTRAP   Run remote bootstrap first. Default: 1
  POSEIDON_MIN_LOG  Default: 7
  POSEIDON_MAX_LOG  Default: 7
  WIDE_FIB_MIN_LOG  Default: 8
  WIDE_FIB_MAX_LOG  Default: 8

This script syncs the current repository contents to a remote CUDA-capable host,
then runs the native CUDA validation flow there under STWO_CUDA_MODE=cuda-ci.
EOF
}

host="${1:-${HOST:-}}"
if [[ -z "${host}" ]]; then
  usage >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ssh_user="${SSH_USER:-root}"
ssh_port="${SSH_PORT:-22}"
remote_dir="${REMOTE_DIR:-}"
if [[ -z "${remote_dir}" ]]; then
  if [[ "${ssh_user}" == "root" ]]; then
    remote_dir="/root/stwo-cuda"
  else
    remote_dir="/home/${ssh_user}/stwo-cuda"
  fi
fi
bootstrap="${BOOTSTRAP:-1}"
poseidon_min_log="${POSEIDON_MIN_LOG:-7}"
poseidon_max_log="${POSEIDON_MAX_LOG:-7}"
wide_fib_min_log="${WIDE_FIB_MIN_LOG:-8}"
wide_fib_max_log="${WIDE_FIB_MAX_LOG:-8}"
remote="${ssh_user}@${host}"
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
log_dir="${repo_root}/logs/remote-cuda/${timestamp}"
log_file="${log_dir}/validation.log"
known_hosts_file="${SSH_KNOWN_HOSTS_FILE:-${log_dir}/known_hosts}"
ssh_opts=(
  -p "${ssh_port}"
  -o BatchMode=yes
  -o StrictHostKeyChecking=accept-new
  -o UserKnownHostsFile="${known_hosts_file}"
)
if [[ -n "${SSH_IDENTITY_FILE:-}" ]]; then
  ssh_opts+=(-i "${SSH_IDENTITY_FILE}")
  rsync_ssh="ssh -i ${SSH_IDENTITY_FILE} -p ${ssh_port} -o BatchMode=yes -o StrictHostKeyChecking=accept-new"
else
  rsync_ssh="ssh -p ${ssh_port} -o BatchMode=yes -o StrictHostKeyChecking=accept-new"
fi

mkdir -p "${log_dir}"
touch "${known_hosts_file}"

if [[ "${bootstrap}" != "0" ]]; then
  SSH_KNOWN_HOSTS_FILE="${known_hosts_file}" \
    "${repo_root}/scripts/bootstrap_remote_cuda_host.sh" "${host}"
fi

rsync -az --delete \
  --exclude '.git' \
  --exclude 'target' \
  --exclude 'logs/remote-cuda' \
  --exclude '.DS_Store' \
  -e "${rsync_ssh} -o UserKnownHostsFile=${known_hosts_file}" \
  "${repo_root}/" "${remote}:${remote_dir}/"

ssh "${ssh_opts[@]}" "${remote}" "bash -s -- $(printf '%q' "${remote_dir}") $(printf '%q' "${poseidon_min_log}") $(printf '%q' "${poseidon_max_log}") $(printf '%q' "${wide_fib_min_log}") $(printf '%q' "${wide_fib_max_log}")" <<'EOF' | tee "${log_file}"
set -euo pipefail

remote_dir="$1"
poseidon_min_log="$2"
poseidon_max_log="$3"
wide_fib_min_log="$4"
wide_fib_max_log="$5"
cd "${remote_dir}"
if [[ -f "$HOME/.cargo/env" ]]; then
  source "$HOME/.cargo/env"
fi

find_nvcc() {
  local candidate
  if command -v nvcc >/dev/null 2>&1; then
    command -v nvcc
    return 0
  fi

  for candidate in /usr/local/cuda/bin/nvcc /usr/local/cuda-*/bin/nvcc; do
    if [[ -x "${candidate}" ]]; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done

  return 1
}

nvcc_bin="$(find_nvcc || true)"
if [[ -z "${nvcc_bin}" ]]; then
  echo "nvcc is missing on the remote host." >&2
  exit 1
fi

export PATH="$(dirname "${nvcc_bin}"):${PATH}"
cuda_bin_dir="$(dirname "$(readlink -f "${nvcc_bin}")")"
cuda_home="$(cd "${cuda_bin_dir}/.." && pwd)"

export STWO_CUDA_MODE="cuda-ci"
export CUDA_HOME="${cuda_home}"
export CUDAToolkit_ROOT="${cuda_home}"
export NUM_JOBS="${NUM_JOBS:-$(nproc)}"

echo "== system =="
uname -a
echo

echo "== gpu =="
nvidia-smi
nvcc --version
echo "CUDA_HOME=${CUDA_HOME}"
echo

echo "== toolchain =="
rustc -V
cargo -V
ruby -v
echo

echo "== artifact drift checks =="
ruby scripts/validate_registry_artifacts.rb docs/artifacts/component-registry-v1.exemplar.json docs/artifacts/cuda-compatibility-manifest-v1.exemplar.json
ruby scripts/validate_abi_artifacts.rb docs/artifacts/component-registry-v1.exemplar.json docs/artifacts/component-abi-v1.exemplar.json
ruby scripts/validate_dispatch_artifacts.rb docs/artifacts/component-registry-v1.exemplar.json crates/stwo-cuda-sys/cuda/component_dispatch_v1_generated.cuh
ruby scripts/validate_build_inventory_artifacts.rb docs/artifacts/component-registry-v1.exemplar.json crates/stwo-cuda-sys/cuda/component_inventory_v1_generated.cmake
ruby scripts/validate_planner_manifest_module.rb docs/artifacts/component-registry-v1.exemplar.json docs/artifacts/cuda-compatibility-manifest-v1.exemplar.json crates/stwo-cuda/src/backend/cuda/planner_manifest_v1_generated.rs
echo

echo "== standalone pinned pairing =="
STWO_STANDALONE_VALIDATE_MODE=test sh scripts/run_standalone_pinned_validation.sh
echo

echo "== companion package checks =="
cargo check --workspace --tests --features="tracing, prover" --locked
cargo test -p stwo-cuda --test cuda_planner_manifest --features="prover" --locked
echo
EOF

echo
echo "remote validation log saved to ${log_file}"
