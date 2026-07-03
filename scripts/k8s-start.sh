#!/usr/bin/env bash
set -euo pipefail

# Purpose:  Starts the llm-runner Kubernetes deployment on demand: verifies the
#           model file is present, applies the ConfigMap, Service, and
#           Deployment manifests (substituting the host models directory into
#           the Deployment's hostPath volume), scales the Deployment to 1
#           replica, and waits for the rollout to finish.
# Usage:    scripts/k8s-start.sh   (run from the repository root; set
#           MODELS_DIR to override the default "$(pwd)/models")
# Deps:     bash >= 3.2, kubectl, sed
# Exit codes:
#   0  Deployment scaled up and ready.
#   1  Model file missing, or a kubectl command failed.

readonly MODEL_FILENAME="qwen2.5-coder-7b-instruct-q4_k_m.gguf"
readonly K8S_DIR="k8s"
readonly DEPLOYMENT_NAME="llm-runner"
readonly ENDPOINT_URL="http://localhost:30080/v1"

# Verifies the model file is present under the given models directory, failing
# with a pointer to the download script when it is not.
#
# Arguments:
#   $1  Path to the models directory.
# Outputs:
#   None. Exits 1 with a message on stderr if the model file is missing.
require_model_file() {
  local models_dir="${1:?require_model_file requires a models directory}"
  local model_path="$models_dir/$MODEL_FILENAME"
  if [[ ! -f "$model_path" ]]; then
    echo "ERROR: model file not found at '$model_path'" >&2
    echo "Run scripts/download-model.sh to download it first." >&2
    exit 1
  fi
}

# Applies the ConfigMap and Service manifests directly, and the Deployment
# manifest with its hostPath volume substituted to the given models directory.
#
# Arguments:
#   $1  Absolute path to substitute for __MODELS_DIR__ in deployment.yaml.
# Outputs:
#   None. Exits nonzero if any kubectl apply fails.
apply_manifests() {
  local models_dir="${1:?apply_manifests requires a models directory}"
  kubectl apply -f "$K8S_DIR/configmap.yaml"
  kubectl apply -f "$K8S_DIR/service.yaml"
  sed "s|__MODELS_DIR__|$models_dir|" "$K8S_DIR/deployment.yaml" | kubectl apply -f -
}

# Scales the Deployment to 1 replica and waits for the rollout to complete.
#
# Arguments:
#   None.
# Outputs:
#   None. Exits nonzero if the scale or rollout-status command fails.
start_deployment() {
  kubectl scale "deployment/$DEPLOYMENT_NAME" --replicas=1
  kubectl rollout status "deployment/$DEPLOYMENT_NAME" --timeout=600s
}

# Prints the endpoint URL and an example request for the running server.
#
# Arguments:
#   None.
# Outputs:
#   STDOUT: the endpoint URL and an example curl invocation.
print_endpoint() {
  echo "llm-runner is ready at $ENDPOINT_URL"
  echo "Example: curl $ENDPOINT_URL/models"
}

main() {
  local models_dir="${MODELS_DIR:-$(pwd)/models}"
  require_model_file "$models_dir"
  apply_manifests "$models_dir"
  start_deployment
  print_endpoint
}

main "$@"
