#!/usr/bin/env bash
set -euo pipefail

# Purpose:  Downloads the Qwen2.5-Coder-7B-Instruct GGUF model weights from
#           HuggingFace into models/, verifying integrity against a sha256
#           checksum fetched at runtime from the HuggingFace tree API.
#           Idempotent: skips the download when a verified copy already exists.
# Usage:    scripts/download-model.sh   (run from the repository root)
# Deps:     bash >= 3.2, curl, shasum, python3
# Exit codes:
#   0  Model already present and verified, or downloaded and verified successfully.
#   1  Download failed, checksum mismatch, or the expected checksum could not be
#      determined from the HuggingFace tree API.

readonly MODEL_REPO="Qwen/Qwen2.5-Coder-7B-Instruct-GGUF"
readonly MODEL_FILENAME="qwen2.5-coder-7b-instruct-q4_k_m.gguf"
readonly MODELS_DIR="models"
readonly TREE_API_URL="https://huggingface.co/api/models/${MODEL_REPO}/tree/main"
readonly DOWNLOAD_URL="https://huggingface.co/${MODEL_REPO}/resolve/main/${MODEL_FILENAME}"

# Extracts the sha256 checksum for a named file from a HuggingFace tree API
# JSON response, read from stdin.
#
# Arguments:
#   $1  The "path" value of the tree entry to find.
# Outputs:
#   STDOUT: the sha256 hex digest.
#   Returns 1 if no LFS entry with a matching path is found.
extract_lfs_sha256() {
  local filename="${1:?extract_lfs_sha256 requires a filename}"
  python3 -c '
import json
import sys

filename = sys.argv[1]
entries = json.load(sys.stdin)
for entry in entries:
    if entry.get("path") == filename:
        oid = (entry.get("lfs") or {}).get("oid")
        if oid:
            print(oid)
            sys.exit(0)
sys.exit(1)
' "$filename"
}

# Fetches the expected sha256 checksum for MODEL_FILENAME from the HuggingFace
# tree API.
#
# Arguments:
#   None.
# Outputs:
#   STDOUT: the expected sha256 hex digest.
#   Returns 1 with a message on stderr if the API request fails or the response
#   has no LFS entry for MODEL_FILENAME.
fetch_expected_sha256() {
  local tree_json
  if ! tree_json="$(curl -sS --fail "$TREE_API_URL")"; then
    echo "ERROR: failed to fetch $TREE_API_URL" >&2
    return 1
  fi

  local sha256
  if ! sha256="$(printf '%s' "$tree_json" | extract_lfs_sha256 "$MODEL_FILENAME")"; then
    echo "ERROR: no LFS entry for '$MODEL_FILENAME' found in $TREE_API_URL" >&2
    return 1
  fi

  echo "$sha256"
}

# Computes the sha256 checksum of a file.
#
# Arguments:
#   $1  Path to the file to checksum.
# Outputs:
#   STDOUT: the sha256 hex digest.
compute_sha256() {
  local file="${1:?compute_sha256 requires a file path}"
  shasum -a 256 "$file" | awk '{print $1}'
}

# Checks whether a file's sha256 checksum matches an expected value.
#
# Arguments:
#   $1  Path to the file to check.
#   $2  Expected sha256 hex digest.
# Outputs:
#   Returns 0 if the checksums match, 1 otherwise.
checksum_matches() {
  local file="${1:?checksum_matches requires a file path}"
  local expected="${2:?checksum_matches requires an expected checksum}"
  [[ "$(compute_sha256 "$file")" == "$expected" ]]
}

# Downloads the model file from HuggingFace into the given path, resuming a
# previous partial download if one is present at that path.
#
# Arguments:
#   $1  Destination path for the downloaded (possibly partial) file.
# Outputs:
#   None. Exits nonzero if the download fails.
download_model() {
  local destination="${1:?download_model requires a destination path}"
  curl -L --fail -C - -o "$destination" "$DOWNLOAD_URL"
}

# Prints the final model path and its human-readable size.
#
# Arguments:
#   $1  Path to the downloaded model file.
# Outputs:
#   STDOUT: "<path> (<size>)".
print_success() {
  local file="${1:?print_success requires a file path}"
  local size
  size="$(du -h "$file" | awk '{print $1}')"
  echo "$file ($size)"
}

# Runs the full download workflow: skip if a verified copy already exists,
# otherwise download to a .part file, verify it, and move it into place.
#
# Arguments:
#   None.
# Outputs:
#   STDOUT: a status line reporting either the already-present check or the
#   final downloaded file's path and size.
main() {
  mkdir -p "$MODELS_DIR"

  local final_file="$MODELS_DIR/$MODEL_FILENAME"
  local part_file="$final_file.part"

  local expected_sha256
  expected_sha256="$(fetch_expected_sha256)"

  if [[ -f "$final_file" ]] && checksum_matches "$final_file" "$expected_sha256"; then
    echo "already present, checksum OK"
    exit 0
  fi

  download_model "$part_file"

  if ! checksum_matches "$part_file" "$expected_sha256"; then
    rm -f "$part_file"
    echo "ERROR: checksum mismatch for downloaded '$MODEL_FILENAME'" >&2
    exit 1
  fi

  mv "$part_file" "$final_file"
  print_success "$final_file"
}

main "$@"
