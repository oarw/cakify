#define AppVersion GetEnv("CAKIFY_VERSION")

#if AppVersion == ""
  #error CAKIFY_VERSION must be set before compiling the installer.
#endif

[Setup]
AppId={{AB39EE20-8067-4C26-A558-8BCB80E13BCC}
AppName=Cakify
AppVersion={#AppVersion}
AppPublisher=Cakify
DefaultDirName={localappdata}\Programs\Cakify
DefaultGroupName=Cakify
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayName=Cakify
UninstallDisplayIcon={app}\Cakify.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupLogging=yes
CloseApplications=yes
RestartApplications=no
DisableProgramGroupPage=yes
OutputDir=..\..\release-dist
OutputBaseFilename=Cakify-Setup

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\..\target\release\cakify-desktop.exe"; DestDir: "{app}"; DestName: "Cakify.exe"; Flags: ignoreversion

[Icons]
Name: "{group}\Cakify"; Filename: "{app}\Cakify.exe"
Name: "{autodesktop}\Cakify"; Filename: "{app}\Cakify.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\Cakify.exe"; Description: "{cm:LaunchProgram,Cakify}"; Flags: nowait postinstall skipifsilent
