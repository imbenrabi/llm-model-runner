#!/usr/bin/env bash
set -euo pipefail

# Purpose:  Stops the llm-runner Kubernetes deployment on demand by scaling it
#           to zero replicas. Idempotent: does nothing if the deployment does
#           not exist.
# Usage:    scripts/k8s-stop.sh
# Deps:     bash >= 3.2, kubectl
# Exit codes:
#   0  Deployment scaled down, or was already absent.
#   1  A kubectl command failed.

readonly DEPLOYMENT_NAME="llm-runner"

# Checks whether the Deployment exists in the current context/namespace.
#
# Arguments:
#   None.
# Outputs:
#   Returns 0 if the Deployment exists, 1 otherwise.
deployment_exists() {
  kubectl get "deployment/$DEPLOYMENT_NAME" >/dev/null 2>&1
}

# Scales the Deployment to zero replicas and confirms the result.
#
# Arguments:
#   None.
# Outputs:
#   STDOUT: confirmation that the deployment was scaled down.
stop_deployment() {
  kubectl scale "deployment/$DEPLOYMENT_NAME" --replicas=0
  echo "$DEPLOYMENT_NAME scaled down to 0 replicas"
}

main() {
  if ! deployment_exists; then
    echo "nothing to stop"
    exit 0
  fi
  stop_deployment
}

main "$@"
