# Der Connector: eine App für Windows und Mac, das Office im Browser

Zurück zur [README](../README.md) · [Online-Lobby](online-lobby.md) · [Windows-App](windows-app.md) · [Mac-App](mac-app.md)

Die App für Windows und macOS ist ein **Connector**: ein kleines Fenster, das dich mit einer Lobby
verbindet, dich dort anmeldet und das Office in deinem **normalen Browser** öffnet. Die Lobby läuft
auf deinem [Umbrel](online-lobby.md) (oder einem anderen Server), und dort laufen auch alle Worker.
Dein Computer zeigt nur das Office.

Das Wichtigste: **Jeder Spieler meldet sich im Connector mit seinem eigenen Claude und GitHub an.**
Deine Worker laufen dann mit deinem Claude-Abo, und auf GitHub erscheinen Kommentare, Commits und
Pull Requests unter deinem Namen, auch wenn alles auf dem Umbrel läuft.

## So geht's

1. **Connector installieren**: unter Windows `Agent-Office-Setup-<version>.exe`
   ([Windows-App](windows-app.md)), auf dem Mac das Disk-Image ([Mac-App](mac-app.md)).
2. **Lobby beitreten**: die Adresse der Lobby eintragen, z. B. `100.78.107.162:4600` (die Adresse deines
   Umbrel im Tailnet oder im NetBird-Netz) oder `umbrel.local:4600` im Heimnetz, und **Verbinden**. Oder
   gleich den **Einladungslink** einfügen, den du bekommen hast (`http://…:4600/join#…`).
3. **Anmelden**:
   - Mit einem Einladungslink wählst du deinen Namen und ein Passwort: Das ist dein Konto in der Lobby.
   - Später meldest du dich mit Name und Passwort an. Der Connector merkt sich die Anmeldung (unter
     Windows und macOS verschlüsselt mit dem Schlüsselbund des Systems) und ist beim nächsten Start
     gleich wieder drin.
   - Wer die Lobby aufgesetzt hat, kommt beim ersten Mal mit dem **Office-Passwort** herein (Name leer
     lassen; bei der Umbrel-App steht es auf ihrer Seite) und legt dann mit **👤 Dein eigenes Konto**
     sein eigenes Konto an, als Admin.
4. **🔐 Deine Anmeldungen**:
   - **✳️ Claude → Mit Claude anmelden**: Claudes Anmeldeseite geht in deinem Browser auf. Melde dich
     mit deinem eigenen Claude-Konto an, kopier den Code, den sie dir zeigt, und füge ihn im Connector
     ein. Oder füge einen Token aus `claude setup-token` ein.
   - **🐙 GitHub → Mit GitHub anmelden**: Der Connector zeigt einen Einmal-Code. **Code kopieren und
     GitHub öffnen**, dort einfügen, bestätigen; der Connector merkt es von selbst. Oder füge einen
     GitHub-Token ein.
   - Teilt der Host sein Claude (im Office unter ☰ → 🔑 Accounts), gibt es **Claude des Hosts
     nutzen**. Du kannst jederzeit zwischen deinem und seinem wechseln.
5. **🌐 Office im Browser öffnen**: Dein Browser öffnet das Office, schon angemeldet als du.

Als Admin machst du unter **👥 Mitspieler einladen** einen Einladungslink (einmal gültig, 7 Tage) und
schickst ihn weiter. Wer ihn bekommt, fügt ihn im eigenen Connector unter **Lobby beitreten** ein.

## Sprache und Bildschirm teilen im Browser

Browser erlauben Mikrofon und Bildschirm nur auf `https://`-Seiten und auf Adressen des eigenen
Computers. Eine Lobby über `http://` auf einem anderen Gerät (dein Umbrel) ist keins von beiden.
Deshalb reicht der Connector so eine Lobby über `http://127.0.0.1:48600` an deinen Browser weiter
(`desktop/passthrough.cjs`): Für den Browser ist das dein eigener Computer, also gehen Sprache und
Bildschirm teilen. **Lass den Connector dafür laufen.** Unter Windows läuft er nach dem Schließen des
Fensters im Infobereich weiter (Rechtsklick auf das Symbol: Connector anzeigen, Office öffnen,
Beenden), auf dem Mac im Dock.

