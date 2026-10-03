# Sourced by ~/.zshrc after installing rfig.
if (( ! $+functions[_main_complete] )); then
  autoload -Uz compinit
  compinit -D
fi
zmodload zsh/stat

typeset -gaU _rfig_hits
typeset -gA _rfig_option_kinds
typeset -gA _rfig_branch_candidates
typeset -gA _rfig_usage
_rfig_usage=()
typeset -g _rfig_option_source='' _rfig_option_stamp=''
typeset -g _rfig_ghost=''
typeset -g _rfig_history_dir=''
typeset -ga _rfig_history_lines
typeset -gi _rfig_selected=1 _rfig_pulse=0 _rfig_injected=0 _rfig_popular_count=0

_rfig_load_usage() {
  local kind context label key weight
  local file="$HOME/.config/rfig/usage.log"
  [[ -r $file ]] || return
  while IFS=$'\x1f' read -r kind context label; do
    [[ ( $kind == C || $kind == R ) && -n $label ]] || continue
    key="$context"$'\x1f'"$label"
    weight=1
    [[ $kind == C ]] && weight=3
    _rfig_usage[$key]=$(( ${_rfig_usage[$key]:-0} + weight ))
  done < "$file"
}
_rfig_load_usage

_rfig_record() {
  local kind=$1 context=$2 label=$3 key weight=1
  [[ -n $label && $context != *$'\n'* && $context != *$'\r'* && $context != *$'\x1f'* \
    && $label != *$'\n'* && $label != *$'\r'* && $label != *$'\x1f'* ]] || return
  local file="$HOME/.config/rfig/usage.log"
  # ponytail: append-only log; compact it if startup parsing becomes slow.
  (umask 077; mkdir -p -- "${file:h}" && printf '%s\x1f%s\x1f%s\n' "$kind" "$context" "$label" >> "$file") || return
  key="$context"$'\x1f'"$label"
  [[ $kind == C ]] && weight=3
  _rfig_usage[$key]=$(( ${_rfig_usage[$key]:-0} + weight ))
}

_rfig_record_line() {
  local line=$1 context='' token
  [[ $line != *$'\n'* ]] || return
  local -a words=( ${(z)line} )
  for token in "${words[@]}"; do
    [[ $token == '|' || $token == ';' || $token == '&&' || $token == '||' ]] && break
    token=${(Q)token}
    _rfig_record R "$context" "$token"
    context+="${context:+ }$token"
  done
}

