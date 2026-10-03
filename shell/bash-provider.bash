# User text is passed only as data, never evaluated.
for base in /opt/homebrew/etc/profile.d/bash_completion.sh /usr/local/etc/profile.d/bash_completion.sh /usr/share/bash-completion/bash_completion; do
    if [[ -r $base ]]; then source "$base" >/dev/null 2>&1; break; fi
done
source "$1" >/dev/null 2>&1
COMP_LINE=$2 COMP_POINT=${#2} COMP_TYPE=9 COMP_KEY=9
shift 2
COMP_WORDS=( "$@" )
COMP_CWORD=$(( ${#COMP_WORDS[@]} - 1 ))
name=${COMP_WORDS[0]}
if ! complete -p "$name" >/dev/null 2>&1; then
  if declare -F _completion_loader >/dev/null; then _completion_loader "$name" >/dev/null 2>&1; fi
fi
if ! complete -p "$name" >/dev/null 2>&1 && [[ $name != *[^a-zA-Z0-9_.+-]* && -r $HOME/.config/rfig/generated/$name.bash ]]; then
    source "$HOME/.config/rfig/generated/$name.bash" >/dev/null 2>&1
fi
spec=$(complete -p "$name" 2>/dev/null) || exit 0
COMPREPLY=()
if [[ $spec =~ -F[[:space:]]+([a-zA-Z_][a-zA-Z_0-9]*) ]]; then
  handler=${BASH_REMATCH[1]}
  # bash-completion's generic file fallback is not a command definition.
  [[ $handler == _minimal || $handler == _comp_complete_minimal ]] && exit 0
  "$handler" "$name" "${COMP_WORDS[COMP_CWORD]}" "${COMP_WORDS[COMP_CWORD-1]}" >/dev/null 2>&1
else
  # Let Bash interpret the trusted registration; the current word stays an argument.
  spec=${spec#complete }
  spec=${spec% *}
  mapfile -t COMPREPLY < <(eval 'compgen '"$spec"' -- "${COMP_WORDS[COMP_CWORD]}"' 2>/dev/null)
fi
printf 'provider\n'
printf '%s\n' "${COMPREPLY[@]}"
