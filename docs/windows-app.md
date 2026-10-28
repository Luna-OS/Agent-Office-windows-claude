# OpenAgenticOffice als Windows-App

Zurück zur [README](../README.md) · [Connector](connector.md) · [Mac-App](mac-app.md) · [Online-Lobby](online-lobby.md)

Die Windows-App ist ein normales Programm mit Installer: kein Terminal, kein Node.js, kein Rust nötig.
Sie ist ein **[Connector](connector.md)**: Sie verbindet dich mit einer **[Online-Lobby](online-lobby.md)**
(einem Office auf deinem Umbrel oder einem Server, wo auch die Arbeiter laufen) und öffnet das Office in
deinem normalen Browser. Dieselbe App gibt es auch [für den Mac](mac-app.md).

Ohne Server startet sie das Office auch auf diesem Computer. Dafür bringt sie den Server, den
Browser-Client, die Agenten-CLI `office-agenten` und die GitHub CLI mit, dazu eine Verknüpfung
**„Claude Code (ohne API-Key)“**, die Claude Code mit deinem **Claude-Abo** (Pro, Max, Team oder
Enterprise) startet statt mit einem Anthropic-API-Key.

## Installieren

1. Lade `OpenAgenticOffice-Setup-<version>.exe` vom neuesten Release (`v0.1.…`, als „Latest“ markiert)
   auf der Releases-Seite des Repositorys (oder als Artefakt des Workflows **Windows app** unter
   *Actions*). Die `.zip`-Datei daneben ist nur der Server für die Kommandozeile, nicht die App.
2. Starte den Installer. Er installiert nur für deinen Windows-Benutzer (keine Admin-Rechte nötig) nach
   `%LOCALAPPDATA%\Programs\OpenAgenticOffice`, den Ordner kannst du ändern. Windows SmartScreen kennt
   die unsignierte App nicht: *Weitere Informationen → Trotzdem ausführen*.
3. Der Installer legt auf dem Desktop und im Startmenü an:
   - **OpenAgenticOffice**: der Connector
   - **Claude Code (ohne API-Key)**: eine Konsole mit Claude Code, angemeldet über dein Claude-Abo

Ohne Installer geht es auch mit dem Skript: `irm https://raw.githubusercontent.com/Luna-OS/OpenAgenticOffice/main/install.ps1 | iex`
legt nur Server, Client und CLI nach `%LOCALAPPDATA%\open-agentic-office\bin` und setzt den PATH
(`open-agentic-office` starten); den Connector gibt es nur über den Installer.

Beim ersten Start zeigt der Connector **🌐 Lobby beitreten** (Adresse eintragen, z. B.
`100.78.107.162:4600`, oder einen Einladungslink einfügen) und **🖥️ Ohne Server** (das Office auf diesem
Computer). Wie es weitergeht: [Connector](connector.md).

Schließt du das Fenster, läuft der Connector im **Infobereich** (unten rechts) weiter, solange er dem
Browser eine Lobby durchleitet oder das Office hier läuft: Klick aufs Symbol holt ihn zurück, Rechtsklick
→ **Beenden** beendet ihn und das Office auf diesem Computer.

Fürs Office auf diesem Computer brauchst du außerdem:

1. **Git for Windows**: für Projekte und Arbeiter-Worktrees, und Claude Code braucht es unter Windows
   selbst. Installieren: `winget install --id Git.Git -e` (oder `desktop\tools\install-git.cmd`).
2. **GitHub-Anmeldung**: `gh auth login` im Browser (`desktop\tools\github-login.cmd` macht es für dich).
   Ohne Anmeldung kann der Aufzug deine Repositorys nicht auflisten.
3. **Claude Code**: mit dem offiziellen Installer (`irm https://claude.ai/install.ps1 | iex`). Die
   Verknüpfung macht das ebenfalls.

Die **GitHub CLI** (`gh`) bringt die App mit (Ordner `werkzeuge`); sie steht beim Start des Servers vorn im
PATH. Hast du `gh` selbst installiert, kommt es dahinter.

## Claude Code ohne API-Key

