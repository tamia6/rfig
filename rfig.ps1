# PowerShell 7.2+: the host executes accepted input; rfig owns terminal editing.
if ($PSVersionTable.PSVersion -lt [version]'7.2') {
    throw 'rfig requires PowerShell 7.2 or later (pwsh).'
}
if ($env:RFIG_PROVIDER) { return }
# Load before installing the host hook so lazy PSReadLine import cannot replace it.
Import-Module PSReadLine -ErrorAction Stop
function global:PSConsoleHostReadLine {
    $anchorX = [Console]::CursorLeft
    $anchorY = [Console]::CursorTop
    if ($IsWindows) { $anchorY = [Math]::Max(0, $anchorY - [Console]::WindowTop) }
    $snapshot = Join-Path ([IO.Path]::GetTempPath()) ('rfig.' + [guid]::NewGuid().ToString('N'))
    [IO.Directory]::CreateDirectory($snapshot) > $null
    if (-not $IsWindows) { [IO.File]::SetUnixFileMode($snapshot, [IO.UnixFileMode]448) }
    $encoding = [Text.UTF8Encoding]::new($false)
    $process = $null
    try {
        $info = [Diagnostics.ProcessStartInfo]::new()
        $info.FileName = (Get-Command rfig -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
        $info.UseShellExecute = $false
        $info.WorkingDirectory = (Get-Location -PSProvider FileSystem).ProviderPath
        $info.Environment['PWD'] = $PWD.Path
        $info.Environment['RFIG_PROVIDER'] = '1'
        $info.Environment['RFIG_ANCHOR_X'] = [string]$anchorX
        $info.Environment['RFIG_ANCHOR_Y'] = [string]$anchorY
        foreach ($argument in @('edit', 'powershell', $snapshot, '', '0', '0')) { $info.ArgumentList.Add($argument) }
        $process = [Diagnostics.Process]::Start($info)
        while (-not $process.HasExited) {
            $request = Join-Path $snapshot 'request'
            if ([IO.File]::Exists($request)) {
                $data = [IO.File]::ReadAllText($request, $encoding)
                [IO.File]::Delete($request)
                $split = $data.IndexOf("`n")
                $id = $data.Substring(0, $split)
                $line = $data.Substring($split + 1)
                $reply = [Collections.Generic.List[string]]::new()
                try {
                    # Registered completers run in the current session; input remains data.
                    $completion = [Management.Automation.CommandCompletion]::CompleteInput($line, $line.Length, $null)
                    $start = [Text.Encoding]::UTF8.GetByteCount($line.Substring(0, $completion.ReplacementIndex))
                    $reply.Add("$start`tfalse")
                    foreach ($item in $completion.CompletionMatches) {
                        $label = $item.ListItemText -replace '[\r\n\t]', ' '
                        $insert = $item.CompletionText
                        if ($insert -match '[\r\n\t]') { continue }
                        $description = $item.ToolTip -replace '[\r\n\t]', ' '
                        $kind = switch ($item.ResultType.ToString()) {
                            'Command' { 'command' }; 'ParameterName' { 'option' }; default { 'argument' }
                        }
                        $reply.Add("$label`t$insert`t$description`t$kind")
                    }
                } catch { $reply.Clear(); $reply.Add("$([Text.Encoding]::UTF8.GetByteCount($line))`tfalse") }
                [IO.File]::WriteAllLines((Join-Path $snapshot "response.$id.tmp"), $reply, $encoding)
                [IO.File]::Move((Join-Path $snapshot "response.$id.tmp"), (Join-Path $snapshot "response.$id"))
            } else { Start-Sleep -Milliseconds 5 }
        }
        $resultFile = Join-Path $snapshot 'result'
        if (-not [IO.File]::Exists($resultFile)) { throw "rfig editor exited with code $($process.ExitCode)" }
        $result = [IO.File]::ReadAllText($resultFile, $encoding) -split "`n", 3
        $line = $result[2]
        [Console]::WriteLine($line)
        if ($result[0] -eq 'execute') { return $line }
        if ($result[0] -eq 'eof') { exit }
        return ''
    } finally {
        if ($null -ne $process) {
            if (-not $process.HasExited) { $process.Kill($true); $process.WaitForExit() }
            $process.Dispose()
        }
        [IO.Directory]::Delete($snapshot, $true)
    }
}

$completionScript = Join-Path $PSScriptRoot 'rfig-completion.ps1'
if (-not (Test-Path $completionScript)) { $completionScript = Join-Path $PSScriptRoot 'shell/rfig-completion.ps1' }
. $completionScript
