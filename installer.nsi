OutFile "${STELLAR_CLI_INSTALLER}"
InstallDir "$PROGRAMFILES\Stellar CLI"
RequestExecutionLevel admin

; Define WM_SETTINGCHANGE since NSIS doesn’t natively recognize it
!define WM_SETTINGCHANGE 0x1A

Section "Install"
    SetOutPath "$INSTDIR"
    File "stellar.exe"
    File "stellar.ico"
    WriteUninstaller "$INSTDIR\Uninstall.exe"

    ; Create a shortcut in the Start Menu
    CreateDirectory "$SMPROGRAMS\Stellar CLI"
    CreateShortCut "$SMPROGRAMS\Stellar CLI\Uninstall.lnk" "$INSTDIR\Uninstall.exe" "" "$INSTDIR\Uninstall.exe" 0

    ; Add an entry to the Windows "Programs and Features" list
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Stellar CLI" "DisplayName" "Stellar CLI"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Stellar CLI" "UninstallString" "$INSTDIR\Uninstall.exe"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Stellar CLI" "DisplayIcon" "$INSTDIR\stellar.ico"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Stellar CLI" "DisplayVersion" "${STELLAR_CLI_VERSION}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Stellar CLI" "Publisher" "Stellar"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Stellar CLI" "InstallLocation" "$INSTDIR"

    ; Add install directory to the PATH
    ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path"
    StrCpy $1 "$0;$INSTDIR"
    WriteRegStr HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path" "$1"

    ; Notify Windows that the PATH has changed
    System::Call 'user32::SendMessageA(i 0xFFFF, i ${WM_SETTINGCHANGE}, i 0, i 0)'
SectionEnd

Section "Uninstall"
    Delete "$INSTDIR\stellar.exe"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir "$INSTDIR"

    ; Remove the Start Menu shortcut
    Delete "$SMPROGRAMS\Stellar CLI\Uninstall.lnk"
    RMDir "$SMPROGRAMS\Stellar CLI"

    ; Remove the entry from "Programs and Features"
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Stellar CLI"

    ; Restore PATH without the installation directory
    ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path"
    StrCpy $1 "$0"  ; Store the original PATH in $1

    ; Remove install directory from PATH (manual string removal)
    StrLen $2 "$INSTDIR"
    loop:
    StrCpy $3 "$1" "$2"
    StrCmp $3 "$INSTDIR" 0 +3
    StrCpy $1 "$1" "" $2
    goto loop

    ; Write the modified PATH back to registry
    WriteRegStr HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path" "$1"

    ; Notify Windows that the PATH has changed
    System::Call 'user32::SendMessageA(i 0xFFFF, i ${WM_SETTINGCHANGE}, i 0, i 0)'
SectionEnd
