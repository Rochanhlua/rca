# rcac / rca fish completion
for cmd in rcac rca
    complete -c $cmd -f
    complete -c $cmd -n __fish_use_subcommand -a lex -d "show tokens"
    complete -c $cmd -n __fish_use_subcommand -a parse -d "show syntax tree"
    complete -c $cmd -n __fish_use_subcommand -a check -d "type check only"
    complete -c $cmd -n __fish_use_subcommand -a ir -d "show bytecode"
    complete -c $cmd -n __fish_use_subcommand -a ir-opt -d "show optimized bytecode"
    complete -c $cmd -n __fish_use_subcommand -a run -d "run on the VM"
    complete -c $cmd -n __fish_use_subcommand -a run-tree -d "run on the tree interpreter"
    complete -c $cmd -n __fish_use_subcommand -a compile -d "generate native object"
    complete -c $cmd -n __fish_use_subcommand -a build -d "compile and link"
    complete -c $cmd -n __fish_use_subcommand -a lint -d "report warnings"
    complete -c $cmd -n __fish_use_subcommand -a test -d "run .rca tests"
    complete -c $cmd -n __fish_use_subcommand -a fmt -d "print canonical formatting"
    complete -c $cmd -n __fish_use_subcommand -a doc -d "print Markdown API docs"
    complete -c $cmd -n __fish_use_subcommand -a bench -d "run file N times"
    complete -c $cmd -n __fish_use_subcommand -a watch -d "re-run on change"
    complete -c $cmd -n __fish_use_subcommand -a completion -d "print shell completion"
    complete -c $cmd -n __fish_use_subcommand -a pkg -d "package manager"
    complete -c $cmd -n "__fish_seen_subcommand_from pkg" -a "init add install"
end
