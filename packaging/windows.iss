; Compile from app/ after cargo build --release and build-windows-lua.ps1.
; AppId and install directory stay stable across upgrades.
#ifndef AppVersion
  #error AppVersion must be supplied to ISCC
#endif
#ifndef OutputName
  #define OutputName "PixelCraft-Setup"
#endif
[Setup]
AppId={{C2A8C5DF-3977-47D9-9543-2681958D7084}
AppName=PixelCraft
AppVersion={#AppVersion}
AppPublisher=PixelCraft contributors
DefaultDirName={localappdata}\Programs\PixelCraft
DefaultGroupName=PixelCraft
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\dist
OutputBaseFilename={#OutputName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
LicenseFile=..\LICENSE
UninstallDisplayIcon={app}\pixelcraft.exe
CloseApplications=yes

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "..\target\release\pixelcraft.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\runtime\LUA-LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\PixelCraft"; Filename: "{app}\pixelcraft.exe"
Name: "{autodesktop}\PixelCraft"; Filename: "{app}\pixelcraft.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\pixelcraft.exe"; Description: "Open PixelCraft"; Flags: nowait postinstall skipifsilent
