#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
usage: run_runpod_remote.sh [options] -- <remote command>

Spin up a Runpod GPU pod, wait until SSH is actually usable, sync the current
repository, run one remote command, optionally collect remote artifacts, and
tear the pod down.

Options:
  --pod-name NAME        Pod name. Default: stwo-cuda-<timestamp>
  --artifact PATH        Remote path under the synced repo to copy back after the run.
                         May be passed multiple times.
  --log-label LABEL      Local log directory label. Default: runpod-remote
  --no-bootstrap         Skip remote bootstrap.
  --keep-pod             Keep the pod running after the command completes.

Environment:
  RUNPOD_PLAN_ONLY             If 1, print the resolved plan and exit.
  RUNPOD_GPU_ID                Default: NVIDIA GeForce RTX 5090
  RUNPOD_TEMPLATE_ID           Default: runpod-torch-v280
  RUNPOD_IMAGE                 Optional image; mutually exclusive with template id.
  RUNPOD_CLOUD_TYPE            Default: SECURE
  RUNPOD_DATA_CENTER_IDS       Optional comma-separated datacenter ids
  RUNPOD_CONTAINER_DISK_GB     Default: 40
  RUNPOD_VOLUME_GB             Default: 40
  RUNPOD_VOLUME_MOUNT_PATH     Default: /workspace
  RUNPOD_PORTS                 Default: 8888/http,22/tcp
  RUNPOD_USE_PUBLIC_IP         Default: 1
  RUNPOD_SSH_USER              Default: root
  RUNPOD_REMOTE_DIR            Default: /workspace/stwo-cuda
  RUNPOD_READY_TIMEOUT_SEC     Default: 600
  RUNPOD_POLL_INTERVAL_SEC     Default: 5
  RUNPOD_CREATE_RETRIES        Default: 8
  RUNPOD_CREATE_RETRY_SLEEP_SEC
                               Default: 20. Applied between create rounds.
  RUNPOD_SSH_PRIVATE_KEY       Default: ~/.runpod/ssh/stwo-cuda-runpod-ed25519
  RUNPOD_SSH_PUBLIC_KEY        Default: <private>.pub
  RUNPOD_SSH_KEY_COMMENT       Default: stwo-cuda-runpod
  RUNPOD_ALLOW_SSH_INFO_KEY_FALLBACK
                               Default: 1. Allow fallback to the Runpod-advertised
                               SSH key path if the explicit ED25519 key is not accepted.
  RUNPOD_ACCEPT_MACHINE_LOCATIONS
                               Optional comma-separated pod-create machine.location
                               values that are considered acceptable.
  RUNPOD_REMOTE_ENV            Optional shell prefix injected before the remote command.
  RUNPOD_REMOTE_LOG_ROOT       Default: logs/runpod
EOF
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

json_escape() {
  ruby -rjson -e 'puts JSON.generate(STDIN.read)'
}

json_get() {
  local expr="$1"
  ruby -rjson -e "data = JSON.parse(STDIN.read); value = ${expr}; puts(value.is_a?(String) ? value : JSON.generate(value))"
}

json_has_error() {
  ruby -rjson -e 'data = JSON.parse(STDIN.read); exit(data["error"] ? 0 : 1)'
}

wait_for_ssh_info() {
  local pod_id="$1"
  local deadline="$2"
  local attempt_json

  while (( SECONDS < deadline )); do
    attempt_json="$(runpodctl ssh info "$pod_id" 2>/dev/null || true)"
    if [[ -n "$attempt_json" ]] && ! printf '%s' "$attempt_json" | json_has_error; then
      printf '%s' "$attempt_json"
      return 0
    fi
    sleep "$poll_interval_sec"
  done

  echo "timed out waiting for Runpod SSH info for pod $pod_id" >&2
  return 1
}

