<#
.SYNOPSIS
  Sunborn Local BYOK 1-Click Desktop Launcher
.DESCRIPTION
  Launches the Sunborn Gateway on 127.0.0.1:8080 in background and opens
  the local control dashboard in the user's default browser.
#>

param(
    [switch]$NoBrowser = $false,
    [int]$Port = 8080
)

$ErrorActionPreference = "Continue"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "   SUNBORN LOCAL BYOK DESKTOP RUNTIME                    " -ForegroundColor White
Write-Host "==========================================================" -ForegroundColor Cyan

$LocalAppDir = Join-Path $env:LOCALAPPDATA "Sunborn"
$BinPath = Join-Path $LocalAppDir "bin\sunborn-gateway.exe"

# If not in LOCALAPPDATA, check local workspace build
if (-not (Test-Path $BinPath)) {
    $WorkspaceBin = Join-Path $PSScriptRoot "..\target\release\sunborn-gateway.exe"
    if (Test-Path $WorkspaceBin) {
        $BinPath = (Resolve-Path $WorkspaceBin).Path
    } else {
        $DebugBin = Join-Path $PSScriptRoot "..\target\debug\sunborn-gateway.exe"
        if (Test-Path $DebugBin) {
            $BinPath = (Resolve-Path $DebugBin).Path
        }
    }
}

if (-not (Test-Path $BinPath)) {
    Write-Error "Sunborn gateway binary not found at $BinPath. Please run scripts/install-sunborn.ps1 first."
    exit 1
}

$HealthUrl = "http://127.0.0.1:$Port/health"
$CockpitUrl = "http://127.0.0.1:$Port/cockpit"
$StatusUrl = "http://127.0.0.1:$Port/v1/local/status"
$FrontendUrl = "http://localhost:3000"

# Check if already running
$AlreadyRunning = $false
try {
    $res = Invoke-RestMethod -Uri $HealthUrl -TimeoutSec 1 -ErrorAction SilentlyContinue
    if ($res.status -eq "healthy") {
        $AlreadyRunning = $true
        Write-Host "[+] Sunborn Gateway is already running at $HealthUrl" -ForegroundColor Green
    }
} catch {
    $AlreadyRunning = $false
}

if (-not $AlreadyRunning) {
    Write-Host "[*] Starting Sunborn Gateway ($BinPath)..." -ForegroundColor Yellow
    $DataDir = Join-Path $LocalAppDir "data"
    if (-not (Test-Path $DataDir)) {
        New-Item -ItemType Directory -Path $DataDir -Force | Out-Null
    }

    $EnvMap = @{
        "SUNBORN_ENV" = "local"
        "SUNBORN_SERVER__HOST" = "127.0.0.1"
        "SUNBORN_SERVER__PORT" = "$Port"
        "SUNBORN_CACHE_BACKEND" = "memory"
        "SUNBORN_VECTOR_BACKEND" = "memory"
        "SUNBORN_DB_BACKEND" = "memory"
        "SUNBORN_EXECUTION_STORE_PATH" = (Join-Path $DataDir "execution.wal")
    }

    $Psi = New-Object System.Diagnostics.ProcessStartInfo
    $Psi.FileName = $BinPath
    $Psi.UseShellExecute = $false
    $Psi.CreateNoWindow = $true
    $Psi.WindowStyle = [System.Diagnostics.ProcessWindowStyle]::Hidden

    foreach ($k in $EnvMap.Keys) {
        $Psi.EnvironmentVariables[$k] = $EnvMap[$k]
    }

    $Process = [System.Diagnostics.Process]::Start($Psi)
    Write-Host "[*] Process PID: $($Process.Id)" -ForegroundColor DarkGray

    # Poll /health for up to 10 seconds
    $Healthy = $false
    for ($i = 0; $i -lt 20; $i++) {
        Start-Sleep -Milliseconds 500
        try {
            $probe = Invoke-RestMethod -Uri $HealthUrl -TimeoutSec 1 -ErrorAction SilentlyContinue
            if ($probe.status -eq "healthy") {
                $Healthy = $true
                break
            }
        } catch {}
    }

    if ($Healthy) {
        Write-Host "[+] Sunborn Gateway successfully listening on 127.0.0.1:$Port" -ForegroundColor Green
    } else {
        Write-Warning "[-] Gateway startup timeout. Check logs in $DataDir"
    }
}

if (-not $NoBrowser) {
    # Default to built-in embedded local cockpit UI; switch to full Next.js control plane if running
    $TargetUrl = $CockpitUrl
    try {
        $fe = Invoke-WebRequest -Uri $FrontendUrl -TimeoutSec 1 -ErrorAction SilentlyContinue
        if ($fe.StatusCode -eq 200) {
            $TargetUrl = $FrontendUrl
        }
    } catch {}

    Write-Host "[+] Opening browser: $TargetUrl" -ForegroundColor Cyan
    Start-Process $TargetUrl
}

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "   Local Cockpit: $CockpitUrl" -ForegroundColor White
Write-Host "   Local status:  $StatusUrl" -ForegroundColor White
Write-Host "   Encrypted vault: $LocalAppDir\vault.enc" -ForegroundColor DarkGray
Write-Host "==========================================================" -ForegroundColor Cyan
