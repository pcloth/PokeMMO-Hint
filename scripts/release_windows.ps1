# Build and package the lightweight Windows x64 release. PowerShell 5.1+.
[CmdletBinding()]
param(
    [string]$OutputDirectory = '',
    [switch]$Force
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repoDir = Split-Path -Parent $PSScriptRoot
$projectDir = Join-Path $repoDir 'windows-ocr'
$target = 'x86_64-pc-windows-msvc'
$stage = $null
$tempZip = $null
$oldFlags = $env:RUSTFLAGS
$oldEncodedFlags = $env:CARGO_ENCODED_RUSTFLAGS

function Invoke-Checked {
    param([string]$Program, [string[]]$Arguments)
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed (exit $LASTEXITCODE)." }
}
function Write-Utf8 {
    param([string]$Path, [string]$Text, [switch]$Bom)
    [IO.File]::WriteAllText($Path, $Text, [Text.UTF8Encoding]::new([bool]$Bom))
}
function Assert-X64PE {
    param([string]$Path)
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 64 -or $bytes[0] -ne 77 -or $bytes[1] -ne 90) { throw 'Application is not a Windows PE executable.' }
    $offset = [BitConverter]::ToInt32($bytes, 0x3c)
    if ($offset -lt 0 -or $offset + 24 -gt $bytes.Length -or [BitConverter]::ToUInt32($bytes, $offset) -ne 0x4550 -or [BitConverter]::ToUInt16($bytes, $offset + 4) -ne 0x8664) {
        throw 'Expected an x64 Windows PE executable.'
    }
}

