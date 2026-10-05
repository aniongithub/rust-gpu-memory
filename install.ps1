# Installs the gpu-memory CLI (native Windows binary) from the latest GitHub
# release. The repository is currently PRIVATE, so set $env:GH_TOKEN (or
# $env:GITHUB_TOKEN) to a token with read access. Once the repo is public the
# token is not required and the script keeps working.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File install.ps1
#   $env:GPU_MEMORY_VERSION = "v0.1.0"; powershell -ExecutionPolicy Bypass -File install.ps1
#
# The binary is installed to %LOCALAPPDATA%\gpu-memory\bin\gpu-memory.exe and
# that directory is added to your user PATH.

$ErrorActionPreference = "Stop"

$Repo       = "aniongithub/rust-gpu-memory"
$BinName    = "gpu-memory.exe"
$InstallDir = Join-Path $env:LOCALAPPDATA "gpu-memory\bin"
$Api        = "https://api.github.com"
$Token      = $env:GH_TOKEN
if (-not $Token) { $Token = $env:GITHUB_TOKEN }

if (-not $Token) {
    Write-Warning "No GH_TOKEN/GITHUB_TOKEN set; the repo is currently private, so a token with read access is required."
}

# GET with an auth header when a token is set. With $OutFile, downloads the
# raw bytes (used for the release asset); otherwise returns parsed JSON.
function Invoke-Api {
    param([string]$Uri, [string]$OutFile = $null)
    $Headers = @{}
    if ($Token) { $Headers["Authorization"] = "Bearer $Token" }
    if ($OutFile) {
        $Headers["Accept"] = "application/octet-stream"
        Invoke-WebRequest -Uri $Uri -Headers $Headers -OutFile $OutFile -UseBasicParsing
    } else {
        Invoke-RestMethod -Uri $Uri -Headers $Headers
    }
}

# Detect the target triple from the CPU architecture.
switch ($env:PROCESSOR_ARCHITECTURE) {
    "AMD64" { $Triple = "x86_64-pc-windows-msvc" }
    "ARM64" { throw "ARM64 Windows is not published yet (no aarch64-pc-windows-msvc asset)." }
    default { throw "Unsupported architecture: $env:PROCESSOR_ARCHITECTURE" }
}

# Resolve the tag: pinned via GPU_MEMORY_VERSION, else latest.
if ($env:GPU_MEMORY_VERSION) {
    $Tag = $env:GPU_MEMORY_VERSION
} else {
    $Tag = (Invoke-Api "$Api/repos/$Repo/releases/latest").tag_name
}
if (-not $Tag.StartsWith("v")) { $Tag = "v$Tag" }

# Assets are named by target triple, not version (see release.yml).
$Asset = "gpu-memory-$Triple.zip"
$Release = Invoke-Api "$Api/repos/$Repo/releases/tags/$Tag"
$AssetObj = $Release.assets | Where-Object { $_.name -eq $Asset } | Select-Object -First 1
if (-not $AssetObj) { throw "Asset '$Asset' not found on release $Tag." }

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$Zip = Join-Path $env:TEMP "gpu-memory-$Tag.zip"
Invoke-Api $AssetObj.browser_download_url $Zip

Expand-Archive -Path $Zip -DestinationPath $InstallDir -Force
Remove-Item $Zip -Force

# Add the install dir to the user PATH if it is not already there.
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    Write-Host "Added $InstallDir to your user PATH (open a new terminal to pick it up)."
}

Write-Host "Installed gpu-memory $($Tag) to $InstallDir\$BinName"
& (Join-Path $InstallDir $BinName) --version
