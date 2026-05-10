mod metadata;
mod treesitter;

use metadata::*;
use tracing::Level;
use treesitter::*;

use anstyle::Style;
use clap::{Parser, Subcommand};
use std::error::Error;
use std::process::exit;

use clap::builder::Styles;
use clap_verbosity_flag::InfoLevel;

use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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
    #[command(subcommand)]
    command: Commands,
}

/// flint duplicates, flint stale
#[derive(Debug, Subcommand)]
enum Commands {
    #[command(arg_required_else_help = false)]
    /// Check flake inputs for updates
    Stale {
        /// Path to the flake.nix file (ex. folder) (should be folder end, not ending with .nix)
        #[arg(short, long, default_value_t = String::from("PWD"), env = "FLINT_PATH", global = true)]
        path: String,
        /// Timeout duration in milliseconds
        #[arg(
            short,
            long,
            default_value_t = 15_000,
            env = "FLINT_CMD_TIMEOUT",
            global = true
        )]
        timeout: u64,
        #[arg(
            short,
            long,
            default_value_t = 1209600,
            env = "FLINT_UPDATE_THRESHOLD",
            conflicts_with_all(["fix", "dry_run"])
        )]
        update_threshold: u64,
    },
    Duplicates {
        /// Apply flake input consolidation
        #[arg(short, long, default_value_t = false)]
        fix: bool,
        /// Create the linted flake file as `temp.nix` for inspection
        #[arg(short, long, default_value_t = false, required_if_eq("fix", "true"))]
        dry_run: bool,
    },
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    // should try making it use a specific env var not "default RUST_LOG" (FLINT_LOG_LEVEL)
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::from(cli.verbosity.tracing_level_filter().to_string()));

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

    let quiet = cli.verbosity.tracing_level().unwrap() >= Level::WARN;

    // TODO: pass through the flake path...
    match &cli.command {
        Commands::Duplicates {
            fix,
            dry_run,
            path,
            timeout,
        } => {
            print_input_summary(*update_threshold, *timeout, quiet);
            // do if fix, cehck the git status of the flake, dont want to modify the changes unless specified -> make arg for
            exit(1);
        }
        Commands::Stale {
            path,
            timeout,
            update_threshold,
        } => {
            print_input_summary(*update_threshold, *timeout, quiet);
        }
    }

    Ok(())
}
