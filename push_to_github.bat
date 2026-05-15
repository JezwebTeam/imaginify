@echo off
REM ----------------------------------------------------------------
REM  Imaginify - initial push to GitHub
REM  Pushes everything in this folder to git@github.com:abnercalapiz/imaginify.git
REM  Run this once. After the first push, use normal git commands.
REM ----------------------------------------------------------------
setlocal
cd /d "%~dp0"

echo === Imaginify - push to GitHub ===
echo Repo: git@github.com:abnercalapiz/imaginify.git
echo Folder: %CD%
echo.

where git >nul 2>nul
if errorlevel 1 (
    echo Git is not installed or not on PATH.
    echo Install Git for Windows from https://git-scm.com/download/win
    pause
    exit /b 1
)

REM Clean up any leftover .git from a previous failed init
if exist .git (
    echo Removing existing .git folder...
    attrib -r -h -s .git /s /d >nul 2>nul
    rmdir /s /q .git
)

REM Clean up generated artefacts that shouldn't be committed
if exist build rmdir /s /q build
if exist dist rmdir /s /q dist
if exist __pycache__ rmdir /s /q __pycache__
if exist icon.ico del icon.ico
if exist icon.png del icon.png

echo Initialising git repo...
git init -b main || (echo Failed to init repo & pause & exit /b 1)
git config user.email "abner@jezweb.net"
git config user.name "Abner"

echo Adding files...
git add .
git status --short
echo.

echo Creating initial commit...
git commit -m "Initial commit: Imaginify v1.0.0 - made by JezPress" || (echo Commit failed & pause & exit /b 1)
echo.

echo Adding remote...
git remote add origin git@github.com:abnercalapiz/imaginify.git

echo Pushing to GitHub...
echo (You may be prompted for your SSH passphrase.)
git push -u origin main
if errorlevel 1 (
    echo.
    echo Push failed. Common causes:
    echo   - The repository doesn't exist yet on GitHub. Create it first
    echo     at https://github.com/new (name: imaginify, do NOT initialise
    echo     with README/.gitignore).
    echo   - SSH key isn't set up. Test with:  ssh -T git@github.com
    echo   - The repo already has commits. To overwrite use:  git push -f -u origin main
    pause
    exit /b 1
)

echo.
echo === SUCCESS ===
echo View at: https://github.com/abnercalapiz/imaginify
echo.
pause
endlocal
