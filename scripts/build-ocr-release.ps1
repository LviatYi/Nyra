[CmdletBinding()]
param(
    [string]$WorkspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
    [string]$LeptonicaVersion = "1.84.1",
    [string]$TesseractVersion = "5.3.4",
    [string[]]$Languages = @("eng", "chi", "chi_sim")
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Require-Command {
    param([string]$Name)

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Missing required command: $Name"
    }
}

function Ensure-Directory {
    param([string]$Path)

    if (-not (Test-Path $Path)) {
        New-Item -ItemType Directory -Path $Path | Out-Null
    }
}

function Download-And-Extract {
    param(
        [string]$Url,
        [string]$DestinationRoot,
        [string]$Name
    )

    $archivePath = Join-Path $DestinationRoot "$Name.zip"
    $extractRoot = Join-Path $DestinationRoot "$Name-extract"
    $finalPath = Join-Path $DestinationRoot $Name

    if (Test-Path $finalPath) {
        return $finalPath
    }

    Ensure-Directory $DestinationRoot
    if (Test-Path $archivePath) {
        Remove-Item -Force $archivePath
    }
    if (Test-Path $extractRoot) {
        Remove-Item -Recurse -Force $extractRoot
    }

    Write-Host "Downloading $Name from $Url"
    Invoke-WebRequest -Uri $Url -OutFile $archivePath
    Expand-Archive -Path $archivePath -DestinationPath $extractRoot -Force

    $expanded = Get-ChildItem -Path $extractRoot -Directory | Select-Object -First 1
    if (-not $expanded) {
        throw "Failed to extract $Name"
    }

    Move-Item -Force $expanded.FullName $finalPath
    Remove-Item -Recurse -Force $extractRoot
    Remove-Item -Force $archivePath
    return $finalPath
}

function Find-VsGenerator {
    $candidates = @(
        "Visual Studio 17 2022",
        "Visual Studio 16 2019"
    )

    foreach ($candidate in $candidates) {
        try {
            & cmake -G $candidate -A x64 --version *> $null
            return $candidate
        } catch {
        }
    }

    throw "Unable to find a supported Visual Studio CMake generator."
}

function Patch-LeptonicaSource {
    param([string]$LeptonicaSource)

    $bmpioPath = Join-Path $LeptonicaSource "src\bmpio.c"
    if (-not (Test-Path $bmpioPath)) {
        throw "Leptonica source is missing $bmpioPath"
    }

    $bmpio = Get-Content $bmpioPath -Raw
    $debugBlock = @"
#if DEBUG
    {l_uint8  *pcmptr;
        pcmptr = (l_uint8 *)pixGetColormap(pix)->array;
        lept_stderr("Pix colormap[0] = %c%c%c%d\n",
                    pcmptr[0], pcmptr[1], pcmptr[2], pcmptr[3]);
        lept_stderr("Pix colormap[1] = %c%c%c%d\n",
                    pcmptr[4], pcmptr[5], pcmptr[6], pcmptr[7]);
    }
#endif  /* DEBUG */
"@

    if ($bmpio.Contains($debugBlock)) {
        $bmpio = $bmpio.Replace($debugBlock, "")
        Set-Content -Path $bmpioPath -Value $bmpio -NoNewline
    }
}

function Invoke-CMake {
    param([string[]]$Arguments)

    & cmake @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "cmake failed with exit code $LASTEXITCODE"
    }
}

function Invoke-CMakeBuild {
    param([string[]]$Arguments)

    & cmake @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "cmake build failed with exit code $LASTEXITCODE"
    }
}

function Sync-File {
    param(
        [string]$Source,
        [string]$Destination
    )

    Ensure-Directory (Split-Path -Parent $Destination)
    Copy-Item -Force $Source $Destination
}

Require-Command "cmake"
Require-Command "cl"

$workspaceRoot = (Resolve-Path $WorkspaceRoot).Path
$buildRoot = Join-Path $workspaceRoot ".build"
$installRoot = Join-Path $workspaceRoot ".ocr-release"
$downloadRoot = Join-Path $workspaceRoot ".deps"
$generator = Find-VsGenerator

$leptonicaSource = Download-And-Extract `
    -Url "https://github.com/DanBloomberg/leptonica/archive/refs/tags/$LeptonicaVersion.zip" `
    -DestinationRoot $downloadRoot `
    -Name "leptonica-$LeptonicaVersion"
$tesseractSource = Download-And-Extract `
    -Url "https://github.com/tesseract-ocr/tesseract/archive/refs/tags/$TesseractVersion.zip" `
    -DestinationRoot $downloadRoot `
    -Name "tesseract-$TesseractVersion"

Patch-LeptonicaSource -LeptonicaSource $leptonicaSource

