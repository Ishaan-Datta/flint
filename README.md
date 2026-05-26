<div align="center">
  <h1 id="header">FLint</h1>
  <h3>Flake Lock lint</h3>

  <a href="https://github.com/Ishaan-Datta/flint/actions"> <img src="https://github.com/Ishaan-Datta/flint/actions/workflows/build/badge.svg" alt="Build Status"></a>
  <a href="https://github.com/Ishaan-Datta/flint/blob/main/LICENSE"><img src="https://img.shields.io/github/license/Ishaan-Datta/flint?label=License" alt="License"/>
  <a href="https://deps.rs/repo/github/Ishaan-Datta/flint"><img src="https://deps.rs/repo/github/Ishaan-Datta/flint/status.svg" alt="Dependency Status"/>
  <br/>

  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./assets/intro.gif">
    <source media="(prefers-color-scheme: light)" srcset="./assets/intro.gif">
    <img alt="Intro GIF" src="./assets/intro.gif">
  </picture>

  <a href="#what-does-it-do">Synopsis</a> | <a href="#features">Features</a> | <a href="#usage">Usage</a> | <a href="#contributing">Contributing</a>
  <br/>
</div>

## What Does it Do?

`flint` is a small maintenance tool for Nix flakes, focused on keeping lock files
healthy and easy to reason about. It helps identify inputs that have fallen
behind upstream and highlights duplicated dependencies that can make a flake
larger, noisier, or harder to maintain than necessary.

The goal is to provide a simple, scriptable utility that fits naturally into
local development, pre-commit checks, direnv workflows, and continuous
integration. `flint` is intentionally narrow in scope: it does not try to replace
the Nix CLI, but instead handles a few recurring flake hygiene tasks with clear
output and safe defaults.

## Features

- **Stale input detection**: Highlight inputs that appear to have fallen behind their upstream sources, making it easier to keep flakes current.
- **Duplicate dependency detection**: Find repeated transitive dependencies that may be unnecessarily expanding the lock graph.
- **Automatic maintenance workflows**: Provide optional fix and update flows for both updating specific stale inputs and resolving duplicate dependencies.
- **AST-based fixes**: rewrite `flake.nix` with Tree-sitter syntax-aware manipulation instead of plain string replacement.
- **Safer writes**: fixed files are validated before replacement, can be backed up to `flake.nix.bak`, and are checked for unstaged git changes before being overwritten.
- **Scriptable output**: supports quiet mode for check-style usage and `--yes` for non-interactive write operations.

### Installation

As this package isn't published on Nixpkgs, for system installation you will need to add the flake input and add the package using either the overlay or default package:

`flake.nix`
```nix
{
  inputs = {
    flint = {
      url = "github:Ishaan-Datta/flint";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
  };
}
```

Adding it using the overlay:
```nix
{ inputs, pkgs, ... }:

{
  nixpkgs.overlays = [
    inputs.flint.overlays.default
  ];

  home.packages = [
    pkgs.flint
  ];
}
```

Adding it using the default package:
```nix
{ inputs, pkgs, ... }:

{
  home.packages = [
    inputs.flint.packages.${pkgs.stdenv.hostPlatform.system}.default
  ];
}
```

Flint also installs generated man pages and shell completions. Once the package is added, the flint manual page and supported shell completions are made available through the normal NixOS/Home-Manager profile mechanisms.

For trying the utility without installing:
```bash
nix run github:Ishaan-Datta/flint
```

## Usage

```bash
flint [OPTIONS] <COMMAND>
```

### Global options

| Option               | Environment variable | Default | Description                                             |
| -------------------- | -------------------- | ------- | ------------------------------------------------------- |
| `-p, --path <PATH>`  | `FLINT_FLAKE_PATH`   | `.`     | Directory containing `flake.nix`.                       |
| `-t, --timeout <MS>` | `FLINT_CMD_TIMEOUT`  | `25000` | Timeout for external commands, in milliseconds.         |
| `-y, --yes`          | `FLINT_OVERRIDE`     | `false` | Skip prompts before overwriting unstaged changes.       |
| `-v, --verbose...`   | `FLINT_LOG_LEVEL`    | `info`  | Increase tracing verbosity.                             |
| `-q, --quiet`        | `FLINT_LOG_LEVEL`    | `false` | Enable quiet/check mode and suppress normal output.     |

