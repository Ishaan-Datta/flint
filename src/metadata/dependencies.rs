use std::collections::HashMap;
use std::process::Command;

const DEPENDENCIES_CMD: &str = r#"nix flake metadata --json --no-write-lock-file . \
  | jq '.locks.nodes
        | map_values(
            (.inputs // {})
            | map_values(
                if type == "array" then .
                else [.]
                end
              )
          )'
"#;

/// Get the current input dependencies for the existing flake.nix
// TODO: make this use the command module
pub fn get_input_deps() -> Result<HashMap<String, Vec<String>>, anyhow::Error> {
    let deps_output = Command::new("sh").args(["-c", DEPENDENCIES_CMD]).output()?;
    if deps_output.status.success() {
        let deps_map: HashMap<String, HashMap<String, Vec<String>>> =
            serde_json::from_slice(&deps_output.stdout)?;
        let mut filtered_deps_map = deps_map.clone();
        for input in deps_map.keys() {
            if deps_map.get(input).expect("Checked").is_empty() {
                filtered_deps_map.remove(input);
            }
        }
        // inputs with transitive deps
        tracing::trace!("{filtered_deps_map:#?}");

        let mut defined = Vec::<&String>::new();
        if filtered_deps_map.contains_key("root") {
            defined.extend(
                filtered_deps_map
                    .get("root")
                    .expect("Checked before")
                    .keys(),
            );
            tracing::trace!("defined: {defined:#?}")
        }

        let mut dupe_map = HashMap::<String, Vec<String>>::new();
        for (input, deps) in filtered_deps_map.iter() {
            if defined.contains(&input) {
                tracing::trace!("{deps:#?}");

                let mut new_targets = Vec::<String>::new();
                for (dependency, target) in deps.iter() {
                    if target.len() > 1 || target.is_empty() {
                        tracing::debug!("irregular target: {target:?} for {dependency}");
                    } else {
                        if ends_with_2_to_99(&target[0]) {
                            tracing::debug!("Found potential duplicate: {}", target[0]);

                            // TODO: should make sure that the dependency is declared somewhere else in the thing, otherwise comment strange things are afoot..
                            let new_target =
                                format!("inputs.{dependency}.follows = \"{dependency}\";");
                            tracing::trace!(
                                "[Input: {input}]Setting target for {dependency}: {new_target}"
                            );
                            new_targets.push(new_target);
                        }
                    }
                }
                if !new_targets.is_empty() {
                    dupe_map.insert(input.to_string(), new_targets);
                }
            } else {
                // todo: this is still including "root"
                tracing::debug!("input: {input} not found in the declared list");
            }
        }

        tracing::trace!("{dupe_map:#?}");
        return Ok(dupe_map);
    } else {
        anyhow::bail!("failure");
    }
}

fn lint_flake_inputs(fix: bool, dry_run: bool, quiet: bool) {
    let dupe_map = get_input_deps().unwrap();
    // if no fix or check_updates, it will jsut check for duplicate entries, and report them

    // // path for fixing:
    // let source = fs::read_to_string("flake.nix")?;
    // let rewritten = rewrite_flake_inputs(&source, dupe_map);
    // tracing::trace!("{rewritten}");
    // fs::write("temp.nix", rewritten)?;

    // // path for getting url times:
    // let input_ruls = get_input_urls()?;
    // get_modified_times(input_ruls);

    // for each path, get the error if there is one, print it and then exit 1
}

fn ends_with_2_to_99(s: &str) -> bool {
    if let Some((_, n)) = s.rsplit_once('_') {
        if let Ok(v) = n.parse::<u8>() {
            return (2..=99).contains(&v);
        }
    }
    false
}
