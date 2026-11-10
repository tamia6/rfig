# Run from a source checkout or a directory containing a prebuilt rfig.exe.
$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSVersion -lt [version]'7.2') { throw 'PowerShell 7.2+ is required.' }
$root = $PSScriptRoot
if (Test-Path (Join-Path $root 'Cargo.toml')) {
    Push-Location $root
    try { cargo build --release --locked; if ($LASTEXITCODE) { throw 'cargo build failed' } } finally { Pop-Location }
    $binary = Join-Path $root 'target/release/rfig.exe'
} else { $binary = Join-Path $root 'rfig.exe' }
$bin = Join-Path $HOME '.local/bin'
New-Item -ItemType Directory -Force $bin > $null
Copy-Item $binary (Join-Path $bin 'rfig.exe') -Force
$env:PATH = "$bin;$env:PATH"
$previousProfile = $env:RFIG_POWERSHELL_PROFILE
try {
    $env:RFIG_POWERSHELL_PROFILE = $PROFILE
    & (Join-Path $bin 'rfig.exe') setup --shell powershell
    if ($LASTEXITCODE) { throw 'rfig setup failed' }
} finally { $env:RFIG_POWERSHELL_PROFILE = $previousProfile }
. (Join-Path $HOME '.config/rfig/rfig.ps1')
