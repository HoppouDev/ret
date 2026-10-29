; Inno Setup script for the Windows installer of ret
;
; It installs the two programs (ret.exe and ret-ui.exe), the launcher and the
; Android application of the same release into the installation directory,
; which defaults to %LOCALAPPDATA%\Programs\ret.
;
; Build it with Inno Setup 6.3 or later (https://jrsoftware.org/isinfo.php):
;
;     iscc /DAppVersion=0.2.0 packaging\windows\ret.iss
;
; AppVersion comes from the release tag without its leading v. The files come
; from FilesDir, which defaults to the layout of the ret-win64 zip that the
; packaging/release script and the release workflow produce:
;
;     iscc /DAppVersion=0.2.0 /DFilesDir=C:\path\to\dist\ret-win64 packaging\windows\ret.iss

#define AppName "ret"
#define AppPublisher "HoppouDev"
#define AppURL "https://github.com/HoppouDev/ret"

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

#ifndef FilesDir
  #define FilesDir "..\..\dist\ret-win64"
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
OutputBaseFilename=ret-setup-{#AppVersion}
OutputDir=..\..\dist
SetupIconFile=..\..\assets\ret.ico
UninstallDisplayName={#AppName}
UninstallDisplayIcon={app}\ret-ui.exe
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
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
    RegWriteStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Path);
  end;
end;

// Remove the installation directory from the user Path
procedure RemoveFromUserPath;
var
  App, Path, Part, Remaining: String;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Path) then
    Exit;
  App := NormalizeDir(ExpandConstant('{app}'));
  Remaining := '';
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
    if (Part <> '') and (NormalizeDir(Part) <> App) then
    begin
      if Remaining = '' then
        Remaining := Part
      else
        Remaining := Remaining + ';' + Part;
    end;
  end;
  if Remaining = '' then
    RegDeleteValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path')
  else
    RegWriteStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Remaining);
end;

// Point RET_APK at the installed apk, unless the user set their own value
procedure SetRetApkDefault;
var
  Current: String;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'RET_APK', Current) then
    RegWriteStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'RET_APK', InstalledApkVar);
end;

// Remove the RET_APK value we set, keep one the user changed
procedure UnsetRetApkDefault;
var
  Current: String;
begin
  if RegQueryStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'RET_APK', Current) then
  begin
    if Lowercase(Current) = Lowercase(InstalledApkVar) then
      RegDeleteValue(HKEY_CURRENT_USER, EnvironmentKey, 'RET_APK');
  end;
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
  end;
end;
