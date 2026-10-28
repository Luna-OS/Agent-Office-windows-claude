; Windows-Installer von OpenAgenticOffice (NSIS). Aufruf (aus dem Repo-Wurzelordner):
;   makensis /DVERSION=0.1.5 /DQUELLE=desktop\staging desktop\build\installer.nsi
; Erwartet in QUELLE: die Desktop-Hülle, den Server, dist\, claude\, tools\ (und werkzeuge\ mit gh.exe).
; Installiert nur für den aktuellen Benutzer (keine Administratorrechte nötig).

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef QUELLE
  !define QUELLE "desktop\staging"
!endif

Unicode true
Name "OpenAgenticOffice"
OutFile "..\release\OpenAgenticOffice-Setup-${VERSION}.exe"
InstallDir "$LOCALAPPDATA\Programs\OpenAgenticOffice"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!define MUI_ICON "icon.ico"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "German"

Section "Installieren"
  SetOutPath "$INSTDIR"
  File /r "..\..\${QUELLE}\*.*"
  WriteUninstaller "$INSTDIR\Deinstallieren.exe"

  ; Verknüpfungen: das Office (Connector) und Claude Code mit dem Claude-Abo, ohne API-Key.
  CreateShortCut "$DESKTOP\OpenAgenticOffice.lnk" "$INSTDIR\open-agentic-office-desktop.exe"
  CreateShortCut "$SMPROGRAMS\OpenAgenticOffice.lnk" "$INSTDIR\open-agentic-office-desktop.exe"
  SetOutPath "$PROFILE"
  CreateShortCut "$DESKTOP\Claude Code (ohne API-Key).lnk" "$INSTDIR\claude\claude-code.cmd" "" "$INSTDIR\claude\claude-code.ico" 0 SW_SHOWNORMAL "" "Claude Code mit deinem Claude-Abo starten - kein API-Key noetig"
  CreateShortCut "$SMPROGRAMS\Claude Code (ohne API-Key).lnk" "$INSTDIR\claude\claude-code.cmd" "" "$INSTDIR\claude\claude-code.ico" 0 SW_SHOWNORMAL "" "Claude Code mit deinem Claude-Abo starten - kein API-Key noetig"
  SetOutPath "$INSTDIR"

  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenAgenticOffice" "DisplayName" "OpenAgenticOffice"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenAgenticOffice" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenAgenticOffice" "UninstallString" "$INSTDIR\Deinstallieren.exe"
SectionEnd

Section "Uninstall"
  Delete "$DESKTOP\OpenAgenticOffice.lnk"
  Delete "$SMPROGRAMS\OpenAgenticOffice.lnk"
  Delete "$DESKTOP\Claude Code (ohne API-Key).lnk"
  Delete "$SMPROGRAMS\Claude Code (ohne API-Key).lnk"
  RMDir /r "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenAgenticOffice"
SectionEnd
