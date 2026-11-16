; Mpt — per-user installer. No admin prompt: everything goes in the user's
; own profile, so an install is just a copy plus shortcuts. Built by CI with
;   ISCC assets\installer.iss /DAppVersion=x.y.z /DSourceDir=<dir with mpt.exe + readme.txt>
; Run from the repo root's out\ directory in CI.

#ifndef AppVersion
#define AppVersion "0.0.0"
#endif

#ifndef SourceDir
#define SourceDir "."
#endif

[Setup]
AppId={{7C1E3A58-2B44-4E8D-9A16-50C9B4F2D3A1}
AppName=Mpt
AppVersion={#AppVersion}
AppPublisher=Moonkip
AppPublisherURL=https://github.com/moonkip18/Mpt
DefaultDirName={localappdata}\Programs\Mpt
DisableDirPage=yes
DisableProgramGroupPage=yes
; Per-user install: no elevation, no UAC.
PrivilegesRequired=lowest
OutputDir=.
OutputBaseFilename=Mpt-Setup-{#AppVersion}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=no
SetupIconFile=app.ico
UninstallDisplayName=Mpt
UninstallDisplayIcon={app}\mpt.exe

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked
Name: "autostart"; Description: "Start Mpt when Windows starts"; Flags: unchecked

[Files]
Source: "{#SourceDir}\mpt.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\readme.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Mpt"; Filename: "{app}\mpt.exe"
Name: "{autodesktop}\Mpt"; Filename: "{app}\mpt.exe"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Mpt"; ValueData: """{app}\mpt.exe"""; Tasks: autostart; Flags: uninsdeletevalue

[Run]
Filename: "{app}\mpt.exe"; Description: "Run Mpt"; Flags: nowait postinstall skipifsilent
