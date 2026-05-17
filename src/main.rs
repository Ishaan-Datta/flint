use flint::ast::write::rewrite_flake_inputs;
use flint::metadata::*;

use std::time::Duration;
use anstyle::Style;
use clap::builder::Styles;
use clap::{Parser, Subcommand};
use clap_verbosity_flag::InfoLevel;
use std::error::Error;
use std::process::exit;
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use clap_verbosity_flag::tracing::LevelFilter;

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
    help_template = "
{name} {version}
{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}
"
)]
/// Flake Lock lint
struct Cli {
    #[command(flatten)]
    pub verbosity: clap_verbosity_flag::Verbosity<InfoLevel>,
    /// Path to the flake.nix file (ex. folder) (should be folder, not ending with .nix)
    /// Relative or absolute, ex. "." or "~/flake_path"
    #[arg(
        short, long,
        default_value_t = { 
            ".".to_string()
        },
        env = "FLINT_FLAKE_PATH",
        global = true
    )]
    path: String,
    /// Timeout duration in milliseconds
    #[arg(
        short,
        long,
        default_value_t = 25_000,
        env = "FLINT_CMD_TIMEOUT",
        global = true
    )]
    timeout: u64,
    #[command(subcommand)]
    command: Commands,
}

// TODO: add example values for each of the args that are the default values?
#[derive(Debug, Subcommand)]
enum Commands {
    #[command(arg_required_else_help = false)]
    /// Check flake inputs for updates
    Stale {
        /// Threshold for classifying inputs as "stale" in seconds
        #[arg(short, long, default_value_t = 1209600, env = "FLINT_UPDATE_THRESHOLD")]
        update_threshold: u64,
    },
    /// Check flake inputs for redundant transitive dependencies
    Duplicates {
        /// Apply flake input consolidation
        #[arg(short, long, default_value_t = false)]
        fix: bool,
        /// Don't show an interactive prompt if the flake renaming will override existing changes
        #[arg(short, long = "override", default_value_t = false)]
        override_bool: bool,
        /// Rename the original flake file to `flake.nix.bak` for restoring later
        #[arg(short, long, default_value_t = true)]
        backup: bool,
    },
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    let mut filter = EnvFilter::try_from_env("FLINT_LOG_LEVEL")
        .unwrap_or_else(|_| EnvFilter::from(cli.verbosity.tracing_level_filter().to_string()));
    let mut quiet = false;

    // Work-around since clap CLI treats -q/--quiet as a level decrement instead of a silence
    if cli.verbosity.is_present() && cli.verbosity.tracing_level_filter() <= LevelFilter::WARN {
        filter = EnvFilter::new("off");
        quiet = true;
    }

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

    let flake_dir_path = match get_flake_path(&cli.path.clone(), timeout) {
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
        Commands::Duplicates { fix, override_bool, backup } => {
            rewrite_flake_inputs(*fix, quiet, timeout, *override_bool, *backup, flake_dir_path, );
            exit(0);
        }
        Commands::Stale { update_threshold } => {
            let update_threshold = Duration::from_secs(*update_threshold);
            print_input_summary(update_threshold, timeout, quiet, flake_dir_path);
            exit(0);
        }
    }
}
