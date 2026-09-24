# Sourced by ~/.zshrc after installing rfig.
if (( ! $+functions[_main_complete] )); then
  autoload -Uz compinit
  compinit -D
fi
zmodload zsh/stat

typeset -gaU _rfig_hits
typeset -gA _rfig_option_kinds
typeset -g _rfig_option_source='' _rfig_option_stamp=''
typeset -gi _rfig_selected=1 _rfig_injected=0

_rfig_compadd() {
  local -a hits descriptions
  local item index kind
  for item in "$@"; do
    if [[ $item == -A || $item == -D || $item == -O ]]; then
      builtin compadd "$@"
      return
    fi
  done
  builtin compadd -A hits -D descriptions "$@"
  for (( index = 1; index <= $#hits; index++ )); do
    item=$hits[index]
    [[ -n $item && $item != *$'\n'* && $item != *$'\t'* ]] || continue
    if [[ -n ${_rfig_option_kinds[$item]-} ]]; then
      kind=${_rfig_option_kinds[$item]}
    elif [[ $item == --?* ]]; then
      kind=option
    elif [[ $item == -* ]]; then
      kind=flag
    elif (( CURRENT == 1 )); then
      kind=command
    elif [[ ${_type-} == *command* ]] || { (( CURRENT == 2 )) && [[ $curcontext == *:argument-1 ]]; }; then
      kind=subcommand
    else
      kind=argument
    fi
    _rfig_hits+=( "$item"$'\t'"${(q)item}"$'\t'"${descriptions[index]//$'\n'/ }"$'\t'"$kind" )
  done
  builtin compadd "$@"
}

_rfig_capture() {
  _rfig_hits=()
  typeset -g _rfig_start=$(( CURSOR - ${#PREFIX} ))
  local previous=${functions[compadd]-} had_previous=$+functions[compadd]
  functions[compadd]=$functions[_rfig_compadd]
  {
    _main_complete
  } always {
    if (( had_previous )); then
      functions[compadd]=$previous
    else
      unfunction compadd
    fi
  }
  compstate[insert]=''
  compstate[list]=''
}
zle -C _rfig_capture complete-word _rfig_capture

_rfig_preview() {
  POSTDISPLAY=''
  region_highlight=(${region_highlight:#*memo=rfig})
  _rfig_hits=()
  _rfig_injected=0
  [[ -n $BUFFER && $BUFFER != *$'\n'* ]] || return

  local original_buffer=$BUFFER original_cursor=$CURSOR command_name=${BUFFER%% *}
  local option_file="$HOME/.config/rfig/options/$command_name.tsv" stamp=''
  local -A file_stat
  if [[ $command_name != */* ]] && zstat -H file_stat "$option_file" 2>/dev/null; then
    stamp="${file_stat[ino]}:${file_stat[mtime]}"
  fi
  if [[ $command_name != $_rfig_option_source || $stamp != $_rfig_option_stamp ]]; then
    _rfig_option_kinds=()
    if [[ -n $stamp && -r $option_file ]]; then
      local option_name option_kind
      while IFS=$'\t' read -r option_name option_kind; do
        [[ $option_name == -* && ( $option_kind == option || $option_kind == flag ) ]] || continue
        _rfig_option_kinds[$option_name]=$option_kind
      done < "$option_file"
    fi
    _rfig_option_source=$command_name
    _rfig_option_stamp=$stamp
  fi
  if (( $+commands[$command_name] )) && [[ -z ${_comps[$command_name]-} ]]; then
    return
  fi
  if [[ $BUFFER == $command_name && $CURSOR == ${#BUFFER} && -n ${_comps[$command_name]-} ]]; then
    BUFFER+=' '
    CURSOR=${#BUFFER}
    _rfig_injected=1
  fi
  zle _rfig_capture
  BUFFER=$original_buffer
  CURSOR=$original_cursor

  (( $#_rfig_hits )) || return
  local -a subcommands flags
  local record label
  for record in "${_rfig_hits[@]}"; do
    label=${record%%$'\t'*}
    if [[ $label == -* ]]; then
      flags+=( "$record" )
    else
      subcommands+=( "$record" )
    fi
  done
  _rfig_hits=( "${subcommands[@]}" "${flags[@]}" )
  (( _rfig_selected > $#_rfig_hits )) && _rfig_selected=$#_rfig_hits
  local first=$(( _rfig_selected > 5 ? _rfig_selected - 4 : 1 ))
  local index description row start kind icon color rest
  rest=${_rfig_hits[_rfig_selected]#*$'\t'}
  rest=${rest#*$'\t'}
  description=${rest%%$'\t'*}
  for (( index = first; index <= $#_rfig_hits && index < first + 5; index++ )); do
    record=${_rfig_hits[index]}
    label=${record%%$'\t'*}
    kind=${record##*$'\t'}
    [[ -n $label ]] || continue
    case $kind in
      command) icon='⌘'; color=cyan ;;
      subcommand) icon='↳'; color=magenta ;;
      argument) icon='●'; color=green ;;
      option) icon='◇'; color=blue ;;
      flag) icon='⚑'; color=yellow ;;
    esac
    POSTDISPLAY+=$'\n'
    start=$(( ${#BUFFER} + ${#POSTDISPLAY} ))
    printf -v row ' %s %-42.42s' "$icon" "$label"
    POSTDISPLAY+=$row
    if (( index == _rfig_selected )); then
      region_highlight+=( "$start $((start + ${#row})) standout memo=rfig" )
      region_highlight+=( "$((start + 1)) $((start + 2)) standout,bg=$color memo=rfig" )
    else
      region_highlight+=( "$((start + 1)) $((start + 2)) fg=$color memo=rfig" )
    fi
  done
  POSTDISPLAY+=$'\n'
  printf -v row ' %-44.44s' "$description"
  POSTDISPLAY+=$row
}

_rfig_after_edit() {
  zle ".$WIDGET" "$@"
  _rfig_selected=1
  _rfig_preview
  zle -R
}
for _rfig_widget in self-insert backward-delete-char delete-char backward-kill-word kill-word kill-whole-line bracketed-paste; do
  zle -N "$_rfig_widget" _rfig_after_edit
done
unset _rfig_widget

_rfig_up() {
  if (( $#_rfig_hits )); then
    (( _rfig_selected = _rfig_selected > 1 ? _rfig_selected - 1 : 1 ))
  else
    zle up-line-or-history
    _rfig_selected=1
  fi
  _rfig_preview
  zle -R
}

_rfig_down() {
  if (( $#_rfig_hits )); then
    (( _rfig_selected = _rfig_selected < $#_rfig_hits ? _rfig_selected + 1 : $#_rfig_hits ))
  else
    zle down-line-or-history
    _rfig_selected=1
  fi
  _rfig_preview
  zle -R
}

_rfig_choose() {
  if (( ! $#_rfig_hits || CURSOR < ${#BUFFER} )); then
    zle forward-char
    return
  fi
  local -a previous_hits=( "${_rfig_hits[@]}" )
  local record=${_rfig_hits[_rfig_selected]}
  local rest=${record#*$'\t'} insert
  insert=${rest%%$'\t'*}
  local prefix=${BUFFER[1,_rfig_start]} suffix=${BUFFER[$(( CURSOR + 1 )),-1]}
  (( _rfig_injected )) && prefix+=' '
  local space=' '
  [[ -n $suffix || $insert == */ ]] && space=''
  BUFFER="$prefix$insert$space$suffix"
  CURSOR=$(( ${#prefix} + ${#insert} + ${#space} ))
  _rfig_selected=1
  _rfig_preview
  local index same=1
  if (( $#_rfig_hits != $#previous_hits )); then
    same=0
  else
    for (( index = 1; index <= $#_rfig_hits; index++ )); do
      [[ ${_rfig_hits[index]} == ${previous_hits[index]} ]] || { same=0; break; }
    done
  fi
  if (( same )); then
    POSTDISPLAY=''
    region_highlight=(${region_highlight:#*memo=rfig})
    _rfig_hits=()
  fi
  zle reset-prompt
}

_rfig_accept_line() {
  POSTDISPLAY=''
  region_highlight=(${region_highlight:#*memo=rfig})
  _rfig_hits=()
  zle -R
  zle .accept-line
}

zle -N _rfig_up
zle -N _rfig_down
zle -N _rfig_choose
zle -N _rfig_accept_line
for _rfig_keymap in emacs viins; do
  bindkey -M "$_rfig_keymap" '^[[A' _rfig_up
  bindkey -M "$_rfig_keymap" '^[[B' _rfig_down
  bindkey -M "$_rfig_keymap" '^[OA' _rfig_up
  bindkey -M "$_rfig_keymap" '^[OB' _rfig_down
  bindkey -M "$_rfig_keymap" '^[[C' _rfig_choose
  bindkey -M "$_rfig_keymap" '^[OC' _rfig_choose
  bindkey -M "$_rfig_keymap" '^M' _rfig_accept_line
  bindkey -M "$_rfig_keymap" '^I' expand-or-complete
done
unset _rfig_keymap
