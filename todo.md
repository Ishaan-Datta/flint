## Packaging

- make jq a runtime dependency for the package
- copy NH flake nix structure for packaging, man page completion, etc.
- add overlay for the package for easier packaging

## Add:

- add logging statements for stuff that failed to work... - tracing
- would be nice to have progress spinner or something as you construct the flake metadata parallel iterator...

## CLI Options:

- make default to check if there are git modifications to the current flake.lock, make warning y/n prompt to verify they want to potentially override the changes
- by default with no flags it will just check for duplicate entries and let you know
- lets actually experiment with the behaviour a bit, if you already have a staged lock file, then you make a modification to the flake.lock, will it be autostaged?
- so looks like in order to the update check, need to have a lockfile already written?

## Testing:
- add proper tests section for validating all the outputs are correct...
- get gpt to come up with edge cases - assume that the initial flake will be linted 
- cargo nexttest



nix flake metadata --json --recreate-lock-file --no-write-lock-file .
--recreate-lock-file tells Nix to recreate the lock data from scratch, and --no-write-lock-file prevents writing that recreated lock file back to disk. The recreate option is documented as recreating the flake’s lock file from scratch, though newer manuals mark it deprecated in favor of nix flake update for normal workflows.