#define STELLAR_CLI_VERSION GetEnv("STELLAR_CLI_VERSION")
#define STELLAR_CLI_INSTALLER GetEnv("STELLAR_CLI_INSTALLER")

[Setup]
AppName=Stellar CLI
AppVersion={#STELLAR_CLI_VERSION}
DefaultDirName={commonpf}\Stellar CLI
DefaultGroupName=Stellar CLI
OutputBaseFilename=stellar-installer
PrivilegesRequired=admin
LicenseFile=License.txt
UninstallDisplayIcon={app}\stellar.ico
Compression=lzma
SolidCompression=yes

[Files]
Source: "stellar.exe"; DestDir: "{app}"
Source: "stellar.ico"; DestDir: "{app}"
Source: "License.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
; Windows optimizes start menu, and removes the uninstall entry. Unless we
; specify it twice. 🫠
Name: "{group}\Uninstall Stellar CLI"; Filename: "{uninstallexe}"
Name: "{group}\Uninstall Stellar CLI"; Filename: "{uninstallexe}"
Name: "{group}\Stellar Docs"; Filename: "https://stellar.org/docs"

[Registry]
; Add install directory to the system PATH
Root: HKLM; Subkey: "SYSTEM\CurrentControlSet\Control\Session Manager\Environment"; \
    ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Flags: preservestringtype uninsdeletevalue

[UninstallDelete]
; Remove the Start Menu group
Type: filesandordirs; Name: "{group}"

; Remove installed files and directory
Type: files; Name: "{app}\stellar.exe"
Type: files; Name: "{app}\unins000.exe"
Type: files; Name: "{app}\stellar.ico"
Type: dirifempty; Name: "{app}"
