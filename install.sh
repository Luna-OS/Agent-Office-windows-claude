#!/usr/bin/env bash
# OpenAgenticOffice installieren (macOS und Linux) und starten.
#
#   curl -fsSL https://raw.githubusercontent.com/Luna-OS/OpenAgenticOffice/main/install.sh | bash
#
# Es wird kein Node.js gebraucht: das Archiv enthält den fertigen Server (`open-agentic-office`),
# den gebauten Browser-Client (`dist/`) und die Agenten-CLI (`office-agenten`).
#
# Einstellungen (Umgebungsvariablen):
#   OFFICE_VERSION       eine bestimmte Version installieren, z. B. 0.1.12 (Standard: die neueste)
#   OFFICE_INSTALL_ONLY  auf 1 setzen: nur installieren, das Office nicht starten
#   OFFICE_INSTALL_DIR   Zielordner (Standard: ~/.open-agentic-office; die Programme liegen in bin/)
#   OFFICE_ARCHIV        ein lokales .tar.gz statt eines Downloads verwenden (für Tests)
#   OFFICE_REPO          GitHub-Repository (Standard: Luna-OS/OpenAgenticOffice)
#
# Alles nach dem Start (Port, Passwort, Ordner) steht in docs/configuration.md.

set -euo pipefail

REPO="${OFFICE_REPO:-Luna-OS/OpenAgenticOffice}"
ZIEL="${OFFICE_INSTALL_DIR:-$HOME/.open-agentic-office}"
BIN="$ZIEL/bin"

fehler() { echo "Fehler: $*" >&2; exit 1; }
info() { echo "==> $*"; }

# Plattform bestimmen
case "$(uname -s)" in
  Linux) os=linux ;;
  Darwin) os=macos ;;
  *) fehler "Dieses System wird nicht unterstützt. Unter Windows: install.ps1 benutzen." ;;
esac
case "$(uname -m)" in
  x86_64|amd64) arch=x64 ;;
  arm64|aarch64) arch=arm64 ;;
  *) fehler "Diese Prozessor-Architektur wird nicht unterstützt: $(uname -m)" ;;
esac
asset="open-agentic-office-$os-$arch.tar.gz"

command -v curl >/dev/null 2>&1 || fehler "curl fehlt."
command -v tar >/dev/null 2>&1 || fehler "tar fehlt."

# Archiv holen
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
if [ -n "${OFFICE_ARCHIV:-}" ]; then
  info "Verwende das lokale Archiv $OFFICE_ARCHIV"
  cp "$OFFICE_ARCHIV" "$tmp/archiv.tar.gz"
else
  if [ -n "${OFFICE_VERSION:-}" ]; then
    url="https://github.com/$REPO/releases/download/v${OFFICE_VERSION#v}/$asset"
  else
    url="https://github.com/$REPO/releases/latest/download/$asset"
  fi
  info "Lade $url"
  curl -fSL --progress-bar -o "$tmp/archiv.tar.gz" "$url" || fehler "Der Download ist fehlgeschlagen (gibt es diese Version für $os-$arch?)."
fi

# Installieren (ersetzt eine frühere Installation, die Daten in ~/open-agentic-office bleiben unberührt)
info "Installiere nach $BIN"
mkdir -p "$tmp/auspacken" "$ZIEL"
tar -xzf "$tmp/archiv.tar.gz" -C "$tmp/auspacken"
quelle="$tmp/auspacken"
[ -f "$quelle/open-agentic-office" ] || quelle="$(find "$tmp/auspacken" -maxdepth 2 -name open-agentic-office -type f -exec dirname {} \; | head -n 1)"
[ -n "$quelle" ] && [ -f "$quelle/open-agentic-office" ] || fehler "Das Archiv enthält keinen Server (open-agentic-office)."
rm -rf "$BIN"
mkdir -p "$BIN"
cp -R "$quelle"/. "$BIN"/
chmod +x "$BIN/open-agentic-office" 2>/dev/null || true
for b in office-agenten office-pty-wirt; do [ -f "$BIN/$b" ] && chmod +x "$BIN/$b"; done
# macOS: heruntergeladene Programme tragen die Quarantäne-Markierung
[ "$os" = macos ] && xattr -dr com.apple.quarantine "$BIN" 2>/dev/null || true

# PATH
case ":$PATH:" in
  *":$BIN:"*) ;;
  *)
    zeile="export PATH=\"$BIN:\$PATH\"  # OpenAgenticOffice"
    for rc in "$HOME/.zshrc" "$HOME/.bashrc" "$HOME/.profile"; do
      if [ -f "$rc" ] && ! grep -q "# OpenAgenticOffice" "$rc"; then
        echo "$zeile" >> "$rc"
        info "PATH in $rc ergänzt (neues Terminal öffnen, damit er gilt)"
      fi
    done
    export PATH="$BIN:$PATH"
    ;;
esac

# Voraussetzungen prüfen (nur Hinweise)
command -v git >/dev/null 2>&1 || echo "Hinweis: git fehlt. Das Office braucht es für Projekte und Worktrees (macOS: xcode-select --install)."
if command -v gh >/dev/null 2>&1; then
  gh auth status >/dev/null 2>&1 || echo "Hinweis: die GitHub CLI ist nicht angemeldet. Mit 'gh auth login' kommst du an deine Repositorys."
else
  echo "Hinweis: die GitHub CLI (gh) fehlt: https://cli.github.com. Ohne sie gibt es keine Repository-Liste, Issues und Pull Requests."
fi
command -v claude >/dev/null 2>&1 || echo "Hinweis: kein Agent-CLI gefunden (z. B. Claude Code: curl -fsSL https://claude.ai/install.sh | bash)."

info "Fertig. Starten: open-agentic-office"
if [ "${OFFICE_INSTALL_ONLY:-}" = 1 ]; then
  exit 0
fi
exec "$BIN/open-agentic-office"
