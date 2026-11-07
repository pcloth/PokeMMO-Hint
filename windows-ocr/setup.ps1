# Windows PowerShell 5.1+. This file is ASCII to avoid script-encoding ambiguity.
[CmdletBinding()]
param([switch]$InstallOcr, [switch]$NoLaunch)
$ErrorActionPreference = 'Stop'
$projectDir = $PSScriptRoot

function Find-Tesseract {
    $paths = @()
    if ($env:POKEMMO_TESSERACT) { $paths += $env:POKEMMO_TESSERACT }
    $paths += (Join-Path $projectDir 'ocr\tesseract.exe')
    if ($env:ProgramFiles) { $paths += (Join-Path $env:ProgramFiles 'Tesseract-OCR\tesseract.exe') }
    if (${env:ProgramFiles(x86)}) { $paths += (Join-Path ${env:ProgramFiles(x86)} 'Tesseract-OCR\tesseract.exe') }
    $found = Get-Command tesseract.exe -ErrorAction SilentlyContinue
    if ($found) { $paths += $found.Source }
    foreach ($path in $paths) { if (Test-Path -LiteralPath $path -PathType Leaf) { return $path } }
    return $null
}

try {
    $cargoCommand = Get-Command cargo.exe -ErrorAction SilentlyContinue
    if (-not $cargoCommand) { throw 'cargo.exe not found. Install Windows Rust and MSVC Build Tools first.' }
    $ocrExe = Find-Tesseract
    if (-not $ocrExe -and $InstallOcr) {
        $wingetCommand = Get-Command winget.exe -ErrorAction SilentlyContinue
        if (-not $wingetCommand) { throw 'winget not found. Install Tesseract from https://github.com/UB-Mannheim/tesseract/wiki' }
        Write-Host 'Installing Tesseract using winget (may show the installer or a Windows elevation prompt)...'
        & $wingetCommand.Source install --id UB-Mannheim.TesseractOCR --exact --source winget --accept-source-agreements --accept-package-agreements
        if ($LASTEXITCODE -ne 0) { throw "Tesseract installation failed, exit code $LASTEXITCODE" }
        $ocrExe = Find-Tesseract
    }
    if (-not $ocrExe) { throw 'Tesseract not found. Run this script with -InstallOcr, or set POKEMMO_TESSERACT to tesseract.exe.' }
    $env:POKEMMO_TESSERACT = $ocrExe
    Write-Host "OCR engine: $ocrExe"
    & $ocrExe --version
    if ($LASTEXITCODE -ne 0) { throw 'Tesseract could not run.' }

    $modelDir = Join-Path $projectDir 'ocr\tessdata'
    New-Item -ItemType Directory -Force -Path $modelDir | Out-Null
    $modelFile = Join-Path $modelDir 'chi_sim.traineddata'
    if (-not (Test-Path -LiteralPath $modelFile)) {
        $installedModel = Join-Path (Split-Path -Parent $ocrExe) 'tessdata\chi_sim.traineddata'
        if (Test-Path -LiteralPath $installedModel) {
            Copy-Item -LiteralPath $installedModel -Destination $modelFile
        } else {
            Write-Host 'Downloading the small Chinese model from the official tessdata_fast repository...'
            [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
            $temporaryModel = "$modelFile.download"
            try {
                Invoke-WebRequest -UseBasicParsing -Uri 'https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/main/chi_sim.traineddata' -OutFile $temporaryModel
                if ((Get-Item -LiteralPath $temporaryModel).Length -lt 100000) { throw 'Downloaded model is unexpectedly small.' }
                Move-Item -LiteralPath $temporaryModel -Destination $modelFile
            } finally {
                if (Test-Path -LiteralPath $temporaryModel) { Remove-Item -LiteralPath $temporaryModel }
            }
        }
    }
    $env:POKEMMO_TESSDATA = $modelDir
    & $ocrExe --tessdata-dir $modelDir --list-langs
    if ($LASTEXITCODE -ne 0) { throw 'Could not load the model directory.' }

    Push-Location $projectDir
    try {
        & $cargoCommand.Source test --release --offline
        if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed. For linker errors, install Visual Studio C++ Build Tools and Windows SDK.' }
        & $cargoCommand.Source build --release --offline
        if ($LASTEXITCODE -ne 0) { throw 'Windows build failed.' }
    } finally { Pop-Location }
    $application = Join-Path $projectDir 'target\release\pokemmo-hint.exe'
    & $application --text 'No.573'
    if ($LASTEXITCODE -ne 0) { throw 'Bundled data self-check failed.' }
    Write-Host "Built: $application"
    if (-not $NoLaunch) { & $application }
} catch {
    Write-Error $_
    exit 1
}
