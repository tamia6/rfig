complete -c rfig -f
complete -c rfig -n '__fish_use_subcommand' -a 'setup init analyze completion'
complete -c rfig -n '__fish_seen_subcommand_from completion' -a 'zsh bash fish'
complete -c rfig -n '__fish_seen_subcommand_from analyze' -a '(cat ~/.config/rfig/commands.txt 2>/dev/null)'
complete -c rfig -n '__fish_seen_subcommand_from setup' -l shell -r -a 'zsh bash fish'
