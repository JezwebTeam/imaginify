@echo off
REM ----------------------------------------------------------------
REM Imaginify - Windows build script
REM Produces dist\Imaginify.exe (single-file, ~40 MB)
REM ----------------------------------------------------------------
setlocal
cd /d "%~dp0"

echo === Imaginify Windows build ===
echo.

where python >nul 2>nul
if errorlevel 1 (
    echo Python not found on PATH.
    echo Install Python 3.10+ from https://www.python.org/downloads/
    echo and tick "Add Python to PATH" during installation.
    pause
    exit /b 1
)

echo Installing build dependencies...
python -m pip install --upgrade pip >nul
python -m pip install Pillow customtkinter pyinstaller || (echo Pip install failed & pause & exit /b 1)
echo.

echo Generating icon...
python make_icon.py || (echo Icon generation failed & pause & exit /b 1)
echo.

echo Cleaning previous build...
if exist build rmdir /s /q build
if exist dist rmdir /s /q dist
if exist Imaginify.spec del Imaginify.spec
echo.

echo Building Imaginify.exe with PyInstaller (this can take a minute)...
python -m PyInstaller ^
    --onefile ^
    --windowed ^
    --name Imaginify ^
    --icon icon.ico ^
    --collect-data customtkinter ^
    --clean ^
    --noconfirm ^
    imaginify.py

if errorlevel 1 (
    echo.
    echo Build failed. See the messages above.
    pause
    exit /b 1
)

echo.
echo === BUILD SUCCESSFUL ===
echo.
echo Find your app at:  %CD%\dist\Imaginify.exe
echo.
echo Optional: copy it onto your Desktop or pin to the Start menu.
echo.
pause
endlocal
