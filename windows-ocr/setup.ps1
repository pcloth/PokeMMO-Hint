# Windows PowerShell 5.1+. OCR is selected/prepared in the app settings.
[CmdletBinding()]
param([switch]$InstallOcr,[switch]$NoLaunch)
$ErrorActionPreference='Stop'
try {
    if($env:OS -ne 'Windows_NT'){throw 'Run this script on Windows.'}
    $cargo=Get-Command cargo.exe -ErrorAction Stop
    if($InstallOcr){Write-Warning '-InstallOcr is obsolete. Select Windows OCR or PP-OCR in application settings.'}
    Push-Location $PSScriptRoot
    try {
        & $cargo.Source test --release --locked --offline
        if($LASTEXITCODE -ne 0){throw 'Rust tests failed. Check MSVC Build Tools and Windows SDK.'}
        & $cargo.Source build --release --locked --offline
        if($LASTEXITCODE -ne 0){throw 'Windows build failed.'}
    }finally{Pop-Location}
    $application=Join-Path $PSScriptRoot 'target\release\pokemmo-hint.exe'
    Write-Host "Built: $application"
    if(-not $NoLaunch){Start-Process -FilePath $application}
}catch{[Console]::Error.WriteLine($_.Exception.Message);exit 1}
