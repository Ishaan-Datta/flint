Require flint
Set Shell fish

Output stale.gif

Set FontSize 16
Set Width 1200
Set Height 600

# Setup environment
Hide
Type "cd ../flint/tests/common"
Enter
Type "set -g fish_autosuggestion_enabled 0"
Enter
Type "git restore -- flake.nix flake.lock; rm -f flake.nix.bak; clear"
Enter
Show

# Quiet/check-mode showcase
Type "flint -q stale; echo exit status: $status"
Sleep 500ms
Enter
Sleep 10s

# Modify flake.lock for auto-update prompt showcase
Hide
Type "sed -i 's/1767039857/1767039858/g' flake.lock && clear"
Enter
Show

# `flint stale --auto-update` showcase
Type "flint stale --update-threshold 604800 --auto-update -y"
Sleep 500ms
Enter
Sleep 12s

# Reset only the files touched by this tape
Hide
Type "git restore -- flake.nix flake.lock; rm -f flake.nix.bak; clear"
Enter
Show