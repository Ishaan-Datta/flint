mod metadata;
mod treesitter;

use metadata::*;
use treesitter::*;

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand};
use std::{env, error::Error, fs};

// TODO: move this to a trace init module
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Manage flake inputs
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(arg_required_else_help = false)]
    Run {
        /// Run Imbue on all workspace members, exclusive with -p/--package
        #[arg(short, long, default_value_t = false)]
        fix: bool,
        /// Run Imbue without checking for changes to .proto files
        #[arg(short, long, default_value_t = false)]
        dry_run: bool,
        /// Run Imbue without checking for changes to .proto files
        #[arg(short, long, default_value_t = false)]
        check_updates: bool,
        /// Disable logging, just report status code
        #[arg(short, long, default_value_t = false)]
        quiet: bool,
        /// Add debug logging
        #[arg(short, long, default_value_t = false)]
        verbose: bool,
        /// Path to the flake.nix file (ex. folder) (should be folder end, not ending with .nix)
        #[arg(short, long)]
        path: Option<String>,
    },
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    let indicatif_layer = IndicatifLayer::new();
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(indicatif_layer.get_stderr_writer()))
        .with(indicatif_layer)
        .init();

    match &cli.command {
        Commands::Run {
            fix,
            dry_run,
            check_updates,
            quiet,
            verbose,
            path,
        } => {
            // if no fix or check_updates, it will jsut check for duplicate entries, and report them

            // should validate that you cant do both check_updates and dry_run or check_updates and fix
            // if *workspace && package.is_some() {
            //     let mut cmd = Cli::command();
            //     cmd.error(
            //         ErrorKind::ArgumentConflict,
            //         "Can't apply both --workspace and --package flags for imbue command",
            //     )
            //     .exit();
            // }

            // // path for fixing:
            // let source = fs::read_to_string("flake.nix")?;
            // let rewritten = rewrite_flake_inputs(&source, &get_input_deps()?)?;
            // println!("{rewritten}");
            // fs::write("temp.nix", rewritten)?;

            // // path for getting url times:
            // let input_ruls = get_input_urls()?;
            // get_modified_times(input_ruls);

            print_input_summary()?;

            // for each path, get the error if there is one, print it and then exit 1
        }
    }

    Ok(())
}

// should make a config map god object and make everything impl that?
// god object gets initialized, reads flags, if no flag defining a var, check env vars then default value for things
// like the duration before notifying about an update
