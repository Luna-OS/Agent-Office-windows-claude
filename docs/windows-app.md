# Agent Office als Windows-App

Diese Version von [Agent Office](https://github.com/AgentSystemLabs/agent-office) gibt es als
normale Windows-App mit Installer: kein Terminal, kein `npm`, kein eigenes Node.js nötig. Dazu
kommt eine Verknüpfung **„Claude Code (ohne API-Key)“**, die Claude Code mit deinem
**Claude-Abo** (Pro, Max, Team oder Enterprise) startet statt mit einem Anthropic-API-Key.

## Installieren

1. Lade `Agent-Office-Setup-<version>.exe` von der neuesten Release `windows-v…` auf der
   Releases-Seite dieses Repositorys (oder als Artefakt des Workflows **Windows app** unter
   *Actions*).
2. Starte den Installer. Er installiert nur für deinen Windows-Benutzer (keine Admin-Rechte nötig),
   den Ordner kannst du ändern. Windows SmartScreen kennt die unsignierte App nicht: *Weitere
   Informationen → Trotzdem ausführen*.
3. Der Installer legt auf dem Desktop und im Startmenü an:
   - **Agent Office**: das 3D-Office in einem eigenen Fenster
   - **Claude Code (ohne API-Key)**: eine Konsole mit Claude Code, angemeldet über dein Claude-Abo

Die **GitHub CLI** (`gh`) bringt die App selbst mit: damit listet das Office im Aufzug deine
Repositorys, klont sie und zeigt Issues und Pull Requests. Hast du `gh` selbst installiert, wird
deine Version benutzt.

Beim Start prüft die App, was noch fehlt, und bietet es nacheinander an (jeweils mit „Später“
überspringbar):

1. **Git for Windows**: für Projekte und Worker-Worktrees, und Claude Code braucht es unter Windows
   auch selbst. Installiert per `winget install --id Git.Git -e`.
2. **GitHub-Anmeldung**: `gh auth login` im Browser (Code eingeben, bestätigen). Ohne Anmeldung
   meldet der Aufzug „Couldn't list your repositories with gh“.
3. **Claude Code**: mit dem offiziellen Installer (`irm https://claude.ai/install.ps1 | iex`). Die
   Verknüpfung macht das ebenfalls.

Nach einer Installation oder Anmeldung startet das Office von selbst neu und findet das neue
Programm, ohne dass du dich bei Windows neu anmelden musst. Dasselbe geht jederzeit über das Menü
**GitHub → Bei GitHub anmelden …** bzw. **Git for Windows installieren …**.

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
  Variablen, also mit dem Abo-Login von `claude`. Abschaltbar unter *Claude Code → Worker nur mit
  Claude-Abo (API-Key ignorieren)*, falls du doch einen API-Key nutzen willst.
- **Menü „Claude Code“** in der App:
  - *Claude Code öffnen (ohne API-Key)* (Strg+Umschalt+C): dieselbe Konsole wie die Verknüpfung
  - *Mit Claude-Abo anmelden …*: `claude auth login --claudeai`
  - *Anmeldestatus prüfen*: `claude auth status`

Zeigt ein Worker im Office „Claude isn't signed in on this machine“, melde dich einmal über die
Verknüpfung oder *Mit Claude-Abo anmelden …* an – danach starten alle Worker mit dem Abo.

## Wie die App funktioniert

- Die App (Electron, `desktop/main.cjs`) startet das Office mit dem mitgelieferten `node.exe`
  genau wie der Befehl `agent-office` – nur auf `127.0.0.1` (nur dieser Computer), Port 4600 oder
  der nächste freie – und meldet ihr Fenster über einen Einmal-Link an. Kein Passwort nötig.
- Daten liegen wie bei der normalen Installation in `%USERPROFILE%\agent-office` (änderbar unter
  *Agent Office → Daten-Ordner ändern …*); App-Einstellungen in `%APPDATA%\agent-office-desktop`.
- *Agent Office → Im Browser öffnen* öffnet das Office mit einem frischen Anmelde-Link im normalen
  Browser. Links nach außen (GitHub, PRs) öffnen sich immer im Browser.
- Beim Beenden der App werden das Office und seine Worker beendet.

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
- `npm run icons` zeichnet die Icons (`build/icon.ico`, `claude/claude-code.ico`) neu.
