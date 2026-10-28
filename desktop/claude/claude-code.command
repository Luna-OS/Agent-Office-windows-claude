#!/bin/bash
# Claude Code with your Claude subscription (Pro / Max / Team / Enterprise) instead of an API key.
#
# The Mac twin of claude-code.cmd, behind the OpenAgenticOffice app's menu "Claude Code → Claude Code
# öffnen (ohne API-Key)" (⌘⇧C). A set ANTHROPIC_API_KEY (or ANTHROPIC_AUTH_TOKEN) takes precedence
# over the subscription login in Claude Code and is billed per token, so it is removed here, for
# this window only. Nothing else on the Mac changes. Any arguments are passed on to claude, e.g.
#   claude-code.command auth login --claudeai
printf '\033]0;%s\007' 'Claude Code (Abo-Login, ohne API-Key)'
unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN ANTHROPIC_BASE_URL CLAUDE_CODE_USE_BEDROCK CLAUDE_CODE_USE_VERTEX

# Where Claude Code's own installer puts it, and Homebrew, in case this PATH is older than the install.
export PATH="$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

pause() { echo; read -r -p "  Enter drücken zum Schließen … " _; }

if ! command -v claude >/dev/null 2>&1; then
  echo
  echo "  Claude Code ist noch nicht installiert - es wird jetzt installiert"
  echo "  (offizieller Installer: curl -fsSL https://claude.ai/install.sh | bash) ..."
  echo
  curl -fsSL https://claude.ai/install.sh | bash
  hash -r
  if ! command -v claude >/dev/null 2>&1; then
    echo
    echo "  Claude Code konnte nicht installiert werden. Anleitung:"
    echo "  https://code.claude.com/docs/en/setup"
    pause
    exit 1
  fi
fi

if ! xcode-select -p >/dev/null 2>&1 && ! command -v brew >/dev/null 2>&1; then
  echo
  echo "  Hinweis: OpenAgenticOffice und Claude Code brauchen Git. Installieren mit:"
  echo "    xcode-select --install"
fi
echo
echo "  Claude Code mit deinem Claude-Abo - kein API-Key nötig."
echo "  Noch nicht angemeldet? Beim ersten Start \"Claude account with subscription\""
echo "  wählen (oder /login tippen) und im Browser bestätigen."
echo
claude "$@" || pause
