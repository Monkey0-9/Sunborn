<#
.SYNOPSIS
  Starts the entire Sunborn Platform: Axum Rust Gateway (8080) and Next.js Control Plane (3000).
.DESCRIPTION
  This script boots both the high-throughput asynchronous Axum backend gateway
  and the Next.js 15 enterprise frontend control plane with health check verification.
#>

$ErrorActionPreference = "Stop"

Write-Host "=================================================================" -ForegroundColor Cyan
Write-Host "   SUNBORN ENTERPRISE CONTROL PLANE & SEMANTIC GATEWAY LAUNCHER" -ForegroundColor White
Write-Host "=================================================================" -ForegroundColor Cyan

# 1. Verify Rust Toolchain
Write-Host "`n[1/4] Checking Rust runtime toolchain..." -ForegroundColor Yellow
$cargoVersion = & cargo --version
Write-Host "      Detected $cargoVersion" -ForegroundColor Green

# 2. Check if Gateway is already running on port 8080
Write-Host "`n[2/4] Checking Sunborn Gateway (Port 8080)..." -ForegroundColor Yellow
$gatewayRunning = $false
try {
    $res = Invoke-RestMethod -Uri "http://127.0.0.1:8080/health" -TimeoutSec 1 -ErrorAction SilentlyContinue
    if ($res.status -eq "healthy") {
        $gatewayRunning = $true
        Write-Host "      Sunborn Gateway is already running and healthy at http://127.0.0.1:8080" -ForegroundColor Green
    }
} catch {
    $gatewayRunning = $false
}

if (-not $gatewayRunning) {
    Write-Host "      Launching Sunborn Gateway Axum Server on http://0.0.0.0:8080..." -ForegroundColor Cyan
    $env:SUNBORN_ENV = "development"
    $env:SUNBORN_CACHE_BACKEND = "memory"
    $env:SUNBORN_VECTOR_BACKEND = "memory"
    $env:SUNBORN_DB_BACKEND = "memory"
    $env:SUNBORN_ADMIN_KEY = "admin-secret-dev-key"

    Start-Process -FilePath "cargo" -ArgumentList "run", "--bin", "sunborn-gateway" -WindowStyle Minimized

    Write-Host "      Waiting for gateway to initialize..." -NoNewline
    $ready = $false
    for ($i = 0; $i -lt 30; $i++) {
        Start-Sleep -Milliseconds 500
        try {
            $check = Invoke-RestMethod -Uri "http://127.0.0.1:8080/health" -TimeoutSec 1 -ErrorAction SilentlyContinue
            if ($check.status -eq "healthy") {
                $ready = $true
                break
            }
        } catch {}
        Write-Host "." -NoNewline
    }
    Write-Host ""
    if ($ready) {
        Write-Host "      Sunborn Gateway successfully online and responding at http://127.0.0.1:8080" -ForegroundColor Green
    } else {
        Write-Host "      Warning: Gateway health check timed out. Continuing with standalone mode." -ForegroundColor Amber
    }
}

# 3. Check Node.js and Frontend dependencies
Write-Host "`n[3/4] Checking Node.js frontend runtime..." -ForegroundColor Yellow
$nodeVersion = & node --version
Write-Host "      Detected Node.js $nodeVersion" -ForegroundColor Green

if (-not (Test-Path -Path "frontend/node_modules")) {
    Write-Host "      Installing frontend node dependencies..." -ForegroundColor Cyan
    Push-Location frontend
    npm install
    Pop-Location
}

# 4. Start Next.js Development Server
Write-Host "`n[4/4] Starting Next.js Enterprise Control Plane on http://localhost:3000..." -ForegroundColor Yellow
Write-Host "=================================================================" -ForegroundColor Cyan
Write-Host "  SUNBORN CONTROL PLANE IS LIVE:" -ForegroundColor White
Write-Host "  -> Frontend UI:     http://localhost:3000" -ForegroundColor Green
Write-Host "  -> Gateway Backend: http://127.0.0.1:8080" -ForegroundColor Green
Write-Host "  -> Gateway Health:  http://127.0.0.1:8080/health" -ForegroundColor Green
Write-Host "=================================================================" -ForegroundColor Cyan

Push-Location frontend
Start-Process "http://localhost:3000"
npm run dev
Pop-Location
