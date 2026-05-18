use crate::ast::write::rewrite_flake_inputs;
use crate::metadata::{get_flake_path, check_flake_inputs};

use std::time::Duration;
use anstyle::Style;
use clap::builder::Styles;
use clap::{Parser, Subcommand, ValueHint};
use clap_verbosity_flag::InfoLevel;
use std::process::exit;
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use clap_verbosity_flag::tracing::LevelFilter;
use clap::ArgAction;
use color_eyre::Result;

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
    /// May be relative or absolute, ex. "." or ``~/flake_path``
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
    path: String,
    /// Command timeout duration in milliseconds
    #[arg(
        short,
        long,
        default_value_t = 25_000,
        env = "FLINT_CMD_TIMEOUT",
        global = true
    )]
    timeout: u64,
    /// Don't show an interactive prompt if the update operation will override existing changes
    ///
    /// Considers "existing change" if the file has changes tracked by git that are not staged/commited
    #[arg(short, long = "override", default_value_t = false, global = true)]
    override_bool: bool,
    #[command(subcommand)]
    command: Commands,
    #[command(flatten)]
    pub verbosity: clap_verbosity_flag::Verbosity<InfoLevel>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(arg_required_else_help = false, after_help = "\
Examples:
  flint stale
  flint stale --update-threshold 604800
  flint stale --auto-update
  FLINT_UPDATE_THRESHOLD=604800 flint stale
")]
    /// Check flake inputs for updates
    #[command(display_order = 2)]
    Stale {
        /// Threshold for classifying inputs as "stale" in seconds
        #[arg(short, long, default_value_t = 1209600, env = "FLINT_UPDATE_THRESHOLD")]
        update_threshold: u64,
        /// Auto update inputs that are classified as stale
        #[arg(short, long, default_value_t = false)]
        auto_update: bool,
    },
    #[command(arg_required_else_help = false, after_help = "\
Examples:
  flint duplicates
  flint duplicates --fix
  flint duplicates --fix --override
  flint duplicates --fix --no-backup
")]
    /// Check flake inputs for redundant transitive dependencies
    #[command(display_order = 1)]
    Duplicates {
        /// Apply flake input consolidation
        /// 
        /// Uses Treesitter AST parsing to inserting `inputs.<transitive_input>.follows = "<transitive_input>"` to de-dupe extra input instances
        #[arg(short, long, default_value_t = false)]
        fix: bool,
        /// Rename the original flake file to `flake.nix.bak` as a backup
        #[arg(short, long, default_value_t = true, action = ArgAction::Set)]
        backup: bool,
    },
}

pub fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut filter = EnvFilter::try_from_env("FLINT_LOG_LEVEL")
        .unwrap_or_else(|_| EnvFilter::from(cli.verbosity.tracing_level_filter().to_string()));

    // Work-around since clap CLI treats -q/--quiet as a level decrement instead of a silence
    let quiet = if cli.verbosity.is_present() && cli.verbosity.tracing_level_filter() <= LevelFilter::WARN { 
        filter = EnvFilter::new("off"); true 
    } else { 
        false 
    };

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
        .with(filter)
        .init();

    let timeout = Duration::from_millis(cli.timeout);
    let override_bool = cli.override_bool;

    let flake_dir_path = match get_flake_path(&cli.path, timeout) {
        Ok(val) => {
            tracing::info!("> Resolved flake path to: {}", val.display());
            val
        },
        Err(e) => {
            tracing::error!("Failed resolving flake path: {e}");
            exit(1);
        }
    };

    match &cli.command {
        Commands::Duplicates { fix, backup } => {
            rewrite_flake_inputs(*fix, quiet, timeout, override_bool, *backup, &flake_dir_path, );
            exit(0);
        }
        Commands::Stale { update_threshold, auto_update } => {
            let update_threshold = Duration::from_secs(*update_threshold);
            check_flake_inputs(update_threshold, timeout, quiet, *auto_update, override_bool, &flake_dir_path);
            exit(0);
        }
    }
}
