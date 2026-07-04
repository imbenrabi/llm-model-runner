#!/usr/bin/env bash
set -euo pipefail

# Purpose:  Benchmark the local llama-server supervised by llm-runner.
#           Measures TTFT, end-to-end latency, generated tokens/sec, and CPU
#           usage for three representative chat-completion workloads.
#           Uses streaming mode so time-to-first-byte is a real first-token
#           measurement and tokens/sec is wall-clock generation throughput.
# Usage:    scripts/benchmark.sh [HOST] [PORT]
#           Defaults to 127.0.0.1 8080. The model id is fetched from /v1/models.
# Deps:     bash >= 3.2, curl, python3, ps, awk.
# Output:   Markdown-formatted results on stdout.

readonly DEFAULT_HOST="127.0.0.1"
readonly DEFAULT_PORT="8080"
readonly PID_FILE=".runner/llama-server.pid"

HOST="${1:-$DEFAULT_HOST}"
PORT="${2:-$DEFAULT_PORT}"
BASE_URL="http://${HOST}:${PORT}"

# Fetches the first model id from /v1/models.
#
# Arguments:
#   None.
# Outputs:
#   STDOUT: the first model id.
fetch_model_id() {
  curl -s "${BASE_URL}/v1/models" | python3 -c '
import json, sys
data = json.load(sys.stdin)
print(data["data"][0]["id"])
'
}

# Reads the supervised llama-server PID from the pidfile.
#
# Arguments:
#   None.
# Outputs:
#   STDOUT: the PID.
read_server_pid() {
  if [[ ! -f "$PID_FILE" ]]; then
    echo "ERROR: no pidfile at $PID_FILE" >&2
    return 1
  fi
  tr -d '[:space:]' < "$PID_FILE"
}

# Sends a single *streaming* chat completion request and extracts timing and
# token metadata.
#
# Arguments:
#   $1  Output directory (created if absent).
#   $2  JSON request body (must NOT already contain "stream").
# Outputs:
#   Writes sse.txt (raw Server-Sent Events) and timing.csv into $1.
#   STDOUT: "<starttransfer> <total> <predicted_n> <server_tps>".
request_chat() {
  local out_dir="${1:?request_chat requires output directory}"
  local body="${2:?request_chat requires request body}"
  mkdir -p "$out_dir"

  local sse_file="$out_dir/sse.txt"
  local timing_file="$out_dir/timing.csv"
  local stream_body
  stream_body="$(printf '%s' "$body" | python3 -c '
import json, sys
obj = json.load(sys.stdin)
obj["stream"] = True
print(json.dumps(obj, separators=(",", ":")))
')"

  curl -s -N \
    -o "$sse_file" \
    -w 'STARTTRANSFER:%{time_starttransfer}\nTOTAL:%{time_total}\n' \
    -H "Content-Type: application/json" \
    -d "$stream_body" \
    "${BASE_URL}/v1/chat/completions" > "$timing_file"

  local starttransfer total
  starttransfer="$(grep '^STARTTRANSFER:' "$timing_file" | cut -d: -f2)"
  total="$(grep '^TOTAL:' "$timing_file" | cut -d: -f2)"

  local tokens server_tps
  read -r tokens server_tps <<< "$(parse_sse_metadata "$sse_file")"

  printf '%s,%s,%s,%s\n' "$starttransfer" "$total" "$tokens" "$server_tps" > "$timing_file"
  printf '%s %s %s %s\n' "$starttransfer" "$total" "$tokens" "$server_tps"
}

# Parses the final SSE chunk to obtain generated token count and the server's
# own tokens-per-second figure.
#
# Arguments:
#   $1  Path to the SSE text file.
# Outputs:
#   STDOUT: "<predicted_n> <predicted_per_second>".
parse_sse_metadata() {
  local file="${1:?parse_sse_metadata requires a file}"
  python3 -c '
import json, sys
path = sys.argv[1]
tokens = "N/A"
tps = "N/A"
with open(path, "r") as f:
    for line in f:
        line = line.strip()
        if line.startswith("data: "):
            payload = line[len("data: "):]
            if payload == "[DONE]":
                continue
            try:
                obj = json.loads(payload)
            except json.JSONDecodeError:
                continue
            timings = obj.get("timings", {})
            if "predicted_n" in timings:
                tokens = timings["predicted_n"]
            if "predicted_per_second" in timings:
                tps = timings["predicted_per_second"]
print(tokens, tps)
' "$file"
}

