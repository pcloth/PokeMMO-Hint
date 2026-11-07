@echo off
setlocal
cd /d "%~dp0"
if not exist "target\release\pokemmo-hint.exe" (
    echo First run: open Windows PowerShell and run:
    echo powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\setup.ps1"
    pause
    exit /b 1
)
"target\release\pokemmo-hint.exe"
if errorlevel 1 pause