Claude Code kann sich auf zwei Arten anmelden: mit einem **API-Key** (Abrechnung pro Token über die
Anthropic Console) oder mit deinem **Claude-Konto** (im Abo enthalten). Ist die Umgebungsvariable
`ANTHROPIC_API_KEY` gesetzt, nimmt Claude Code immer den Key – auch wenn du ein Abo hast.

Die App sorgt deshalb dafür, dass nur das Abo benutzt wird:

- **Verknüpfung „Claude Code (ohne API-Key)“** (`claude\claude-code.cmd` im Programmordner): entfernt
  `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL` und die Bedrock/Vertex-Schalter nur für
  dieses Fenster, installiert Claude Code bei Bedarf und startet `claude`. Beim ersten Start wählst du
  **„Claude account with subscription“** und bestätigst im Browser; danach bleibt Claude Code
  angemeldet. Am Rest des Computers ändert sich nichts. Zum Anmelden: `claude-code.cmd auth login --claudeai`.
- **Die Arbeiter im Office** laufen mit derselben Anmeldung: der Connector startet den Server ohne diese
  Variablen (Einstellung `nur_abo`, Standard an, in `%APPDATA%\open-agentic-office-desktop\einstellungen.json`).
  Setz sie auf `false`, wenn du doch einen API-Key nutzen willst.

In einer Online-Lobby gilt das nicht: Dort meldest du dich im Office unter **☰ → 🔑 Anmeldungen**
mit deinem eigenen Claude an, und deine Arbeiter laufen damit auf dem Server.

## Wie die App funktioniert

- Die App (Rust, `desktop/`, [tao](https://github.com/tauri-apps/tao) + [wry](https://github.com/tauri-apps/wry),
  unter Windows mit WebView2, das Windows 11 mitbringt) zeigt den Connector; wie er mit der Lobby arbeitet,
  steht unter [Connector](connector.md).
- Das Office auf diesem Computer startet sie als Kindprozess, genau wie der Befehl `open-agentic-office`,
  nur auf `127.0.0.1` (nur dieser Computer), Port 4600 oder der nächste freie. Kein Passwort nötig.
- Mit *Lobby für andere öffnen* startet es stattdessen auf `0.0.0.0` (alle Netzwerke dieses Computers)
  mit einem Lobby-Passwort, das die App sich merkt und über die Umgebung (`OFFICE_PASSWORT`) übergibt.
- Daten liegen wie bei der normalen Installation in `%USERPROFILE%\open-agentic-office` (änderbar mit
  `daten` in der Einstellungsdatei); App-Einstellungen und gemerkte Lobbys in
  `%APPDATA%\open-agentic-office-desktop`.
- Beim Beenden der App werden das Office auf diesem Computer und seine Arbeiter beendet. Eine
  Online-Lobby läuft weiter.

## Selbst bauen

Der Workflow [`.github/workflows/windows-app.yml`](../.github/workflows/windows-app.yml) baut den Installer
auf einem Windows-Runner. Lokal (Rust, [Trunk](https://trunkrs.dev), [NSIS](https://nsis.sourceforge.io)):

```powershell
cd crates\client; trunk build --release; cd ..\..            # Browser-Client nach dist\
cargo build --release -p server -p agenten                    # Server und Agenten-CLI
cargo build --release --manifest-path desktop\Cargo.toml      # die Hülle (eigenes Paket, eigener Zielordner desktop\target)
```

Dann in `desktop\staging` zusammenstellen: `open-agentic-office-desktop.exe`, `open-agentic-office.exe`,
`office-agenten.exe`, `dist\`, `claude\`, `tools\` und `werkzeuge\gh.exe`, und mit
`makensis /DVERSION=0.1.0 desktop\build\installer.nsi` zu `desktop\release\OpenAgenticOffice-Setup-0.1.0.exe`
packen (genau das tut der Workflow). Zum Ausprobieren ohne Installer reicht
`cargo run --manifest-path desktop\Cargo.toml`; die Hülle findet den Server dann in `target\release`
bzw. `target\debug` und den Client in `dist\`.