### Logging Verbosity and Quiet Mode

`flint` logs at `info` by default. `-v` enables debug logging, and `-vv`
enables trace logging. Additional `v`s remain at trace level:
```bash
flint -v stale
flint -vv stale
```

`-q/--quiet` is not a verbosity decrement. It is a hard quiet/check-mode flag:
normal logging is disabled and commands use check-style output behavior:
```bash
flint -q stale
flint -q duplicates
```

**Note:** If the program requires user input and the quiet flag is enabled without the override flag (`-y/--yes`), the command will fail (exit with code 1).

Precedence is:

1. CLI logging flags, if either `-v/--verbose` or `-q/--quiet` is present
2. `FLINT_LOG_LEVEL`, if set
3. default `info`

```bash
FLINT_LOG_LEVEL=trace flint -q stale
FLINT_LOG_LEVEL=off flint -v stale
```

The first command runs in quiet/check mode. The second command runs with debug
logging because -v overrides the environment value.


`FLINT_LOG_LEVEL` accepts normal `tracing_subscriber::EnvFilter` values such as
`error`, `warn`, `info`, `debug`, `trace`, `off`:

```bash
FLINT_LOG_LEVEL=trace flint stale   # Most Verbose
FLINT_LOG_LEVEL=debug flint stale
FLINT_LOG_LEVEL=info flint stale
FLINT_LOG_LEVEL=warn flint stale   
FLINT_LOG_LEVEL=error flint stale   # Least Verbose

FLINT_LOG_LEVEL=off flint stale     # Quiet/check mode
```

Only a bare `FLINT_LOG_LEVEL=off` enables quiet/check mode through the
environment. Values like `warn` and `error` reduce logging but do not enable
quiet/check mode.

### Global environment variables

- `FLINT_FLAKE_PATH`: default path for `--path`.
- `FLINT_CMD_TIMEOUT`: default timeout for external commands, in milliseconds.
- `FLINT_UPDATE_THRESHOLD`: default stale-input threshold, in seconds.
- `FLINT_LOG_LEVEL`: tracing filter used when no CLI logging flag is present. Bare `off` also enables quiet/check mode.
- `FLINT_OVERRIDE`: default choice for skipping interactive prompts

### Commands

#### `flint stale`

Check whether flake inputs are older than the configured update threshold.

<picture>
    <source media="(prefers-color-scheme: dark)" srcset="./assets/stale.gif">
    <source media="(prefers-color-scheme: light)" srcset="./assets/stale.gif">
    <img alt="Flint Stale GIF" src="./assets/stale.gif">
  </picture>

```bash
flint stale
flint stale --update-threshold 604800
FLINT_UPDATE_THRESHOLD=604800 flint stale
```

By default, `flint stale` classifies an input as stale when the remote
`lastModified` timestamp is more than 14 days newer than the timestamp recorded
in `flake.lock`.

To update stale inputs automatically:
```bash
flint stale --auto-update
```

For non-interactive updates:
```bash
flint stale --auto-update --yes
```

`--auto-update` checks for unstaged changes in `flake.lock` before writing unless `--yes` is set.

For pre-commit hooks, direnv activation checks, or CI:

```bash
flint -q stale
```

In quiet mode, `flint` exits non-zero as soon as it sees a stale input.

#### `flint duplicates`

Check for duplicated transitive inputs in the lock metadata.

<picture>
    <source media="(prefers-color-scheme: dark)" srcset="./assets/duplicates.gif">
    <source media="(prefers-color-scheme: light)" srcset="./assets/duplicates.gif">
    <img alt="Flint Duplicates GIF" src="./assets/duplicates.gif">
  </picture>

```bash
flint duplicates
```