Eine Lobby über `https://` öffnet der Browser direkt, ohne den Umweg.

## Ohne Server: das Office auf diesem Computer

Unter **Ohne Server → Auf diesem Computer starten** läuft das Office wie früher auf deinem Computer
(mit dem Node.js, Claude Code und der GitHub CLI, die die App mitbringt), und der Connector meldet
dich dort an. Die Anmeldungen des Office sind dann die deines Computers: das Menü **Dieser Computer**
(oder die Knöpfe im Connector) meldet Claude Code und die GitHub CLI an. Andere lässt du über **Dieser
Computer → Lobby auf diesem Computer für andere öffnen** herein ([so geht's](online-lobby.md#ohne-umbrel-die-lobby-auf-deinem-pc-hosten)).
Das kostet Rechenleistung; soll dein PC geschont werden, nimm den Umbrel.

## Wenn etwas nicht geht

- **„… ist nicht erreichbar“**: Läuft die Umbrel-App? Ist dein Computer im selben Netz (Heimnetz,
  Tailscale oder NetBird)? Teste im Terminal: `curl http://100.78.107.162:4600/login.html`. Bei NetBird
  hilft es manchmal, die Verbindung über das Relay zu zwingen (`NB_FORCE_RELAY=true` für den
  NetBird-Dienst), wenn die direkte Verbindung zwischen zwei Geräten hängt.
- **GitHub- oder Claude-Anmeldung zeigt einen Fehler**: Sie läuft auf dem Lobby-Server; der braucht
  Internet. Der Fehler steht in der Karte. Ein Token geht immer.
- **Der Browser zeigt „Die Lobby antwortet nicht“**: Der Connector reicht weiter, aber die Lobby ist weg.
  Im Connector **‹ Lobbys** und neu verbinden.

## Wie der Connector funktioniert

- `desktop/controller.cjs`: was das Fenster zeigt und tut (Lobby, Anmelden, Anmeldungen, Einladungen,
  Browser). `desktop/ui/` zeichnet es, `desktop/preload.cjs` ist die einzige Verbindung dorthin.
- `desktop/connector.cjs` spricht mit der Lobby, über ihre normalen Anmelde-Routen und die Routen in
  `src/server/http/routes/connector.ts`: `/api/signins` (deine Claude- und GitHub-Anmeldungen, wie
  🔐 Your sign-ins im Office), `/api/accounts/invite` (Einladungen, für Admins) und `/api/link/new`.
  Er ist dabei kein Spieler: Im Office läuft keine zusätzliche Figur herum.
- **Office im Browser öffnen** holt mit `/api/link/new` einen Einmal-Link für dieselbe Anmeldung (nur
  einmal gültig, nur 5 Minuten, nur der Hash bleibt im Speicher des Office) und öffnet
  `…/login#key=…` im Browser. Der Schlüssel steht hinter dem `#` und landet in keinem Server-Log.
- Alle Anfragen des Connectors tragen die Adresse der Lobby als `Origin`, wie die Seite des Office
  selbst; andere Websites können diese Routen mit deinem Cookie nicht benutzen.
- `desktop/passthrough.cjs` leitet jede Anfrage und jeden WebSocket unverändert an die Lobby weiter
  (auch den `Host`), so sieht das Office dieselbe Adresse wie der Browser. Nur
  `/.well-known/agent-office-connector.json` beantwortet er selbst: die echte Adresse der Lobby, damit
  Einladungslinks aus 🔑 Accounts im Browser auf die Lobby zeigen und nicht auf `127.0.0.1`.
