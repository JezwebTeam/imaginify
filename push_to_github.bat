@echo off
REM ----------------------------------------------------------------
REM  Imaginify - push to GitHub using GitHub CLI
REM  Creates the repo (if it doesn't exist), commits, and pushes.
REM ----------------------------------------------------------------
setlocal
cd /d "%~dp0"

echo === Imaginify - push to GitHub (using gh) ===
echo Folder: %CD%
echo.

REM ---- Check gh ----
where gh >nul 2>nul
if errorlevel 1 (
    echo GitHub CLI is not installed or not on PATH.
    echo Install it from https://cli.github.com/  (winget install --id GitHub.cli)
    echo Then re-run this script.
    pause
    exit /b 1
)

REM ---- Check git ----
where git >nul 2>nul
if errorlevel 1 (
    echo Git is not installed.
    echo Install from https://git-scm.com/download/win
    pause
    exit /b 1
)

REM ---- Auth ----
gh auth status >nul 2>nul
if errorlevel 1 (
    echo You aren't signed in to GitHub CLI. Launching browser login...
    gh auth login -h github.com -p https -w
    if errorlevel 1 (
        echo Login failed.
        pause
        exit /b 1
    )
)

REM ---- Clean up any leftover state ----
if exist .git (
    echo Removing existing .git folder...
    attrib -r -h -s .git /s /d >nul 2>nul
    rmdir /s /q .git
)
if exist build rmdir /s /q build
if exist dist rmdir /s /q dist
if exist __pycache__ rmdir /s /q __pycache__
if exist icon.ico del icon.ico
if exist icon.png del icon.png
if exist icon.icns del icon.icns
if exist icon.iconset rmdir /s /q icon.iconset

echo Initialising local git repo...
git init -b main
git config user.email "abner@jezweb.net"
git config user.name "Abner"
git add .
git commit -m "Initial commit: Imaginify v1.0.0 - made by JezPress"
if errorlevel 1 (
    echo Commit failed.
    pause
    exit /b 1
)

echo.
echo Checking whether the GitHub repo already exists...
gh repo view abnercalapiz/imaginify >nul 2>nul
if %ERRORLEVEL%==0 (
    echo Repo exists - adding remote and pushing...
    git remote add origin git@github.com:abnercalapiz/imaginify.git 2>nul
    git push -u origin main
) else (
    echo Creating the repo on GitHub and pushing...
    gh repo create abnercalapiz/imaginify --public --source=. --remote=origin --push --description "Imaginify - a modern desktop image tool by JezPress. Resize, crop, compress and convert images in batch."
)

if errorlevel 1 (
    echo.
    echo Push failed. See the messages above.
    pause
    exit /b 1
)

echo.
echo === SUCCESS ===
gh repo view abnercalapiz/imaginify --web 2>nul
echo Repo: https://github.com/abnercalapiz/imaginify
pause
endlocal