When a duplicate is found, `flint` reports the input that can be consolidated and
the duplicated target currently referenced by the lock graph.

To rewrite `flake.nix` automatically:

```bash
flint duplicates --fix
```

`--fix` inserts missing follows declarations such as:

```bash
inputs.nixpkgs.follows = "nixpkgs";
```

For flake inputs already written as nested attrsets, `flint` inserts the new
line into the existing attrset. For flat URL bindings, it rewrites the binding
into an attrset before inserting follows declarations.

By default, a fix creates a backup at `flake.nix.bak` before replacing the
original file. To skip that backup:

```bash
flint duplicates --fix --no-backup
```

If `flake.nix` has unstaged git changes, flint asks before overwriting it. 
To skip the prompt:

```bash
flint duplicates --fix --yes
```

For CI or hook checks:

```bash
flint -q duplicates
```

In quiet mode, `flint` exits non-zero when duplicate dependencies are found.

## How it works

### Stale Input checks

`flint stale` resolves the requested flake path through `nix flake metadata`, reads the local `lastModified` values from the lock file, evaluates the input URLs from `flake.nix`, and then refreshes each input's remote flake metadata.

Each input is categorized as:
- `OUT OF DATE`: the remote timestamp exceeds the local timestamp by more than the configured threshold.
- `UP TO DATE`: the remote timestamp is within the threshold.
- `ERRORED`: the input could not be fetched or produced inconsistent metadata.

### Duplicate Input Dependency checks

`flint duplicates` evaluates the flake lock graph and looks for root flake inputs whose transitive dependencies point at generated duplicate nodes like `<dependency>_2` through `<dependency>_99`.

When `--fix` is enabled, it edits `flake.nix` with a Nix Tree-sitter AST, validates the generated file with Nix metadata evaluation, and then replaces the original file only after the safety checks pass.

## Contributing

If you have bugs, feature requests, or want to contribute, feel free to create an issue or pull request! 

### Entering the development environment:

Use either:
```
nix develop
```

or assuming you have [direnv](https://github.com/direnv/direnv-vscode) and [nix-direnv](https://github.com/nix-community/nix-direnv) setup:
```
direnv allow
```

### Useful commands:
```
just fmt          # format Rust and TOML
just fmt-check    # check Rust format
just taplo-check  # check TOML format
just cargo-check  # run cargo check with warnings denied
just cargo-fix    # apply clippy fixes
just test         # run cargo-nextest
```

### Development Tooling:

The repository uses `lefthook` for local automation:
- pre-commit: run formatting and stage fixed files
- pre-push: check Rust formatting, TOML formatting, and Rust compilation

### Structure

- `flint/src/cli`: Clap argument definitions and command dispatch.
- `flint/src/metadata`: Nix metadata queries, stale checks, source URL lookup, flake-path resolution, and stale input updates.
- `flint/src/ast`: Tree-sitter-based flake.nix edits for duplicate input consolidation.
- `flint/src/modified_time`: input status types and terminal display helpers.
- `flint/src/command`: external command execution with timeout handling.
- `flint/src/errors`: typed errors for command, parse, status, Tree-sitter, and write failures.
- `xtask`: helper tasks for generated shell completions and man pages.

## Attributions

I built this project as an alternative to [locker](https://github.com/tgirlcloud/locker) by [tgirlcloud](https://github.com/tgirlcloud) and [flint](https://github.com/NotAShelf/flint) by [NotAShelf](https://github.com/NotAShelf) to better suit usage with dev environments/CI (ex. pre-commit hooks, flake shell hooks), and add options for resolving the discovered flake issues with Treesitter.

Many features of this project were ~~blatantly stolen~~ heavily inspired by [nh](https://github.com/nix-community/nh), including nix packaging, development tooling, directory structure, and [cargo-xtask](https://github.com/matklad/cargo-xtask ) setup for manpage and shell completions. The display summary formatting for flake input status was copied from [dix](https://github.com/faukah/dix).

## License

This project is licensed under GPLv3. See [LICENSE](./LICENSE).

