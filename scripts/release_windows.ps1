# Build and package the portable Windows x64 release. PowerShell 5.1+.
[CmdletBinding()]
param(
    [string]$OutputDirectory = '',
    [string]$TesseractDirectory = '',
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
$oldOcrEngine = $env:POKEMMO_TESSERACT
$oldOcrModels = $env:POKEMMO_TESSDATA
$oldPath = $env:PATH
. (Join-Path $PSScriptRoot 'ocr_runtime.ps1')

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

    if (-not $TesseractDirectory) {
        $candidates = @((Join-Path $projectDir 'ocr'))
        if ($env:POKEMMO_TESSERACT) { $candidates += (Split-Path -Parent $env:POKEMMO_TESSERACT) }
        foreach ($key in @('ProgramFiles', 'ProgramFiles(x86)', 'LOCALAPPDATA')) {
            $folder = [Environment]::GetEnvironmentVariable($key)
            if ($folder) {
                $candidates += (Join-Path $folder 'Tesseract-OCR')
                $candidates += (Join-Path $folder 'Programs\Tesseract-OCR')
            }
        }
        $command = Get-Command tesseract.exe -ErrorAction SilentlyContinue
        if ($command) { $candidates += (Split-Path -Parent $command.Source) }
        foreach ($candidate in $candidates) {
            if (Test-Path -LiteralPath (Join-Path $candidate 'tesseract.exe') -PathType Leaf) { $TesseractDirectory = $candidate; break }
        }
    }
    if (-not $TesseractDirectory) { throw 'No Tesseract runtime found. Run windows-ocr/setup.ps1 -InstallOcr -NoLaunch or specify -TesseractDirectory.' }
    $runtime = Get-OcrRuntime -Directory $TesseractDirectory
    $TesseractDirectory = (Resolve-Path -LiteralPath $TesseractDirectory).Path
    $engine = Join-Path $TesseractDirectory 'tesseract.exe'
    $engineVersion = @(& $engine --version 2>&1)
    if ($LASTEXITCODE -ne 0) { throw 'Source Tesseract runtime could not run.' }
    Write-Host ("Bundling OCR runtime: {0} files" -f $runtime.Files.Count)

    # Static MSVC CRT: recipients do not need Rust or a separate VC++ runtime install.
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
    New-Item -ItemType Directory -Path (Join-Path $package 'ocr\tessdata') -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $package 'licenses') -Force | Out-Null
    Copy-Item -LiteralPath $application -Destination (Join-Path $package 'pokemmo-hint.exe')
    Copy-Item -LiteralPath $model -Destination (Join-Path $package 'ocr\tessdata\chi_sim.traineddata')
    foreach ($name in @('README.txt')) {
        Write-Utf8 -Path (Join-Path $package $name) -Text (Get-Content -LiteralPath (Join-Path $projectDir "release\$name") -Raw -Encoding UTF8) -Bom
    }
    $ocrDir = Join-Path $package 'ocr'
    $runtimeFiles = foreach ($path in $runtime.Files) {
        $name = Split-Path -Leaf $path
        Copy-Item -LiteralPath $path -Destination (Join-Path $ocrDir $name)
        [ordered]@{ file = $name; sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() }
    }
    $runtimeLicenseDir = Join-Path $package 'licenses\tesseract-runtime'
    New-Item -ItemType Directory -Force -Path $runtimeLicenseDir | Out-Null
    foreach ($relative in @('doc', 'licenses', 'share\licenses')) {
        $runtimeDoc = Join-Path $TesseractDirectory $relative
        if (Test-Path -LiteralPath $runtimeDoc -PathType Container) { Copy-Item -LiteralPath $runtimeDoc -Destination $runtimeLicenseDir -Recurse }
    }
    foreach ($file in @(Get-ChildItem -LiteralPath $TesseractDirectory -File | Where-Object { $_.Name -match '(?i)^(license|copying|notice|authors)(\.|$)' })) {
        Copy-Item -LiteralPath $file.FullName -Destination $runtimeLicenseDir
    }
    if (-not @(Get-ChildItem -LiteralPath $runtimeLicenseDir -Recurse -File | Where-Object { $_.Name -match '(?i)^(license|copying)' }).Count) {
        throw 'OCR runtime license materials are missing. Supply a distribution containing its license/doc files.'
    }
    Write-Utf8 (Join-Path $ocrDir 'runtime.json') ([ordered]@{
        version_output = @($engineVersion | ForEach-Object { "$_" }); files = @($runtimeFiles)
        system_libraries = $runtime.SystemLibraries
        upstream = 'https://github.com/UB-Mannheim/tesseract/wiki'
        source = 'https://github.com/UB-Mannheim/tesseract'
    } | ConvertTo-Json -Depth 6)
    Copy-Item -LiteralPath (Join-Path $projectDir 'licenses\ocr-components.json') -Destination (Join-Path $package 'licenses')
    Copy-Item -LiteralPath (Join-Path $projectDir 'THIRD_PARTY_NOTICES.txt') -Destination $package
    Copy-Item -LiteralPath (Join-Path $projectDir 'licenses\tessdata-fast-LICENSE.txt') -Destination (Join-Path $package 'licenses')
    $metadata = [ordered]@{
        name = 'PokeMMO Hint'; version = $version; target = $target
        built_at_utc = [DateTime]::UtcNow.ToString('o')
        package = 'portable'; engine_bundled = $true
        engine = 'ocr/tesseract.exe'; runtime_files = $runtime.Files.Count
        model = 'chi_sim.traineddata'; model_sha256 = $expectedHash.ToLowerInvariant()
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
    $unpackedOcr = Join-Path $unpacked 'ocr'
    $packedRuntime = Get-OcrRuntime -Directory $unpackedOcr
    if ($packedRuntime.Files.Count -ne $runtime.Files.Count) { throw 'Packaged OCR dependency count changed.' }
    $env:POKEMMO_TESSERACT = Join-Path $unpackedOcr 'tesseract.exe'
    $env:POKEMMO_TESSDATA = Join-Path $unpackedOcr 'tessdata'
    # Exclude the developer's engine directory and PATH DLLs during all portable checks.
    $env:PATH = "$unpackedOcr;$env:SystemRoot\System32;$env:SystemRoot"
    Invoke-Checked -Program $env:POKEMMO_TESSERACT -Arguments @('--version')
    Invoke-Checked -Program $env:POKEMMO_TESSERACT -Arguments @('--tessdata-dir', $env:POKEMMO_TESSDATA, '--list-langs')
    $smokeImage = Join-Path $stage 'ocr-smoke.bmp'
    Add-Type -AssemblyName System.Drawing
    $bitmap = [Drawing.Bitmap]::new(480, 120)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    $font = [Drawing.Font]::new('Arial', [single]48, [Drawing.FontStyle]::Bold)
    try {
        $graphics.Clear([Drawing.Color]::White)
        $graphics.DrawString('No.573', $font, [Drawing.Brushes]::Black, 8, 12)
        $bitmap.Save($smokeImage, [Drawing.Imaging.ImageFormat]::Bmp)
    } finally { $font.Dispose(); $graphics.Dispose(); $bitmap.Dispose() }
    # This actually runs the packaged OCR engine/model through the packaged application.
    $smokeOutput = Invoke-AppCheck -Program (Join-Path $unpacked 'pokemmo-hint.exe') -Arguments @('--image', $smokeImage)
    if ($smokeOutput -notmatch '(?m)^#573\s') {
        throw ("Packaged OCR smoke test did not recognize No.573: " + $smokeOutput)
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
    $env:POKEMMO_TESSERACT = $oldOcrEngine
    $env:POKEMMO_TESSDATA = $oldOcrModels
    $env:PATH = $oldPath
    if ($stage -and (Test-Path -LiteralPath $stage)) { Remove-Item -LiteralPath $stage -Recurse -Force }
    if ($tempZip -and (Test-Path -LiteralPath $tempZip)) { Remove-Item -LiteralPath $tempZip -Force }
}
