_rfig_complete() {
    local word=${COMP_WORDS[COMP_CWORD]}
    case $COMP_CWORD:${COMP_WORDS[1]} in
        1:*) COMPREPLY=( $(compgen -W 'setup init analyze completion' -- "$word") ) ;;
        2:completion) COMPREPLY=( $(compgen -W 'zsh bash fish' -- "$word") ) ;;
        2:analyze) COMPREPLY=( $(compgen -c -- "$word") ) ;;
        2:setup) COMPREPLY=( $(compgen -W '--shell' -- "$word") ) ;;
        3:setup) COMPREPLY=( $(compgen -W 'zsh bash fish' -- "$word") ) ;;
    esac
}
complete -F _rfig_complete rfig
