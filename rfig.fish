# Fish adapter: rfig edits; Fish executes, expands abbreviations and owns the prompt.
status is-interactive; or return
set -q RFIG_PROVIDER; and return
set -q _rfig_fish_loaded; and return
set -g _rfig_fish_loaded 1
set -g fish_autosuggestion_enabled 0

function _rfig_record_execution --on-event fish_preexec
    command rfig remember "$argv[1]"
end

function _rfig_start
    set -l before (commandline --cut-at-cursor | string collect)
    set -l shown (string length -- "$before")
    commandline --insert -- "$argv[1]"
    set -l line (commandline | string collect)
    set -l point (commandline --cursor)
    set -l snapshot (umask 077; mktemp -d "$TMPDIR/rfig.XXXXXXXX" 2>/dev/null)
    if test -z "$snapshot"
        set snapshot (umask 077; mktemp -d /tmp/rfig.XXXXXXXX)
    end
    test -n "$snapshot"; or return
    functions (functions -n) > "$snapshot/definitions"
    printf 'set -g fish_function_path %s\n' (string join ' ' -- (string escape -- $fish_function_path)) >> "$snapshot/definitions"
    printf 'set -g fish_complete_path %s\n' (string join ' ' -- (string escape -- $fish_complete_path)) >> "$snapshot/definitions"
    complete >> "$snapshot/definitions"
    begin; functions -n; builtin -n; end > "$snapshot/names"
    set -lx RFIG_PROVIDER 1
    set -lx RFIG_SHELL_EXE (status fish-path)
    command rfig edit fish "$snapshot" "$line" "$point" "$shown"
    if test -f "$snapshot/result"
        set -l action
        set -l cursor
        set -l text
        begin
            read action
            read cursor
            read --null text
        end < "$snapshot/result"
        commandline --replace -- "$text"
        commandline --cursor $cursor
        switch $action
            case execute
                commandline -f expand-abbr execute
            case complete
                commandline -f complete
            case search
                commandline -f history-pager
            case escape
                if contains -- $fish_key_bindings fish_vi_key_bindings fish_hybrid_key_bindings
                    set -g fish_bind_mode default
                end
            case eof
                commandline -f exit
        end
    end
    command rm -rf -- "$snapshot"
    commandline -f repaint
end

set -l rfig_modes default insert
if contains -- $fish_key_bindings fish_vi_key_bindings fish_hybrid_key_bindings
    set rfig_modes insert
end
for mode in $rfig_modes
    for code in (seq 32 126)
        set -l key (printf "\\$(printf '%03o' $code)")
        set -l escaped (string escape -- "$key")
        bind -M $mode -- "$key" "_rfig_start $escaped"
    end
end

command rfig completion fish | source
