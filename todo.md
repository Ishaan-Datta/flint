## Packaging

- make jq a runtime dependency for the package
- copy NH flake nix structure for packaging, man page completion, etc.
- add overlay for the package for easier packaging

## Testing:

- add proper tests section for validating all the outputs are correct...
- get gpt to come up with edge cases - assume that the initial flake will be linted 
- cargo nexttest

todo: look into their:
flake.nix
shell.nix
package.nix
cargo.toml
cargo.toml for xtask
nexttest.toml in .config
justfile

add doc comments for struct fields...
make in place unit tests for the stuff that is easy to test, like command stuff

review how argaction:: works for the bool args