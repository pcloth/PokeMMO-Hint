# Build a small Windows x64 release. OCR assets download on demand, never bundled.
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
# GUI-subsystem programs need explicit redirected handles and an explicit wait for CLI checks.
function Convert-NativeArgument {
    param([string]$Value)
    $result = [Text.StringBuilder]::new()
    [void]$result.Append('"')
    $slashes = 0
    foreach ($character in $Value.ToCharArray()) {
        if ($character -eq [char]92) { $slashes++; continue }
        if ($character -eq [char]34) {
            [void]$result.Append([string]::new([char]92, $slashes * 2 + 1))
        } else {
            [void]$result.Append([string]::new([char]92, $slashes))
        }
        [void]$result.Append($character)
        $slashes = 0
    }
    [void]$result.Append([string]::new([char]92, $slashes * 2))
    [void]$result.Append('"')
    $result.ToString()
}
function Invoke-AppCheck {
    param([string]$Program, [string[]]$Arguments)
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $Program
    $info.Arguments = (($Arguments | ForEach-Object { Convert-NativeArgument $_ }) -join ' ')
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.StandardOutputEncoding = [Text.UTF8Encoding]::new($false)
    $info.StandardErrorEncoding = [Text.UTF8Encoding]::new($false)
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $info
    try {
        if (-not $process.Start()) { throw 'Could not start the packaged application check.' }
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(30000)) {
            $process.Kill(); $process.WaitForExit()
            throw 'Application self-check timed out after 30 seconds.'
        }
        $output = $stdout.GetAwaiter().GetResult()
        $errorText = $stderr.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0) { throw ("Application self-check failed: " + $errorText + $output) }
        Write-Host $output
        if ($errorText) { Write-Host $errorText }
        $output
    } finally { $process.Dispose() }
}
function Write-Utf8 {
    param([string]$Path, [string]$Text, [switch]$Bom)
    [IO.File]::WriteAllText($Path, $Text, [Text.UTF8Encoding]::new([bool]$Bom))
}
function Assert-X64Gui {
    param([string]$Path)
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 64 -or $bytes[0] -ne 77 -or $bytes[1] -ne 90) { throw 'Application is not a Windows PE executable.' }
    $offset = [BitConverter]::ToInt32($bytes, 0x3c)
    if ($offset -lt 0 -or $offset + 24 -gt $bytes.Length -or [BitConverter]::ToUInt32($bytes, $offset) -ne 0x4550 -or [BitConverter]::ToUInt16($bytes, $offset + 4) -ne 0x8664) {
        throw 'Expected an x64 Windows PE executable.'
    }
    $optional = $offset + 24
    if ($optional + 70 -gt $bytes.Length -or [BitConverter]::ToUInt16($bytes, $optional) -ne 0x20b -or [BitConverter]::ToUInt16($bytes, $optional + 68) -ne 2) {
        throw 'Expected a Windows GUI-subsystem executable, not a console application.'
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

    # Static CRT for the base EXE. Optional ONNX Runtime reports its own prerequisites.
    $env:RUSTFLAGS = "$oldFlags -C target-feature=+crt-static".Trim()
    $env:CARGO_ENCODED_RUSTFLAGS = $null
    Push-Location $projectDir
    try {
        Invoke-Checked -Program $cargo.Source -Arguments @('test', '--release', '--target', $target, '--target-dir', (Join-Path $projectDir 'target'), '--locked', '--offline')
        Invoke-Checked -Program $cargo.Source -Arguments @('build', '--release', '--target', $target, '--target-dir', (Join-Path $projectDir 'target'), '--locked', '--offline')
    } finally { Pop-Location }
    $application = Join-Path $projectDir "target\$target\release\pokemmo-hint.exe"
    Assert-X64Gui $application
    $null = Invoke-AppCheck -Program $application -Arguments @('--text', 'No.573')
    $null = Invoke-AppCheck -Program $application -Arguments @('--text', 'No.523')

    $stage = Join-Path $outputDir ('.staging-' + [guid]::NewGuid().ToString('N'))
    $package = Join-Path $stage $packageName
    New-Item -ItemType Directory -Path (Join-Path $package 'licenses') -Force | Out-Null
    Copy-Item -LiteralPath $application -Destination (Join-Path $package 'pokemmo-hint.exe')
    foreach ($name in @('README.txt')) {
        Write-Utf8 -Path (Join-Path $package $name) -Text (Get-Content -LiteralPath (Join-Path $projectDir "release\$name") -Raw -Encoding UTF8) -Bom
    }
    Copy-Item -LiteralPath (Join-Path $repoDir 'docs\assets\quick-start-zh-CN.png') -Destination (Join-Path $package 'Quick-Start-zh-CN.png')
    Copy-Item -LiteralPath (Join-Path $projectDir 'THIRD_PARTY_NOTICES.txt') -Destination $package
    Copy-Item -LiteralPath (Join-Path $projectDir 'licenses\paddle-LICENSE.txt') -Destination (Join-Path $package 'licenses')
    $metadata = [ordered]@{
        name = 'PokeMMO Hint'; version = $version; target = $target
        built_at_utc = [DateTime]::UtcNow.ToString('o')
        package = 'portable'; engine_bundled = $false; model_bundled = $false
        default_engine = 'Windows.Media.Ocr'; optional_engine = 'PP-OCRv6 tiny ONNX'
        model_sha256 = '9ef676d6ed3c88256a2d92c640c44f25b0c40947e111b14b8be8f594091563e6'
        static_crt = $true; species = '1-649 base forms'; configuration_included = $false
        coordinate_mode = 'screen'; target_process_required = $false; type_chart = 'generations 2-5'
        windows_subsystem = 'gui'; launch = 'pokemmo-hint.exe'
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
    Assert-X64Gui (Join-Path $unpacked 'pokemmo-hint.exe')
    $null = Invoke-AppCheck -Program (Join-Path $unpacked 'pokemmo-hint.exe') -Arguments @('--text', 'No.573')
    Move-Item -LiteralPath $tempZip -Destination $zip -Force:$Force
    $zipHash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Utf8 $checksum ("$zipHash  $packageName.zip`r`n")
    Write-Host "Release ready: $zip"
    Write-Host "SHA-256 file: $checksum"
    Write-Host 'Upload both files as release assets. Recipients extract the ZIP and double-click pokemmo-hint.exe.'
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    exit 1
} finally {
    $env:RUSTFLAGS = $oldFlags
    $env:CARGO_ENCODED_RUSTFLAGS = $oldEncodedFlags
    if ($stage -and (Test-Path -LiteralPath $stage)) { Remove-Item -LiteralPath $stage -Recurse -Force }
    if ($tempZip -and (Test-Path -LiteralPath $tempZip)) { Remove-Item -LiteralPath $tempZip -Force }
}
