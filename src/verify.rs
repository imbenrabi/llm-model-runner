use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::config;
use crate::error::RunnerError;
use crate::process;

/// A single OpenAI-schema model entry as returned by `GET /v1/models`.
#[derive(Debug, Clone, Deserialize)]
struct ModelInfo {
    id: String,
}

/// Response body of `GET /v1/models`.
#[derive(Debug, Clone, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelInfo>,
}

/// A single chat message in an OpenAI-schema chat completion request.
#[derive(Debug, Clone, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Request body for `POST /v1/chat/completions`.
#[derive(Debug, Clone, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
}

/// The message portion of a single chat completion choice.
#[derive(Debug, Clone, Deserialize)]
struct ChatCompletionChoiceMessage {
    content: String,
}

/// A single choice in a chat completion response.
#[derive(Debug, Clone, Deserialize)]
struct ChatCompletionChoice {
    message: ChatCompletionChoiceMessage,
}

/// Response body of `POST /v1/chat/completions`.
#[derive(Debug, Clone, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatCompletionChoice>,
}

/// Reads an HTTP response body and parses it as JSON, first checking that the
/// response status was 200.
///
/// # Arguments
/// * `response` - The HTTP response to consume.
/// * `context` - A short label identifying the request (e.g. `GET /v1/models`), used
///   in error messages.
///
/// # Returns
/// The parsed body, or a `RunnerError` describing a non-200 status, a read failure, or
/// a JSON parse failure.
fn parse_json_response<T: serde::de::DeserializeOwned>(
    mut response: ureq::http::Response<ureq::Body>,
    context: &str,
) -> Result<T, RunnerError> {
    if response.status() != 200 {
        return Err(RunnerError::Http(format!(
            "{context} returned status {}",
            response.status()
        )));
    }

    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| RunnerError::Io(format!("failed to read {context} body: {e}")))?;

    serde_json::from_str(&body)
        .map_err(|e| RunnerError::Json(format!("failed to parse {context} response: {e}")))
}

/// Runs Test 1 of the `verify` subcommand: fetches `/v1/models` and asserts the
/// model list is non-empty.
///
/// # Arguments
/// * `host` - Host the server is bound to.
/// * `port` - Port the server is listening on.
///
/// # Returns
/// The id of the first model reported, or a `RunnerError` describing the failure.
fn verify_models(host: &str, port: u16) -> Result<String, RunnerError> {
    let url = format!("http://{host}:{port}/v1/models");
    let response = ureq::get(&url)
        .config()
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .call()
        .map_err(|e| RunnerError::Http(format!("GET /v1/models failed: {e}")))?;

    let parsed: ModelsResponse = parse_json_response(response, "GET /v1/models")?;

    parsed
        .data
        .first()
        .map(|model| model.id.clone())
        .ok_or_else(|| RunnerError::Http("/v1/models returned an empty model list".to_string()))
}

/// Runs Test 2 of the `verify` subcommand: sends a chat completion request and
/// asserts a non-empty reply was returned.
///
/// # Arguments
/// * `host` - Host the server is bound to.
/// * `port` - Port the server is listening on.
/// * `model` - Model id to request the completion from.
///
/// # Returns
/// The reply content from the first choice, or a `RunnerError` describing the failure.
fn verify_chat(host: &str, port: u16, model: &str) -> Result<String, RunnerError> {
    let url = format!("http://{host}:{port}/v1/chat/completions");
    let request_body = ChatCompletionRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "Say hello.".to_string(),
        }],
        max_tokens: 64,
    };
    let body_string = serde_json::to_string(&request_body)
        .map_err(|e| RunnerError::Json(format!("failed to encode chat request: {e}")))?;

    let response = ureq::post(&url)
        .header("Content-Type", "application/json")
        .send(&body_string)
        .map_err(|e| RunnerError::Http(format!("POST /v1/chat/completions failed: {e}")))?;

    let parsed: ChatCompletionResponse =
        parse_json_response(response, "POST /v1/chat/completions")?;

    let content = parsed
        .choices
        .first()
        .map(|choice| choice.message.content.clone())
        .unwrap_or_default();

    if content.is_empty() {
        return Err(RunnerError::Http(
            "chat completion returned empty content".to_string(),
        ));
    }

    Ok(content)
}

/// Strips the trailing `:port` from an address, handling bracketed IPv6 literals.
///
/// # Arguments
/// * `endpoint` - An address of the form `host:port` or `[ipv6]:port`.
///
/// # Returns
/// The address portion with the port and any brackets removed.
fn strip_port(endpoint: &str) -> &str {
    if let Some(rest) = endpoint.strip_prefix('[') {
        if let Some((addr, _)) = rest.split_once(']') {
            return addr;
        }
    }
    match endpoint.rsplit_once(':') {
        Some((addr, _port)) => addr,
        None => endpoint,
    }
}