try {
    if ($env:OS -ne 'Windows_NT' -or -not [Environment]::Is64BitOperatingSystem) { throw 'Run this script on 64-bit Windows.' }
    $cargo = Get-Command cargo.exe -ErrorAction Stop
    $manifest = Get-Content -LiteralPath (Join-Path $projectDir 'Cargo.toml') -Raw
    $match = [regex]::Match($manifest, '(?m)^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?)"')
    if (-not $match.Success) { throw 'Could not read package version from Cargo.toml.' }
    $version = $match.Groups[1].Value
    $packageName = "PokeMMO-Hint-v$version-windows-x64"
    if (-not $OutputDirectory) { $OutputDirectory = Join-Path $repoDir 'dist' }
    $outputDir = [IO.Path]::GetFullPath($OutputDirectory)
    New-Item -ItemType Directory -Force -Path $outputDir | Out-Null
    $zip = Join-Path $outputDir "$packageName.zip"
    $checksum = "$zip.sha256"
    if (-not $Force -and ((Test-Path -LiteralPath $zip) -or (Test-Path -LiteralPath $checksum))) {
        throw 'This version already exists. Bump Cargo.toml or pass -Force to replace it.'
    }

    # The committed hash fixes the exact model shipped, even when the URL changes.
    $expectedHash = ((Get-Content -LiteralPath (Join-Path $projectDir 'release\model.sha256') -Raw).Trim() -split '\s+')[0]
    $model = Join-Path $projectDir 'ocr\tessdata\chi_sim.traineddata'
    if (-not (Test-Path -LiteralPath $model -PathType Leaf)) {
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $model) | Out-Null
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
        $download = "$model.download"
        try {
            Write-Host 'Downloading the Chinese OCR model...'
            Invoke-WebRequest -UseBasicParsing -Uri 'https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/main/chi_sim.traineddata' -OutFile $download
            if ((Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash -ne $expectedHash) { throw 'Downloaded model checksum does not match release/model.sha256.' }
            Move-Item -LiteralPath $download -Destination $model
        } finally {
            if (Test-Path -LiteralPath $download) { Remove-Item -LiteralPath $download }
        }
    }
    if ((Get-FileHash -LiteralPath $model -Algorithm SHA256).Hash -ne $expectedHash) {
        throw 'Local model differs from release/model.sha256. Move it aside and rerun to fetch the pinned tessdata_fast model.'
    }

    # Static MSVC CRT: recipients do not need Rust or a separate VC++ runtime install.
    $env:RUSTFLAGS = "$oldFlags -C target-feature=+crt-static".Trim()
    $env:CARGO_ENCODED_RUSTFLAGS = $null
    Push-Location $projectDir
    try {
        Invoke-Checked -Program $cargo.Source -Arguments @('test', '--release', '--target', $target, '--target-dir', (Join-Path $projectDir 'target'), '--locked', '--offline')
        Invoke-Checked -Program $cargo.Source -Arguments @('build', '--release', '--target', $target, '--target-dir', (Join-Path $projectDir 'target'), '--locked', '--offline')
    } finally { Pop-Location }
    $application = Join-Path $projectDir "target\$target\release\pokemmo-hint.exe"
    Assert-X64PE $application
    Invoke-Checked -Program $application -Arguments @('--text', 'No.573')
    Invoke-Checked -Program $application -Arguments @('--text', 'No.523')

    $stage = Join-Path $outputDir ('.staging-' + [guid]::NewGuid().ToString('N'))
    $package = Join-Path $stage $packageName
    New-Item -ItemType Directory -Path (Join-Path $package 'ocr\tessdata') -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $package 'licenses') -Force | Out-Null
    Copy-Item -LiteralPath $application -Destination (Join-Path $package 'pokemmo-hint.exe')
    Copy-Item -LiteralPath $model -Destination (Join-Path $package 'ocr\tessdata\chi_sim.traineddata')
    foreach ($name in @('Start.cmd', 'install-ocr.ps1')) {
        Copy-Item -LiteralPath (Join-Path $projectDir "release\$name") -Destination (Join-Path $package $name)
    }
    foreach ($name in @('README.txt')) {
        Write-Utf8 -Path (Join-Path $package $name) -Text (Get-Content -LiteralPath (Join-Path $projectDir "release\$name") -Raw -Encoding UTF8) -Bom
    }
    Copy-Item -LiteralPath (Join-Path $projectDir 'THIRD_PARTY_NOTICES.txt') -Destination $package
    Copy-Item -LiteralPath (Join-Path $projectDir 'licenses\tessdata-fast-LICENSE.txt') -Destination (Join-Path $package 'licenses')
    $metadata = [ordered]@{
        name = 'PokeMMO Hint'; version = $version; target = $target
        built_at_utc = [DateTime]::UtcNow.ToString('o')
        package = 'lightweight'; engine_bundled = $false
        engine_install = 'UB-Mannheim.TesseractOCR via winget, or existing Tesseract'
        model = 'chi_sim.traineddata'; model_sha256 = $expectedHash.ToLowerInvariant()
        static_crt = $true; species = '1-649 base forms'; configuration_included = $false
    }
    Write-Utf8 (Join-Path $package 'release.json') ($metadata | ConvertTo-Json)
    $files = @(Get-ChildItem -LiteralPath $package -Recurse -File | Sort-Object FullName)
    $lines = foreach ($file in $files) {
        $relative = $file.FullName.Substring($package.Length + 1).Replace('\', '/')
        '{0}  {1}' -f (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant(), $relative
    }
    Write-Utf8 (Join-Path $package 'SHA256SUMS.txt') (($lines -join "`r`n") + "`r`n")
    $tempZip = Join-Path $outputDir ('.package-' + [guid]::NewGuid().ToString('N') + '.zip')
    Compress-Archive -LiteralPath $package -DestinationPath $tempZip -CompressionLevel Optimal

    # Check the actual extracted payload, not just files in the staging directory.
    $checkDir = Join-Path $stage 'verify'
    Expand-Archive -LiteralPath $tempZip -DestinationPath $checkDir
    $unpacked = Join-Path $checkDir $packageName
    foreach ($file in @(Get-ChildItem -LiteralPath $package -Recurse -File)) {
        $relative = $file.FullName.Substring($package.Length + 1)
        $copy = Join-Path $unpacked $relative
        if (-not (Test-Path -LiteralPath $copy -PathType Leaf) -or (Get-FileHash -LiteralPath $copy -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash) {
            throw "Archive verification failed: $relative"
        }
    }
    Assert-X64PE (Join-Path $unpacked 'pokemmo-hint.exe')
    Invoke-Checked -Program (Join-Path $unpacked 'pokemmo-hint.exe') -Arguments @('--text', 'No.573')
    Move-Item -LiteralPath $tempZip -Destination $zip -Force:$Force
    $zipHash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Utf8 $checksum ("$zipHash  $packageName.zip`r`n")
    Write-Host "Release ready: $zip"
    Write-Host "SHA-256 file: $checksum"
    Write-Host 'Upload both files as release assets. Recipients extract the ZIP and double-click Start.cmd.'
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    exit 1
} finally {
    $env:RUSTFLAGS = $oldFlags
    $env:CARGO_ENCODED_RUSTFLAGS = $oldEncodedFlags
    if ($stage -and (Test-Path -LiteralPath $stage)) { Remove-Item -LiteralPath $stage -Recurse -Force }
    if ($tempZip -and (Test-Path -LiteralPath $tempZip)) { Remove-Item -LiteralPath $tempZip -Force }
}
