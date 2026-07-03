use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::error::RunnerError;

/// Default path to the runner configuration file, resolved relative to the
/// current working directory.
pub const DEFAULT_CONFIG_PATH: &str = "config/runner.toml";

/// Configuration for supervising a local `llama-server` process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// Path to the GGUF model file, passed to `llama-server --model`.
    pub model_path: String,
    /// Bind address, passed to `llama-server --host`. Must be loopback-only
    /// (`127.0.0.1` or `::1`); enforced by `validate_config`.
    pub host: String,
    /// Bind port, passed to `llama-server --port`.
    pub port: u16,
    /// Context window size in tokens, passed to `llama-server --ctx-size`.
    pub ctx_size: u32,
    /// Number of layers offloaded to GPU/Metal, passed to `llama-server --n-gpu-layers`.
    pub n_gpu_layers: u32,
}

/// Validates that a configuration's bind host is loopback-only.
///
/// # Arguments
/// * `config` - The configuration to validate.
///
/// # Returns
/// `Ok(())` if `host` is exactly `127.0.0.1` or `::1`, or a `RunnerError` naming the
/// offending value and the loopback-only requirement.
fn validate_config(config: &Config) -> Result<(), RunnerError> {
    if config.host == "127.0.0.1" || config.host == "::1" {
        Ok(())
    } else {
        Err(RunnerError::Config(format!(
            "host must be loopback-only (\"127.0.0.1\" or \"::1\"), got \"{}\"",
            config.host
        )))
    }
}

/// Loads and parses a runner configuration from the given TOML file path.
///
/// # Arguments
/// * `path` - Filesystem path to the TOML configuration file.
///
/// # Returns
/// The parsed `Config`, or a `RunnerError` if the file cannot be read or parsed, or if
/// `host` is not loopback-only.
pub fn load_config(path: &Path) -> Result<Config, RunnerError> {
    let contents = fs::read_to_string(path)
        .map_err(|e| RunnerError::Io(format!("failed to read config {}: {e}", path.display())))?;
    let config: Config = toml::from_str(&contents).map_err(|e| {
        RunnerError::Config(format!("failed to parse config {}: {e}", path.display()))
    })?;
    validate_config(&config)?;
    Ok(config)
}

/// Builds the `llama-server` command-line arguments from a configuration.
///
/// # Arguments
/// * `config` - The runner configuration to translate into CLI flags.
///
/// # Returns
/// A vector of arguments in the exact order `llama-server` expects them.
pub fn server_args(config: &Config) -> Vec<String> {
    vec![
        "--model".to_string(),
        config.model_path.clone(),
        "--host".to_string(),
        config.host.clone(),
        "--port".to_string(),
        config.port.to_string(),
        "--ctx-size".to_string(),
        config.ctx_size.to_string(),
        "--n-gpu-layers".to_string(),
        config.n_gpu_layers.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_committed_config() {
        let raw = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/config/runner.toml"));
        let config: Config = toml::from_str(raw).expect("committed config must parse");
        assert_eq!(
            config,
            Config {
                model_path: "models/qwen2.5-coder-7b-instruct-q4_k_m.gguf".to_string(),
                host: "127.0.0.1".to_string(),
                port: 8080,
                ctx_size: 8192,
                n_gpu_layers: 99,
            }
        );
    }

    #[test]
    fn round_trips_through_toml() {
        let config = Config {
            model_path: "models/other.gguf".to_string(),
            host: "127.0.0.1".to_string(),
            port: 9090,
            ctx_size: 4096,
            n_gpu_layers: 10,
        };
        let serialized = toml::to_string(&config).expect("serialize");
        let parsed: Config = toml::from_str(&serialized).expect("parse");
        assert_eq!(config, parsed);
    }

    #[test]
    fn builds_server_args_in_order() {
        let config = Config {
            model_path: "models/m.gguf".to_string(),
            host: "127.0.0.1".to_string(),
            port: 8080,
            ctx_size: 8192,
            n_gpu_layers: 99,
        };
        let args = server_args(&config);
        assert_eq!(
            args,
            vec![
                "--model".to_string(),
                "models/m.gguf".to_string(),
                "--host".to_string(),
                "127.0.0.1".to_string(),
                "--port".to_string(),
                "8080".to_string(),
                "--ctx-size".to_string(),
                "8192".to_string(),
                "--n-gpu-layers".to_string(),
                "99".to_string(),
            ]
        );
    }

    #[test]
    fn validate_config_accepts_ipv4_loopback() {
        let config = Config {
            model_path: "models/m.gguf".to_string(),
            host: "127.0.0.1".to_string(),
            port: 8080,
            ctx_size: 8192,
            n_gpu_layers: 99,
        };
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn validate_config_accepts_ipv6_loopback() {
        let config = Config {
            model_path: "models/m.gguf".to_string(),
            host: "::1".to_string(),
            port: 8080,
            ctx_size: 8192,
            n_gpu_layers: 99,
        };
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn validate_config_rejects_wildcard_host() {
        let config = Config {
            model_path: "models/m.gguf".to_string(),
            host: "0.0.0.0".to_string(),
            port: 8080,
            ctx_size: 8192,
            n_gpu_layers: 99,
        };
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn validate_config_rejects_lan_host() {
        let config = Config {
            model_path: "models/m.gguf".to_string(),
            host: "192.168.1.50".to_string(),
            port: 8080,
            ctx_size: 8192,
            n_gpu_layers: 99,
        };
        assert!(validate_config(&config).is_err());
    }
}