wait_for_ssh_login() {
  local remote="$1"
  local port="$2"
  local deadline="$3"

  while (( SECONDS < deadline )); do
    if ssh "${ssh_opts[@]}" -p "$port" "$remote" 'echo ready' >/dev/null 2>&1; then
      return 0
    fi
    sleep "$poll_interval_sec"
  done

  echo "timed out waiting for usable SSH login to $remote:$port" >&2
  return 1
}

choose_ssh_login_key() {
  local remote="$1"
  local port="$2"
  local deadline="$3"
  shift 3
  local candidate

  while (( SECONDS < deadline )); do
    for candidate in "$@"; do
      [[ -n "$candidate" && -f "$candidate" ]] || continue
      ssh_opts=(
        -i "$candidate"
        -o BatchMode=yes
        -o IdentitiesOnly=yes
        -o StrictHostKeyChecking=accept-new
        -o UserKnownHostsFile="$known_hosts_file"
      )
      if ssh "${ssh_opts[@]}" -p "$port" "$remote" 'echo ready' >/dev/null 2>&1; then
        selected_ssh_key="$candidate"
        return 0
      fi
    done
    sleep "$poll_interval_sec"
  done

  echo "timed out waiting for usable SSH login to $remote:$port" >&2
  return 1
}

ensure_ssh_key() {
  mkdir -p "$(dirname "$ssh_private_key")"
  if [[ ! -f "$ssh_private_key" || ! -f "$ssh_public_key" ]]; then
    ssh-keygen -t ed25519 -N '' -C "$ssh_key_comment" -f "$ssh_private_key" >/dev/null
  fi
}

ensure_ssh_key_registered() {
  local public_key="$1"
  local escaped_key
  local keys_json

  escaped_key="$(printf '%s' "$public_key" | json_escape)"
  keys_json="$(runpodctl ssh list-keys)"
  if printf '%s' "$keys_json" | json_get "data.fetch(\"keys\", []).any? { |key| key[\"key\"].to_s.strip == ${escaped_key} }"; then
    return 0
  fi

  runpodctl ssh add-key --key-file "$ssh_public_key" >/dev/null
}

