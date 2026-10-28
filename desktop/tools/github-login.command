#!/bin/bash
# Signs the GitHub CLI (gh) in, for the Agent Office Mac app (menu "GitHub", and on start when gh
# isn't signed in yet): the Mac twin of github-login.cmd. The office lists, clones and opens PRs on
# your repositories with it. The app ships its own gh and puts it on the PATH after any gh you
# installed yourself.
printf '\033]0;%s\007' 'Agent Office - Bei GitHub anmelden'

pause() { echo; read -r -p "  Enter drücken zum Schließen … " _; }

if ! command -v gh >/dev/null 2>&1; then
  echo
  echo "  Die GitHub CLI (gh) wurde nicht gefunden."
  echo "  Installieren mit:  brew install gh"
  pause
  exit 1
fi

echo
echo "  Agent Office braucht die GitHub CLI (gh), um deine Repositorys zu sehen,"
echo "  zu klonen und Issues / Pull Requests anzuzeigen."
echo
echo "  Gleich öffnet sich GitHub im Browser: den hier angezeigten Code eingeben"
echo "  und bestätigen. Danach startet Agent Office neu."
echo
if ! gh auth login --web --hostname github.com --git-protocol https; then
  echo
  echo "  Die Anmeldung hat nicht geklappt. Versuch es noch einmal über"
  echo "  das Menü \"GitHub → Bei GitHub anmelden\" in Agent Office."
  pause
  exit 1
fi
# So that git (clone, fetch, push of worker branches) uses the same sign-in.
gh auth setup-git --hostname github.com
echo
echo "  Fertig - du bist bei GitHub angemeldet. Dieses Fenster kann zu."
