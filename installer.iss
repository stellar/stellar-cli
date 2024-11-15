#define STELLAR_CLI_VERSION GetEnv("STELLAR_CLI_VERSION")
#define STELLAR_CLI_INSTALLER GetEnv("STELLAR_CLI_INSTALLER")

[Setup]
AppName=Stellar CLI
AppVersion={#STELLAR_CLI_VERSION}
DefaultDirName={commonpf}\Stellar CLI
DefaultGroupName=Stellar CLI
OutputBaseFilename=stellar-installer
PrivilegesRequired=admin
LicenseFile=LICENSE
UninstallDisplayIcon={app}\stellar.ico
Compression=lzma
SolidCompression=yes
ChangesEnvironment=yes

[Files]
Source: "stellar.exe"; DestDir: "{app}"
Source: "stellar.ico"; DestDir: "{app}"
Source: "LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
; Windows optimizes start menu, and removes the uninstall entry. Unless we
; specify it twice. 🫠
Name: "{group}\Uninstall Stellar CLI"; Filename: "{uninstallexe}"
Name: "{group}\Uninstall Stellar CLI"; Filename: "{uninstallexe}"
Name: "{group}\Stellar Developer Docs"; Filename: "https://stellar.org/docs"

[Code]
const EnvironmentKey = 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment';

procedure EnvAddPath(Path: string);
var
    Paths: string;
begin
    { Retrieve current path (use empty string if entry not exists) }
    if not RegQueryStringValue(HKEY_LOCAL_MACHINE, EnvironmentKey, 'Path', Paths)
    then Paths := '';

    { Skip if string already found in path }
    if Pos(';' + Uppercase(Path) + ';', ';' + Uppercase(Paths) + ';') > 0 then exit;

    { App string to the end of the path variable }
    Paths := Paths + ';'+ Path +';'

    { Overwrite (or create if missing) path environment variable }
    if RegWriteStringValue(HKEY_LOCAL_MACHINE, EnvironmentKey, 'Path', Paths)
    then Log(Format('The [%s] added to PATH: [%s]', [Path, Paths]))
    else Log(Format('Error while adding the [%s] to PATH: [%s]', [Path, Paths]));
end;

procedure EnvRemovePath(Path: string);
var
    Paths: string;
    P: Integer;
begin
    { Skip if registry entry not exists }
    if not RegQueryStringValue(HKEY_LOCAL_MACHINE, EnvironmentKey, 'Path', Paths) then
        exit;

    { Skip if string not found in path }
    P := Pos(';' + Uppercase(Path) + ';', ';' + Uppercase(Paths) + ';');
    if P = 0 then exit;

    { Update path variable }
    Delete(Paths, P - 1, Length(Path) + 1);

    { Overwrite path environment variable }
    if RegWriteStringValue(HKEY_LOCAL_MACHINE, EnvironmentKey, 'Path', Paths)
    then Log(Format('The [%s] removed from PATH: [%s]', [Path, Paths]))
    else Log(Format('Error while removing the [%s] from PATH: [%s]', [Path, Paths]));
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
    if CurStep = ssPostInstall
     then EnvAddPath(ExpandConstant('{app}'));
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
    if CurUninstallStep = usPostUninstall
    then EnvRemovePath(ExpandConstant('{app}'));
end;

[UninstallDelete]
; Remove the Start Menu group
Type: filesandordirs; Name: "{group}"

; Remove installed files and directory
Type: files; Name: "{app}\stellar.exe"
Type: files; Name: "{uninstallexe}"
Type: files; Name: "{app}\stellar.ico"
Type: dirifempty; Name: "{app}"
