## Packaging

- make jq a runtime dependency for the package
- copy NH flake nix structure for packaging, man page completion, etc.
- add overlay for the package for easier packaging

## Testing:
- add proper tests section for validating all the outputs are correct...
- get gpt to come up with edge cases - assume that the initial flake will be linted 
- cargo nexttest

nix flake metadata --json --recreate-lock-file --no-write-lock-file .
--recreate-lock-file tells Nix to recreate the lock data from scratch, and --no-write-lock-file prevents writing that recreated lock file back to disk. The recreate option is documented as recreating the flake’s lock file from scratch, though newer manuals mark it deprecated in favor of nix flake update for normal workflows.


todo: look into their:
flake.nix
shell.nix
package.nix
cargo.toml
cargo.toml for xtask
nexttest.toml in .config
justfile

rename to make your names more descriptive

add doc comments for struct fields...

make this error for the other errors in treesitter, the thing for parsing the inputs

anotate the values you are debug tracing.... also the error isnt being printed correclty:
```
downloading 'https://api.github.com/repos/BirdeeHub/nix-wrapper-modules/commits/HEAD'⏎                                                                      downloading 'https://api.github.com/repos/vicinaehq/extensions/commits/HEAD'
downloading 'https://api.github.com/repos/Ishaan-Datta/sops-nix/commits/master'

1731533236
warning: unable to download 'https://api.github.com/repos/nix-community/impermanence/commits/HEAD': HTTP error 403

response body:

{"message":"API rate limit exceeded for 70.73.21.234. (But here's the good news: Authenticated requests get a higher rate limit. Check out the documentation for more details.)","documentation_url":"https://docs.github.com/rest/overview/resources-in-the-rest-api#rate-limiting"}; using cached version
1773517922
warning: unable to download 'https://api.github.com/repos/Ishaan-Datta/home-manager/commits/master': HTTP error 403

response body:

{"message":"API rate limit exceeded for 70.73.21.234. (But here's the good news: Authenticated requests get a higher rate limit. Check out the documentation for more details.)","documentation_url":"https://docs.github.com/rest/overview/resources-in-the-rest-api#rate-limiting"}; using cached version
1768920986
1778294120
warning: unable to download 'https://api.github.com/repos/sxyazi/yazi/commits/HEAD': HTTP error 403

response body:

{"message":"API rate limit exceeded for 70.73.21.234. (But here's the good news: Authenticated requests get a higher rate limit. Check out the documentation for more details.)","documentation_url":"https://docs.github.com/rest/overview/resources-in-the-rest-api#rate-limiting"}; using cached version
warning: unable to download 'https://api.github.com/repos/nix-community/nixos-generators/commits/HEAD': HTTP error 403

response body:

{"message":"API rate limit exceeded for 70.73.21.234. (But here's the good news: Authenticated requests get a higher rate limit. Check out the documentation for more details.)","documentation_url":"https://docs.github.com/rest/overview/resources-in-the-rest-api#rate-limiting"}; using cached version
warning: unable to download 'https://api.github.com/repos/nix-systems/default/commits/HEAD': HTTP error 403

response body:

{"message":"API rate limit exceeded for 70.73.21.234. (But here's the good news: Authenticated requests get a higher rate limit. Check out the documentation for more details.)","documentation_url":"https://docs.github.com/rest/overview/resources-in-the-rest-api#rate-limiting"}; using cached version
warning: unable to download 'https://api.github.com/repos/Ishaan-Datta/flake-parts/commits/master': HTTP error 403

response body:

{"message":"API rate limit exceeded for 70.73.21.234. (But here's the good news: Authenticated requests get a higher rate limit. Check out the documentation for more details.)","documentation_url":"https://docs.github.com/rest/overview/resources-in-the-rest-api#rate-limiting"}; using cached version
warning: unable to download 'https://api.github.com/repos/Ishaan-Datta/sops-nix/commits/master': HTTP error 403

response body:

{"message":"API rate limit exceeded for 70.73.21.234. (But here's the good news: Authenticated requests get a higher rate limit. Check out the documentation for more details.)","documentation_url":"https://docs.github.com/rest/overview/resources-in-the-rest-api#rate-limiting"}; using cached version
1778294295
1778240325
1778330607
1778090892
1773621294
1778294654
1769548169
1778292630
1778345877
1681028828
1769813415
1777954456
1778003029
warning: unable to download 'https://api.github.com/repos/NixOS/flake-compat/commits/HEAD': HTTP error 403

response body:

{"message":"API rate limit exceeded for 70.73.21.234. (But here's the good news: Authenticated requests get a higher rate limit. Check out the documentation for more details.)","documentation_url":"https://docs.github.com/rest/overview/resources-in-the-rest-api#rate-limiting"}; using cached version
error:
       … while updating the lock file of flake 'github:vicinaehq/extensions/20d6a13d2a389e61619b8540b8af746705409322?narHash=sha256-0hVf9yH%2Bv%2B0YaCqmr0aX0nR4pfmXjW1XhJcJyblJqE0%3D'

       error: cannot write modified lock file of flake 'github:vicinaehq/extensions' (use '--no-write-lock-file' to ignore)
1778260783
```

[✘] vicinae-extensions       Could not fetch the flake metadata for the input source: Failed to parse last modified time from command output: cannot parse integer from empty string

make the function names get_local_modified_times and get_remote_modified_times()


verbosity cli flag kinda sucks... does max level warn, we need it to just silence traces, maybe [ERROR] in red text ANSI for log fatal

y/n prompt if you are about to overwrite existing flake.nix changes

flint duplicates

flint stale