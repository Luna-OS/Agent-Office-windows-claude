# Agent Office als Windows-App

Zurück zur [README](../README.md) · [Connector](connector.md) · [Mac-App](mac-app.md) · [Online-Lobby](online-lobby.md)

Diese Version von [Agent Office](https://github.com/AgentSystemLabs/agent-office) gibt es als
normale Windows-App mit Installer: kein Terminal, kein `npm`, kein eigenes Node.js nötig. Die App ist
ein **[Connector](connector.md)**: Sie verbindet dich mit einer **[Online-Lobby](online-lobby.md)**
(einem Office auf deinem Umbrel oder einem Server, wo auch die Worker laufen), meldet dich dort mit
deinem **eigenen Claude und GitHub** an und öffnet das Office in deinem normalen Browser. Dieselbe App
gibt es auch [für den Mac](mac-app.md).

Ohne Server startet sie das Office auch auf diesem Computer. Dafür bringt sie Node.js und die GitHub
CLI mit, und eine Verknüpfung **„Claude Code (ohne API-Key)“**, die Claude Code mit deinem
**Claude-Abo** (Pro, Max, Team oder Enterprise) startet statt mit einem Anthropic-API-Key.

## Installieren

1. Lade `Agent-Office-Setup-<version>.exe` von der neuesten Release (`v…`, „Agent Office <version>“) auf der
   Releases-Seite dieses Repositorys (oder als Artefakt des Workflows **Windows app** unter
   *Actions*).
2. Starte den Installer. Er installiert nur für deinen Windows-Benutzer (keine Admin-Rechte nötig),
   den Ordner kannst du ändern. Windows SmartScreen kennt die unsignierte App nicht: *Weitere
   Informationen → Trotzdem ausführen*.
3. Der Installer legt auf dem Desktop und im Startmenü an:
   - **Agent Office**: der Connector
   - **Claude Code (ohne API-Key)**: eine Konsole mit Claude Code, angemeldet über dein Claude-Abo

Beim ersten Start zeigt der Connector **🌐 Lobby beitreten** (Adresse eintragen, z. B.
`100.78.107.162:4600`, oder einen Einladungslink einfügen) und **🖥️ Ohne Server** (das Office auf
diesem Computer). Wie es weitergeht: [Connector](connector.md). Der Connector merkt sich, wo du zuletzt
warst; wer die App schon vor dem Connector benutzt hat, landet wie bisher im Office auf diesem Computer.

Schließt du das Fenster, während der Browser das Office über den Connector bekommt, läuft er im
**Infobereich** (unten rechts) weiter: Klick aufs Symbol holt ihn zurück, Rechtsklick → **Beenden**
beendet ihn.

Im Office auf diesem Computer prüft die App, was noch fehlt, und bietet es nacheinander an (jeweils
mit „Später“ überspringbar; für eine Online-Lobby fragt sie nicht, dort läuft alles auf dem Server):

1. **Git for Windows**: für Projekte und Worker-Worktrees, und Claude Code braucht es unter Windows
   auch selbst. Installiert per `winget install --id Git.Git -e`.
2. **GitHub-Anmeldung**: `gh auth login` im Browser (Code eingeben, bestätigen). Ohne Anmeldung
   meldet der Aufzug „Couldn't list your repositories with gh“.
3. **Claude Code**: mit dem offiziellen Installer (`irm https://claude.ai/install.ps1 | iex`). Die
   Verknüpfung macht das ebenfalls.

Die **GitHub CLI** (`gh`) bringt die App selbst mit: damit listet das Office im Aufzug deine
Repositorys, klont sie und zeigt Issues und Pull Requests. Hast du `gh` selbst installiert, wird
deine Version benutzt. Nach einer Installation oder Anmeldung startet das Office von selbst neu und
findet das neue Programm, ohne dass du dich bei Windows neu anmelden musst. Dasselbe geht jederzeit
über das Menü **Dieser Computer → GitHub CLI anmelden …** bzw. **Git for Windows installieren …**.

## Claude Code ohne API-Key

Claude Code kann sich auf zwei Arten anmelden: mit einem **API-Key** (Abrechnung pro Token über die
Anthropic Console) oder mit deinem **Claude-Konto** (im Abo enthalten). Ist die Umgebungsvariable
`ANTHROPIC_API_KEY` gesetzt, nimmt Claude Code immer den Key – auch wenn du ein Abo hast.

Die App sorgt deshalb dafür, dass nur das Abo benutzt wird:

- **Verknüpfung „Claude Code (ohne API-Key)“** (`resources\claude\claude-code.cmd`): entfernt
  `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL` und die Bedrock/Vertex-Schalter
  nur für dieses Fenster, installiert Claude Code bei Bedarf und startet `claude`. Beim ersten Start
  wählst du **„Claude account with subscription“** und bestätigst im Browser; danach bleibt Claude
  Code angemeldet. Am Rest des Computers ändert sich nichts.
- **Die Worker im Office** laufen mit derselben Anmeldung: die App startet das Office ohne diese
  Variablen, also mit dem Abo-Login von `claude`. Abschaltbar unter *Dieser Computer → Worker nur mit
  Claude-Abo (API-Key ignorieren)*, falls du doch einen API-Key nutzen willst.
- **Menü „Dieser Computer“** in der App (Alt zeigt die Menüleiste):
  - *Claude Code öffnen (ohne API-Key)* (Strg+Umschalt+C): dieselbe Konsole wie die Verknüpfung
  - *Claude Code mit Claude-Abo anmelden …*: `claude auth login --claudeai`
  - *Anmeldestatus prüfen*: `claude auth status`

In einer Online-Lobby gilt das nicht: Dort meldest du dich im Connector unter **🔐 Deine Anmeldungen**
mit deinem eigenen Claude an, und deine Worker laufen damit auf dem Server.

Zeigt ein Worker im Office auf diesem Computer „Claude isn't signed in on this machine“, melde dich
einmal über die Verknüpfung oder *Claude Code mit Claude-Abo anmelden …* an – danach starten alle
Worker mit dem Abo.

## Wie die App funktioniert

- Die App (Electron, `desktop/main.cjs`) zeigt den Connector (`desktop/controller.cjs`, `desktop/ui/`);
  wie er mit der Lobby spricht und das Office im Browser öffnet, steht unter [Connector](connector.md).
- Das Office auf diesem Computer startet sie mit dem mitgelieferten `node.exe` genau wie der Befehl
  `agent-office`, nur auf `127.0.0.1` (nur dieser Computer), Port 4600 oder der nächste freie, und
  meldet den Connector über einen Einmal-Link an. Kein Passwort nötig.
- Mit *Lobby auf diesem Computer für andere öffnen* (`desktop/hosting.cjs`) startet es stattdessen auf
  `0.0.0.0` (alle Netzwerke dieses Computers) mit einem Lobby-Passwort, das die App sich merkt und über
  die Umgebung (nicht die Kommandozeile) übergibt. Das eigene Passwort des Office bleibt dabei
  unverändert, beim Schließen der Lobby gilt wieder alles wie vorher.
- Daten liegen wie bei der normalen Installation in `%USERPROFILE%\agent-office` (änderbar unter
  *Dieser Computer → Daten-Ordner ändern …*); App-Einstellungen, die gemerkten Lobbys und deine
  Anmeldungen dort (mit Windows' Datenschutz-API verschlüsselt) in `%APPDATA%\agent-office-desktop`.
- Beim Beenden der App werden das Office auf diesem Computer und seine Worker beendet. Eine
  Online-Lobby läuft weiter.
- Die Grafik stellst du im Office unter ⚙️ Settings → **You → Graphics** ein: **⚖️ Balanced** ist
  der Standard, **🍃 Eco** schont schwächere PCs und Laptops.

## Selbst bauen

Der Workflow [`.github/workflows/windows-app.yml`](../.github/workflows/windows-app.yml) baut den
Installer auf einem Windows-Runner. Lokal unter Windows (Node.js 22):

```powershell
npm ci                 # baut das Office (dist/)
cd desktop
npm ci
npm run dist           # → desktop\release\Agent-Office-Setup-<version>.exe
```

- `npm run stage` stellt `desktop\staging` zusammen: das gebaute Office mit seinen
  Produktions-Abhängigkeiten, ein `node.exe` (geprüft gegen die SHA256-Summen von nodejs.org), die
  GitHub CLI (`gh.exe` der neuesten Version oder `GH_VERSION`, geprüft gegen ihre Checksummen),
  die Claude-Code-Verknüpfung und die GitHub-Hilfsskripte (`desktop\tools`). Ein `afterPack`-Hook kopiert das in den `resources`-Ordner der App.
- `npm start` startet die App aus dem Checkout (mit dem `node` aus dem PATH und dem `dist/` im
  Repository-Root).
- `npm run icons` zeichnet die Icons (`build/icon.ico`, `ui/tray.png`, `claude/claude-code.ico`) neu.
