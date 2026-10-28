@echo off
rem Signs the GitHub CLI (gh) in, for the Agent Office Windows app (menu "GitHub", and on start when
rem gh isn't signed in yet). The office lists, clones and opens PRs on your repositories with it.
rem The app ships its own gh.exe and puts it on the PATH after any gh you installed yourself.
setlocal
title Agent Office - Bei GitHub anmelden
where gh >nul 2>nul
if not errorlevel 1 goto found
echo.
echo  Die GitHub CLI (gh) wurde nicht gefunden.
echo  Installieren mit:  winget install --id GitHub.cli -e
echo.
pause
exit /b 1

:found
echo.
echo  Agent Office braucht die GitHub CLI (gh), um deine Repositorys zu sehen,
echo  zu klonen und Issues / Pull Requests anzuzeigen.
echo.
echo  Gleich oeffnet sich GitHub im Browser: den hier angezeigten Code eingeben
echo  und bestaetigen. Danach startet Agent Office neu.
echo.
gh auth login --web --hostname github.com --git-protocol https
if errorlevel 1 goto failed
rem So that git (clone, fetch, push of worker branches) uses the same sign-in.
gh auth setup-git --hostname github.com
echo.
echo  Fertig - du bist bei GitHub angemeldet.
timeout /t 4 >nul
exit /b 0

:failed
echo.
echo  Die Anmeldung hat nicht geklappt. Versuch es noch einmal ueber
echo  das Menue "GitHub - Bei GitHub anmelden" in Agent Office.
echo.
pause
exit /b 1
