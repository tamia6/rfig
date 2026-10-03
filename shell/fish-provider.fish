source $argv[1] >/dev/null 2>&1
set -l line $argv[2]
set -l name $argv[3]
if string match -qr '^[a-zA-Z0-9_.+-]+$' -- "$name"
    if test -r "$HOME/.config/rfig/generated/$name.fish"
        source "$HOME/.config/rfig/generated/$name.fish" >/dev/null 2>&1
    end
end
# Trigger autoload, then check whether a definition exists. Unknown tools use help cache.
set -l hits (complete -C "$line" 2>/dev/null)
if test (count (complete -c $name)) -gt 0
    printf 'provider\n'
    printf '%s\n' $hits
end
