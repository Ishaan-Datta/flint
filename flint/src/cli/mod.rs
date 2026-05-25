mod verbosity;
use std::{process::exit, time::Duration};

use anstyle::Style;
use clap::{ArgAction, Parser, Subcommand, ValueHint, builder::Styles};
use color_eyre::Result;
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};
use verbosity::*;

use crate::{
    ast::write::rewrite_flake_inputs,
    metadata::{check_flake_inputs, get_flake_path},
};

const fn make_style() -> Styles {
    Styles::plain().header(Style::new().bold()).literal(
        Style::new()
            .bold()
            .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Cyan))),
    )
}

#[derive(Parser, Debug)]
#[command(
    version,
    about,
    long_about = None,
    styles=make_style(),
    propagate_version = false,
    disable_help_subcommand = true,
    help_template = "
{name} {version}
{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}
"
)]
/// Flake Lock lint
pub struct Cli {
    /// Path to the directory containing flake.nix
    ///
    /// May be relative or absolute, ex. "." or "~/flake_path"
    /// Must be a directory, not a .nix file.
    #[arg(
        short, long,
        default_value_t = {
            ".".to_string()
        },
        env = "FLINT_FLAKE_PATH",
        global = true,
        value_hint = ValueHint::DirPath
    )]
    path:          String,
    /// Command timeout duration in milliseconds
    #[arg(
        short,
        long,
        default_value_t = 25_000,
        env = "FLINT_CMD_TIMEOUT",
        global = true
    )]
    timeout:       u64,
    /// Don't show an interactive prompt if the update operation will override
    /// existing changes
    ///
    /// Considers "existing change" if the file has changes tracked by git that
    /// are not staged/committed
    #[arg(
        short = 'y',
        long = "yes",
        default_value_t = false,
        global = true,
        env = "FLINT_OVERRIDE"
    )]
    override_bool: bool,
    #[command(subcommand)]
    command:       Commands,
    #[command(flatten)]
    pub verbosity: Verbosity,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(
        arg_required_else_help = false,
        after_help = "\
Examples:
  flint stale
  flint -q stale
  flint stale --update-threshold 604800
  flint stale --auto-update
  flint stale --auto-update --yes
  FLINT_UPDATE_THRESHOLD=604800 flint stale
  FLINT_LOG_LEVEL=off flint stale
"
    )]
    /// Check flake inputs for updates
    #[command(display_order = 2)]
    Stale {
        /// Threshold for classifying inputs as "stale" in seconds
        #[arg(
            short,
            long,
            default_value_t = 1209600,
            env = "FLINT_UPDATE_THRESHOLD"
        )]
        update_threshold: u64,
        /// Auto update inputs that are classified as stale
        #[arg(short, long, default_value_t = false)]
        auto_update:      bool,
    },
    #[command(
        arg_required_else_help = false,
        after_help = "\
Examples:
  flint duplicates
  flint -q duplicates
  flint duplicates --fix
  flint duplicates --fix --yes
  flint duplicates --fix --no-backup
  FLINT_LOG_LEVEL=off flint duplicates
"
    )]
    /// Check flake inputs for redundant transitive dependencies
    #[command(display_order = 1)]
    Duplicates {
        /// Apply flake input consolidation
        ///
        /// Uses Tree-sitter AST parsing to insert
        /// `inputs.<transitive_input>.follows = "<transitive_input>"` to
        /// de-dupe extra input instances
        #[arg(short, long, default_value_t = false)]
        fix:    bool,
        /// Disable copying the original flake file contents to `flake.nix.bak`
        /// as a backup
        #[arg(long = "no-backup", default_value_t = true, action = ArgAction::SetFalse)]
        backup: bool,
    },
}

pub fn main() -> Result<()> {
    let cli = Cli::parse();

    let logging = cli.verbosity.resolve()?;
    let quiet = logging.quiet;

    let indicatif_layer = IndicatifLayer::new();
    let format = fmt::layer()
        .with_writer(indicatif_layer.get_stderr_writer())
        .with_ansi_sanitization(false)
        .with_level(false)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .without_time()
        .compact();

    tracing_subscriber::registry()
        .with(indicatif_layer)
        .with(format)
        .with(logging.filter)
        .init();

    let timeout = Duration::from_millis(cli.timeout);
    let override_bool = cli.override_bool;

    let flake_dir_path = match get_flake_path(&cli.path, timeout) {
        Ok(val) => {
            tracing::info!("Resolved flake path to: {}", val.display());
            val
        },
        Err(e) => {
            tracing::error!("Failed resolving flake path: {e}");
            exit(1);
        },
    };

    match &cli.command {
        Commands::Duplicates { fix, backup } => {
            rewrite_flake_inputs(
                *fix,
                quiet,
                timeout,
                override_bool,
                *backup,
                &flake_dir_path,
            );
            exit(0);
        },
        Commands::Stale {
            update_threshold,
            auto_update,
        } => {
            let update_threshold = Duration::from_secs(*update_threshold);
            check_flake_inputs(
                update_threshold,
                timeout,
                quiet,
                *auto_update,
                override_bool,
                &flake_dir_path,
            );
            exit(0);
        },
    }
}
