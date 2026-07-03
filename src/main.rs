mod config;
mod error;
mod process;
mod verify;

use error::RunnerError;
use std::path::PathBuf;

/// The parsed top-level CLI command and its options.
enum Command {
    /// Spawns and supervises `llama-server`.
    Start { config_path: PathBuf },
    /// Stops the supervised `llama-server` process. Always targets the pidfile at
    /// its fixed location; does not accept `--config`.
    Stop,
    /// Reports the health of the supervised `llama-server` process, using the
    /// host/port read from the given configuration file.
    Status { config_path: PathBuf },
    /// Exercises the OpenAI-compatible endpoints, optionally checking for network
    /// egress, using the host/port read from the given configuration file.
    Verify { egress: bool, config_path: PathBuf },
}

/// Consumes the next argument as the value for `--config`, converting it to a path.
///
/// # Arguments
/// * `iter` - The argument iterator positioned just after the `--config` flag.
///
/// # Returns
/// The parsed config path, or a `RunnerError::Config` if no value follows `--config`.
fn take_config_value(iter: &mut std::slice::Iter<'_, String>) -> Result<PathBuf, RunnerError> {
    let value = iter
        .next()
        .ok_or_else(|| RunnerError::Config("--config requires a value".to_string()))?;
    Ok(PathBuf::from(value))
}

/// Parses raw CLI arguments (excluding the program name) into a `Command`.
///
/// # Arguments
/// * `args` - The process arguments, excluding `argv[0]`.
///
/// # Returns
/// The parsed `Command`, or a `RunnerError` if the arguments are missing or malformed.
fn parse_args(args: &[String]) -> Result<Command, RunnerError> {
    let mut iter = args.iter();
    let subcommand = iter
        .next()
        .ok_or_else(|| RunnerError::Config("missing subcommand".to_string()))?;

    match subcommand.as_str() {
        "start" => {
            let mut config_path = PathBuf::from(config::DEFAULT_CONFIG_PATH);
            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--config" => {
                        config_path = take_config_value(&mut iter)?;
                    }
                    other => {
                        return Err(RunnerError::Config(format!(
                            "unrecognized argument: {other}"
                        )));
                    }
                }
            }
            Ok(Command::Start { config_path })
        }
        "stop" => {
            if let Some(other) = iter.next() {
                return Err(RunnerError::Config(format!(
                    "unrecognized argument: {other}"
                )));
            }
            Ok(Command::Stop)
        }
        "status" => {
            let mut config_path = PathBuf::from(config::DEFAULT_CONFIG_PATH);
            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--config" => {
                        config_path = take_config_value(&mut iter)?;
                    }
                    other => {
                        return Err(RunnerError::Config(format!(
                            "unrecognized argument: {other}"
                        )));
                    }
                }
            }
            Ok(Command::Status { config_path })
        }
        "verify" => {
            let mut egress = false;
            let mut config_path = PathBuf::from(config::DEFAULT_CONFIG_PATH);
            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--egress" => egress = true,
                    "--config" => {
                        config_path = take_config_value(&mut iter)?;
                    }
                    other => {
                        return Err(RunnerError::Config(format!(
                            "unrecognized argument: {other}"
                        )));
                    }
                }
            }
            Ok(Command::Verify {
                egress,
                config_path,
            })
        }
        other => Err(RunnerError::Config(format!("unknown subcommand: {other}"))),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let command = match parse_args(&args) {
        Ok(command) => command,
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(2);
        }
    };

    let result = match command {
        Command::Start { config_path } => process::handle_start(&config_path),
        Command::Stop => process::handle_stop(),
        Command::Status { config_path } => process::handle_status(&config_path),
        Command::Verify {
            egress,
            config_path,
        } => verify::handle_verify(egress, &config_path),
    };

    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
