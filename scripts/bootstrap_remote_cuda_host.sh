#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
usage: bootstrap_remote_cuda_host.sh <host>

Environment:
  SSH_USER    Remote SSH user. Default: root
  SSH_PORT    Remote SSH port. Default: 22
  SSH_IDENTITY_FILE  Optional SSH private key path
  SSH_KNOWN_HOSTS_FILE  Optional known_hosts file path

This script prepares a Hetzner dedicated GPU host for stwo-cuda validation.
It installs the general build dependencies and Rust toolchain, then verifies
that NVIDIA driver and CUDA toolkit are already present.
EOF
}

host="${1:-${HOST:-}}"
if [[ -z "${host}" ]]; then
  usage >&2
  exit 1
fi

ssh_user="${SSH_USER:-root}"
ssh_port="${SSH_PORT:-22}"
remote="${ssh_user}@${host}"
known_hosts_file="${SSH_KNOWN_HOSTS_FILE:-${HOME}/.ssh/known_hosts}"
ssh_opts=(
  -p "${ssh_port}"
  -o BatchMode=yes
  -o IdentitiesOnly=yes
  -o StrictHostKeyChecking=accept-new
  -o UserKnownHostsFile="${known_hosts_file}"
)
if [[ -n "${SSH_IDENTITY_FILE:-}" ]]; then
  ssh_opts+=(-i "${SSH_IDENTITY_FILE}")
fi

ssh "${ssh_opts[@]}" "${remote}" 'bash -s' <<'EOF'
set -euo pipefail

if ! command -v apt-get >/dev/null 2>&1; then
  echo "bootstrap_remote_cuda_host.sh currently supports Ubuntu or Debian hosts only." >&2
  exit 1
fi

if [[ "$(id -u)" -eq 0 ]]; then
  SUDO=""
elif command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then
  SUDO="sudo -n"
else
  echo "bootstrap_remote_cuda_host.sh requires root or passwordless sudo on the remote host." >&2
  exit 1
fi

export DEBIAN_FRONTEND=noninteractive
${SUDO} apt-get update
${SUDO} apt-get install -y \
  build-essential \
  ca-certificates \
  clang \
  cmake \
  curl \
  git \
  jq \
  pkg-config \
  python3 \
  rsync \
  ruby-full \
  libssl-dev

if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi

if [[ -f "$HOME/.cargo/env" ]]; then
  # rustup installs cargo into ~/.cargo/bin and exposes this helper.
  source "$HOME/.cargo/env"
fi
rustup toolchain install stable --profile minimal
rustup default stable

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

if ! command -v nvidia-smi >/dev/null 2>&1; then
  echo "nvidia-smi is missing. Install the NVIDIA driver on the remote host first." >&2
  exit 1
fi

nvcc_bin="$(find_nvcc || true)"
if [[ -z "${nvcc_bin}" ]]; then
  echo "nvcc is missing. Install the CUDA toolkit on the remote host first." >&2
  exit 1
fi
export PATH="$(dirname "${nvcc_bin}"):${PATH}"

cuda_bin_dir="$(dirname "$(readlink -f "${nvcc_bin}")")"
cuda_home="$(cd "${cuda_bin_dir}/.." && pwd)"

echo "== remote toolchain =="
rustc -V
cargo -V
ruby -v

echo "== remote gpu =="
nvidia-smi
nvcc --version
echo "CUDA_HOME=${cuda_home}"
EOF
