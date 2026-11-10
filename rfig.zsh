# Sourced by ~/.zshrc after installing rfig.
if (( ! $+functions[_main_complete] )); then
  autoload -Uz compinit
  compinit -D
fi
zmodload zsh/stat
zmodload zsh/zutil

typeset -gaU _rfig_hits
typeset -gA _rfig_option_kinds
typeset -gA _rfig_branch_candidates
typeset -gA _rfig_usage
_rfig_usage=()
typeset -g _rfig_option_source='' _rfig_option_stamp=''
typeset -g _rfig_history_match=''
typeset -g _rfig_history_dir=''
typeset -ga _rfig_history_lines
typeset -gi _rfig_selected=1 _rfig_injected=0

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
  (( $#_rfig_history_lines > 1000 )) && _rfig_history_lines=( "${_rfig_history_lines[-1000,-1][@]}" )
}

_rfig_history_record() {
  local line=$1 file="$HOME/.config/rfig/history.tsv"
  [[ -n $line && $line != *$'\n'* && $line != *$'\r'* && $line != *$'\x1f'* \
    && $PWD != *$'\n'* && $PWD != *$'\r'* && $PWD != *$'\x1f'* ]] || return
  [[ -o hist_ignore_space && $line == ' '* ]] && return
  (umask 077; mkdir -p -- "${file:h}" && touch -- "$file" && chmod 600 "$file" && printf '%s\x1f%s\n' "$PWD" "$line" >> "$file") || return
  [[ $_rfig_history_dir == $PWD ]] || _rfig_history_load_dir
  _rfig_history_lines+=( "$line" )
  (( $#_rfig_history_lines > 1000 )) && _rfig_history_lines=( "${_rfig_history_lines[-1000,-1][@]}" )
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
      ranked=( "${ranked[1,position-1][@]}" "$record" "${ranked[position,-1][@]}" )
      scores=( "${scores[1,position-1][@]}" "$score" "${scores[position,-1][@]}" )
    fi
  done
  reply=( "${ranked[@]}" )
}

_rfig_compadd() {
  local -a hits descriptions _rfig_records
  local -A addopts
  local item insert index kind hidden_prefix hidden_suffix file_name
  for item in "$@"; do
    if [[ $item == -A || $item == -D || $item == -O ]]; then
      builtin compadd "$@"
      return
    fi
  done
  zparseopts -E -A addopts a k q Q f e n U l 1 2 C P: S: p: s: i: I: W: d: J: X: x: V: o:: r: R: D: O: A: E: M: F:
  hidden_prefix=${addopts[-p]-}
  hidden_suffix=${addopts[-s]-}
  if (( ! ${+addopts[-Q]} )); then
    [[ -n $hidden_prefix ]] && hidden_prefix=${(q)hidden_prefix}
    [[ -n $hidden_suffix ]] && hidden_suffix=${(q)hidden_suffix}
  fi
  builtin compadd -A hits -D descriptions "$@"
  for (( index = 1; index <= $#hits; index++ )); do
    # -A quotes the match body, but omits compadd's surrounding insertion fields.
    insert=$hits[index]
    item=${(Q)insert}
    [[ -n $item && $item != *$'\n'* && $item != *$'\t'* ]] || continue
    file_name=${(Q)${addopts[-W]-$hidden_prefix}}$item
    if (( ${+addopts[-f]} )) && [[ -d $file_name && $insert != */ && -z $hidden_suffix ]]; then
      insert+='/'
    fi
    insert="${addopts[-i]-}${addopts[-P]-}$hidden_prefix$insert$hidden_suffix${addopts[-S]-}${addopts[-I]-}"
    (( ${+addopts[-U]} )) || insert="$IPREFIX$insert$ISUFFIX"
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
    _rfig_records+=( "$item"$'\t'"$insert"$'\t'"${descriptions[index]//$'\n'/ }"$'\t'"$kind" )
  done
  # Deduplicate once per batch instead of rescanning the unique array for each hit.
  _rfig_hits+=( "${_rfig_records[@]}" )
  builtin compadd "$@"
}

_rfig_capture() {
  _rfig_hits=()
  _rfig_branch_candidates=()
  typeset -g _rfig_start=$(( CURSOR - ${#PREFIX} ))
  local previous=${functions[compadd]-} had_previous=$+functions[compadd]
  functions[compadd]=$functions[_rfig_compadd]
  {
    # Candidates arrive through compadd; provider diagnostics must not redraw ZLE.
    _main_complete >/dev/null 2>&1
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
  _rfig_history_match=''
  [[ -n $BUFFER && $CURSOR == ${#BUFFER} && $BUFFER != *$'\n'* ]] || return
  local suggestion
  for suggestion in "${(@Oa)_rfig_history_lines}"; do
    [[ $suggestion == "$BUFFER"* && $suggestion != "$BUFFER" && $suggestion != *[[:cntrl:]]* ]] || continue
    _rfig_history_match=$suggestion
    return
  done
}

_rfig_render() {
  region_highlight=(${region_highlight:#*memo=rfig})
  POSTDISPLAY=''
  (( $#_rfig_hits )) || return
  (( CURSOR == ${#BUFFER} )) || return
  local record=${_rfig_hits[_rfig_selected]}
  local kind=${record##*$'\t'} rest=${record#*$'\t'} insert suggestion prospective
  insert=${rest%%$'\t'*}
  if [[ $kind == history ]]; then
    [[ $insert == "$BUFFER"* ]] || return
    prospective=$insert
  else
    local prefix=${BUFFER[1,_rfig_start]} space=' '
    (( _rfig_injected )) && prefix+=' '
    [[ $insert == */ ]] && space=''
    prospective="$prefix$insert$space"
  fi
  suggestion=${prospective[$(( CURSOR + 1 )),-1]}
  POSTDISPLAY=$suggestion
  if [[ -n $suggestion ]]; then
    region_highlight+=( "$(( ${#BUFFER} )) $(( ${#BUFFER} + ${#suggestion} )) fg=8 memo=rfig" )
  fi
  local count_start=$(( ${#BUFFER} + ${#POSTDISPLAY} + 1 ))
  local count_text=" ${_rfig_selected}/${#_rfig_hits}"
  POSTDISPLAY+=$'\n'$count_text
  region_highlight+=( "$count_start $((count_start + ${#count_text})) fg=8 memo=rfig" )
  local first=$(( _rfig_selected > 5 ? _rfig_selected - 4 : 1 ))
  local index row start label description icon color visible width label_width
  width=$(( COLUMNS > 1 ? COLUMNS - 1 : 0 ))
  label_width=$(( width > 52 ? 36 : width / 2 ))
  for (( index = first; index <= $#_rfig_hits && index < first + 5; index++ )); do
    record=${_rfig_hits[index]}
    label=${record%%$'\t'*}
    rest=${record#*$'\t'}
    rest=${rest#*$'\t'}
    description=${rest%%$'\t'*}
    kind=${record##*$'\t'}
    case $kind in
      history) icon='◷'; color=8 ;;
      command) icon='⌘'; color=cyan ;;
      subcommand) icon='↳'; color=magenta ;;
      argument) icon='●'; color=green ;;
      option) icon='◇'; color=blue ;;
      flag) icon='⚑'; color=yellow ;;
      *) icon='●'; color=green ;;
    esac
    visible=$(( index - first + 1 ))
    local pointer=' '
    (( index == _rfig_selected )) && pointer='›'
    printf -v row '%s%d %s %-*.*s %s' "$pointer" "$visible" "$icon" $label_width $label_width "$label" "$description"
    row=${row[1,width]}
    POSTDISPLAY+=$'\n'
    start=$(( ${#BUFFER} + ${#POSTDISPLAY} ))
    POSTDISPLAY+=$row
    region_highlight+=( "$start $((start + ${#row})) fg=$color memo=rfig" )
    if (( index == _rfig_selected )); then
      region_highlight+=( "$start $((start + ${#row})) bold memo=rfig" )
    fi
  done
}

_rfig_complete() {
  POSTDISPLAY=''
  region_highlight=(${region_highlight:#*memo=rfig})
  _rfig_hits=()
  _rfig_branch_candidates=()
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
      source "$HOME/.config/rfig/generated/$command_name.zsh" >/dev/null 2>&1
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
    elif [[ ${external_lines[1]-} == cache ]]; then
      for record in "${external_lines[@]:1}"; do
        item=${record%%$'\t'*}
        kind=${record#*$'\t'}
        [[ -n $item && $item == "$typed"* && ( $kind == subcommand || $kind == option || $kind == flag ) ]] || continue
        _rfig_hits+=( "$item"$'\t'"${(q)item}"$'\t'$'\t'"$kind" )
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
    if [[ $typed_prefix == --* && $typed_prefix != *=* ]]; then
      [[ $label == --* ]] || continue
    elif [[ $typed_prefix == -* && $typed_prefix != *=* ]]; then
      [[ $label == -* && $label != --* ]] || continue
    fi
    if [[ $label == -* ]]; then
      flags+=( "$record" )
    else
      subcommands+=( "$record" )
    fi
  done
  local context=${original_buffer[1,_rfig_start]}
  local -a context_words=( ${(z)context} )
  context="${(j: :)context_words}"
  _rfig_rank_group "$context" "${subcommands[@]}" "${flags[@]}"
  _rfig_hits=( "${reply[@]}" )
  (( $#_rfig_hits )) || return
  (( _rfig_selected > $#_rfig_hits )) && _rfig_selected=$#_rfig_hits
}

_rfig_preview() {
  _rfig_complete
  _rfig_history_suggest
  if [[ -n $_rfig_history_match ]]; then
    _rfig_hits=( "$_rfig_history_match"$'\t'"$_rfig_history_match"$'\tHistory\thistory' "${_rfig_hits[@]}" )
  fi
  (( _rfig_selected > $#_rfig_hits )) && _rfig_selected=$#_rfig_hits
  (( _rfig_selected < 1 )) && _rfig_selected=1
  _rfig_render
}

_rfig_after_edit() {
  zle ".$WIDGET" "$@"
  _rfig_selected=1
  _rfig_preview
  zle -R
}
for _rfig_widget in self-insert backward-delete-char delete-char backward-kill-word kill-word kill-whole-line bracketed-paste backward-char beginning-of-line; do
  zle -N "$_rfig_widget" _rfig_after_edit
done
unset _rfig_widget

_rfig_redraw_move() {
  if (( $#_rfig_hits )); then
    _rfig_render
  else
    _rfig_preview
  fi
  zle -R
}

_rfig_up() {
  if (( $#_rfig_hits )); then
    (( _rfig_selected = _rfig_selected > 1 ? _rfig_selected - 1 : 1 ))
  fi
  _rfig_redraw_move
}

_rfig_down() {
  if (( $#_rfig_hits )); then
    (( _rfig_selected = _rfig_selected < $#_rfig_hits ? _rfig_selected + 1 : $#_rfig_hits ))
  fi
  _rfig_redraw_move
}

_rfig_select_number() {
  local number=$1 first=$(( _rfig_selected > 5 ? _rfig_selected - 4 : 1 ))
  (( $#_rfig_hits )) || return
  (( first + number - 1 <= $#_rfig_hits )) || return
  _rfig_selected=$(( first + number - 1 ))
  _rfig_render
  zle -R
}

_rfig_choose() {
  if (( CURSOR < ${#BUFFER} )); then
    zle forward-char
    return
  fi
  if (( ! $#_rfig_hits )); then
    zle forward-char
    return
  fi
  local -a previous_hits=( "${_rfig_hits[@]}" )
  local record=${_rfig_hits[_rfig_selected]}
  local label=${record%%$'\t'*}
  local rest=${record#*$'\t'} insert
  insert=${rest%%$'\t'*}
  if [[ ${record##*$'\t'} == history ]]; then
    BUFFER=$insert
    CURSOR=${#BUFFER}
    _rfig_selected=1
    _rfig_preview
    zle -R
    return
  fi
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
  zle -R
}

_rfig_tab() {
  if (( $#_rfig_hits )); then
    _rfig_choose
  else
    zle expand-or-complete
  fi
}

_rfig_end_of_line() {
  zle .end-of-line
  _rfig_preview
  zle -R
}

_rfig_accept_line() {
  _rfig_history_record "$BUFFER"
  _rfig_record_line "$BUFFER"
  POSTDISPLAY=''
  _rfig_history_match=''
  region_highlight=(${region_highlight:#*memo=rfig})
  _rfig_hits=()
  zle -R
  zle .accept-line
}

zle -N _rfig_up
zle -N _rfig_down
zle -N _rfig_choose
zle -N _rfig_end_of_line
zle -N _rfig_accept_line
zle -N _rfig_tab
for _rfig_number in 1 2 3 4 5; do
  eval "_rfig_select_${_rfig_number}() { _rfig_select_number ${_rfig_number} }"
  zle -N "_rfig_select_${_rfig_number}"
done
unset _rfig_number
for _rfig_keymap in emacs viins; do
  # Alt/Option arrows are portable. Super/Command sequences are accepted when
  # the terminal is configured to forward them instead of handling them itself.
  bindkey -M "$_rfig_keymap" '^[[1;3A' _rfig_up
  bindkey -M "$_rfig_keymap" '^[[1;9A' _rfig_up
  bindkey -M "$_rfig_keymap" '^[[1;3D' _rfig_up
  bindkey -M "$_rfig_keymap" '^[[1;9D' _rfig_up
  bindkey -M "$_rfig_keymap" '^[[1;3B' _rfig_down
  bindkey -M "$_rfig_keymap" '^[[1;9B' _rfig_down
  bindkey -M "$_rfig_keymap" '^[[1;3C' _rfig_down
  bindkey -M "$_rfig_keymap" '^[[1;9C' _rfig_down
  for _rfig_number in 1 2 3 4 5; do
    bindkey -M "$_rfig_keymap" "^[${_rfig_number}" "_rfig_select_${_rfig_number}"
    printf -v _rfig_key $'\e[%d;3u' $(( 48 + _rfig_number ))
    bindkey -M "$_rfig_keymap" "$_rfig_key" "_rfig_select_${_rfig_number}"
    printf -v _rfig_key $'\e[%d;9u' $(( 48 + _rfig_number ))
    bindkey -M "$_rfig_keymap" "$_rfig_key" "_rfig_select_${_rfig_number}"
  done
  bindkey -M "$_rfig_keymap" '^M' _rfig_accept_line
  bindkey -M "$_rfig_keymap" '^E' _rfig_end_of_line
  bindkey -M "$_rfig_keymap" '^I' _rfig_tab
done
unset _rfig_keymap _rfig_number _rfig_key

# Rust owns keyboard input; ZLE remains the native completion provider.
# Noninteractive provider/unit-test shells keep access to the completion helpers above.
if [[ -o interactive && -z ${RFIG_PROVIDER-} ]] && (( $+commands[rfig] )); then
  zmodload zsh/zselect
  _rfig_count_bytes() {
    setopt localoptions nomultibyte
    _rfig_byte_value=${#1}
  }
  _rfig_prefix_bytes() {
    setopt localoptions nomultibyte
    _rfig_byte_value=${1[1,$2]}
  }
  _rfig_rust_edit() {
    setopt localoptions nomonitor
    if [[ $OSTYPE == linux* ]]; then
      # Linux ZLE can redraw over the Rust menu on SIGWINCH.
      setopt localtraps
      trap '' WINCH
    fi
    local snapshot action point updated child request id query prefix record rest label insert description kind
    local original_buffer=$BUFFER original_cursor=$CURSOR updated_key='' _rfig_byte_value
    POSTDISPLAY=''
    region_highlight=(${region_highlight:#*memo=rfig})
    zle -R
    snapshot=$(umask 077; mktemp -d "${TMPDIR:-/tmp}/rfig.XXXXXXXX") || return
    prefix=${original_buffer[1,original_cursor]}
    _rfig_count_bytes "$prefix"
    local byte_point=$_rfig_byte_value
    {
      local initial_key=''
      [[ $WIDGET == _rfig_rust_edit ]] && initial_key=$KEYS
      RFIG_INITIAL_KEY=$initial_key RFIG_PROVIDER=1 command rfig edit zsh "$snapshot" "$original_buffer" "$byte_point" "$byte_point" &
      child=$!
      while kill -0 "$child" 2>/dev/null; do
        if [[ -f $snapshot/request ]]; then
          request=$(<"$snapshot/request")
          command rm -f -- "$snapshot/request"
          id=${request%%$'\n'*}
          query=${request#*$'\n'}
          BUFFER=$query CURSOR=${#query}
          _rfig_complete
          prefix=${BUFFER[1,_rfig_start]}
          _rfig_count_bytes "$prefix"
          point=$_rfig_byte_value
          {
            print -r -- "$point"$'\t'"$_rfig_injected"
            for record in "${_rfig_hits[@]}"; do
              label=${record%%$'\t'*}; rest=${record#*$'\t'}
              insert=${rest%%$'\t'*}; rest=${rest#*$'\t'}
              description=${rest%%$'\t'*}; kind=${rest##*$'\t'}
              print -r -- "$label"$'\t'"$insert"$'\t'"$description"$'\t'"$kind"
            done
          } > "$snapshot/response.$id.tmp"
          command mv -- "$snapshot/response.$id.tmp" "$snapshot/response.$id"
        else
          zselect -t 1 2>/dev/null
        fi
      done
      wait "$child" 2>/dev/null
      if [[ -f $snapshot/result ]]; then
        { IFS= read -r action; IFS= read -r point; IFS= read -r -d '' updated || true; } < "$snapshot/result"
        BUFFER=$updated
        _rfig_prefix_bytes "$updated" "$point"
        CURSOR=${#_rfig_byte_value}
        if [[ $action == native ]]; then
          IFS= read -r -d '' updated_key < "$snapshot/key" || true
          [[ -f $snapshot/selection ]] && _rfig_selected=$(<"$snapshot/selection")
        fi
      else
        BUFFER=$original_buffer CURSOR=$original_cursor
      fi
    } always {
      if kill -0 "$child" 2>/dev/null; then kill -TERM "$child" 2>/dev/null; wait "$child" 2>/dev/null; fi
      command rm -rf -- "$snapshot"
      POSTDISPLAY=''
    }
    case $action in
      execute) zle .accept-line ;;
      complete) zle .expand-or-complete ;;
      search) zle .history-incremental-search-backward ;;
      escape) [[ $KEYMAP == viins ]] && zle .vi-cmd-mode ;;
      eof) zle -U $'\x04' ;;
      native) _rfig_preview; _rfig_render; zle -R; zle -U "$updated_key" ;;
      *) zle -R ;;
    esac
  }
  _rfig_rust_insert() {
    zle .self-insert
    _rfig_rust_edit
  }
  zle -N self-insert _rfig_rust_insert
  zle -N _rfig_rust_edit
  # Handoff bindings also cover an empty prompt. All in-session keys use crossterm.
  for _rfig_keymap in emacs viins; do
    bindkey -M "$_rfig_keymap" '^I' _rfig_rust_edit
    for _rfig_key in '^[[1;3A' '^[[1;3B' '^[[1;3C' '^[[1;3D' '^[^[[A' '^[^[[B' '^[^[[C' '^[^[[D'; do
      bindkey -M "$_rfig_keymap" "$_rfig_key" _rfig_rust_edit
    done
  done
  unset _rfig_keymap _rfig_key
fi
