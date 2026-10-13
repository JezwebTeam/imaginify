; ============================================================
;  Imaginify - Windows installer (Inno Setup)
;  Produces: dist\Imaginify-Setup-<version>.exe
;  Wraps:    dist\Imaginify.exe (the Tauri app, copied there by
;            build_installer_windows.bat or release.yml)
;
;  Requires Inno Setup 6+:  https://jrsoftware.org/isdl.php
;  Build it via:            build_installer_windows.bat
; ============================================================

#define MyAppName       "Imaginify"
#define MyAppVersion    "1.0.1"
#define MyAppPublisher  "JezPress"
#define MyAppURL        "https://jezpress.com/"
#define MyAppExeName    "Imaginify.exe"
; Microsoft's WebView2 Evergreen bootstrapper (downloads the runtime when run).
#define WebView2Setup   "installer_windows\MicrosoftEdgeWebview2Setup.exe"
#define WebView2Client  "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

[Setup]
; AppId is the stable identity used for upgrades and uninstall lookups.
; Do NOT change it between versions or upgrades won't replace old installs.
; 2.x keeps 1.x's AppId and exe name, so it upgrades 1.x in place.
AppId={{1A9A715E-1503-4D87-B8E0-3F36F4E7D8E0}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
LicenseFile=LICENSE.txt
OutputDir=dist
OutputBaseFilename=Imaginify-Setup-{#MyAppVersion}
SetupIconFile=src-tauri\icons\icon.ico
Compression=lzma2/ultra
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName={#MyAppName} {#MyAppVersion}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "dist\Imaginify.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "LICENSE.txt";        DestDir: "{app}"; Flags: ignoreversion
; Extracted on demand by PrepareToInstall, never installed.
Source: "{#WebView2Setup}";   Flags: dontcopy

[InstallDelete]
; 1.x installed README.md next to the app; 2.x does not ship it.
Type: files; Name: "{app}\README.md"

[Icons]
Name: "{group}\{#MyAppName}";       Filename: "{app}\{#MyAppExeName}"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent

[Code]
function HasWebView2(RootKey: Integer): Boolean;
var
  Version: String;
begin
  Result := RegQueryStringValue(RootKey, '{#WebView2Client}', 'pv', Version)
    and (Version <> '') and (Version <> '0.0.0.0');
end;

function NeedsWebView2: Boolean;
begin
  Result := not (HasWebView2(HKLM32) or HasWebView2(HKLM64) or HasWebView2(HKCU));
end;

// Imaginify draws its window with WebView2. Windows 11 ships it; older
// Windows 10 machines may not. Install it BEFORE any app file is replaced,
// and stop Setup if it is still missing afterwards, so an upgrade never
// swaps a working 1.x for an app that cannot open.
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ResultCode: Integer;
begin
  Result := '';
  if not NeedsWebView2 then
    exit;
  WizardForm.PreparingLabel.Caption := 'Installing the Microsoft Edge WebView2 Runtime...';
  ExtractTemporaryFile('MicrosoftEdgeWebview2Setup.exe');
  if not Exec(ExpandConstant('{tmp}\MicrosoftEdgeWebview2Setup.exe'), '/silent /install', '',
              SW_HIDE, ewWaitUntilTerminated, ResultCode) then
    ResultCode := -1;
  if NeedsWebView2 then
    Result := 'Imaginify needs the Microsoft Edge WebView2 Runtime, and it could not be installed '
      + '(code ' + IntToStr(ResultCode) + '). Nothing has been changed. Check the internet '
      + 'connection and run Setup again, or install the runtime from '
      + 'https://go.microsoft.com/fwlink/p/?LinkId=2124703 first.';
end;