_rfig_history_load_dir() {
  _rfig_history_dir=$PWD
  _rfig_history_lines=()
  local dir line file="$HOME/.config/rfig/history.tsv"
  [[ -r $file ]] || return
  while IFS=$'\x1f' read -r dir line; do
    [[ $dir == $_rfig_history_dir && -n $line ]] || continue
    _rfig_history_lines+=( "$line" )
  done < "$file"
  (( $#_rfig_history_lines > 1000 )) && _rfig_history_lines=( "${_rfig_history_lines[-1000,-1]}" )
}

_rfig_history_record() {
  local line=$1 file="$HOME/.config/rfig/history.tsv"
  [[ -n $line && $line != *$'\n'* && $line != *$'\r'* && $line != *$'\x1f'* \
    && $PWD != *$'\n'* && $PWD != *$'\r'* && $PWD != *$'\x1f'* ]] || return
  [[ -o hist_ignore_space && $line == ' '* ]] && return
  (umask 077; mkdir -p -- "${file:h}" && touch -- "$file" && chmod 600 "$file" && printf '%s\x1f%s\n' "$PWD" "$line" >> "$file") || return
  [[ $_rfig_history_dir == $PWD ]] || _rfig_history_load_dir
  _rfig_history_lines+=( "$line" )
  (( $#_rfig_history_lines > 1000 )) && _rfig_history_lines=( "${_rfig_history_lines[-1000,-1]}" )
}

autoload -Uz add-zsh-hook
add-zsh-hook chpwd _rfig_history_load_dir
_rfig_history_load_dir

_rfig_rank_group() {
  local context=$1 record label key score position
  shift
  local -a ranked=() scores=()
  for record in "$@"; do
    label=${record%%$'\t'*}
    key="$context"$'\x1f'"$label"
    score=${_rfig_usage[$key]:-0}
    if (( score == 0 )); then
      ranked+=( "$record" )
      scores+=( 0 )
      continue
    fi
    position=1
    while (( position <= $#ranked && scores[position] >= score )); do
      (( position++ ))
    done
    if (( position == 1 )); then
      ranked=( "$record" "${ranked[@]}" )
      scores=( "$score" "${scores[@]}" )
    elif (( position > $#ranked )); then
      ranked+=( "$record" )
      scores+=( "$score" )
    else
      ranked=( "${ranked[1,position-1]}" "$record" "${ranked[position,-1]}" )
      scores=( "${scores[1,position-1]}" "$score" "${scores[position,-1]}" )
    fi
  done
  reply=( "${ranked[@]}" )
}

_rfig_compadd() {
  local -a hits descriptions
  local item insert index kind
  for item in "$@"; do
    if [[ $item == -A || $item == -D || $item == -O ]]; then
      builtin compadd "$@"
      return
    fi
  done
  builtin compadd -A hits -D descriptions "$@"
  for (( index = 1; index <= $#hits; index++ )); do
    # -A returns the shell-ready insertion, including quoting applied by zsh.
    # Keep it intact; quoting it again inserts literal backslashes into arguments.
    insert=$hits[index]
    item=${(Q)insert}
    [[ -n $item && $item != *$'\n'* && $item != *$'\t'* ]] || continue
    if [[ ${curtag-} == *branch* || ${curtag-} == heads* || ${curtag-} == *-heads* ]]; then
      _rfig_branch_candidates[$item]=1
    fi
    if [[ -n ${_rfig_option_kinds[$item]-} ]]; then
      kind=${_rfig_option_kinds[$item]}
    elif [[ $item == --?* ]]; then
      kind=option
    elif [[ $item == -* ]]; then
      kind=flag
    elif [[ $curcontext == *:-command-:* ]]; then
      kind=command
    elif [[ ${_type-} == *command* ]] || { (( CURRENT == 2 )) && [[ $curcontext == *:argument-1 ]]; }; then
      kind=subcommand
    else
      kind=argument
    fi
    _rfig_hits+=( "$item"$'\t'"$insert"$'\t'"${descriptions[index]//$'\n'/ }"$'\t'"$kind" )
  done
  builtin compadd "$@"
}

_rfig_capture() {
  _rfig_hits=()
  _rfig_branch_candidates=()
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

_rfig_history_suggest() {
  _rfig_ghost=''
  [[ -n $BUFFER && $CURSOR == ${#BUFFER} && $BUFFER != *$'\n'* ]] || return
  local suggestion
  for suggestion in "${(@Oa)_rfig_history_lines}"; do
    [[ $suggestion == "$BUFFER"* && $suggestion != "$BUFFER" ]] || continue
    _rfig_ghost=${suggestion#$BUFFER}
    return
  done
}

_rfig_show_ghost() {
  POSTDISPLAY=$_rfig_ghost
  [[ -n $_rfig_ghost ]] &&
    region_highlight+=( "${#BUFFER} $(( ${#BUFFER} + ${#_rfig_ghost} )) fg=8 memo=rfig" )
}

_rfig_render() {
  region_highlight=(${region_highlight:#*memo=rfig})
  _rfig_show_ghost
  (( $#_rfig_hits )) || return
  local first=$(( _rfig_selected > 5 ? _rfig_selected - 4 : 1 ))
  local index description row start kind icon color rest record label icon_offset label_offset width
  local -a popular_icons=( '①' '②' '③' )
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
    (( index <= _rfig_popular_count )) && icon=$popular_icons[index]
    POSTDISPLAY+=$'\n'
    start=$(( ${#BUFFER} + ${#POSTDISPLAY} ))
    icon_offset=2
    label_offset=4
    width=41
    if (( index == _rfig_selected )); then
      if (( _rfig_pulse )); then
        printf -v row ' %s %s   %-38.38s' '➜' "$icon" "$label"
        icon_offset=3
        label_offset=7
        width=38
      else
        printf -v row '→ %s %-41.41s' "$icon" "$label"
      fi
    elif (( _rfig_pulse && (index == _rfig_selected - 1 || index == _rfig_selected + 1) )); then
      printf -v row '  %s  %-40.40s' "$icon" "$label"
      label_offset=5
      width=40
    else
      printf -v row '  %s %-41.41s' "$icon" "$label"
    fi
    POSTDISPLAY+=$row
    if (( index == _rfig_selected )); then
      if (( _rfig_pulse )); then
        region_highlight+=( "$((start + 1)) $((start + 2)) fg=cyan,bold memo=rfig" )
      else
        region_highlight+=( "$start $((start + 1)) fg=cyan,bold memo=rfig" )
      fi
    fi
    region_highlight+=( "$((start + icon_offset)) $((start + icon_offset + 1)) fg=$color memo=rfig" )
    if (( index == _rfig_selected || (_rfig_pulse && (index == _rfig_selected - 1 || index == _rfig_selected + 1)) )); then
      local label_style=bold
      (( index == _rfig_selected )) && label_style=fg=cyan,bold
      region_highlight+=( "$((start + label_offset)) $((start + label_offset + (${#label} < width ? ${#label} : width))) $label_style memo=rfig" )
    fi
  done
  POSTDISPLAY+=$'\n'
  printf -v row ' %-44.44s' "$description"
  POSTDISPLAY+=$row
}

_rfig_preview() {
  POSTDISPLAY=''
  region_highlight=(${region_highlight:#*memo=rfig})
  _rfig_history_suggest
  _rfig_show_ghost
  _rfig_hits=()
  _rfig_branch_candidates=()
  _rfig_popular_count=0
  _rfig_injected=0
  [[ -n $BUFFER && $BUFFER != *$'\n'* ]] || return

  local original_buffer=$BUFFER original_cursor=$CURSOR command_name=${BUFFER%% *} record
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
  if (( $+commands[$command_name] )) && [[ -z ${_comps[$command_name]-} ]] && [[ $command_name != *[^a-zA-Z0-9_.+-]* ]]; then
    local completion_dir
    for completion_dir in "${fpath[@]}"; do
      if [[ -r "$completion_dir/_$command_name" ]]; then
        autoload -Uz "_$command_name"
        compdef "_$command_name" "$command_name"
        break
      fi
    done
    if [[ -z ${_comps[$command_name]-} && -r "$HOME/.config/rfig/generated/$command_name.zsh" ]]; then
      source "$HOME/.config/rfig/generated/$command_name.zsh" 2>/dev/null
    fi
  fi
  if (( $+commands[$command_name] )) && [[ -z ${_comps[$command_name]-} ]]; then
    local remainder=${original_buffer#$command_name} fallback="$HOME/.config/rfig/fallback/$command_name.tsv"
    [[ $original_cursor == ${#original_buffer} && ( -z $remainder || $remainder == ' '* ) ]] || return
    remainder=${remainder#' '}
    local typed=${remainder##*' '}
    typeset -g _rfig_start=$(( original_cursor - ${#typed} ))
    [[ $original_buffer == $command_name ]] && _rfig_injected=1
    local external='' item kind description
    if (( $+commands[rfig] )); then
      local completion_line=$original_buffer
      (( _rfig_injected )) && completion_line+=' '
      external=$(command rfig external "$command_name" "$completion_line" 2>/dev/null)
    fi
    local -a external_lines=( ${(f)external} )
    if [[ ${external_lines[1]-} == provider ]]; then
      for record in "${external_lines[@]:1}"; do
        item=${record%%$'\t'*}
        description=${record#*$'\t'}
        [[ $record == *$'\t'* ]] || description=''
        [[ -n $item && $item == "$typed"* ]] || continue
        if [[ $item == --* ]]; then kind=option
        elif [[ $item == -* ]]; then kind=flag
        elif [[ $remainder != *' '* ]]; then kind=subcommand
        else kind=argument
        fi
        _rfig_hits+=( "$item"$'\t'"${(q)item}"$'\t'"$description"$'\t'"$kind" )
      done
    else
      [[ $remainder != *' '* ]] || return
      [[ -r $fallback ]] || return
      while IFS=$'\t' read -r item kind; do
        [[ -n $item && $item == "$remainder"* && ( $kind == subcommand || $kind == option || $kind == flag ) ]] || continue
        _rfig_hits+=( "$item"$'\t'"${(q)item}"$'\t'$'\t'"$kind" )
      done < "$fallback"
    fi
  else
    if [[ $BUFFER == $command_name && $CURSOR == ${#BUFFER} && -n ${_comps[$command_name]-} ]]; then
      BUFFER+=' '
      CURSOR=${#BUFFER}
      _rfig_injected=1
    fi
    zle _rfig_capture
    BUFFER=$original_buffer
    CURSOR=$original_cursor
  fi

  (( $#_rfig_hits )) || return
  local -a subcommands flags
  local label typed_prefix=${original_buffer[$((_rfig_start + 1)),$original_cursor]}
  if (( $#_rfig_branch_candidates )); then
    local -a branch_hits
    for record in "${_rfig_hits[@]}"; do
      label=${record%%$'\t'*}
      [[ $label == -* || -n ${_rfig_branch_candidates[$label]-} ]] && branch_hits+=( "$record" )
    done
    _rfig_hits=( "${branch_hits[@]}" )
  fi
  for record in "${_rfig_hits[@]}"; do
    label=${record%%$'\t'*}
    if [[ $typed_prefix == --* ]]; then
      [[ $label == --* ]] || continue
    elif [[ $typed_prefix == -* ]]; then
      [[ $label == -* && $label != --* ]] || continue
    fi
    if [[ $label == -* ]]; then
      flags+=( "$record" )
    else
      subcommands+=( "$record" )
    fi
  done
  local context=${original_buffer[1,_rfig_start]} key
  local -a context_words=( ${(z)context} )
  context="${(j: :)context_words}"
  _rfig_rank_group "$context" "${subcommands[@]}" "${flags[@]}"
  _rfig_hits=( "${reply[@]}" )
  (( $#_rfig_hits )) || return
  for record in "${_rfig_hits[@]}"; do
    label=${record%%$'\t'*}
    key="$context"$'\x1f'"$label"
    (( ${_rfig_usage[$key]:-0} > 0 )) || break
    (( ++_rfig_popular_count == 3 )) && break
  done
  (( _rfig_selected > $#_rfig_hits )) && _rfig_selected=$#_rfig_hits
  _rfig_render
}

_rfig_after_edit() {
  zle ".$WIDGET" "$@"
  _rfig_selected=1
  _rfig_pulse=0
  _rfig_preview
  zle -R
}
for _rfig_widget in self-insert backward-delete-char delete-char backward-kill-word kill-word kill-whole-line bracketed-paste backward-char beginning-of-line; do
  zle -N "$_rfig_widget" _rfig_after_edit
done
unset _rfig_widget

_rfig_redraw_move() {
  local previous=$1
  if (( $#_rfig_hits )); then
    if (( previous != _rfig_selected )); then
      _rfig_pulse=1
      _rfig_render
      zle -R
      sleep 0.06
      _rfig_pulse=0
    fi
    _rfig_render
  else
    _rfig_preview
  fi
  zle -R
}

_rfig_up() {
  local previous=$_rfig_selected
  if (( $#_rfig_hits )); then
    (( _rfig_selected = _rfig_selected > 1 ? _rfig_selected - 1 : 1 ))
  else
    zle up-line-or-history
    _rfig_selected=1
  fi
  _rfig_redraw_move $previous
}

_rfig_down() {
  local previous=$_rfig_selected
  if (( $#_rfig_hits )); then
    (( _rfig_selected = _rfig_selected < $#_rfig_hits ? _rfig_selected + 1 : $#_rfig_hits ))
  else
    zle down-line-or-history
    _rfig_selected=1
  fi
  _rfig_redraw_move $previous
}

_rfig_choose() {
  if (( CURSOR < ${#BUFFER} )); then
    zle forward-char
    return
  fi
  if (( ! $#_rfig_hits )); then
    if [[ -n $_rfig_ghost ]]; then
      BUFFER+=$_rfig_ghost
      CURSOR=${#BUFFER}
      _rfig_preview
      zle -R
    else
      zle forward-char
    fi
    return
  fi
  local -a previous_hits=( "${_rfig_hits[@]}" )
  local record=${_rfig_hits[_rfig_selected]}
  local label=${record%%$'\t'*}
  local rest=${record#*$'\t'} insert
  insert=${rest%%$'\t'*}
  local prefix=${BUFFER[1,_rfig_start]} suffix=${BUFFER[$(( CURSOR + 1 )),-1]}
  local -a context_words=( ${(z)prefix} )
  local context="${(j: :)context_words}"
  (( _rfig_injected )) && prefix+=' '
  local space=' '
  [[ -n $suffix || $insert == */ ]] && space=''
  BUFFER="$prefix$insert$space$suffix"
  CURSOR=$(( ${#prefix} + ${#insert} + ${#space} ))
  _rfig_record C "$context" "$label"
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
    _rfig_hits=()
    _rfig_render
  fi
  zle reset-prompt
}

_rfig_accept_ghost() {
  if [[ -n $_rfig_ghost && $CURSOR == ${#BUFFER} ]]; then
    BUFFER+=$_rfig_ghost
    CURSOR=${#BUFFER}
  else
    zle .end-of-line
  fi
  _rfig_preview
  zle -R
}

_rfig_accept_line() {
  [[ -n $_rfig_ghost && $CURSOR == ${#BUFFER} ]] && BUFFER+=$_rfig_ghost
  _rfig_history_record "$BUFFER"
  _rfig_record_line "$BUFFER"
  POSTDISPLAY=''
  _rfig_ghost=''
  region_highlight=(${region_highlight:#*memo=rfig})
  _rfig_hits=()
  zle -R
  zle .accept-line
}

zle -N _rfig_up
zle -N _rfig_down
zle -N _rfig_choose
zle -N _rfig_accept_ghost
zle -N _rfig_accept_line
for _rfig_keymap in emacs viins; do
  bindkey -M "$_rfig_keymap" '^[[A' _rfig_up
  bindkey -M "$_rfig_keymap" '^[[B' _rfig_down
  bindkey -M "$_rfig_keymap" '^[OA' _rfig_up
  bindkey -M "$_rfig_keymap" '^[OB' _rfig_down
  bindkey -M "$_rfig_keymap" '^[[C' _rfig_choose
  bindkey -M "$_rfig_keymap" '^[OC' _rfig_choose
  bindkey -M "$_rfig_keymap" '^M' _rfig_accept_line
  bindkey -M "$_rfig_keymap" '^E' _rfig_accept_ghost
  bindkey -M "$_rfig_keymap" '^I' expand-or-complete
done
unset _rfig_keymap
