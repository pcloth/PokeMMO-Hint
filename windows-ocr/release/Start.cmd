@echo off
setlocal
cd /d "%~dp0"
set "POKEMMO_TESSERACT=%~dp0ocr\tesseract.exe"
set "POKEMMO_TESSDATA=%~dp0ocr\tessdata"
set "PATH=%~dp0ocr;%PATH%"
if not exist "%~dp0pokemmo-hint.exe" goto incomplete
if not exist "%POKEMMO_TESSERACT%" goto incomplete
if not exist "%POKEMMO_TESSDATA%\chi_sim.traineddata" goto incomplete
"%POKEMMO_TESSERACT%" --version >nul 2>&1
if errorlevel 1 (
    echo Bundled OCR engine could not run. Extract the complete ZIP again.
    echo Keep every file inside the ocr folder. This release requires Windows x64.
    pause
    exit /b 1
)
"%~dp0pokemmo-hint.exe"
if errorlevel 1 (
    echo Application failed. Read the message above and README.txt.
    pause
    exit /b 1
)
exit /b 0
:incomplete
echo Release files are missing. Extract the complete ZIP before starting.
echo Do not copy only the EXE or run it inside the ZIP viewer.
pause
exit /b 1
