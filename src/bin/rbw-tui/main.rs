#![warn(clippy::all)]
#![deny(warnings)]
// Thin crate entrypoint for the Bitwarden TUI.

mod app;
mod browser;
mod config;
mod domain;
mod form;
mod generator;
mod rbw;
mod text_input;

use std::{fs, path::PathBuf};

use anyhow::Result;
use clap::{Parser, ValueEnum};

use crate::domain::Scope;

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
            CliScope::Vault => Self::Vault,
            CliScope::Site => Self::Site,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let has_url = !cli.url.is_empty();
    let scope = cli
        .scope
        .map_or(if has_url { Scope::Site } else { Scope::Vault }, Into::into);
    let result =
        app::run(cli.url, cli.username, scope, cli.output_file.is_some())?;
    if let Some(path) = cli.output_file {
        fs::write(path, result)?;
    }
    Ok(())
}