/// Determines whether a single `host:port` (or bracketed IPv6) endpoint is loopback.
///
/// # Arguments
/// * `endpoint` - An address of the form `host:port`, `[ipv6]:port`, or a wildcard `*:port`.
///
/// # Returns
/// `true` if the endpoint's address is exactly `127.0.0.1` or `::1`; `false` otherwise,
/// including wildcard binds and any other configured, LAN, or external address.
fn is_loopback_address(endpoint: &str) -> bool {
    let address = strip_port(endpoint.trim());
    address == "127.0.0.1" || address == "::1"
}

/// Classifies whether a single `lsof -nP -a -p <pid> -i` output line represents
/// loopback-only network activity.
///
/// # Arguments
/// * `line` - One data line of `lsof` output (not the header line).
///
/// # Returns
/// `true` if every address on the line is loopback, `false` if any address is external,
/// a LAN address, or a wildcard bind.
fn is_loopback_line(line: &str) -> bool {
    let name_field = match line.split_whitespace().collect::<Vec<_>>().as_slice() {
        [.., name, state] if state.starts_with('(') => (*name).to_string(),
        [.., name] => (*name).to_string(),
        [] => return true,
    };

    name_field.split("->").all(is_loopback_address)
}

/// Runs the `--egress` check: inspects the network connections held open by the
/// supervised `llama-server` process and fails if any is non-loopback.
///
/// # Arguments
/// * `pid` - PID of the supervised `llama-server` process.
///
/// # Returns
/// `Ok(())` if every connection is loopback-only, or a `RunnerError` describing the first
/// non-loopback line found.
fn run_egress_check(pid: u32) -> Result<(), RunnerError> {
    let output = Command::new("lsof")
        .args(["-nP", "-a", "-p", &pid.to_string(), "-i"])
        .output()
        .map_err(|e| RunnerError::Process(format!("failed to run lsof: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let data_lines = stdout.lines().skip(1);

    for line in data_lines {
        if line.trim().is_empty() {
            continue;
        }
        if !is_loopback_line(line) {
            return Err(RunnerError::Process(format!(
                "non-loopback network activity detected: {line}"
            )));
        }
    }

    println!(
        "egress check passed: llama-server (pid {pid}) shows no non-loopback network activity"
    );
    Ok(())
}

/// Runs the `verify` subcommand: exercises `/v1/models` and `/v1/chat/completions`,
/// and optionally checks for non-loopback network activity.
///
/// # Arguments
/// * `egress` - Whether to additionally run the zero-egress `lsof` check.
/// * `config_path` - Path to the TOML configuration file to load.
///
/// # Returns
/// `Ok(())` if all requested checks pass, or a `RunnerError` describing the first failure.
pub fn handle_verify(egress: bool, config_path: &Path) -> Result<(), RunnerError> {
    let cfg = config::load_config(config_path)?;

    let model_id = verify_models(&cfg.host, cfg.port)?;
    println!("/v1/models: found model \"{model_id}\"");

    let reply = verify_chat(&cfg.host, cfg.port, &model_id)?;
    println!("chat reply: {reply}");

    if egress {
        let pid_path = PathBuf::from(process::PID_FILE);
        let pid = process::read_pidfile(&pid_path)?.ok_or_else(|| {
            RunnerError::Process("cannot run --egress check: no pidfile found".to_string())
        })?;
        run_egress_check(pid)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_loopback_listen_line_as_pass() {
        assert!(is_loopback_line(
            "llama-serv 123 ben 10u IPv4 0x1 0t0 TCP 127.0.0.1:8080 (LISTEN)"
        ));
    }

    #[test]
    fn classifies_loopback_established_line_as_pass() {
        assert!(is_loopback_line(
            "llama-serv 123 ben 11u IPv4 0x1 0t0 TCP 127.0.0.1:54321->127.0.0.1:9000 (ESTABLISHED)"
        ));
    }

    #[test]
    fn classifies_ipv6_loopback_line_as_pass() {
        assert!(is_loopback_line(
            "llama-serv 123 ben 12u IPv6 0x1 0t0 TCP [::1]:8080 (LISTEN)"
        ));
    }

    #[test]
    fn classifies_external_address_line_as_fail() {
        assert!(!is_loopback_line(
            "llama-serv 123 ben 13u IPv4 0x1 0t0 TCP 127.0.0.1:54321->93.184.216.34:443 (ESTABLISHED)"
        ));
    }

    #[test]
    fn classifies_wildcard_bind_line_as_fail() {
        assert!(!is_loopback_line(
            "llama-serv 123 ben 14u IPv4 0x1 0t0 TCP *:8080 (LISTEN)"
        ));
    }

    #[test]
    fn classifies_lan_ip_line_as_fail() {
        assert!(!is_loopback_line(
            "llama-serv 123 ben 15u IPv4 0x1 0t0 TCP 192.168.1.50:8080 (LISTEN)"
        ));
    }

    #[test]
    fn classifies_ipv4_wildcard_bind_address_line_as_fail() {
        assert!(!is_loopback_line(
            "llama-serv 123 ben 16u IPv4 0x1 0t0 TCP 0.0.0.0:8080 (LISTEN)"
        ));
    }
}