value_in_csv() {
  local needle="$1"
  local haystack="$2"
  local item

  IFS=',' read -r -a csv_items <<< "$haystack"
  for item in "${csv_items[@]}"; do
    item="${item#"${item%%[![:space:]]*}"}"
    item="${item%"${item##*[![:space:]]}"}"
    if [[ "$item" == "$needle" ]]; then
      return 0
    fi
  done
  return 1
}

create_pod_with_retries() {
  local create_json="$1"
  shift
  local create_round=1
  local attempt=1
  local attempt_log
  local attempt_json
  local attempt_rc
  local location
  local attempt_pod_id
  local selected_data_center_id
  local dc_label
  local -a data_center_candidates=()
  local -a create_args_for_attempt=()

  if [[ -n "$data_center_ids" ]]; then
    IFS=',' read -r -a data_center_candidates <<< "$data_center_ids"
  else
    data_center_candidates=("")
  fi

  while (( create_round <= create_retries )); do
    for selected_data_center_id in "${data_center_candidates[@]}"; do
      dc_label="${selected_data_center_id:-auto}"
      attempt_log="$local_log_dir/pod-create-attempt-${attempt}-${dc_label}.log"
      attempt_json="$local_log_dir/pod-create-attempt-${attempt}-${dc_label}.json"
      create_args_for_attempt=("$@")
      if [[ -n "$selected_data_center_id" ]]; then
        create_args_for_attempt+=(--data-center-ids "$selected_data_center_id")
      fi

      set +e
      runpodctl "${create_args_for_attempt[@]}" >"$attempt_log" 2>&1
      attempt_rc=$?
      set -e

      if (( attempt_rc == 0 )); then
        cp "$attempt_log" "$attempt_json"
        if [[ -n "$accept_machine_locations" ]]; then
          location="$(cat "$attempt_json" | json_get 'data.fetch("machine", {}).fetch("location", "")')"
          if [[ -n "$location" ]] && ! value_in_csv "$location" "$accept_machine_locations"; then
            attempt_pod_id="$(cat "$attempt_json" | json_get 'data["id"]')"
            printf 'rejecting pod %s due to machine.location=%s\n' "$attempt_pod_id" "$location" >>"$attempt_log"
            runpodctl pod delete "$attempt_pod_id" >/dev/null 2>&1 || true
            attempt=$((attempt + 1))
            continue
          fi
        fi

        cp "$attempt_json" "$create_json"
        return 0
      fi

      attempt=$((attempt + 1))
    done

    if (( create_round < create_retries )); then
      sleep "$create_retry_sleep_sec"
    fi
    create_round=$((create_round + 1))
  done

  echo "failed to create a usable pod after ${create_retries} create rounds; see $local_log_dir/pod-create-attempt-*.log" >&2
  return 1
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

require_cmd runpodctl
require_cmd ruby
require_cmd rsync
require_cmd ssh
require_cmd ssh-keygen

plan_only="${RUNPOD_PLAN_ONLY:-0}"
gpu_id="${RUNPOD_GPU_ID:-NVIDIA GeForce RTX 5090}"
template_id="${RUNPOD_TEMPLATE_ID:-runpod-torch-v280}"
image="${RUNPOD_IMAGE:-}"
cloud_type="${RUNPOD_CLOUD_TYPE:-SECURE}"
data_center_ids="${RUNPOD_DATA_CENTER_IDS:-}"
container_disk_gb="${RUNPOD_CONTAINER_DISK_GB:-40}"
volume_gb="${RUNPOD_VOLUME_GB:-40}"
volume_mount_path="${RUNPOD_VOLUME_MOUNT_PATH:-/workspace}"
ports="${RUNPOD_PORTS:-8888/http,22/tcp}"
use_public_ip="${RUNPOD_USE_PUBLIC_IP:-1}"
ssh_user="${RUNPOD_SSH_USER:-root}"
remote_dir="${RUNPOD_REMOTE_DIR:-/workspace/stwo-cuda}"
ready_timeout_sec="${RUNPOD_READY_TIMEOUT_SEC:-600}"
poll_interval_sec="${RUNPOD_POLL_INTERVAL_SEC:-5}"
create_retries="${RUNPOD_CREATE_RETRIES:-8}"
create_retry_sleep_sec="${RUNPOD_CREATE_RETRY_SLEEP_SEC:-20}"
ssh_private_key="${RUNPOD_SSH_PRIVATE_KEY:-$HOME/.runpod/ssh/stwo-cuda-runpod-ed25519}"
ssh_public_key="${RUNPOD_SSH_PUBLIC_KEY:-${ssh_private_key}.pub}"
ssh_key_comment="${RUNPOD_SSH_KEY_COMMENT:-stwo-cuda-runpod}"
allow_ssh_info_key_fallback="${RUNPOD_ALLOW_SSH_INFO_KEY_FALLBACK:-1}"
accept_machine_locations="${RUNPOD_ACCEPT_MACHINE_LOCATIONS:-}"
remote_env="${RUNPOD_REMOTE_ENV:-}"
remote_log_root="${RUNPOD_REMOTE_LOG_ROOT:-logs/runpod}"
bootstrap=1
keep_pod=0
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
pod_name="stwo-cuda-${timestamp}"
log_label="runpod-remote"
artifact_paths=()

while (($#)); do
  case "$1" in
    --pod-name)
      pod_name="$2"
      shift 2
      ;;
    --artifact)
      artifact_paths+=("$2")
      shift 2
      ;;
    --log-label)
      log_label="$2"
      shift 2
      ;;
    --no-bootstrap)
      bootstrap=0
      shift
      ;;
    --keep-pod)
      keep_pod=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    --)
      shift
      break
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if (($# == 0)); then
  echo "missing remote command" >&2
  usage >&2
  exit 1
fi

remote_command="$(printf '%q ' "$@")"
remote_command="${remote_command% }"

if [[ -n "$template_id" && -n "$image" ]]; then
  echo "set either RUNPOD_TEMPLATE_ID or RUNPOD_IMAGE, not both" >&2
  exit 1
fi

if [[ -z "$template_id" && -z "$image" ]]; then
  echo "one of RUNPOD_TEMPLATE_ID or RUNPOD_IMAGE must be set" >&2
  exit 1
fi

ensure_ssh_key
public_key="$(tr -d '\n' < "$ssh_public_key")"
ensure_ssh_key_registered "$public_key"

local_log_dir="$repo_root/${remote_log_root}/${timestamp}-${log_label}"
mkdir -p "$local_log_dir"
known_hosts_file="$local_log_dir/known_hosts"
touch "$known_hosts_file"
cat > "$local_log_dir/run-config.txt" <<EOF
pod_name=$pod_name
gpu_id=$gpu_id
template_id=${template_id:-}
image=${image:-}
cloud_type=$cloud_type
data_center_ids=$data_center_ids
container_disk_gb=$container_disk_gb
volume_gb=$volume_gb
volume_mount_path=$volume_mount_path
ports=$ports
use_public_ip=$use_public_ip
ssh_user=$ssh_user
remote_dir=$remote_dir
ready_timeout_sec=$ready_timeout_sec
poll_interval_sec=$poll_interval_sec
create_retries=$create_retries
create_retry_sleep_sec=$create_retry_sleep_sec
create_retry_candidate_pool=${data_center_ids:-auto}
ssh_private_key=$ssh_private_key
ssh_public_key=$ssh_public_key
allow_ssh_info_key_fallback=$allow_ssh_info_key_fallback
accept_machine_locations=$accept_machine_locations
bootstrap=$bootstrap
keep_pod=$keep_pod
remote_env=$remote_env
remote_command=$remote_command
artifacts=${artifact_paths[*]:-}
EOF

if [[ "$plan_only" == "1" ]]; then
  cat <<EOF
runpod plan:
  pod_name: $pod_name
  gpu_id: $gpu_id
  template_id: ${template_id:-<none>}
  image: ${image:-<none>}
  cloud_type: $cloud_type
  remote_dir: $remote_dir
  bootstrap: $bootstrap
  keep_pod: $keep_pod
  artifacts: ${artifact_paths[*]:-<none>}
  remote_command: $remote_command
  local_log_dir: $local_log_dir
EOF
  exit 0
fi

create_json="$local_log_dir/pod-create.json"
create_args=(
  pod create
  --name "$pod_name"
  --gpu-id "$gpu_id"
  --cloud-type "$cloud_type"
  --container-disk-in-gb "$container_disk_gb"
  --volume-in-gb "$volume_gb"
  --volume-mount-path "$volume_mount_path"
  --ports "$ports"
  --env "{\"PUBLIC_KEY\":\"$public_key\"}"
)
if [[ -n "$template_id" ]]; then
  create_args+=(--template-id "$template_id")
else
  create_args+=(--image "$image")
fi
if [[ "$use_public_ip" == "1" && "$cloud_type" == "COMMUNITY" ]]; then
  create_args+=(--public-ip)
fi

create_pod_with_retries "$create_json" "${create_args[@]}"
pod_id="$(cat "$create_json" | json_get 'data["id"]')"

cleanup() {
  if [[ -n "${pod_id:-}" && "$keep_pod" != "1" ]]; then
    runpodctl pod delete "$pod_id" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

ssh_info_json="$(wait_for_ssh_info "$pod_id" $((SECONDS + ready_timeout_sec)))"
printf '%s\n' "$ssh_info_json" > "$local_log_dir/ssh-info.json"

host="$(printf '%s' "$ssh_info_json" | json_get 'data["ip"]')"
ssh_port="$(printf '%s' "$ssh_info_json" | json_get 'data["port"]')"
remote="${ssh_user}@${host}"
selected_ssh_key="$ssh_private_key"
ssh_info_key_path=""
if [[ "$allow_ssh_info_key_fallback" == "1" ]]; then
  ssh_info_key_path="$(printf '%s' "$ssh_info_json" | json_get 'data.fetch("ssh_key", {}).fetch("path", "")')"
fi

if [[ -n "$ssh_info_key_path" && "$ssh_info_key_path" != "$ssh_private_key" ]]; then
  choose_ssh_login_key \
    "$remote" \
    "$ssh_port" \
    $((SECONDS + ready_timeout_sec)) \
    "$ssh_private_key" \
    "$ssh_info_key_path"
else
  choose_ssh_login_key \
    "$remote" \
    "$ssh_port" \
    $((SECONDS + ready_timeout_sec)) \
    "$ssh_private_key"
fi

if [[ "$bootstrap" == "1" ]]; then
  SSH_IDENTITY_FILE="$selected_ssh_key" \
  SSH_PORT="$ssh_port" \
  SSH_USER="$ssh_user" \
  SSH_KNOWN_HOSTS_FILE="$known_hosts_file" \
    "$repo_root/scripts/bootstrap_remote_cuda_host.sh" "$host"
fi

rsync_ssh="ssh -i $selected_ssh_key -p $ssh_port -o BatchMode=yes -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=$known_hosts_file"
ssh "${ssh_opts[@]}" -p "$ssh_port" "$remote" "mkdir -p $(printf '%q' "$remote_dir")"

rsync -az --no-owner --no-group --delete \
  --exclude '.git' \
  --exclude 'target' \
  --exclude 'logs' \
  --exclude 'logs/remote-cuda' \
  --exclude '.DS_Store' \
  -e "$rsync_ssh" \
  "$repo_root/" "${remote}:${remote_dir}/"

remote_log="$local_log_dir/remote-command.log"
ssh "${ssh_opts[@]}" -p "$ssh_port" "$remote" "bash -s -- $(printf '%q' "$remote_dir") $(printf '%q' "$remote_env") $(printf '%q' "$remote_command")" 2>&1 <<'EOF' | tee "$remote_log"
set -euo pipefail

remote_dir="$1"
remote_env="$2"
remote_command="$3"

cd "$remote_dir"
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
    if [[ -x "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

nvcc_bin="$(find_nvcc || true)"
if [[ -z "$nvcc_bin" ]]; then
  echo "nvcc is missing on the remote host." >&2
  exit 1
fi

export PATH="$(dirname "$nvcc_bin"):$PATH"
cuda_bin_dir="$(dirname "$(readlink -f "$nvcc_bin")")"
cuda_home="$(cd "$cuda_bin_dir/.." && pwd)"
export CUDA_HOME="$cuda_home"
export CUDAToolkit_ROOT="$cuda_home"
export NUM_JOBS="${NUM_JOBS:-$(nproc)}"

echo "== system =="
uname -a
echo

echo "== gpu =="
nvidia-smi
nvcc --version
echo "CUDA_HOME=$CUDA_HOME"
echo

echo "== toolchain =="
if command -v rustc >/dev/null 2>&1; then
  rustc -V
else
  echo "rustc missing"
fi
if command -v cargo >/dev/null 2>&1; then
  cargo -V
else
  echo "cargo missing"
fi
if command -v ruby >/dev/null 2>&1; then
  ruby -v
else
  echo "ruby missing"
fi
echo

echo "== remote command =="
echo "$remote_env $remote_command"
echo

if [[ -n "$remote_env" ]]; then
  eval "$remote_env $remote_command"
else
  eval "$remote_command"
fi
EOF

if ((${#artifact_paths[@]} > 0)); then
  mkdir -p "$local_log_dir/artifacts"
  for artifact in "${artifact_paths[@]}"; do
    rsync -azR --no-owner --no-group \
      -e "$rsync_ssh" \
      "${remote}:${remote_dir}/${artifact}" "$local_log_dir/artifacts/" || true
  done
fi

echo
echo "runpod log directory: $local_log_dir"
