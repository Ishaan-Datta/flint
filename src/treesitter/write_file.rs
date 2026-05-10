// this is where the atomic rewrite and validate command should happen

const VALIDATE_FILE_CMD: &str = r#"nix eval --json --impure --expr '(import ./flake.nix).inputs'"#;

// validating syntax: nix-instantiate --parse ./flake.nix >/dev/null

// validating the flake.nix is actually valid: nix flake check --no-build --no-write-lock-file .

// write to a temporary file temp.nix
// validate it
// rename the main one to flake.nix.bak
// replace the main one by renaming temp.nix to flake.nix
