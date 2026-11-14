#compdef rcac rca
_rcac() {
    local -a commands
    commands=(
        'lex:show tokens'
        'parse:show syntax tree'
        'check:type check only'
        'ir:show bytecode'
        'ir-opt:show optimized bytecode'
        'run:run on the VM'
        'run-tree:run on the tree-walking interpreter'
        'compile:generate native object file'
        'build:compile and link'
        'lint:report warnings'
        'test:run .rca tests'
        'fmt:print canonical formatting'
        'doc:print Markdown API docs'
        'bench:run file N times'
        'watch:re-run on change'
        'completion:print shell completion'
        'pkg:package manager'
    )
    if (( CURRENT == 2 )); then
        _describe 'command' commands
    elif [[ $words[2] == pkg && $CURRENT -eq 3 ]]; then
        compadd init add install
    elif (( CURRENT == 3 )); then
        _files
    fi
}
_rcac "$@"