# Computes average and maximum values from a whitespace-separated sample file.
#
# Arguments:
#   $1  Path to a file with one numeric sample per line.
# Outputs:
#   STDOUT: "avg <avg> max <max>".
avg_max_from_samples() {
  local file="${1:?avg_max_from_samples requires a sample file}"
  awk '
    NF { sum += $1; count++; if ($1 > max) max = $1 }
    END {
      if (count == 0) { print "avg 0.0 max 0.0"; exit }
      printf "avg %.1f max %.1f\n", sum / count, max
    }
  ' "$file"
}

# Computes throughput metrics for one workload iteration.
#
# Arguments:
#   $1  Workload name.
#   $2  Iteration number.
#   $3  Request JSON body.
#   $4  Server PID.
# Outputs:
#   STDOUT: a markdown table row.
benchmark_workload_iter() {
  local name="$1"
  local iter="$2"
  local body="$3"
  local pid="$4"

  local out_dir=".bench/${name}/${iter}"
  local samples_file
  samples_file="$(mktemp)"

  # Start CPU sampler before the request.
  (
    while true; do
      ps -o %cpu= -p "$pid" 2>/dev/null || true
      sleep 0.5
    done
  ) > "$samples_file" &
  local sampler=$!

  local metadata
  metadata="$(request_chat "$out_dir" "$body")"

  kill "$sampler" 2>/dev/null || true
  wait "$sampler" 2>/dev/null || true

  local starttransfer total tokens server_tps
  starttransfer="$(echo "$metadata" | awk '{print $1}')"
  total="$(echo "$metadata" | awk '{print $2}')"
  tokens="$(echo "$metadata" | awk '{print $3}')"
  server_tps="$(echo "$metadata" | awk '{print $4}')"

  local cpu_stats cpu_avg cpu_max
  cpu_stats="$(avg_max_from_samples "$samples_file")"
  cpu_avg="$(echo "$cpu_stats" | awk '{print $2}')"
  cpu_max="$(echo "$cpu_stats" | awk '{print $4}')"

  rm -f "$samples_file"

  local gen_time tps
  gen_time="$(awk -v t="$total" -v s="$starttransfer" 'BEGIN { printf "%.3f", t - s }')"
  tps="$(awk -v toks="$tokens" -v t="$gen_time" 'BEGIN { printf "%.2f", toks / t }')"

  printf '| %s | %s | %s | %s | %s | %s | %s | %s | %s | %s |\n' \
    "$name" "$iter" "$tokens" "$starttransfer" "$total" "$gen_time" "$tps" "$server_tps" "$cpu_avg" "$cpu_max"
}

# Runs all benchmark iterations and prints a markdown table.
#
# Arguments:
#   $1  Model id.
#   $2  Server PID.
# Outputs:
#   STDOUT: markdown results.
run_benchmarks() {
  local model_id="$1"
  local pid="$2"

  echo "### Benchmark parameters"
  echo "- Base URL: ${BASE_URL}"
  echo "- Model: ${model_id}"
  echo "- Server PID: ${pid}"
  echo "- Iterations per workload: 3 (plus one warm-up)"
  echo "- Streaming: enabled (true TTFT / wall-clock throughput)"
  echo ""
  echo "| Workload | Iter | Predicted tokens | TTFT (s) | Total (s) | Gen time (s) | Wall tok/s | Server tok/s | CPU avg % | CPU max % |"
  echo "|---|---|---|---|---|---|---|---|---|---|"

  # Warm-up: short request to ensure Metal is loaded.
  request_chat ".bench/warmup" "$(printf '%s' '{"model":"'"$model_id"'","messages":[{"role":"user","content":"Say hello."}],"max_tokens":64}')" >/dev/null

  local body

  # Workload 1: short prompt, short output.
  body='{"model":"'"$model_id"'","messages":[{"role":"user","content":"Say hello."}],"max_tokens":64}'
  for i in 1 2 3; do
    benchmark_workload_iter "short" "$i" "$body" "$pid"
  done

  # Workload 2: short prompt, longer output.
  body='{"model":"'"$model_id"'","messages":[{"role":"user","content":"Explain Rust ownership in three sentences."}],"max_tokens":256}'
  for i in 1 2 3; do
    benchmark_workload_iter "medium" "$i" "$body" "$pid"
  done

  # Workload 3: coding prompt, moderate output.
  body='{"model":"'"$model_id"'","messages":[{"role":"user","content":"Write a Python function that reads a CSV and returns a list of dicts."}],"max_tokens":512,"temperature":0.7}'
  for i in 1 2 3; do
    benchmark_workload_iter "code" "$i" "$body" "$pid"
  done
}

main() {
  local model_id pid
  model_id="$(fetch_model_id)"
  pid="$(read_server_pid)"
  run_benchmarks "$model_id" "$pid"
}

main "$@"
