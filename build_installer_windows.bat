@echo off
REM ----------------------------------------------------------------
REM  Imaginify - Windows installer build
REM  Compiles installer_windows.iss with Inno Setup 6.
REM  Output: dist\Imaginify-Setup-<version>.exe
REM
REM  Prerequisite: install Inno Setup 6 from https://jrsoftware.org/isdl.php
REM  (or:  winget install --id JRSoftware.InnoSetup)
REM ----------------------------------------------------------------
setlocal
cd /d "%~dp0"

echo === Imaginify Windows installer build ===
echo.

REM Make sure the app .exe exists first.
if not exist "dist\Imaginify.exe" (
    echo dist\Imaginify.exe not found.
    echo Run build_windows.bat first to build the .exe, then re-run this script.
    pause
    exit /b 1
)

REM Locate ISCC.exe (Inno Setup Compiler).
set "ISCC="
if exist "%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe" set "ISCC=%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"
if exist "%ProgramFiles%\Inno Setup 6\ISCC.exe"      set "ISCC=%ProgramFiles%\Inno Setup 6\ISCC.exe"
if "%ISCC%"=="" (
    where ISCC.exe >nul 2>nul
    if not errorlevel 1 set "ISCC=ISCC.exe"
)
if "%ISCC%"=="" (
    echo Inno Setup Compiler (ISCC.exe) not found.
    echo Install Inno Setup 6 from https://jrsoftware.org/isdl.php
    echo (or:  winget install --id JRSoftware.InnoSetup)
    pause
    exit /b 1
)

echo Using compiler: %ISCC%
echo.

"%ISCC%" installer_windows.iss
if errorlevel 1 (
    echo.
    echo Installer build failed. See messages above.
    pause
    exit /b 1
)

echo.
echo === INSTALLER BUILD SUCCESSFUL ===
echo.
for %%F in (dist\Imaginify-Setup-*.exe) do echo Output: %%~fF
echo.
echo Unsigned installers trigger SmartScreen "unrecognized app" warnings.
echo Users need to click "More info" then "Run anyway" on first launch.
echo.
pause
endlocal
