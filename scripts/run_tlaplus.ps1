# ==============================================================================
# Sunborn Formal Verification: TLA+ Model Checking Gate (PowerShell)
# Runs TLC on EpochFencing and OutboxAtLeastOnce specifications.
# ==============================================================================
$ErrorActionPreference = "Stop"

$RootDir = Split-Path -Parent $PSScriptRoot
$FormalDir = Join-Path $RootDir "formal"
$JarPath = Join-Path $FormalDir "tla2tools.jar"
$MetaDir = Join-Path $RootDir "target\tlc-states"

if (!(Test-Path $JarPath)) {
    Write-Host "=== Downloading TLA+ Toolkit (tla2tools.jar) ===" -ForegroundColor Cyan
    Invoke-WebRequest -Uri "https://github.com/tlaplus/tlaplus/releases/latest/download/tla2tools.jar" -OutFile "$JarPath.tmp"
    Move-Item -Force "$JarPath.tmp" $JarPath
}

if (!(Get-Command java -ErrorAction SilentlyContinue)) {
    throw "FAIL: Java 17+ is required to run TLC"
}

Push-Location $FormalDir
try {
    function Run-TLC($module, $config, $label) {
        Write-Host "=== Model-Checking $label ($config) ===" -ForegroundColor Yellow
        $output = & java -XX:+UseParallelGC -cp tla2tools.jar tlc2.TLC -metadir $MetaDir -config $config -deadlock $module 2>&1
        $exitCode = $LASTEXITCODE
        $output | Select-Object -Last 12 | ForEach-Object { Write-Host $_ }
        if ($exitCode -ne 0) {
            throw "TLC model check failed for $module with config $config"
        }
        Write-Host ""
    }

    Run-TLC "EpochFencing" "EpochFencing.cfg" "epoch monotonicity / persist-before-publish"
    Run-TLC "OutboxAtLeastOnce" "OutboxAtLeastOnce.cfg" "outbox safety invariants"
    Run-TLC "OutboxAtLeastOnce" "OutboxLiveness.cfg" "outbox bounded liveness"

    Write-Host "PASS: all TLA+ models checked with no invariant or property violation" -ForegroundColor Green
} finally {
    Pop-Location
}
