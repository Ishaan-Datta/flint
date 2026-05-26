Require flint
Set Shell fish

Output duplicates.gif

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

# `flint duplicates` check showcase
Type "flint duplicates"
Sleep 500ms
Enter
Sleep 4s

# Clear Screen
Hide
Type "clear"
Enter
Show

# Quiet/check-mode showcase
Type "flint -q duplicates; echo exit status: $status"
Sleep 2s
Enter
Sleep 4s

# Modify flake.nix for fix prompt showcase
Hide
Type "sed -i 's/Configuration/configuration/g' flake.nix && clear"
Enter
Show

# `flint duplicates --fix -y` showcase
Type "flint duplicates --fix -y"
Sleep 500ms
Enter
Sleep 6s

# Reset only the files touched by this tape
Hide
Type "git restore -- flake.nix flake.lock; rm -f flake.nix.bak; clear"
Enter
Show