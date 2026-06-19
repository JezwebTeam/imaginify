; ============================================================
;  Imaginify - Windows installer (Inno Setup)
;  Produces: dist\Imaginify-Setup-<version>.exe
;  Wraps:    dist\Imaginify.exe (built by build_windows.bat)
;
;  Requires Inno Setup 6+:  https://jrsoftware.org/isdl.php
;  Build it via:            build_installer_windows.bat
; ============================================================

#define MyAppName       "Imaginify"
#define MyAppVersion    "1.0.0"
#define MyAppPublisher  "JezPress"
#define MyAppURL        "https://jezpress.com/"
#define MyAppExeName    "Imaginify.exe"

[Setup]
; AppId is the stable identity used for upgrades and uninstall lookups.
; Do NOT change it between versions or upgrades won't replace old installs.
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
SetupIconFile=icon.ico
Compression=lzma2/ultra
SolidCompression=yes
WizardStyle=modern
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
Source: "README.md";          DestDir: "{app}"; Flags: ignoreversion isreadme

[Icons]
Name: "{group}\{#MyAppName}";       Filename: "{app}\{#MyAppExeName}"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent
