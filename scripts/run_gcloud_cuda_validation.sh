#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
usage: run_gcloud_cuda_validation.sh

Environment:
  GCP_PROJECT           Default: current gcloud project
  GCP_ZONE              Default: us-central1-a
  GCP_MACHINE_TYPE      Default: n1-standard-4
  GCP_GPU_TYPE          Default: nvidia-tesla-t4
  GCP_GPU_COUNT         Default: 1
  GCP_IMAGE_FAMILY      Default: common-cu128-ubuntu-2204-nvidia-570
  GCP_IMAGE_PROJECT     Default: deeplearning-platform-release
  GCP_BOOT_DISK_SIZE    Default: 100GB
  GCP_BOOT_DISK_TYPE    Default: pd-ssd
  GCP_INSTANCE_PREFIX   Default: stwo-cuda-gpu
  GCP_SSH_USER          Default: current local user
  GCP_SSH_PUBLIC_KEY    Default: first available of ~/.ssh/google_compute_engine.pub, ~/.ssh/id_ed25519.pub, ~/.ssh/id_rsa.pub
  GCP_SSH_PRIVATE_KEY   Default: matching private key for the chosen public key
  BOOTSTRAP             Passed through to run_remote_cuda_validation.sh. Default: 1
  REMOTE_VALIDATION_SCRIPT  Default: scripts/run_remote_cuda_validation.sh

This script creates a temporary Spot GPU VM on GCP, runs the repo's remote
CUDA validation flow on it, and then deletes the VM.
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

project="${GCP_PROJECT:-$(gcloud config get-value core/project 2>/dev/null)}"
zone="${GCP_ZONE:-us-central1-a}"
region="${zone%-*}"
machine_type="${GCP_MACHINE_TYPE:-n1-standard-4}"
gpu_type="${GCP_GPU_TYPE:-nvidia-tesla-t4}"
gpu_count="${GCP_GPU_COUNT:-1}"
image_family="${GCP_IMAGE_FAMILY:-common-cu128-ubuntu-2204-nvidia-570}"
image_project="${GCP_IMAGE_PROJECT:-deeplearning-platform-release}"
boot_disk_size="${GCP_BOOT_DISK_SIZE:-100GB}"
boot_disk_type="${GCP_BOOT_DISK_TYPE:-pd-ssd}"
instance_prefix="${GCP_INSTANCE_PREFIX:-stwo-cuda-gpu}"
instance_name="${instance_prefix}-$(date -u +%m%d-%H%M%S)"
ssh_user="${GCP_SSH_USER:-$(id -un)}"
bootstrap="${BOOTSTRAP:-1}"
remote_validation_script="${REMOTE_VALIDATION_SCRIPT:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/run_remote_cuda_validation.sh}"
ssh_known_hosts_file="${GCP_SSH_KNOWN_HOSTS_FILE:-$(mktemp /tmp/stwo-cuda-known-hosts.XXXXXX)}"

if [[ -z "${project}" ]]; then
  echo "No active GCP project is configured. Set GCP_PROJECT or run gcloud config set project <id>." >&2
  exit 1
fi

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require_cmd gcloud
require_cmd jq

find_public_key() {
  local candidate
  for candidate in \
    "${HOME}/.ssh/google_compute_engine.pub" \
    "${HOME}/.ssh/id_ed25519.pub" \
    "${HOME}/.ssh/id_rsa.pub"
  do
    if [[ -f "${candidate}" ]]; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done
  return 1
}

ssh_public_key_path="${GCP_SSH_PUBLIC_KEY:-$(find_public_key || true)}"
if [[ -z "${ssh_public_key_path}" || ! -f "${ssh_public_key_path}" ]]; then
  echo "No public SSH key found. Set GCP_SSH_PUBLIC_KEY to a readable .pub file." >&2
  exit 1
fi

ssh_public_key="$(tr -d '\n' < "${ssh_public_key_path}")"
ssh_private_key_path="${GCP_SSH_PRIVATE_KEY:-${ssh_public_key_path%.pub}}"

ssh_wait_opts=(
  -o BatchMode=yes
  -o StrictHostKeyChecking=accept-new
  -o UserKnownHostsFile="${ssh_known_hosts_file}"
  -o ConnectTimeout=5
)
if [[ -f "${ssh_private_key_path}" ]]; then
  ssh_wait_opts+=(-i "${ssh_private_key_path}")
fi

quota_metric_for_gpu_type() {
  local accelerator="$1"
  local family

  case "${accelerator}" in
    nvidia-tesla-*)
      family="${accelerator#nvidia-tesla-}"
      ;;
    nvidia-*)
      family="${accelerator#nvidia-}"
      ;;
    *)
      echo "Unsupported accelerator name for quota lookup: ${accelerator}" >&2
      return 1
      ;;
  esac

  family="$(printf '%s' "${family}" | tr '[:lower:]-' '[:upper:]_')"
  printf 'PREEMPTIBLE_NVIDIA_%s_GPUS\n' "${family}"
}

cleanup() {
  if gcloud compute instances describe "${instance_name}" --project="${project}" --zone="${zone}" >/dev/null 2>&1; then
    echo "Deleting ${instance_name}..."
    gcloud compute instances delete "${instance_name}" \
      --project="${project}" \
      --zone="${zone}" \
      --quiet || true
  fi
}

trap cleanup EXIT INT TERM

global_gpu_quota="$(
  gcloud compute project-info describe --project="${project}" --format=json |
    jq -r '.quotas[] | select(.metric=="GPUS_ALL_REGIONS") | .limit'
)"
if [[ -z "${global_gpu_quota}" || "${global_gpu_quota}" == "null" ]]; then
  echo "Could not read GPUS_ALL_REGIONS quota for project ${project}." >&2
  exit 1
fi

if ! awk "BEGIN {exit !(${global_gpu_quota} > 0)}"; then
  echo "Project ${project} has GPUS_ALL_REGIONS=${global_gpu_quota}. Regional accelerator quota alone is not sufficient; GPU VM creation will fail until the global quota is raised." >&2
  exit 1
fi

quota_metric="$(quota_metric_for_gpu_type "${gpu_type}")"
regional_gpu_quota="$(
  gcloud compute regions describe "${region}" --project="${project}" --format=json |
    jq -r --arg metric "${quota_metric}" '.quotas[] | select(.metric==$metric) | .limit'
)"
if [[ -z "${regional_gpu_quota}" || "${regional_gpu_quota}" == "null" ]]; then
  echo "Could not find regional quota metric ${quota_metric} in ${region}." >&2
  exit 1
fi

if ! awk "BEGIN {exit !(${regional_gpu_quota} >= ${gpu_count})}"; then
  echo "Region ${region} has ${quota_metric}=${regional_gpu_quota}, which is insufficient for ${gpu_count}x ${gpu_type}." >&2
  exit 1
fi

echo "Using ${project} in ${zone} with ${gpu_count}x ${gpu_type} (regional quota ${quota_metric}=${regional_gpu_quota})."

echo "Creating ${instance_name} in ${zone}..."
gcloud compute instances create "${instance_name}" \
  --project="${project}" \
  --zone="${zone}" \
  --machine-type="${machine_type}" \
  --accelerator="type=${gpu_type},count=${gpu_count}" \
  --provisioning-model=SPOT \
  --maintenance-policy=TERMINATE \
  --instance-termination-action=DELETE \
  --image-family="${image_family}" \
  --image-project="${image_project}" \
  --boot-disk-size="${boot_disk_size}" \
  --boot-disk-type="${boot_disk_type}" \
  --metadata=enable-oslogin=FALSE,ssh-keys="${ssh_user}:${ssh_public_key}"

echo "Waiting for RUNNING state and external IP..."
while true; do
  vm_status="$(gcloud compute instances describe "${instance_name}" --project="${project}" --zone="${zone}" --format='value(status)')"
  vm_ip="$(gcloud compute instances describe "${instance_name}" --project="${project}" --zone="${zone}" --format='value(networkInterfaces[0].accessConfigs[0].natIP)')"
  echo "${vm_status} ${vm_ip}"
  if [[ "${vm_status}" == "RUNNING" && -n "${vm_ip}" ]]; then
    break
  fi
  sleep 5
done

echo "Waiting for SSH..."
while ! ssh "${ssh_wait_opts[@]}" "${ssh_user}@${vm_ip}" 'echo ssh-ready' >/dev/null 2>&1; do
  sleep 5
done

SSH_USER="${ssh_user}" SSH_IDENTITY_FILE="${ssh_private_key_path}" \
SSH_KNOWN_HOSTS_FILE="${ssh_known_hosts_file}" BOOTSTRAP="${bootstrap}" \
  "${remote_validation_script}" "${vm_ip}"
