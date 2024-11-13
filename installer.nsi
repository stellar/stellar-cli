!addplugindir nsis
!include LogicLib.nsh

OutFile "${STELLAR_CLI_INSTALLER}"
InstallDir "$PROGRAMFILES\Stellar CLI"
RequestExecutionLevel admin
ShowInstDetails Show
Unicode True

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
    EnVar::SetHKLM
    EnVar::Check "Path" "$INSTDIR"
    Pop $0
    ${If} $0 = 0
      DetailPrint "Path already has $INSTDIR"
    ${Else}
      EnVar::AddValue "Path" "$INSTDIR"
      Pop $0 ; 0 on success
    ${EndIf}

    ; Notify Windows that the PATH has changed
    System::Call 'user32::SendMessageA(i 0xFFFF, i ${WM_SETTINGCHANGE}, i 0, i 0)'
SectionEnd

Section "Uninstall"
    Delete "$INSTDIR\stellar.exe"
    Delete "$INSTDIR\Uninstall.exe"
    Delete "$INSTDIR\stellar.ico"
    RMDir "$INSTDIR"

    ; Remove the Start Menu shortcut
    Delete "$SMPROGRAMS\Stellar CLI\Uninstall.lnk"
    RMDir "$SMPROGRAMS\Stellar CLI"

    ; Remove the entry from "Programs and Features"
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Stellar CLI"

    ; Remove install directory from PATH
    EnVar::SetHKLM
    EnVar::DeleteValue "Path" "$INSTDIR"
    Pop $0
    ${If} $0 = 0
      DetailPrint "$INSTDIR was removed from Path"
    ${Else}
      DetailPrint "Unable to remove $INSTDIR from Path"
    ${EndIf}

    ; Notify Windows that the PATH has changed
    System::Call 'user32::SendMessageA(i 0xFFFF, i ${WM_SETTINGCHANGE}, i 0, i 0)'
SectionEnd
