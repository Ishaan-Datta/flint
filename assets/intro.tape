Require flint
Set Shell fish

Output intro.gif

Set FontSize 16
Set Width 1200
Set Height 650

# Setup environment
Hide
Type "cd ../flint/tests/common"
Enter
Type "set -g fish_autosuggestion_enabled 0"
Enter
Type "git restore -- flake.nix flake.lock; rm -f flake.nix.bak; clear"
Enter
Show

# Modify flake.lock for showcase
Hide 
Type "sed -i 's/1767039857/1767039858/g' flake.lock && clear"
Enter
Show

# `flint stale` showcase
Type "flint stale --auto-update"
Sleep 500ms
Enter
Wait+Line@15s /y\/N/
Sleep 1s
Type "y"
Sleep 500ms
Enter
Sleep 3s

# Modify flake.nix for showcase
Hide
Type "sed -i 's/Configuration/configuration/g' flake.nix && clear"
Enter
Show

# `flint duplicates` showcase
Type "flint duplicates --fix"
Sleep 500ms
Enter
Wait+Line@15s /y\/N/
Sleep 1s
Type "y"
Sleep 500ms
Enter
Sleep 3s

# Reset only the files touched by this tape
Hide
Type "git restore -- flake.nix flake.lock; rm -f flake.nix.bak; clear"
Enter
Show
