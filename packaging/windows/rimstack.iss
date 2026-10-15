[Setup]
AppId={{6B0E3C2A-8F4D-4C55-9E1B-2F7A9D4C1E83}
AppName=RimStack
AppVersion={#Version}
AppPublisher=RimStack
DefaultDirName={autopf}\RimStack
DefaultGroupName=RimStack
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputBaseFilename=RimStack-{#Version}-setup
Compression=lzma2
SolidCompression=yes
UninstallDisplayIcon={app}\rimstack.exe
WizardStyle=modern

[Files]
Source: "{#Stage}\*"; DestDir: "{app}"; Flags: recursesubdirs ignoreversion

[Icons]
Name: "{group}\RimStack"; Filename: "{app}\rimstack.exe"
Name: "{autodesktop}\RimStack"; Filename: "{app}\rimstack.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Run]
Filename: "{app}\rimstack.exe"; Description: "{cm:LaunchProgram,RimStack}"; Flags: nowait postinstall skipifsilent
