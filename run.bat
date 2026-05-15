@echo off
REM Imaginify launcher - installs Pillow + CustomTkinter if missing, then runs the app.
cd /d "%~dp0"

where python >nul 2>nul
if errorlevel 1 (
    echo Python is not installed or not on PATH.
    echo Install it from https://www.python.org/downloads/
    echo and tick "Add Python to PATH" during installation.
    pause
    exit /b 1
)

python -c "import PIL, customtkinter" 2>nul
if errorlevel 1 (
    echo First-time setup - installing required packages...
    python -m pip install --upgrade pip
    python -m pip install Pillow customtkinter
    if errorlevel 1 (
        echo.
        echo Package install failed. Check your internet connection and try again.
        pause
        exit /b 1
    )
)

start "" pythonw imaginify.py
