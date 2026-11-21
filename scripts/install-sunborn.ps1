<#
.SYNOPSIS
  Sunborn Local BYOK Windows Desktop Installer
.DESCRIPTION
  Installs Sunborn AI Gateway as a self-contained local user desktop application.
  Creates local runtime directory, desktop & start menu shortcuts, and registers
  Windows uninstaller.
#>

param(
    [string]$TargetDir = "$env:LOCALAPPDATA\Sunborn",
    [switch]$SkipBuild = $false,
    [switch]$LaunchAfterInstall = $false
)

$ErrorActionPreference = "Stop"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "   SUNBORN LOCAL BYOK WINDOWS INSTALLER                  " -ForegroundColor White
Write-Host "==========================================================" -ForegroundColor Cyan

# 1. Resolve source binary
$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$ReleaseBin = Join-Path $ProjectRoot "target\release\sunborn-gateway.exe"
$DebugBin = Join-Path $ProjectRoot "target\debug\sunborn-gateway.exe"
$BundledBin = Join-Path $PSScriptRoot "bin\sunborn-gateway.exe"
$LocalBin = Join-Path $PSScriptRoot "sunborn-gateway.exe"

$SourceBin = $null
if (Test-Path $BundledBin) {
    $SourceBin = $BundledBin
} elseif (Test-Path $LocalBin) {
    $SourceBin = $LocalBin
} elseif (Test-Path $ReleaseBin) {
    $SourceBin = $ReleaseBin
} elseif (Test-Path $DebugBin) {
    $SourceBin = $DebugBin
} elseif (-not $SkipBuild) {
    $CargoCmd = Get-Command cargo -ErrorAction SilentlyContinue
    if ($CargoCmd) {
        Write-Host "[1/5] Building sunborn-gateway release binary..." -ForegroundColor Yellow
        Push-Location $ProjectRoot
        try {
            & cargo build --release -p sunborn-gateway
            if ($LASTEXITCODE -eq 0 -and (Test-Path $ReleaseBin)) {
                $SourceBin = $ReleaseBin
            } else {
                Write-Host "Release build failed, checking debug..." -ForegroundColor Yellow
                & cargo build -p sunborn-gateway
                if ($LASTEXITCODE -eq 0 -and (Test-Path $DebugBin)) {
                    $SourceBin = $DebugBin
                }
            }
        } finally {
            Pop-Location
        }
    } else {
        Write-Host "[-] Cargo build toolchain not found on this clean machine." -ForegroundColor Yellow
        Write-Host "[-] Use pre-packaged distribution zip or install Rust toolchain." -ForegroundColor Yellow
    }
}

if (-not $SourceBin -or -not (Test-Path $SourceBin)) {
    Write-Error "Could not locate sunborn-gateway.exe. For clean machines without a Rust compiler, please download the self-contained Sunborn Desktop release package (containing pre-compiled binaries and embedded local cockpit)."
    exit 1
}

Write-Host "[+] Using binary: $SourceBin" -ForegroundColor Green

# 2. Create Target Directories
$BinDir = Join-Path $TargetDir "bin"
$DataDir = Join-Path $TargetDir "data"

Write-Host "[2/5] Creating application directories in $TargetDir..." -ForegroundColor Yellow
New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
New-Item -ItemType Directory -Path $DataDir -Force | Out-Null

# 3. Copy Application Files
Write-Host "[3/5] Deploying binary and desktop runner..." -ForegroundColor Yellow
Copy-Item -Path $SourceBin -Destination (Join-Path $BinDir "sunborn-gateway.exe") -Force
Copy-Item -Path (Join-Path $PSScriptRoot "sunborn-desktop.ps1") -Destination (Join-Path $BinDir "sunborn-desktop.ps1") -Force
Copy-Item -Path (Join-Path $PSScriptRoot "sunborn-desktop.bat") -Destination (Join-Path $BinDir "sunborn-desktop.bat") -Force
Copy-Item -Path (Join-Path $PSScriptRoot "uninstall-sunborn.ps1") -Destination (Join-Path $TargetDir "uninstall.ps1") -Force

# 4. Create Desktop and Start Menu Shortcuts
Write-Host "[4/5] Creating desktop & start menu shortcuts..." -ForegroundColor Yellow
$WshShell = New-Object -ComObject WScript.Shell

$DesktopShortcutPath = Join-Path ([Environment]::GetFolderPath("Desktop")) "Sunborn AI Gateway.lnk"
$Shortcut = $WshShell.CreateShortcut($DesktopShortcutPath)
$Shortcut.TargetPath = (Join-Path $BinDir "sunborn-desktop.bat")
$Shortcut.WorkingDirectory = $BinDir
$Shortcut.Description = "Sunborn High-Performance Semantic AI Gateway"
$Shortcut.Save()
Write-Host "      Created: $DesktopShortcutPath" -ForegroundColor DarkGray

$StartMenuDir = Join-Path ([Environment]::GetFolderPath("Programs")) "Sunborn"
if (-not (Test-Path $StartMenuDir)) {
    New-Item -ItemType Directory -Path $StartMenuDir -Force | Out-Null
}
$StartMenuShortcutPath = Join-Path $StartMenuDir "Sunborn AI Gateway.lnk"
$StartShortcut = $WshShell.CreateShortcut($StartMenuShortcutPath)
$StartShortcut.TargetPath = (Join-Path $BinDir "sunborn-desktop.bat")
$StartShortcut.WorkingDirectory = $BinDir
$StartShortcut.Description = "Sunborn High-Performance Semantic AI Gateway"
$StartShortcut.Save()
Write-Host "      Created: $StartMenuShortcutPath" -ForegroundColor DarkGray

# 5. Register in Windows Add/Remove Programs
Write-Host "[5/5] Registering in Windows Add/Remove Programs..." -ForegroundColor Yellow
$UninstallRegKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Sunborn"
if (-not (Test-Path $UninstallRegKey)) {
    New-Item -Path $UninstallRegKey -Force | Out-Null
}
Set-ItemProperty -Path $UninstallRegKey -Name "DisplayName" -Value "Sunborn AI Local Gateway"
Set-ItemProperty -Path $UninstallRegKey -Name "DisplayVersion" -Value "1.0.0"
Set-ItemProperty -Path $UninstallRegKey -Name "Publisher" -Value "Sunborn Platform"
Set-ItemProperty -Path $UninstallRegKey -Name "InstallLocation" -Value $TargetDir
Set-ItemProperty -Path $UninstallRegKey -Name "UninstallString" -Value "powershell.exe -ExecutionPolicy Bypass -File `"$TargetDir\uninstall.ps1`""
Set-ItemProperty -Path $UninstallRegKey -Name "QuietUninstallString" -Value "powershell.exe -ExecutionPolicy Bypass -File `"$TargetDir\uninstall.ps1`" -Silent"

Write-Host "`n==========================================================" -ForegroundColor Green
Write-Host "   INSTALLATION COMPLETE!                                 " -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Green
Write-Host "   Application Path: $BinDir\sunborn-gateway.exe"
Write-Host "   Encrypted Vault:  $TargetDir\vault.enc"
Write-Host "   WAL Storage:      $DataDir\execution.wal"
Write-Host "   Launcher:         $DesktopShortcutPath"
Write-Host "==========================================================" -ForegroundColor Green

if ($LaunchAfterInstall) {
    Write-Host "`n[*] Launching Sunborn Desktop..." -ForegroundColor Cyan
    & (Join-Path $BinDir "sunborn-desktop.ps1")
}
