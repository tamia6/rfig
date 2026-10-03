# Bash 4.4+ / Readline adapter. Commands are always executed by Bash itself.
[[ $- == *i* && -z ${RFIG_PROVIDER-} ]] || return
if (( BASH_VERSINFO[0] < 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] < 4) )); then
  printf 'rfig: Bash 4.4+ is required (macOS: brew install bash).\n' >&2
  return
fi
[[ ${_RFIG_BASH_LOADED-} == 1 ]] && return
_RFIG_BASH_LOADED=1
_rfig_prompt_anchor() {
  [[ $PS1 == '\[\e7\]'* ]] || PS1='\[\e7\]'"$PS1"
}
# Preserve the user's prompt hooks, including hooks that replace PS1.
if [[ $(declare -p PROMPT_COMMAND 2>/dev/null) == 'declare -a '* ]]; then
  PROMPT_COMMAND+=( _rfig_prompt_anchor )
else
  PROMPT_COMMAND="${PROMPT_COMMAND-}"$'\n''_rfig_prompt_anchor'
fi
_rfig_prompt_anchor

_rfig_edit() {
  local snapshot action point updated keymap
  snapshot=$(umask 077; mktemp -d "${TMPDIR:-/tmp}/rfig.XXXXXXXX") || return
  (umask 077; declare -f > "$snapshot/definitions"; complete -p >> "$snapshot/definitions"; compgen -c > "$snapshot/names")
  local prompt=${PS1@P}
  prompt=${prompt//$'\001'/}
  prompt=${prompt//$'\002'/}
  local prompt_tail=$prompt prompt_rows=0
  while [[ $prompt_tail == *$'\n'* ]]; do
    ((prompt_rows+=1))
    prompt_tail=${prompt_tail#*$'\n'}
  done
  printf '\0338%s' "$prompt"
  RFIG_PROMPT_ROWS=$prompt_rows RFIG_SHELL_EXE=$BASH RFIG_PROVIDER=1 command rfig edit bash "$snapshot" "$READLINE_LINE" "$READLINE_POINT" 0
  if [[ -f $snapshot/result ]]; then
    { IFS= read -r action; IFS= read -r point; IFS= read -r -d '' updated || :; } < "$snapshot/result"
    READLINE_LINE=$updated READLINE_POINT=$point
  else
    action=cancel
  fi
  rm -rf -- "$snapshot"
  case $action in
    execute) action=accept-line ;;
    complete) action=complete ;;
    search) action=reverse-search-history ;;
    escape) if [[ -o vi ]]; then action=vi-movement-mode; else action=redraw-current-line; fi ;;
    eof) action='"\C-d"' ;;
    *) action=redraw-current-line ;;
  esac
  for keymap in emacs-standard vi-insert; do
    bind -m "$keymap" '"\C-x\C-a": '"$action"
  done
}

_rfig_accept_native() {
  command rfig remember "$READLINE_LINE"
  bind -m emacs-standard '"\C-x\C-a":accept-line'
  bind -m vi-insert '"\C-x\C-a":accept-line'
}

for _rfig_map in emacs-standard vi-insert; do
  bind -m "$_rfig_map" -x '"\C-x\C-r":_rfig_edit'
  bind -m "$_rfig_map" -x '"\C-x\C-j":_rfig_accept_native'
  bind -m "$_rfig_map" '"\C-m":"\C-x\C-j\C-x\C-a"'
  for ((_rfig_code=32;_rfig_code<127;_rfig_code++)); do
    printf -v _rfig_key "\\$(printf '%03o' "$_rfig_code")"
    _rfig_key=${_rfig_key//\\/\\\\}
    _rfig_key=${_rfig_key//\"/\\\"}
    bind -m "$_rfig_map" "\"$_rfig_key\":\"\\C-v$_rfig_key\\C-x\\C-r\\C-x\\C-a\""
  done
  bind -m "$_rfig_map" '"\C-x\C-a":redraw-current-line'
done
unset _rfig_map _rfig_code _rfig_key

source <(command rfig completion bash)
