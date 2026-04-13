#![warn(clippy::all)]
#![deny(warnings)]
// Thin crate entrypoint for the Bitwarden TUI.

mod app;
mod browser;
mod clipboard;
mod config;
mod create;
mod domain;
mod generator;
mod rbw;

use std::{fs, path::PathBuf};

use anyhow::Result;
use clap::{Parser, ValueEnum};

use crate::{
    clipboard::{DEFAULT_CLIPBOARD_TIMEOUT_SECONDS, run_clipboard_helper},
    domain::Scope,
};

/// CLI options for the interactive app.
#[derive(Debug, Parser)]
#[command(name = "bitwarden-tui")]
struct Cli {
    /// Current page URL used for site filtering and draft defaults.
    #[arg(long, default_value = "")]
    url: String,

    /// Optional username context used for ranking and draft defaults.
    #[arg(long, default_value = "")]
    username: String,

    /// Initial filter mode.
    #[arg(long, value_enum)]
    scope: Option<CliScope>,

    /// Optional file path where selection JSON should be written.
    #[arg(long)]
    output_file: Option<PathBuf>,

    /// Internal detached clipboard helper mode.
    #[arg(long, hide = true)]
    clear_clipboard_helper: bool,

    /// Internal clipboard helper token.
    #[arg(long, default_value = "", hide = true)]
    token: String,

    /// Internal clipboard helper digest.
    #[arg(long, default_value = "", hide = true)]
    clipboard_digest: String,

    /// Internal clipboard helper timeout in seconds.
    #[arg(long, default_value_t = DEFAULT_CLIPBOARD_TIMEOUT_SECONDS, hide = true)]
    timeout: u64,
}

/// Accepted values for the user-facing scope flag.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliScope {
    Vault,
    Site,
}

impl From<CliScope> for Scope {
    fn from(value: CliScope) -> Self {
        match value {
            CliScope::Vault => Scope::Vault,
            CliScope::Site => Scope::Site,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.clear_clipboard_helper {
        std::process::exit(run_clipboard_helper(
            &cli.token,
            &cli.clipboard_digest,
            cli.timeout,
        )?);
    }
    let has_url = !cli.url.is_empty();
    let scope =
        cli.scope
            .map(Into::into)
            .unwrap_or(if has_url { Scope::Site } else { Scope::Vault });
    let result = app::run(cli.url, cli.username, scope, cli.output_file.is_some())?;
    if let Some(path) = cli.output_file {
        fs::write(path, result)?;
    }
    Ok(())
}
