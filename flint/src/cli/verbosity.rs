use std::{env, str::FromStr};

use clap::{ArgAction, Args};
use color_eyre::{Result, eyre::eyre};
use tracing_subscriber::{EnvFilter, filter::LevelFilter};

const LOG_ENV: &str = "FLINT_LOG_LEVEL";
const DEFAULT_LOG_LEVEL: LevelFilter = LevelFilter::INFO;

#[derive(Debug, Default, Clone, Copy, Args)]
#[command(about = None, long_about = None)]
pub struct Verbosity {
    /// Increase logging verbosity
    ///
    /// Can be repeated:
    /// -v enables debug logging
    /// -vv enables trace logging
    #[arg(
        short = 'v',
        long = "verbose",
        action = ArgAction::Count,
        global = true,
        conflicts_with = "quiet",
    )]
    verbose: u8,

    /// Suppress normal output and enable check-style quiet mode
    #[arg(
        short = 'q',
        long = "quiet",
        action = ArgAction::SetTrue,
        global = true,
        conflicts_with = "verbose",
    )]
    quiet: bool,
}

impl Verbosity {
    pub fn is_present(self) -> bool {
        self.quiet || self.verbose > 0
    }

    pub fn resolve(self) -> Result<LoggingConfig> {
        if let Some(level) = self.cli_level() {
            return Ok(LoggingConfig::from_level(level));
        }

        if let Some(value) = env::var_os(LOG_ENV) {
            let value = value
                .into_string()
                .map_err(|_| eyre!("{LOG_ENV} must be valid UTF-8"))?;

            if value.trim().is_empty() {
                return Ok(LoggingConfig::from_level(DEFAULT_LOG_LEVEL));
            }

            let filter = EnvFilter::try_new(&value)
                .map_err(|err| eyre!("invalid {LOG_ENV}={value:?}: {err}"))?;

            // Only a bare "off" env value enables quiet/check mode.
            // EnvFilter directives like "flint=off" only affect logging.
            let quiet = LevelFilter::from_str(&value)
                .map(|level| level == LevelFilter::OFF)
                .unwrap_or(false);

            return Ok(LoggingConfig { filter, quiet });
        }

        Ok(LoggingConfig::from_level(DEFAULT_LOG_LEVEL))
    }

    fn cli_level(self) -> Option<LevelFilter> {
        if self.quiet {
            Some(LevelFilter::OFF)
        } else if self.verbose > 0 {
            Some(match self.verbose {
                1 => LevelFilter::DEBUG,
                _ => LevelFilter::TRACE,
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub filter: EnvFilter,
    pub quiet:  bool,
}

impl LoggingConfig {
    fn from_level(level: LevelFilter) -> Self {
        Self {
            filter: EnvFilter::new(level.to_string()),
            quiet:  level == LevelFilter::OFF,
        }
    }
}
