Register-ArgumentCompleter -Native -CommandName rfig -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)
    @('setup', 'init', 'analyze', 'completion', 'terminal-keys', '--shell', 'powershell', 'zsh', 'bash', 'fish') |
        Where-Object { $_.StartsWith($wordToComplete) } |
        ForEach-Object { [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_) }
}
