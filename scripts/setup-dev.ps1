[CmdletBinding()]
param(
    [switch]$ForceRebuild
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$workspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$leptonicaLib = Join-Path $workspaceRoot ".ocr-release\leptonica\lib\leptonica-1.84.1.lib"
$tesseractLib = Join-Path $workspaceRoot ".ocr-release\tesseract\lib\tesseract53.lib"
$tessdataDir = Join-Path $workspaceRoot ".ocr-release\tessdata"
$appDataRoot = Join-Path $env:APPDATA "tesseract-rs"

function Require-Command {
    param([string]$Name)

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Missing required command: $Name"
    }
}

function Ensure-Directory {
    param([string]$Path)

    if (-not (Test-Path $Path)) {
        New-Item -ItemType Directory -Force -Path $Path | Out-Null
    }
}

function Sync-IfPresent {
    param(
        [string]$Source,
        [string]$Destination
    )

    if (Test-Path $Source) {
        Ensure-Directory (Split-Path -Parent $Destination)
        Copy-Item -Force $Source $Destination
    }
}

Sync-IfPresent `
    -Source (Join-Path $appDataRoot "leptonica\lib\leptonica.lib") `
    -Destination $leptonicaLib
Sync-IfPresent `
    -Source (Join-Path $appDataRoot "tesseract\lib\tesseract.lib") `
    -Destination $tesseractLib

foreach ($language in @("eng", "chi", "chi_sim")) {
    Sync-IfPresent `
        -Source (Join-Path $appDataRoot "tessdata\$language.traineddata") `
        -Destination (Join-Path $tessdataDir "$language.traineddata")
}

$needsBuild = $ForceRebuild -or -not (Test-Path $leptonicaLib) -or -not (Test-Path $tesseractLib) -or -not (Test-Path $tessdataDir)

if ($needsBuild) {
    Require-Command "cmake"

    if (-not (Get-Command "cl" -ErrorAction SilentlyContinue)) {
        throw "MSVC compiler not found. Open a Developer PowerShell for Visual Studio and rerun scripts\\setup-dev.ps1."
    }

    Write-Host "Building OCR release dependencies..."
    & (Join-Path $PSScriptRoot "build-ocr-release.ps1") -WorkspaceRoot $workspaceRoot
    if ($LASTEXITCODE -ne 0) {
        throw "build-ocr-release.ps1 failed with exit code $LASTEXITCODE"
    }
}

Write-Host ""
Write-Host "Setup complete."
Write-Host "You can now run cargo commands from $workspaceRoot"
