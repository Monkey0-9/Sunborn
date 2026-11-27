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
    [switch]$SkipBuild = $false,
    # Offline/air-gapped signing: RFC 3161 timestamping requires network access and
    # blocks indefinitely when the timestamp authority is unreachable.
    [switch]$NoTimestamp = $false
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
if (Test-Path (Join-Path $ProjectRoot "sbom.cyclonedx.json")) {
    Copy-Item -Path (Join-Path $ProjectRoot "sbom.cyclonedx.json") -Destination (Join-Path $StageDir "sbom.cyclonedx.json") -Force
    Copy-Item -Path (Join-Path $ProjectRoot "sbom.cyclonedx.json") -Destination (Join-Path $OutputDir "sbom.cyclonedx.json") -Force
}

# Optional code-signing for production release distribution
if ($env:SIGNING_CERT_PFX -and (Test-Path $env:SIGNING_CERT_PFX)) {
    Write-Host "Signing executable with release certificate..." -ForegroundColor Cyan
    # Load the PFX directly with its password. `Get-PfxCertificate` prompts
    # interactively for a password-protected PFX and blocks unattended CI runs.
    if ($env:SIGNING_CERT_PASSWORD) {
        $cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2(
            $env:SIGNING_CERT_PFX,
            $env:SIGNING_CERT_PASSWORD,
            [System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::EphemeralKeySet
        )
    } else {
        $cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($env:SIGNING_CERT_PFX)
    }
    $signArgs = @{
        FilePath = (Join-Path $StageDir "bin\sunborn-gateway.exe")
        Certificate = $cert
    }
    if (-not $NoTimestamp) {
        $signArgs.TimestampServer = "http://timestamp.digicert.com"
    }
    $sig = Set-AuthenticodeSignature @signArgs
    if (-not $sig.SignerCertificate) {
        Write-Error "Code signing failed: $($sig.StatusMessage)"
        exit 1
    }
    Write-Host "[+] Authenticode signature applied (status: $($sig.Status)); timestamp: $(if ($NoTimestamp) { 'omitted' } else { 'requested' })" -ForegroundColor Green
}

# Generate build provenance metadata
$gitCommit = "unknown"
try {
    $gitCommit = (& git rev-parse HEAD 2>$null).Trim()
} catch {}

$BinSha256 = (Get-FileHash -Path (Join-Path $StageDir "bin\sunborn-gateway.exe") -Algorithm SHA256).Hash.ToLower()

$BuilderName = if ($env:GITHUB_ACTIONS -eq "true") { "GitHub Actions" } else { "local build" }

# This is an unsigned, self-described build manifest. It is NOT SLSA
# provenance: a SLSA attestation must be issued and signed by a trusted
# builder. The release workflow does that separately with GitHub's
# actions/attest-build-provenance; this file only records reproducible build
# inputs (commit, binary digest) next to the artifact.
$BuildMetadata = [ordered]@{
    manifest_type = "sunborn-build-metadata-v1"
    signed = $false
    product = "Sunborn Desktop BYOK Gateway"
    version = $Version
    target = "windows-x64"
    build_date_utc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    commit_sha = $gitCommit
    builder = $BuilderName
    binary = @{
        name = "sunborn-gateway.exe"
        sha256 = $BinSha256
        architecture = "x86_64"
        os = "windows"
    }
    supply_chain = @{
        sbom = "sbom.cyclonedx.json"
        slsa_attestation = "not embedded; issued by CI via actions/attest-build-provenance when the release workflow runs"
        code_signing = if ($env:SIGNING_CERT_PFX) { "Authenticode attempted with the configured certificate" } else { "not signed (SIGNING_CERT_PFX was not provided)" }
        verification_guidance = "Check SHA256SUMS.txt; when signed, verify Authenticode with Get-AuthenticodeSignature; when available, verify the GitHub artifact attestation with 'gh attestation verify'"
    }
}
$MetadataJson = ($BuildMetadata | ConvertTo-Json -Depth 5)
Set-Content -Path (Join-Path $StageDir "BUILD-METADATA.json") -Value $MetadataJson -Encoding UTF8
Set-Content -Path (Join-Path $OutputDir "BUILD-METADATA.json") -Value $MetadataJson -Encoding UTF8

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

# 4. Generate cryptographic checksums (SHA-256 and SHA-512)
$Sha256 = (Get-FileHash -Path $ZipFilePath -Algorithm SHA256).Hash.ToLower()
$ShaPath = Join-Path $OutputDir "SHA256SUMS.txt"
Set-Content -Path $ShaPath -Value "$Sha256  $ZipFileName" -Encoding UTF8

$Sha512 = (Get-FileHash -Path $ZipFilePath -Algorithm SHA512).Hash.ToLower()
$ShaPath512 = Join-Path $OutputDir "SHA512SUMS.txt"
Set-Content -Path $ShaPath512 -Value "$Sha512  $ZipFileName" -Encoding UTF8

# 5. Cleanup stage directory
Remove-Item -Path (Join-Path $OutputDir "stage") -Recurse -Force

$SizeMB = [math]::Round((Get-Item $ZipFilePath).Length / 1MB, 2)

Write-Host "`n==========================================================" -ForegroundColor Green
Write-Host "   PACKAGING COMPLETE!                                    " -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Green
Write-Host "   Archive: $ZipFilePath" -ForegroundColor White
Write-Host "   SHA-256: $Sha256" -ForegroundColor White
Write-Host "   Payload Size: $SizeMB MB" -ForegroundColor White
Write-Host "   Target: Any clean Windows 10/11 x64 (Zero dev tools needed)" -ForegroundColor White
Write-Host "==========================================================" -ForegroundColor Green
