param(
    [string]$RuntimeDirectory
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
if (!$RuntimeDirectory) { $RuntimeDirectory = Join-Path $projectRoot 'target/debug/runtime' }
$RuntimeDirectory = [IO.Path]::GetFullPath($RuntimeDirectory)
& (Join-Path $PSScriptRoot 'install-bun-runtime.ps1') -RuntimeDirectory $RuntimeDirectory
$bun = Join-Path $RuntimeDirectory 'bun.exe'

Push-Location $projectRoot
try {
    & $bun install --frozen-lockfile --ignore-scripts
    if ($LASTEXITCODE -ne 0) {
        throw 'Failed to install development dependencies.'
    }
} finally {
    Pop-Location
}

Write-Output "Development environment ready"
