# Windows PowerShell 5.1+. ASCII source; release README is UTF-8.
[CmdletBinding()]
param([switch]$CheckOnly, [switch]$Launch)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Find-OcrEngine {
    $candidates = @()
    if ($env:POKEMMO_TESSERACT) { $candidates += $env:POKEMMO_TESSERACT }
    $candidates += (Join-Path $PSScriptRoot 'ocr\tesseract.exe')
    foreach ($key in @('ProgramFiles', 'ProgramFiles(x86)', 'LOCALAPPDATA')) {
        $folder = [Environment]::GetEnvironmentVariable($key)
        if ($folder) {
            $candidates += (Join-Path $folder 'Tesseract-OCR\tesseract.exe')
            $candidates += (Join-Path $folder 'Programs\Tesseract-OCR\tesseract.exe')
        }
    }
    $command = Get-Command tesseract.exe -ErrorAction SilentlyContinue
    if ($command) { $candidates += $command.Source }
    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) { return $candidate }
    }
    return $null
}

try {
    if ($env:OS -ne 'Windows_NT') { throw 'This package requires Windows.' }
    $engine = Find-OcrEngine
    if (-not $engine -and $CheckOnly) { exit 2 }
    if (-not $engine) {
        $winget = Get-Command winget.exe -ErrorAction SilentlyContinue
        if (-not $winget) {
            throw 'winget is unavailable. Install Tesseract manually from https://github.com/UB-Mannheim/tesseract/wiki and run Start.cmd again.'
        }
        Write-Host 'Installing the OCR engine. Windows may request administrator permission.'
        & $winget.Source install --id UB-Mannheim.TesseractOCR --exact --source winget --accept-source-agreements --accept-package-agreements
        if ($LASTEXITCODE -ne 0) { throw "Tesseract installation failed (exit $LASTEXITCODE)." }
        $engine = Find-OcrEngine
        if (-not $engine) { throw 'Installation finished but tesseract.exe was not found. Reopen Start.cmd or set POKEMMO_TESSERACT.' }
    }
    $models = Join-Path $PSScriptRoot 'ocr\tessdata'
    if (-not (Test-Path -LiteralPath (Join-Path $models 'chi_sim.traineddata') -PathType Leaf)) {
        throw 'Chinese model is missing. Extract the complete release ZIP again.'
    }
    & $engine --version
    if ($LASTEXITCODE -ne 0) { throw 'The installed Tesseract engine could not run.' }
    $languages = @(& $engine --tessdata-dir $models --list-langs 2>&1)
    if ($LASTEXITCODE -ne 0 -or -not ($languages | Where-Object { "$_".Trim() -eq 'chi_sim' })) {
        throw 'Tesseract could not find the packaged Chinese model.'
    }
    Write-Host 'OCR engine and packaged Chinese model are ready.'
    if ($Launch) {
        $application = Join-Path $PSScriptRoot 'pokemmo-hint.exe'
        if (-not (Test-Path -LiteralPath $application -PathType Leaf)) { throw 'Application is missing. Extract the complete ZIP.' }
        $env:POKEMMO_TESSERACT = $engine
        $env:POKEMMO_TESSDATA = $models
        & $application
        exit $LASTEXITCODE
    }
    exit 0
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    exit 1
}
