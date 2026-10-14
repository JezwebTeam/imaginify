@echo off
REM ----------------------------------------------------------------
REM  Imaginify - Windows app + installer build
REM  1. builds the Tauri app (npm run build:exe)
REM  2. copies it to dist\Imaginify.exe
REM  3. downloads Microsoft's WebView2 bootstrapper if missing
REM  4. compiles installer_windows.iss with Inno Setup 6
REM  Output: dist\Imaginify.exe and dist\Imaginify-Setup-<version>.exe
REM
REM  Prerequisites: Rust (rustup), Node.js, Visual Studio Build Tools
REM  ("Desktop development with C++"), Inno Setup 6
REM  (winget install --id JRSoftware.InnoSetup). Optional: NASM for
REM  faster JPEG encoding (winget install --id NASM.NASM).
REM  release.yml runs this same script with stdin from NUL.
REM ----------------------------------------------------------------
setlocal
cd /d "%~dp0"

echo === Imaginify Windows build ===
echo.

if not exist "node_modules" (
    call npm ci
    if errorlevel 1 goto :fail
)
call npm run build:exe
if errorlevel 1 goto :fail

if not exist "dist" mkdir dist
copy /y "target\release\imaginify.exe" "dist\Imaginify.exe" >nul
if errorlevel 1 (
    echo target\release\imaginify.exe was not built.
    goto :fail
)

REM WebView2 bootstrapper: download to a .part file and keep it only when
REM the download completed, so a broken file is never reused by a later build.
if not exist "installer_windows" mkdir installer_windows
set "WV2=installer_windows\MicrosoftEdgeWebview2Setup.exe"
if exist "%WV2%" for %%A in ("%WV2%") do if %%~zA LSS 100000 del /q "%WV2%"
if exist "%WV2%" goto :have_wv2
echo Downloading the WebView2 bootstrapper...
curl.exe -fsSL -o "%WV2%.part" "https://go.microsoft.com/fwlink/p/?LinkId=2124703"
if errorlevel 1 goto :wv2_fail
move /y "%WV2%.part" "%WV2%" >nul
if errorlevel 1 goto :wv2_fail
:have_wv2
REM Package the bootstrapper only if Microsoft signed it (checks cached copies too).
powershell -NoProfile -Command "$s = Get-AuthenticodeSignature -LiteralPath '%WV2%'; Write-Host ('Bootstrapper signature: ' + $s.Status + ' ' + $s.SignerCertificate.Subject); if ($s.Status -ne 'Valid' -or $s.SignerCertificate.Subject -notmatch '(^|, )O=Microsoft Corporation(,|$)') { exit 1 }"
if errorlevel 1 goto :wv2_bad_signature

REM Locate ISCC.exe (Inno Setup Compiler).
REM NOTE: do NOT use  if "%ISCC%"==""  inside a parenthesized block --
REM       ProgramFiles(x86) path contains ')' which breaks cmd's parser.
set "ISCC="
if exist "%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"        set "ISCC=%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"
if not defined ISCC if exist "%ProgramFiles%\Inno Setup 6\ISCC.exe"        set "ISCC=%ProgramFiles%\Inno Setup 6\ISCC.exe"
if not defined ISCC if exist "%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" set "ISCC=%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe"
if not defined ISCC where ISCC.exe >nul 2>nul && set "ISCC=ISCC.exe"

if not defined ISCC (
    echo Inno Setup Compiler ISCC.exe not found.
    echo Install Inno Setup 6 from https://jrsoftware.org/isdl.php
    echo   winget install --id JRSoftware.InnoSetup
    goto :fail
)

echo Using compiler: %ISCC%
echo.

"%ISCC%" installer_windows.iss
if errorlevel 1 goto :fail

echo.
echo === BUILD SUCCESSFUL ===
echo.
for %%F in (dist\Imaginify-Setup-*.exe) do echo Output: %%~fF
echo Output: %CD%\dist\Imaginify.exe
echo.
echo Unsigned builds trigger SmartScreen "unrecognized app" warnings.
echo Users need to click "More info" then "Run anyway" on first launch.
echo.
pause
endlocal
exit /b 0

:wv2_bad_signature
del /q "%WV2%"
echo The WebView2 bootstrapper is not signed by Microsoft; deleted it. Run the build again.
goto :fail

:wv2_fail
if exist "%WV2%.part" del /q "%WV2%.part"
echo Could not download the WebView2 bootstrapper.

:fail
echo.
echo Build failed. See messages above.
pause
endlocal
exit /b 1
