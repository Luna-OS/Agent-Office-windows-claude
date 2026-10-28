@echo off
rem Installs Git for Windows for the Agent Office Windows app (menu "GitHub", and on start when git
rem is missing). The office needs git for projects and worker worktrees, and Claude Code needs its
rem Git Bash on Windows.
setlocal
title Agent Office - Git for Windows installieren
where winget >nul 2>nul
if not errorlevel 1 goto winget
echo.
echo  winget fehlt auf diesem Computer. Lade Git for Windows bitte von
echo  https://git-scm.com/download/win herunter und installiere es.
echo.
start "" "https://git-scm.com/download/win"
pause
exit /b 1

:winget
echo.
echo  Git for Windows wird installiert ...
echo.
winget install --id Git.Git -e --source winget --accept-package-agreements --accept-source-agreements
if errorlevel 1 goto failed
echo.
echo  Fertig - Agent Office startet neu und findet Git jetzt.
timeout /t 4 >nul
exit /b 0

:failed
echo.
echo  Die Installation hat nicht geklappt. Alternativ: https://git-scm.com/download/win
echo.
pause
exit /b 1
