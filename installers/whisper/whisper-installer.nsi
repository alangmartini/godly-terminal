; Godly Whisper — standalone NSIS installer
; Installs godly-whisper.exe to %LOCALAPPDATA%\godly-whisper\

!include "MUI2.nsh"

Name "Godly Whisper"
OutFile "..\..\installations\whisper\godly-whisper-setup.exe"
InstallDir "$LOCALAPPDATA\godly-whisper"
RequestExecutionLevel user

; --- UI ---
!define MUI_ICON "..\..\src-tauri\icons\icon.ico"
!define MUI_UNICON "..\..\src-tauri\icons\icon.ico"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Install"
    SetOutPath $INSTDIR

    ; Copy binary and version metadata
    File "staging\godly-whisper.exe"
    File "staging\version.json"

    ; Create uninstaller
    WriteUninstaller "$INSTDIR\uninstall.exe"

    ; Start Menu shortcut (under Godly Terminal folder)
    CreateDirectory "$SMPROGRAMS\Godly Terminal"
    CreateShortCut "$SMPROGRAMS\Godly Terminal\Uninstall Godly Whisper.lnk" "$INSTDIR\uninstall.exe"

    ; Registry entry for Add/Remove Programs
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GodlyWhisper" \
        "DisplayName" "Godly Whisper"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GodlyWhisper" \
        "UninstallString" '"$INSTDIR\uninstall.exe"'
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GodlyWhisper" \
        "InstallLocation" "$INSTDIR"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GodlyWhisper" \
        "Publisher" "Godly Terminal"
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GodlyWhisper" \
        "NoModify" 1
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GodlyWhisper" \
        "NoRepair" 1
SectionEnd

Section "Uninstall"
    Delete "$INSTDIR\godly-whisper.exe"
    Delete "$INSTDIR\version.json"
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"

    Delete "$SMPROGRAMS\Godly Terminal\Uninstall Godly Whisper.lnk"
    ; Only remove the folder if empty (don't delete other Godly Terminal shortcuts)
    RMDir "$SMPROGRAMS\Godly Terminal"

    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GodlyWhisper"
SectionEnd
