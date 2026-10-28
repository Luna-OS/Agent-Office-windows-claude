; Extra shortcuts the Agent Office installer creates (electron-builder includes this file, see
; "nsis.include" in desktop/package.json): "Claude Code (ohne API-Key)" on the desktop and in the
; Start menu. It runs resources\claude\claude-code.cmd, which starts Claude Code with the Claude
; subscription login (Pro / Max) and installs Claude Code first if it is missing.

!macro customInstall
  ; The shortcut's "Start in" folder: the user's profile.
  SetOutPath "$PROFILE"
  CreateShortCut "$DESKTOP\Claude Code (ohne API-Key).lnk" "$INSTDIR\resources\claude\claude-code.cmd" "" "$INSTDIR\resources\claude\claude-code.ico" 0 SW_SHOWNORMAL "" "Claude Code mit deinem Claude-Abo starten - kein API-Key noetig"
  CreateShortCut "$SMPROGRAMS\Claude Code (ohne API-Key).lnk" "$INSTDIR\resources\claude\claude-code.cmd" "" "$INSTDIR\resources\claude\claude-code.ico" 0 SW_SHOWNORMAL "" "Claude Code mit deinem Claude-Abo starten - kein API-Key noetig"
  SetOutPath "$INSTDIR"
!macroend

!macro customUnInstall
  Delete "$DESKTOP\Claude Code (ohne API-Key).lnk"
  Delete "$SMPROGRAMS\Claude Code (ohne API-Key).lnk"
!macroend
