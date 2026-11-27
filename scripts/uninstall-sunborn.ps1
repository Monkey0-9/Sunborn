<#
.SYNOPSIS
  Sunborn Local BYOK Windows Desktop Uninstaller
.DESCRIPTION
  Removes Sunborn AI Gateway installation, shortcuts, and registry keys.
#>

param(
    [string]$TargetDir = "$env:LOCALAPPDATA\Sunborn",
    [switch]$Silent = $false,
    [switch]$PurgeData = $false
)

$ErrorActionPreference = "Continue"

if (-not $Silent) {
    Write-Host "==========================================================" -ForegroundColor Red
    Write-Host "   SUNBORN LOCAL GATEWAY UNINSTALLER                     " -ForegroundColor White
    Write-Host "==========================================================" -ForegroundColor Red
}

# 1. Stop any running sunborn-gateway processes
$Procs = Get-Process -Name "sunborn-gateway" -ErrorAction SilentlyContinue
if ($Procs) {
    if (-not $Silent) { Write-Host "[*] Stopping running Sunborn Gateway processes..." -ForegroundColor Yellow }
    $Procs | Stop-Process -Force -ErrorAction SilentlyContinue
}

# 2. Remove Shortcuts
$DesktopShortcut = Join-Path ([Environment]::GetFolderPath("Desktop")) "Sunborn AI Gateway.lnk"
if (Test-Path $DesktopShortcut) {
    Remove-Item $DesktopShortcut -Force -ErrorAction SilentlyContinue
}

$StartMenuDir = Join-Path ([Environment]::GetFolderPath("Programs")) "Sunborn"
if (Test-Path $StartMenuDir) {
    Remove-Item $StartMenuDir -Recurse -Force -ErrorAction SilentlyContinue
}

# 3. Remove Registry Entry
$UninstallRegKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Sunborn"
if (Test-Path $UninstallRegKey) {
    Remove-Item -Path $UninstallRegKey -Recurse -Force -ErrorAction SilentlyContinue
}

# 4. Remove Binary & optionally Data
$BinDir = Join-Path $TargetDir "bin"
if (Test-Path $BinDir) {
    Remove-Item $BinDir -Recurse -Force -ErrorAction SilentlyContinue
}

if ($PurgeData) {
    if (Test-Path $TargetDir) {
        Remove-Item $TargetDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    if (-not $Silent) { Write-Host "[+] Removed application and purged data store." -ForegroundColor Green }
} else {
    if (-not $Silent) {
        Write-Host "[+] Sunborn Gateway binary and shortcuts uninstalled." -ForegroundColor Green
        Write-Host "    Note: Encrypted credentials in $TargetDir were preserved." -ForegroundColor DarkGray
        Write-Host "    To delete them permanently, re-run with -PurgeData flag." -ForegroundColor DarkGray
    }
}
