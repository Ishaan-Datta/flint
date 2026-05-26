#![allow(dead_code)]

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use flint::metadata::get_input_urls;
use tempfile::{TempDir, tempdir};

pub(crate) fn make_flake_file(
    flake_content: &str,
    temp_dir: &TempDir,
) -> Result<PathBuf, anyhow::Error> {
    let flake_file_path = temp_dir.path().join("flake.nix");
    std::fs::write(&flake_file_path, flake_content)?;
    Ok(flake_file_path)
}

pub(crate) fn make_lock_file(
    lock_content: &str,
    temp_dir: &TempDir,
) -> Result<PathBuf, anyhow::Error> {
    let lock_file_path = temp_dir.path().join("flake.lock");
    std::fs::write(&lock_file_path, lock_content)?;
    Ok(lock_file_path)
}

pub(crate) fn run_git_command(
    dir_path: impl AsRef<Path>,
    args: &[&str],
) -> Result<(), anyhow::Error> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir_path.as_ref())
        .output()
        .with_context(|| format!("failed to run `git {}`", args.join(" ")))?;

    if !output.status.success() {
        bail!(
            "`git {}` failed:\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    Ok(())
}

pub(crate) fn make_git_directory() -> Result<TempDir, anyhow::Error> {
    let dir = tempdir().context("failed to create temporary directory")?;
    run_git_command(dir.path(), &["init"])?;
    Ok(dir)
}

pub(crate) fn stage_git_file(
    dir_path: impl AsRef<Path>,
    file_name: &str,
) -> Result<(), anyhow::Error> {
    run_git_command(dir_path, &["add", file_name])
}

pub(crate) fn compare_file_lines(expected: &str, result: &str) {
    let expected_lines: Vec<&str> =
        expected.trim_start_matches('\n').lines().collect();
    let result_lines: Vec<&str> =
        result.trim_start_matches('\n').lines().collect();

    for (index, (exp_line, res_line)) in
        expected_lines.iter().zip(result_lines.iter()).enumerate()
    {
        assert_eq!(
            exp_line, res_line,
            "Mismatch on line {}:\n  expected: {:?}\n  got:      {:?}\n\nFull \
             expected:\n{}\n\nFull result:\n{}",
            index, exp_line, res_line, expected, result
        );
    }
}

pub(crate) fn assert_file_matches(
    path: impl AsRef<std::path::Path>,
    expected: &str,
) -> Result<(), anyhow::Error> {
    let actual = fs::read_to_string(path.as_ref()).with_context(|| {
        format!("failed to read {}", path.as_ref().display())
    })?;

    compare_file_lines(expected, &actual);

    assert_eq!(
        expected.trim_start_matches('\n').lines().count(),
        actual.trim_start_matches('\n').lines().count(),
        "file had unexpected extra or missing \
         lines:\n\nexpected:\n{}\n\nactual:\n{}",
        expected,
        actual,
    );

    Ok(())
}

pub(crate) fn edits(pairs: &[(&str, &[&str])]) -> HashMap<String, Vec<String>> {
    pairs
        .iter()
        .map(|(input, lines)| {
            (
                (*input).to_owned(),
                lines.iter().map(|line| (*line).to_owned()).collect(),
            )
        })
        .collect()
}

pub(crate) fn assert_flake_eq(expected: &str, actual: &str) {
    compare_file_lines(expected, actual);

    assert_eq!(
        expected.trim_start_matches('\n').lines().count(),
        actual.trim_start_matches('\n').lines().count(),
        "flake had unexpected extra or missing \
         lines:\n\nexpected:\n{}\n\nactual:\n{}",
        expected,
        actual,
    );
}

pub(crate) fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(crate) fn assert_single_input_url(
    input_name: &str,
    url: &str,
) -> Result<()> {
    let dir = tempdir()?;
    let flake_contents = format!(
        r#"
{{
  description = "single input provider fixture";

  inputs = {{
    "{input_name}" = {{
      url = "{url}";
    }};
  }};

  outputs = {{ self, ... }}: {{ }};
}}
"#,
        input_name = input_name,
        url = url,
    );

    make_flake_file(&flake_contents, &dir)?;

    let urls = get_input_urls(TIMEOUT, dir.path())?;

    let expected = HashMap::from([(input_name.to_string(), url.to_string())]);

    assert_eq!(urls, expected);

    Ok(())
}

pub(crate) const TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) const SHORT_FLAKE_CONTENT: &str = r#"
{
  description = "integration test replacement flake";

  outputs = { self }: { };
}
"#;

