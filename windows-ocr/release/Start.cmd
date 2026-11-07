@echo off
setlocal
cd /d "%~dp0"
if not exist "pokemmo-hint.exe" (
    echo Application missing. Extract the complete release ZIP before starting.
    pause
    exit /b 1
)
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0install-ocr.ps1" -Launch
if errorlevel 1 (
    echo Startup failed. Read the message above and README.txt.
    pause
    exit /b 1
)
exit /b 0
