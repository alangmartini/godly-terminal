!macro NSIS_HOOK_PREINSTALL
  ; Rename running binaries so NSIS can copy new ones without "file in use" errors.
  ; Old processes keep running via file handle on the renamed files.
  Rename "$INSTDIR\godly-daemon.exe" "$INSTDIR\godly-daemon.exe.old"
  Rename "$INSTDIR\godly-mcp.exe" "$INSTDIR\godly-mcp.exe.old"
  Rename "$INSTDIR\godly-notify.exe" "$INSTDIR\godly-notify.exe.old"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Clean up renamed binaries (may fail if processes still running — that's OK)
  Delete "$INSTDIR\godly-daemon.exe.old"
  Delete "$INSTDIR\godly-mcp.exe.old"
  Delete "$INSTDIR\godly-notify.exe.old"
!macroend
