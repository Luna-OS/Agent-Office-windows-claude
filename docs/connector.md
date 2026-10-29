# Der Connector: eine App für Windows und Mac, das Office im Browser

Zurück zur [README](../README.md) · [Server](vermittler.md) · [Zusammen spielen](online-lobby.md) · [Windows-App](windows-app.md) · [Mac-App](mac-app.md)

Die App für Windows und macOS ist ein **Connector**: ein kleines Fenster, das dich mit eurem Server
verbindet (der Umbrel-App [Agent Office Vermittler](vermittler.md)), das Office in deinem **normalen
Browser** öffnet und die Arbeiter, die du einstellst, auf deinem Computer laufen lässt. Das Office selbst
läuft auf dem Server. Lobbys, ein Office auf diesem Computer und Einladungslinks gibt es nicht: Der
Connector spricht nur mit dem Server, mit seiner Adresse und seinem Passwort.

## So geht's

1. **Connector installieren**: unter Windows `Agent-Office-Setup-<version>.exe`
   ([Windows-App](windows-app.md)), auf dem Mac das Disk-Image ([Mac-App](mac-app.md)).
2. **🛰️ Server**: die Adresse des Servers (z. B. `100.78.107.162:4600`, die Adresse deines Umbrel im
   Tailnet oder im NetBird-Netz, oder `umbrel.local:4600` im Heimnetz), das **Passwort des Servers** (bei
   der Umbrel-App steht es auf ihrer Seite im App Store) und **deinen Namen** eintragen, dann
   **Verbinden**. Beim ersten Mal legt der Server dein Konto unter diesem Namen an; mit demselben Namen
   bist du danach immer du, auch von einem anderen Computer.
3. Der Connector merkt sich Adresse, Name und Passwort (das Passwort und deine Sitzung verschlüsselt mit
   dem Schlüsselbund des Systems) und verbindet sich beim nächsten Start von selbst.
