@echo off
REM ----------------------------------------------------------------
REM  Imaginify - push current repo changes to GitHub
REM  Target repo: https://github.com/JezwebTeam/imaginify
REM ----------------------------------------------------------------
setlocal
cd /d "%~dp0"

set "REPO=JezwebTeam/imaginify"
set "REMOTE_URL=https://github.com/%REPO%.git"
set "COMMIT_MESSAGE=%~1"
if "%COMMIT_MESSAGE%"=="" set "COMMIT_MESSAGE=Update Imaginify"

echo === Imaginify - push to GitHub ===
echo Folder: %CD%
echo Repo:   https://github.com/%REPO%
echo.

where git >nul 2>nul
if errorlevel 1 (
    echo Git is not installed.
    echo Install from https://git-scm.com/download/win
    pause
    exit /b 1
)

where gh >nul 2>nul
if errorlevel 1 (
    echo GitHub CLI is not installed or not on PATH.
    echo Install it from https://cli.github.com/  (winget install --id GitHub.cli)
    pause
    exit /b 1
)

gh auth status >nul 2>nul
if errorlevel 1 (
    echo You are not signed in to GitHub CLI. Launching browser login...
    gh auth login -h github.com -p https -w
    if errorlevel 1 (
        echo Login failed.
        pause
        exit /b 1
    )
)

if not exist .git (
    echo Initialising local git repo...
    git init -b main
    if errorlevel 1 (
        echo Git init failed.
        pause
        exit /b 1
    )
)

git remote get-url origin >nul 2>nul
if errorlevel 1 (
    git remote add origin "%REMOTE_URL%"
) else (
    git remote set-url origin "%REMOTE_URL%"
)

echo.
echo Checking repository access...
gh repo view %REPO% >nul 2>nul
if errorlevel 1 (
    echo GitHub repo %REPO% was not found or you do not have access.
    echo Create it first, then rerun this script.
    pause
    exit /b 1
)

echo.
echo Updating local branch from origin/main...
git pull --ff-only origin main
if errorlevel 1 (
    echo Pull failed. Commit, stash, or resolve local changes, then rerun this script.
    pause
    exit /b 1
)

echo.
echo Current changes:
git status --short
echo.

git add README.md build_macos.sh build_windows.bat run.bat run_macos.sh make_icon.py imaginify.py requirements.txt requirements-build.txt .gitignore .gitattributes push_to_github.bat version.json
if errorlevel 1 (
    echo Git add failed.
    pause
    exit /b 1
)

git diff --cached --quiet
if not errorlevel 1 (
    echo Nothing staged to commit.
) else (
    git commit -m "%COMMIT_MESSAGE%"
    if errorlevel 1 (
        echo Commit failed.
        pause
        exit /b 1
    )
)

echo.
echo Pushing to GitHub...
git push -u origin main
if errorlevel 1 (
    echo Push failed. See the messages above.
    pause
    exit /b 1
)

echo.
echo === SUCCESS ===
echo Repo: https://github.com/%REPO%
pause
endlocal