pub(crate) const INVALID_FLAKE_CONTENT: &str = r#"
{
  description = "invalid flake";

  outputs = { self }: {
"#;

pub(crate) const VALID_FLAKE_CONTENT: &str = r#"
{
  description = "NixOS System Configuration";

  outputs =
    inputs@{ flake-parts, ... }:
    let
      inherit (inputs.nixpkgs.lib.fileset) toList fileFilter;
      import-tree =
        path:
        toList (fileFilter (file: file.hasExt "nix" && !(inputs.nixpkgs.lib.hasPrefix "_" file.name)) path);
    in
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = import inputs.systems;
      debug = true;
      imports = import-tree ./modules;
    };

  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixos-unstable/nixexprs.tar.xz";
    nixpkgs-stable.url = "https://channels.nixos.org/nixos-25.11/nixexprs.tar.xz";
    systems.url = "github:nix-systems/default";

    flake-utils = {
      url = "github:numtide/flake-utils";
      inputs.systems.follows = "systems";
    };

    impermanence = {
      url = "github:nix-community/impermanence";
      inputs.home-manager.follows = "home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    home-manager = {
      url = "github:nix-community/home-manager/master";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    nix-index-database = {
      url = "github:nix-community/nix-index-database";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    disko = {
      url = "github:nix-community/disko/latest";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-parts = {
      url = "github:hercules-ci/flake-parts/main";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };

    nixos-generators = {
      url = "github:nix-community/nixos-generators";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    vicinae = {
      url = "github:vicinaehq/vicinae";
    };

    vicinae-extensions = {
      url = "github:vicinaehq/extensions";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    yazi.url = "github:sxyazi/yazi";

    wrappers = {
      url = "github:Lassulus/wrappers";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    wrapper-modules.url = "github:BirdeeHub/nix-wrapper-modules";
  };
}
"#;

pub(crate) const VALID_FLAKE_LOCK_CONTENT: &str = r#"
{
  "nodes": {
    "disko": {
      "inputs": {
        "nixpkgs": [
          "nixpkgs"
        ]
      },
      "locked": {
        "lastModified": 1768920986,
        "narHash": "sha256-CNzzBsRhq7gg4BMBuTDObiWDH/rFYHEuDRVOwCcwXw4=",
        "owner": "nix-community",
        "repo": "disko",
        "rev": "de5708739256238fb912c62f03988815db89ec9a",
        "type": "github"
      },
      "original": {
        "owner": "nix-community",
        "ref": "latest",
        "repo": "disko",
        "type": "github"
      }
    },
    "flake-compat": {
      "flake": false,
      "locked": {
        "lastModified": 1767039857,
        "narHash": "sha256-vNpUSpF5Nuw8xvDLj2KCwwksIbjua2LZCqhV1LNRDns=",
        "owner": "NixOS",
        "repo": "flake-compat",
        "rev": "5edf11c44bc78a0d334f6334cdaf7d60d732daab",
        "type": "github"
      },
      "original": {
        "owner": "NixOS",
        "repo": "flake-compat",
        "type": "github"
      }
    },
    "flake-parts": {
      "inputs": {
        "nixpkgs-lib": [
          "nixpkgs"
        ]
      },
      "locked": {
        "lastModified": 1776451893,
        "narHash": "sha256-fQa2toT19Mp5rZRq2eEwccdfsb5zSsiu8Ie2B02bmE0=",
        "owner": "Ishaan-Datta",
        "repo": "flake-parts",
        "rev": "eb98f6d33615fb489df68401770bcf645862f7de",
        "type": "github"
      },
      "original": {
        "owner": "Ishaan-Datta",
        "ref": "master",
        "repo": "flake-parts",
        "type": "github"
      }
    },
    "flake-utils": {
      "inputs": {
        "systems": [
          "systems"
        ]
      },
      "locked": {
        "lastModified": 1731533236,
        "narHash": "sha256-l0KFg5HjrsfsO/JpG+r7fRrqm12kzFHyUHqHCVpMMbI=",
        "owner": "numtide",
        "repo": "flake-utils",
        "rev": "11707dc2f618dd54ca8739b309ec4fc024de578b",
        "type": "github"
      },
      "original": {
        "owner": "numtide",
        "repo": "flake-utils",
        "type": "github"
      }
    },
    "flake-utils_2": {
      "inputs": {
        "systems": "systems_4"
      },
      "locked": {
        "lastModified": 1731533236,
        "narHash": "sha256-l0KFg5HjrsfsO/JpG+r7fRrqm12kzFHyUHqHCVpMMbI=",
        "owner": "numtide",
        "repo": "flake-utils",
        "rev": "11707dc2f618dd54ca8739b309ec4fc024de578b",
        "type": "github"
      },
      "original": {
        "owner": "numtide",
        "repo": "flake-utils",
        "type": "github"
      }
    },
    "home-manager": {
      "inputs": {
        "nixpkgs": [
          "nixpkgs"
        ]
      },
      "locked": {
        "lastModified": 1776452385,
        "narHash": "sha256-GlzikirXePsRmG1I7poIyaO3iuONCXsAxVKCm1JwPYw=",
        "owner": "Ishaan-Datta",
        "repo": "home-manager",
        "rev": "1348ee39c6014ee7273e068baebd9dcf505d3b55",
        "type": "github"
      },
      "original": {
        "owner": "Ishaan-Datta",
        "ref": "master",
        "repo": "home-manager",
        "type": "github"
      }
    },
    "impermanence": {
      "inputs": {
        "home-manager": [
          "home-manager"
        ],
        "nixpkgs": [
          "nixpkgs"
        ]
      },
      "locked": {
        "lastModified": 1769548169,
        "narHash": "sha256-03+JxvzmfwRu+5JafM0DLbxgHttOQZkUtDWBmeUkN8Y=",
        "owner": "nix-community",
        "repo": "impermanence",
        "rev": "7b1d382faf603b6d264f58627330f9faa5cba149",
        "type": "github"
      },
      "original": {
        "owner": "nix-community",
        "repo": "impermanence",
        "type": "github"
      }
    },
    "nix-index-database": {
      "inputs": {
        "nixpkgs": [
          "nixpkgs"
        ]
      },
      "locked": {
        "lastModified": 1775970782,
        "narHash": "sha256-7jt9Vpm48Yy5yAWigYpde+HxtYEpEuyzIQJF4VYehhk=",
        "owner": "nix-community",
        "repo": "nix-index-database",
        "rev": "bedba5989b04614fc598af9633033b95a937933f",
        "type": "github"
      },
      "original": {
        "owner": "nix-community",
        "repo": "nix-index-database",
        "type": "github"
      }
    },
    "nixlib": {
      "locked": {
        "lastModified": 1736643958,
        "narHash": "sha256-tmpqTSWVRJVhpvfSN9KXBvKEXplrwKnSZNAoNPf/S/s=",
        "owner": "nix-community",
        "repo": "nixpkgs.lib",
        "rev": "1418bc28a52126761c02dd3d89b2d8ca0f521181",
        "type": "github"
      },
      "original": {
        "owner": "nix-community",
        "repo": "nixpkgs.lib",
        "type": "github"
      }
    },
    "nixos-generators": {
      "inputs": {
        "nixlib": "nixlib",
        "nixpkgs": [
          "nixpkgs"
        ]
      },
      "locked": {
        "lastModified": 1769813415,
        "narHash": "sha256-nnVmNNKBi1YiBNPhKclNYDORoHkuKipoz7EtVnXO50A=",
        "owner": "nix-community",
        "repo": "nixos-generators",
        "rev": "8946737ff703382fda7623b9fab071d037e897d5",
        "type": "github"
      },
      "original": {
        "owner": "nix-community",
        "repo": "nixos-generators",
        "type": "github"
      }
    },
    "nixpkgs": {
      "locked": {
        "lastModified": 1775423009,
        "narHash": "sha256-vPKLpjhIVWdDrfiUM8atW6YkIggCEKdSAlJPzzhkQlw=",
        "owner": "nixos",
        "repo": "nixpkgs",
        "rev": "68d8aa3d661f0e6bd5862291b5bb263b2a6595c9",
        "type": "github"
      },
      "original": {
        "owner": "nixos",
        "ref": "nixos-unstable",
        "repo": "nixpkgs",
        "type": "github"
      }
    },
    "nixpkgs-stable": {
      "locked": {
        "lastModified": 1776221942,
        "narHash": "sha256-lLXp2pPsSspaqyX8mNzdHDauRUKjui6HL00Ngjqspik=",
        "rev": "1766437c5509f444c1b15331e82b8b6a9b967000",
        "type": "tarball",
        "url": "https://releases.nixos.org/nixos/25.11/nixos-25.11.9320.1766437c5509/nixexprs.tar.xz"
      },
      "original": {
        "type": "tarball",
        "url": "https://channels.nixos.org/nixos-25.11/nixexprs.tar.xz"
      }
    },
    "nixpkgs_2": {
      "locked": {
        "lastModified": 1776169885,
        "narHash": "sha256-Gk2T0tDDDAs319hp/ak+bAIUG5bPMvnNEjPV8CS86Fg=",
        "rev": "4bd9165a9165d7b5e33ae57f3eecbcb28fb231c9",
        "type": "tarball",
        "url": "https://releases.nixos.org/nixos/unstable/nixos-26.05pre980183.4bd9165a9165/nixexprs.tar.xz"
      },
      "original": {
        "type": "tarball",
        "url": "https://channels.nixos.org/nixos-unstable/nixexprs.tar.xz"
      }
    },
    "nixpkgs_3": {
      "locked": {
        "lastModified": 1772542754,
        "narHash": "sha256-WGV2hy+VIeQsYXpsLjdr4GvHv5eECMISX1zKLTedhdg=",
        "owner": "NixOS",
        "repo": "nixpkgs",
        "rev": "8c809a146a140c5c8806f13399592dbcb1bb5dc4",
        "type": "github"
      },
      "original": {
        "owner": "NixOS",
        "ref": "nixos-unstable",
        "repo": "nixpkgs",
        "type": "github"
      }
    },
    "nixpkgs_4": {
      "locked": {
        "lastModified": 1775579569,
        "narHash": "sha256-/m3yyS/EnXqoPGBJYVy4jTOsirdgsEZ3JdN2gGkBr14=",
        "owner": "NixOS",
        "repo": "nixpkgs",
        "rev": "dfd9566f82a6e1d55c30f861879186440614696e",
        "type": "github"
      },
      "original": {
        "owner": "NixOS",
        "ref": "nixpkgs-unstable",
        "repo": "nixpkgs",
        "type": "github"
      }
    },
    "nixpkgs_5": {
      "locked": {
        "lastModified": 1772419343,
        "narHash": "sha256-QU3Cd5DJH7dHyMnGEFfPcZDaCAsJQ6tUD+JuUsYqnKU=",
        "owner": "NixOS",
        "repo": "nixpkgs",
        "rev": "93178f6a00c22fcdee1c6f5f9ab92f2072072ea9",
        "type": "github"
      },
      "original": {
        "owner": "NixOS",
        "ref": "nixpkgs-unstable",
        "repo": "nixpkgs",
        "type": "github"
      }
    },
    "root": {
      "inputs": {
        "disko": "disko",
        "flake-parts": "flake-parts",
        "flake-utils": "flake-utils",
        "home-manager": "home-manager",
        "impermanence": "impermanence",
        "nix-index-database": "nix-index-database",
        "nixos-generators": "nixos-generators",
        "nixpkgs": "nixpkgs_2",
        "nixpkgs-stable": "nixpkgs-stable",
        "systems": "systems",
        "vicinae": "vicinae",
        "vicinae-extensions": "vicinae-extensions",
        "wrapper-modules": "wrapper-modules",
        "wrappers": "wrappers",
        "yazi": "yazi"
      }
    },
    "systems": {
      "locked": {
        "lastModified": 1680978846,
        "narHash": "sha256-Gtqg8b/v49BFDpDetjclCYXm8mAnTrUzR0JnE2nv5aw=",
        "owner": "nix-systems",
        "repo": "x86_64-linux",
        "rev": "2ecfcac5e15790ba6ce360ceccddb15ad16d08a8",
        "type": "github"
      },
      "original": {
        "owner": "nix-systems",
        "repo": "x86_64-linux",
        "type": "github"
      }
    },
    "systems_2": {
      "locked": {
        "lastModified": 1681028828,
        "narHash": "sha256-Vy1rq5AaRuLzOxct8nz4T6wlgyUR7zLU309k9mBC768=",
        "owner": "nix-systems",
        "repo": "default",
        "rev": "da67096a3b9bf56a91d16901293e51ba5b49a27e",
        "type": "github"
      },
      "original": {
        "owner": "nix-systems",
        "repo": "default",
        "type": "github"
      }
    },
    "systems_3": {
      "locked": {
        "lastModified": 1681028828,
        "narHash": "sha256-Vy1rq5AaRuLzOxct8nz4T6wlgyUR7zLU309k9mBC768=",
        "owner": "nix-systems",
        "repo": "default",
        "rev": "da67096a3b9bf56a91d16901293e51ba5b49a27e",
        "type": "github"
      },
      "original": {
        "owner": "nix-systems",
        "repo": "default",
        "type": "github"
      }
    },
    "systems_4": {
      "locked": {
        "lastModified": 1681028828,
        "narHash": "sha256-Vy1rq5AaRuLzOxct8nz4T6wlgyUR7zLU309k9mBC768=",
        "owner": "nix-systems",
        "repo": "default",
        "rev": "da67096a3b9bf56a91d16901293e51ba5b49a27e",
        "type": "github"
      },
      "original": {
        "owner": "nix-systems",
        "repo": "default",
        "type": "github"
      }
    },
    "vicinae": {
      "inputs": {
        "nixpkgs": "nixpkgs_3",
        "systems": "systems_2"
      },
      "locked": {
        "lastModified": 1776435302,
        "narHash": "sha256-MSmlvbsg2kc2DdQGBR+3Shta+Spgi4A2k5tkbTnrro8=",
        "owner": "vicinaehq",
        "repo": "vicinae",
        "rev": "9fb1f6d2f882ebf36ab19919e99ca36ad7e06c9b",
        "type": "github"
      },
      "original": {
        "owner": "vicinaehq",
        "repo": "vicinae",
        "type": "github"
      }
    },
    "vicinae-extensions": {
      "inputs": {
        "flake-compat": "flake-compat",
        "nixpkgs": [
          "nixpkgs"
        ],
        "systems": "systems_3",
        "vicinae": "vicinae_2"
      },
      "locked": {
        "lastModified": 1775911073,
        "narHash": "sha256-Fa5JvMFVwBzbnOjEV2Cer8ak0zF/CDwdHT7+wslL30w=",
        "owner": "vicinaehq",
        "repo": "extensions",
        "rev": "d12bcb134d45dedad1a28a18e1cd8807353338d0",
        "type": "github"
      },
      "original": {
        "owner": "vicinaehq",
        "repo": "extensions",
        "type": "github"
      }
    },
    "vicinae_2": {
      "inputs": {
        "nixpkgs": [
          "vicinae-extensions",
          "nixpkgs"
        ],
        "systems": [
          "vicinae-extensions",
          "systems"
        ]
      },
      "locked": {
        "lastModified": 1768856963,
        "narHash": "sha256-u5bWDuwk6oieTnvm1YjNotcYK8iJSddH5+S68+X4TSc=",
        "owner": "vicinaehq",
        "repo": "vicinae",
        "rev": "934bc0ad47be6dbd6498a0dac655c4613fd0ab27",
        "type": "github"
      },
      "original": {
        "owner": "vicinaehq",
        "repo": "vicinae",
        "type": "github"
      }
    },
    "wrapper-modules": {
      "inputs": {
        "nixpkgs": "nixpkgs_4"
      },
      "locked": {
        "lastModified": 1776415533,
        "narHash": "sha256-pJDLkxfpCUMymjVc62e+SQxw7vjTqHyHCdcNJxEzAxM=",
        "owner": "BirdeeHub",
        "repo": "nix-wrapper-modules",
        "rev": "5d8fa106ba1b0c0d582cc06dc328e802b201d2fc",
        "type": "github"
      },
      "original": {
        "owner": "BirdeeHub",
        "repo": "nix-wrapper-modules",
        "type": "github"
      }
    },
    "wrappers": {
      "inputs": {
        "nixpkgs": [
          "nixpkgs"
        ]
      },
      "locked": {
        "lastModified": 1775600302,
        "narHash": "sha256-2fgKImv78CXIcfo1RsY7EI4uMZ84x/MggA5rrusYc7c=",
        "owner": "Lassulus",
        "repo": "wrappers",
        "rev": "9d8397d8ef1ac35763085f3338589f558128f7db",
        "type": "github"
      },
      "original": {
        "owner": "Lassulus",
        "repo": "wrappers",
        "type": "github"
      }
    },
    "yazi": {
      "inputs": {
        "flake-utils": "flake-utils_2",
        "nixpkgs": "nixpkgs_5"
      },
      "locked": {
        "lastModified": 1776356189,
        "narHash": "sha256-VzBmJuQfi3iRC9rkHZ5QeWYZtMHffko3iYqFzMVsrFk=",
        "owner": "sxyazi",
        "repo": "yazi",
        "rev": "ae4c138f49e00a64b478318ed9c7e9072fef8c52",
        "type": "github"
      },
      "original": {
        "owner": "sxyazi",
        "repo": "yazi",
        "type": "github"
      }
    }
  },
  "root": "root",
  "version": 7
}
"#;
