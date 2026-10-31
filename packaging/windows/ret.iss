; Inno Setup script for the Windows installer of ret
;
; It installs the two programs (ret.exe and ret-ui.exe), the launcher and the
; Android application of the same release into the installation directory,
; which defaults to %LOCALAPPDATA%\Programs\ret.
;
; Build it with Inno Setup 6.3 or later (https://jrsoftware.org/isinfo.php):
;
;     iscc /DAppVersion=0.3.0 /DAppArch=x86_64 packaging\windows\ret.iss
;
; AppVersion comes from the release tag without its leading v. AppArch must be
; x86_64 or arm64. FilesDir defaults to dist\ret-windows-<AppArch>.
; Stage ret.exe, ret-ui.exe, ret-run.cmd and ret.apk there, or set FilesDir:
;
;     iscc /DAppVersion=0.3.0 /DAppArch=arm64 /DFilesDir=C:\staging packaging\windows\ret.iss

#define AppName "ret"
#define AppPublisher "HoppouDev"
#define AppURL "https://github.com/HoppouDev/ret"

#ifndef AppVersion
  #error AppVersion must be supplied from the release tag
#endif

#ifndef AppArch
  #error AppArch must be supplied as x86_64 or arm64
#endif

#if AppArch == "x86_64"
  #define InstallArch "x64os"
#elif AppArch == "arm64"
  #define InstallArch "arm64"
#else
  #error AppArch must be x86_64 or arm64
#endif

#ifndef FilesDir
  #define FilesDir "..\..\dist\ret-windows-" + AppArch
#endif

[Setup]
AppId={{8F6BAB99-2589-437A-B98F-A99E2A9578EC}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/issues
AppUpdatesURL={#AppURL}/releases
DefaultDirName={autopf}\ret
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
LicenseFile=..\..\LICENSE
OutputBaseFilename=ret-setup-{#AppVersion}-windows-{#AppArch}
#ifndef OutputDir
  #define OutputDir "..\..\dist"
#endif
OutputDir={#OutputDir}
SetupIconFile=..\..\assets\ret.ico
UninstallDisplayName={#AppName}
UninstallDisplayIcon={app}\ret-ui.exe
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
ArchitecturesAllowed={#InstallArch}
ArchitecturesInstallIn64BitMode={#InstallArch}
PrivilegesRequired=lowest
ChangesEnvironment=yes

[Tasks]
Name: "addtopath"; Description: "Add the installation directory to the Path (to run ret.exe from any terminal)"; \
    GroupDescription: "Environment:"
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; \
    GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#FilesDir}\ret.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#FilesDir}\ret-ui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#FilesDir}\ret-run.cmd"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#FilesDir}\ret.apk"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\ret-ui.exe"; WorkingDir: "{app}"; \
    Comment: "Reverse tethering for Android over adb"
Name: "{autoprograms}\{#AppName} reverse tethering"; Filename: "{app}\ret-run.cmd"; WorkingDir: "{app}"; \
    Comment: "Start the client of every device and the relay server"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\ret-ui.exe"; WorkingDir: "{app}"; \
    Tasks: desktopicon

[Run]
Filename: "{app}\ret-ui.exe"; Description: "{cm:LaunchProgram,{#AppName}}"; \
    WorkingDir: "{app}"; Flags: nowait postinstall skipifsilent

[Code]
const
  EnvironmentKey = 'Environment';
  InstallerStateKey = 'Software\HoppouDev\ret\Installer';

// The directory of the installed apk, the default value of the RET_APK
// variable of ret
function InstalledApkVar: String;
begin
  Result := ExpandConstant('{app}') + '\ret.apk';
end;

// Compare two directories without their trailing backslash and their case
function NormalizeDir(Dir: String): String;
begin
  Result := Lowercase(RemoveBackslashUnlessRoot(Dir));
end;

// Whether the semicolon separated list of directories contains Dir
function PathContainsDir(Path, Dir: String): Boolean;
var
  Part: String;
begin
  Result := False;
  while Path <> '' do
  begin
    if Pos(';', Path) > 0 then
    begin
      Part := Copy(Path, 1, Pos(';', Path) - 1);
      Path := Copy(Path, Pos(';', Path) + 1, MaxInt);
    end
    else
    begin
      Part := Path;
      Path := '';
    end;
    if (Part <> '') and (NormalizeDir(Part) = NormalizeDir(Dir)) then
    begin
      Result := True;
      Exit;
    end;
  end;
end;

// Add the installation directory to the user Path
procedure AddToUserPath;
var
  App, Path: String;
begin
  App := ExpandConstant('{app}');
  if not RegQueryStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Path) then
    Path := '';
  if not PathContainsDir(Path, App) then
  begin
    if Path = '' then
      Path := App
    else
      Path := Path + ';' + App;
    if RegWriteExpandStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Path) then
      RegWriteStringValue(HKEY_CURRENT_USER, InstallerStateKey, 'PathAdded', App);
  end;
end;

// Remove the installation directory from the user Path
procedure RemoveFromUserPath;
var
  App, Path, Part, Remaining, OwnedPath: String;
  First, Last: Boolean;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, InstallerStateKey, 'PathAdded', OwnedPath) then
    Exit;
  if NormalizeDir(OwnedPath) <> NormalizeDir(ExpandConstant('{app}')) then
    Exit;
  if not RegQueryStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Path) then
  begin
    RegDeleteValue(HKEY_CURRENT_USER, InstallerStateKey, 'PathAdded');
    Exit;
  end;
  App := NormalizeDir(ExpandConstant('{app}'));
  Remaining := '';
  First := True;
  while True do
  begin
    Last := Pos(';', Path) = 0;
    if not Last then
    begin
      Part := Copy(Path, 1, Pos(';', Path) - 1);
      Path := Copy(Path, Pos(';', Path) + 1, MaxInt);
    end
    else
    begin
      Part := Path;
      Path := '';
    end;
    if NormalizeDir(Part) <> App then
    begin
      if not First then
        Remaining := Remaining + ';';
      Remaining := Remaining + Part;
      First := False;
    end;
    if Last then
      Break;
  end;
  if First then
    RegDeleteValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path')
  else
    RegWriteExpandStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Remaining);
  RegDeleteValue(HKEY_CURRENT_USER, InstallerStateKey, 'PathAdded');
end;

// Point RET_APK at the installed apk, unless the user set their own value
procedure SetRetApkDefault;
var
  Current: String;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'RET_APK', Current) then
    if RegWriteStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'RET_APK', InstalledApkVar) then
      RegWriteStringValue(HKEY_CURRENT_USER, InstallerStateKey, 'ApkSet', InstalledApkVar);
end;

// Remove the RET_APK value we set, keep one the user changed
procedure UnsetRetApkDefault;
var
  Current, OwnedApk: String;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, InstallerStateKey, 'ApkSet', OwnedApk) then
    Exit;
  if Lowercase(OwnedApk) <> Lowercase(InstalledApkVar) then
    Exit;
  if RegQueryStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'RET_APK', Current) then
  begin
    if Lowercase(Current) = Lowercase(InstalledApkVar) then
      RegDeleteValue(HKEY_CURRENT_USER, EnvironmentKey, 'RET_APK');
  end;
  RegDeleteValue(HKEY_CURRENT_USER, InstallerStateKey, 'ApkSet');
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    if WizardIsTaskSelected('addtopath') then
      AddToUserPath;
    SetRetApkDefault;
  end;
end;

// The uninstaller does not broadcast the changes of the environment, so
// Windows picks up the removed Path entry and RET_APK the next time the user
// signs in
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    RemoveFromUserPath;
    UnsetRetApkDefault;
    RegDeleteKeyIfEmpty(HKEY_CURRENT_USER, InstallerStateKey);
  end;
end;
