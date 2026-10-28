@echo off
rem Claude Code with your Claude subscription (Pro / Max / Team / Enterprise) instead of an API key.
rem
rem Installed by the Agent Office Windows app as the shortcut "Claude Code (ohne API-Key)".
rem A set ANTHROPIC_API_KEY (or ANTHROPIC_AUTH_TOKEN) takes precedence over the subscription login
rem in Claude Code and is billed per token, so it is removed here, for this window only. Nothing
rem else on the computer changes. Any arguments are passed on to claude, e.g.
rem   claude-code.cmd auth login --claudeai
setlocal
title Claude Code (Abo-Login, ohne API-Key)
set "ANTHROPIC_API_KEY="
set "ANTHROPIC_AUTH_TOKEN="
set "ANTHROPIC_BASE_URL="
set "CLAUDE_CODE_USE_BEDROCK="
set "CLAUDE_CODE_USE_VERTEX="

rem Where Claude Code's own installer puts it, in case this PATH is older than the install.
if exist "%USERPROFILE%\.local\bin\claude.exe" set "PATH=%USERPROFILE%\.local\bin;%PATH%"

where claude >nul 2>nul
if not errorlevel 1 goto installed
echo.
echo  Claude Code ist noch nicht installiert - es wird jetzt installiert
echo  (offizieller Installer: irm https://claude.ai/install.ps1 ^| iex) ...
echo.
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://claude.ai/install.ps1 | iex"
if exist "%USERPROFILE%\.local\bin\claude.exe" set "PATH=%USERPROFILE%\.local\bin;%PATH%"
where claude >nul 2>nul
if not errorlevel 1 goto installed
echo.
echo  Claude Code konnte nicht installiert werden. Anleitung:
echo  https://code.claude.com/docs/en/setup
echo.
pause
exit /b 1

:installed
where git >nul 2>nul
if errorlevel 1 (
  echo.
  echo  Hinweis: Claude Code braucht unter Windows "Git for Windows".
  echo  Installieren mit:  winget install --id Git.Git -e
)
echo.
echo  Claude Code mit deinem Claude-Abo - kein API-Key noetig.
echo  Noch nicht angemeldet? Beim ersten Start "Claude account with subscription"
echo  waehlen (oder /login tippen) und im Browser bestaetigen.
echo.
claude %*
if errorlevel 1 pause
endlocal