$leptonicaBuild = Join-Path $buildRoot "leptonica-release"
$tesseractBuild = Join-Path $buildRoot "tesseract-release"
$leptonicaInstall = Join-Path $installRoot "leptonica"
$tesseractInstall = Join-Path $installRoot "tesseract"
$tessdataInstall = Join-Path $installRoot "tessdata"

if (Test-Path $leptonicaBuild) { Remove-Item -Recurse -Force $leptonicaBuild }
if (Test-Path $tesseractBuild) { Remove-Item -Recurse -Force $tesseractBuild }

Invoke-CMake -Arguments @(
    "-S", $leptonicaSource,
    "-B", $leptonicaBuild,
    "-G", $generator,
    "-A", "x64",
    "-DCMAKE_POLICY_VERSION_MINIMUM:STRING=3.5",
    "-DCMAKE_BUILD_TYPE=Release",
    "-DBUILD_PROG=OFF",
    "-DBUILD_SHARED_LIBS=OFF",
    "-DENABLE_ZLIB=OFF",
    "-DENABLE_PNG=OFF",
    "-DENABLE_JPEG=OFF",
    "-DENABLE_TIFF=OFF",
    "-DENABLE_WEBP=OFF",
    "-DENABLE_OPENJPEG=OFF",
    "-DENABLE_GIF=OFF",
    "-DNO_CONSOLE_IO=ON",
    "-DMINIMUM_SEVERITY=L_SEVERITY_NONE",
    "-DSW_BUILD=OFF",
    "-DHAVE_LIBZ=0",
    "-DENABLE_LTO=OFF",
    "-DCMAKE_INSTALL_PREFIX=$leptonicaInstall"
)
Invoke-CMakeBuild -Arguments @(
    "--build", $leptonicaBuild,
    "--config", "Release",
    "--target", "install"
)

$leptonicaLib = Join-Path $leptonicaInstall "lib\leptonica-$LeptonicaVersion.lib"
$leptonicaConfigDir = Join-Path $leptonicaInstall "lib\cmake\leptonica"

Invoke-CMake -Arguments @(
    "-S", $tesseractSource,
    "-B", $tesseractBuild,
    "-G", $generator,
    "-A", "x64",
    "-DCMAKE_POLICY_VERSION_MINIMUM:STRING=3.5",
    "-DCMAKE_BUILD_TYPE=Release",
    "-DBUILD_TRAINING_TOOLS=OFF",
    "-DBUILD_SHARED_LIBS=OFF",
    "-DDISABLE_ARCHIVE=ON",
    "-DDISABLE_CURL=ON",
    "-DDISABLE_OPENCL=ON",
    "-DLeptonica_DIR=$leptonicaConfigDir",
    "-DCMAKE_PREFIX_PATH=$leptonicaInstall",
    "-DCMAKE_INSTALL_PREFIX=$tesseractInstall",
    "-DTESSDATA_PREFIX=$tessdataInstall",
    "-DGRAPHICS_DISABLED=ON",
    "-DDISABLED_LEGACY_ENGINE=OFF",
    "-DUSE_OPENCL=OFF",
    "-DOPENMP_BUILD=OFF",
    "-DBUILD_TESTS=OFF",
    "-DENABLE_LTO=OFF",
    "-DINSTALL_CONFIGS=ON"
)
Invoke-CMakeBuild -Arguments @(
    "--build", $tesseractBuild,
    "--config", "Release",
    "--target", "install"
)

Ensure-Directory $tessdataInstall
foreach ($language in $Languages) {
    $trainedData = Join-Path $tessdataInstall "$language.traineddata"
    if (-not (Test-Path $trainedData)) {
        $url = "https://github.com/tesseract-ocr/tessdata_best/raw/main/$language.traineddata"
        Write-Host "Downloading $language.traineddata"
        Invoke-WebRequest -Uri $url -OutFile $trainedData
    }
}

$appDataRoot = Join-Path $env:APPDATA "tesseract-rs"
Sync-File -Source $leptonicaLib -Destination (Join-Path $appDataRoot "cache\leptonica\leptonica.lib")
Sync-File -Source $leptonicaLib -Destination (Join-Path $appDataRoot "leptonica\lib\leptonica.lib")
Sync-File -Source (Join-Path $tesseractInstall "lib\tesseract53.lib") -Destination (Join-Path $appDataRoot "cache\tesseract\tesseract.lib")
Sync-File -Source (Join-Path $tesseractInstall "lib\tesseract53.lib") -Destination (Join-Path $appDataRoot "tesseract\lib\tesseract.lib")

foreach ($language in $Languages) {
    Sync-File `
        -Source (Join-Path $tessdataInstall "$language.traineddata") `
        -Destination (Join-Path $appDataRoot "tessdata\$language.traineddata")
}

Write-Host ""
Write-Host "OCR release dependencies are ready."
Write-Host "Leptonica: $leptonicaLib"
Write-Host "Tesseract: $(Join-Path $tesseractInstall 'lib\tesseract53.lib')"
Write-Host "Tessdata: $tessdataInstall"
