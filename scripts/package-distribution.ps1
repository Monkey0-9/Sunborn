<#
.SYNOPSIS
  Sunborn Desktop Self-Contained Release Packaging Engine
.DESCRIPTION
  Builds the production release binary and packages a self-contained, 1-click
  Windows distribution zip containing all binaries, scripts, and embedded UI.
  Clean user machines do NOT need Rust, Node.js, or any developer tools installed.
#>

param(
    [string]$Version = "1.0.0",
    [string]$OutputDir = "$PSScriptRoot\..\target\dist",
    [switch]$SkipBuild = $false
)

$ErrorActionPreference = "Stop"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "   SUNBORN DESKTOP SELF-CONTAINED PACKAGING ENGINE        " -ForegroundColor White
Write-Host "   Version: $Version                                      " -ForegroundColor White
Write-Host "==========================================================" -ForegroundColor Cyan

$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$ReleaseBin = Join-Path $ProjectRoot "target\release\sunborn-gateway.exe"

# 1. Compile release binary if missing or requested
if (-not $SkipBuild -or -not (Test-Path $ReleaseBin)) {
    Write-Host "[1/4] Compiling optimized release binary..." -ForegroundColor Yellow
    Push-Location $ProjectRoot
    try {
        & cargo build --release -p sunborn-gateway
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Release compilation failed."
            exit 1
        }
    } finally {
        Pop-Location
    }
}

if (-not (Test-Path $ReleaseBin)) {
    Write-Error "Release binary not found at $ReleaseBin"
    exit 1
}

# 2. Stage distribution payload
$StageDir = Join-Path $OutputDir "stage\sunborn-desktop-v$Version"
if (Test-Path $StageDir) {
    Remove-Item -Path $StageDir -Recurse -Force
}
New-Item -ItemType Directory -Path (Join-Path $StageDir "bin") -Force | Out-Null

Write-Host "[2/4] Staging self-contained distribution payload..." -ForegroundColor Yellow
Copy-Item -Path $ReleaseBin -Destination (Join-Path $StageDir "bin\sunborn-gateway.exe") -Force
Copy-Item -Path (Join-Path $PSScriptRoot "install-sunborn.ps1") -Destination (Join-Path $StageDir "install-sunborn.ps1") -Force
Copy-Item -Path (Join-Path $PSScriptRoot "uninstall-sunborn.ps1") -Destination (Join-Path $StageDir "uninstall-sunborn.ps1") -Force
Copy-Item -Path (Join-Path $PSScriptRoot "sunborn-desktop.ps1") -Destination (Join-Path $StageDir "sunborn-desktop.ps1") -Force
Copy-Item -Path (Join-Path $PSScriptRoot "sunborn-desktop.bat") -Destination (Join-Path $StageDir "sunborn-desktop.bat") -Force
Copy-Item -Path (Join-Path $ProjectRoot "README.md") -Destination (Join-Path $StageDir "README.md") -Force
Copy-Item -Path (Join-Path $ProjectRoot "LICENSE") -Destination (Join-Path $StageDir "LICENSE") -Force

# Create convenient 1-click Install.bat for non-technical users
$BatContent = @"
@echo off
setlocal
echo Starting Sunborn 1-Click Desktop Installer...
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0install-sunborn.ps1" -LaunchAfterInstall
pause
endlocal
"@
Set-Content -Path (Join-Path $StageDir "Install.bat") -Value $BatContent -Encoding UTF8

# 3. Create zip distribution archive
$ZipFileName = "sunborn-desktop-v$Version-windows-x64.zip"
$ZipFilePath = Join-Path $OutputDir $ZipFileName

Write-Host "[3/4] Compressing into standalone archive: $ZipFileName..." -ForegroundColor Yellow
if (Test-Path $ZipFilePath) {
    Remove-Item -Path $ZipFilePath -Force
}
Compress-Archive -Path "$StageDir\*" -DestinationPath $ZipFilePath -CompressionLevel Optimal

# 4. Cleanup stage directory
Remove-Item -Path (Join-Path $OutputDir "stage") -Recurse -Force

$SizeMB = [math]::Round((Get-Item $ZipFilePath).Length / 1MB, 2)

Write-Host "`n==========================================================" -ForegroundColor Green
Write-Host "   PACKAGING COMPLETE!                                    " -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Green
Write-Host "   Archive: $ZipFilePath" -ForegroundColor White
Write-Host "   Payload Size: $SizeMB MB" -ForegroundColor White
Write-Host "   Target: Any clean Windows 10/11 x64 (Zero dev tools needed)" -ForegroundColor White
Write-Host "==========================================================" -ForegroundColor Green