4. **🖥️ Deine Arbeiter auf diesem Computer**: Die App startet im Hintergrund deinen
   [Arbeitsplatz](vermittler.md#dein-arbeitsplatz-deine-arbeiter-auf-deinem-pc). Die Karten zeigen die
   Anmeldungen dieses Computers, mit denen deine Arbeiter laufen: **✳️ Claude → Mit Claude anmelden**
   öffnet ein Konsolenfenster (auf dem Mac ein Terminal) für dein Claude-Abo, **🐙 GitHub → Mit GitHub
   anmelden** das für die GitHub CLI. Sobald es geklappt hat, zeigt die Karte von selbst ✅ mit deinem Konto.
5. **🔐 Deine Anmeldungen auf dem Server**: Mit deinem **GitHub** dort handelt das Office auf GitHub als
   du (Kommentare, Pull Requests) und klont die Repos, die du als Etage hinzufügst. **Mit GitHub
   anmelden** zeigt einen Einmal-Code (**Code kopieren und GitHub öffnen**, dort einfügen, bestätigen),
   oder du fügst einen Token ein. Ist dein Computer schon angemeldet: **@dein-name von diesem Computer
   übernehmen**. Lässt du deine Arbeiter auf dem Server laufen (**Lieber auf dem Server laufen lassen**),
   gibt es hier auch die Karte für **Claude**.
6. **🌐 Office im Browser öffnen**: Dein Browser öffnet das Office, schon angemeldet als du.

Unten im Fenster: **Einstellungen** (Adresse, Passwort, Name ändern; das Passwortfeld leer lassen, um das
gespeicherte zu behalten) und **Verbindung trennen** (deine Arbeiter hier stoppen und im Office Pause
machen; beim nächsten Start verbindet sich der Connector dann nicht von selbst).

## In der Menüleiste

Schließt du das Fenster (auf dem Mac auch beim Minimieren), läuft der Connector weiter: auf dem Mac **oben
in der Menüleiste** (das Bürohaus-Symbol, ohne Symbol im Dock), unter Windows **unten im Infobereich**.
Von dort:

- **Office im Browser öffnen**
- **Connector öffnen**
- **Einstellungen …**
- **Verbindung trennen** (bzw. **Verbinden**, wenn getrennt)
- **Agent Office beenden**

Oben im Menü steht, ob und mit wem du verbunden bist. Erst **Beenden** stoppt die App und mit ihr deine
Arbeiter auf diesem Computer (sie machen im Office Pause, bis du wieder da bist).

## Sprache und Bildschirm teilen im Browser

Browser erlauben Mikrofon und Bildschirm nur auf `https://`-Seiten und auf Adressen des eigenen
Computers. Ein Server über `http://` auf einem anderen Gerät (dein Umbrel) ist keins von beiden.
Deshalb reicht der Connector so einen Server über `http://127.0.0.1:48600` an deinen Browser weiter
(`desktop/passthrough.cjs`): Für den Browser ist das dein eigener Computer, also gehen Sprache und
Bildschirm teilen. **Lass den Connector dafür laufen** (in der Menüleiste reicht).

Einen Server über `https://` öffnet der Browser direkt, ohne den Umweg.

## Wenn etwas nicht geht

- **„… ist nicht erreichbar“**: Läuft die Umbrel-App? Ist dein Computer im selben Netz (Heimnetz,
  Tailscale oder NetBird)? Teste im Terminal: `curl http://100.78.107.162:4600/login.html`. Bei NetBird
  hilft es manchmal, die Verbindung über das Relay zu zwingen (`NB_FORCE_RELAY=true` für den
  NetBird-Dienst), wenn die direkte Verbindung zwischen zwei Geräten hängt.
- **„Das Passwort des Servers stimmt nicht.“**: Das Passwort steht bei der Umbrel-App auf ihrer Seite im
  App Store.
- **„… antwortet mit HTTP 503“** (oder ein anderer Fehler): Auf dem Umbrel läuft vermutlich noch der alte
  Vermittler (bis 0.1.34), der nur durchreichte. Aktualisiere die App im App Store
  ([umsteigen](vermittler.md#vom-alten-vermittler-0134-umsteigen)).
- **GitHub- oder Claude-Anmeldung zeigt einen Fehler**: Die Anmeldung auf dem Server läuft dort; der
  braucht Internet. Der Fehler steht in der Karte. Ein Token geht immer.
- **Der Browser zeigt „Der Server antwortet nicht“**: Der Connector reicht weiter, aber der Server ist weg.
  Im Connector **Einstellungen** und neu verbinden.

## Wie der Connector funktioniert

- `desktop/controller.cjs`: was das Fenster zeigt und tut (Server, Verbinden, Anmeldungen, Arbeitsplatz,
  Browser). `desktop/ui/` zeichnet es, `desktop/preload.cjs` ist die einzige Verbindung dorthin.
  `desktop/main.cjs` baut das Fenster, die Menüs und das Symbol in der Menüleiste und startet den Arbeitsplatz.
- `desktop/local-signins.cjs` liest die Anmeldungen dieses Computers (`claude auth status`, `gh api user`)
  für deine Arbeiter hier; `gh auth token` gibt GitHubs Anmeldung an den Server weiter, wenn du es willst.
- `desktop/connector.cjs` spricht mit dem Server: `/api/login` mit Name und Passwort (dein Konto, beim
  ersten Mal angelegt), `/api/whoami`, und die Routen in `src/server/http/routes/connector.ts`:
  `/api/signins` (deine Claude- und GitHub-Anmeldungen, wie 🔐 Your sign-ins im Office) und `/api/link/new`.
  Er ist dabei kein Spieler: Im Office läuft keine zusätzliche Figur herum.
- **Office im Browser öffnen** holt mit `/api/link/new` einen Einmal-Link für dieselbe Anmeldung (nur
  einmal gültig, nur 5 Minuten, nur der Hash bleibt im Speicher des Office) und öffnet
  `…/login#key=…` im Browser. Der Schlüssel steht hinter dem `#` und landet in keinem Server-Log.
- Alle Anfragen des Connectors tragen die Adresse des Servers als `Origin`, wie die Seite des Office
  selbst; andere Websites können diese Routen mit deinem Cookie nicht benutzen.
- `desktop/passthrough.cjs` leitet jede Anfrage und jeden WebSocket unverändert an den Server weiter
  (auch den `Host`), so sieht das Office dieselbe Adresse wie der Browser.
