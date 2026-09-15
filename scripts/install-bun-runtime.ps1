param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$RuntimeDirectory
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$RuntimeDirectory = [IO.Path]::GetFullPath($RuntimeDirectory)
$manifest = Get-Content -LiteralPath (Join-Path $projectRoot 'runtime/bun.json') -Raw | ConvertFrom-Json

New-Item -ItemType Directory -Path $RuntimeDirectory -Force | Out-Null
$bun = Join-Path $RuntimeDirectory 'bun.exe'
$bunReady = $false
if (Test-Path -LiteralPath $bun -PathType Leaf) {
    try {
        $actualVersion = & $bun --version
        $bunReady = $LASTEXITCODE -eq 0 -and $actualVersion -eq $manifest.version
    } catch {
        $bunReady = $false
    }
}

if (!$bunReady) {
    $cache = Join-Path $projectRoot 'target/bun-cache'
    New-Item -ItemType Directory -Path $cache -Force | Out-Null
    $archive = Join-Path $cache $manifest.archive
    if (!(Test-Path -LiteralPath $archive -PathType Leaf)) {
        $releaseUrl = "https://github.com/oven-sh/bun/releases/download/bun-v$($manifest.version)"
        Invoke-WebRequest -Uri "$releaseUrl/$($manifest.archive)" -OutFile $archive
    }
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $manifest.sha256) {
        throw 'Bun archive SHA256 mismatch.'
    }
    Expand-Archive -LiteralPath $archive -DestinationPath $cache -Force
    $cachedBun = Join-Path $cache "$([IO.Path]::GetFileNameWithoutExtension($manifest.archive))/bun.exe"
    $actualVersion = & $cachedBun --version
    if ($LASTEXITCODE -ne 0 -or $actualVersion -ne $manifest.version) {
        throw 'Bun version mismatch.'
    }
    Copy-Item -LiteralPath $cachedBun -Destination $bun -Force
}

foreach ($file in @('runner.ts', 'context.d.ts', 'bun.json', 'bunfig.toml')) {
    Copy-Item -LiteralPath (Join-Path $projectRoot "runtime/$file") -Destination $RuntimeDirectory -Force
}

Write-Output "Bun runtime ready: $RuntimeDirectory (Bun $($manifest.version))"
