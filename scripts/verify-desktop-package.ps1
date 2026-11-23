<#
.SYNOPSIS
  Sunborn Desktop Distribution Verification Script
.DESCRIPTION
  Empirically verifies the integrity, cryptographic digests (SHA-256 / SHA-512),
  and payload completeness of the packaged Sunborn Desktop Windows ZIP distribution.
  Can be run on any clean Windows machine without developer tools.
#>

param(
    [string]$DistDir = "$PSScriptRoot\..\target\dist",
    [string]$Version = "1.0.0"
)

$ErrorActionPreference = "Stop"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "   SUNBORN DESKTOP PACKAGE VERIFICATION ENGINE            " -ForegroundColor White
Write-Host "==========================================================" -ForegroundColor Cyan

$ZipName = "sunborn-desktop-v$Version-windows-x64.zip"
$ZipPath = Join-Path $DistDir $ZipName
$Sha256File = Join-Path $DistDir "SHA256SUMS.txt"
$Sha512File = Join-Path $DistDir "SHA512SUMS.txt"

# 1. Verify existence of archive and checksum files
Write-Host "[1/4] Checking file existence..." -ForegroundColor Yellow
if (-not (Test-Path $ZipPath)) {
    Write-Error "Package not found: $ZipPath"
    exit 1
}
if (-not (Test-Path $Sha256File)) {
    Write-Error "SHA256SUMS.txt not found in $DistDir"
    exit 1
}
Write-Host "  -> Found package: $ZipPath" -ForegroundColor Green

# 2. Verify SHA-256 digest
Write-Host "[2/4] Verifying SHA-256 cryptographic digest..." -ForegroundColor Yellow
$ComputedSha256 = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash.ToLower()
$ExpectedSha256Line = (Get-Content $Sha256File | Where-Object { $_ -match $ZipName }).Trim()
$ExpectedSha256 = ($ExpectedSha256Line -split "\s+")[0].ToLower()

if ($ComputedSha256 -ne $ExpectedSha256) {
    Write-Error "SHA-256 mismatch! Computed: $ComputedSha256, Expected: $ExpectedSha256"
    exit 1
}
Write-Host "  -> SHA-256 MATCH: $ComputedSha256" -ForegroundColor Green

# 3. Verify SHA-512 digest if present
if (Test-Path $Sha512File) {
    Write-Host "[3/4] Verifying SHA-512 cryptographic digest..." -ForegroundColor Yellow
    $ComputedSha512 = (Get-FileHash -Path $ZipPath -Algorithm SHA512).Hash.ToLower()
    $ExpectedSha512Line = (Get-Content $Sha512File | Where-Object { $_ -match $ZipName }).Trim()
    $ExpectedSha512 = ($ExpectedSha512Line -split "\s+")[0].ToLower()

    if ($ComputedSha512 -ne $ExpectedSha512) {
        Write-Error "SHA-512 mismatch! Computed: $ComputedSha512, Expected: $ExpectedSha512"
        exit 1
    }
    Write-Host "  -> SHA-512 MATCH: $ComputedSha512" -ForegroundColor Green
} else {
    Write-Host "[3/4] Skipping SHA-512 (file not found)" -ForegroundColor DarkGray
}

# 4. Verify ZIP archive contents and manifest
Write-Host "[4/4] Inspecting package payload structure..." -ForegroundColor Yellow
Add-Type -AssemblyName System.IO.Compression.FileSystem
$Archive = [System.IO.Compression.ZipFile]::OpenRead($ZipPath)

$RequiredEntries = @(
    "bin/sunborn-gateway.exe",
    "Install.bat",
    "install-sunborn.ps1",
    "uninstall-sunborn.ps1",
    "sunborn-desktop.ps1",
    "sunborn-desktop.bat",
    "README.md",
    "LICENSE",
    "BUILD-METADATA.json"
)

$MissingEntries = @()
foreach ($entry in $RequiredEntries) {
    $found = $Archive.Entries | Where-Object { $_.FullName -replace "\\","/" -eq $entry }
    if (-not $found) {
        $MissingEntries += $entry
    }
}

$Archive.Dispose()

if ($MissingEntries.Count -gt 0) {
    Write-Error "Package is missing required files: $($MissingEntries -join ', ')"
    exit 1
}

Write-Host "  -> All $( $RequiredEntries.Count ) required payload entries verified inside archive." -ForegroundColor Green

Write-Host "`n==========================================================" -ForegroundColor Green
Write-Host "   VERIFICATION SUCCESS: Package is authentic & complete! " -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Green
exit 0
