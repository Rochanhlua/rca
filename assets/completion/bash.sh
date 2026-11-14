# rcac / rca bash completion
_rcac() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    if [[ $COMP_CWORD -eq 1 ]]; then
        COMPREPLY=( $(compgen -W "lex parse check ir ir-opt run run-tree compile build lint test fmt doc bench watch completion pkg" -- "$cur") )
    elif [[ "${COMP_WORDS[1]}" == "pkg" && $COMP_CWORD -eq 2 ]]; then
        COMPREPLY=( $(compgen -W "init add install" -- "$cur") )
    elif [[ $COMP_CWORD -eq 2 ]]; then
        COMPREPLY=( $(compgen -f -- "$cur") )
    fi
}
complete -F _rcac rcac
complete -F _rcac rca
