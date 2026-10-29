# Zusammen spielen: das Office auf eurem Server

Zurück zur [README](../README.md) · [Server](vermittler.md) · [Connector](connector.md) · [Windows-App](windows-app.md) · [Mac-App](mac-app.md) · [Umbrel-App im Detail](umbrel.md)

Euer Agent Office läuft auf einem Server, zum Beispiel eurem [Umbrel](https://umbrel.com): der Umbrel-App
**[Agent Office Vermittler](vermittler.md)**. Dort sind die Etagen, das Board, der Chat und das Dach, und
es ist immer offen, solange der Server läuft. Keiner muss gastgeben, und es gibt keine Lobbys und keine
Einladungslinks. Jeder Spieler verbindet sich mit dem [Connector](connector.md) für Windows oder macOS:
Adresse des Servers, sein Passwort, der eigene Name. Die Arbeiter, die jemand einstellt, laufen auf
seinem eigenen PC, mit seinem eigenen Claude-Abo.

Das ist **nicht** Claudes eigene Cloud (claude.ai/code): Das Office läuft auf eurem eigenen Server, die
Arbeiter auf euren eigenen PCs.

## 1. Den Server aufsetzen

Einmal, von dem, der den Umbrel hat: die App „Agent Office Vermittler“ in euren Community App Store legen
und installieren ([so geht's](vermittler.md#einrichten), Details in [umbrel.md](umbrel.md)). Auf der Seite
der App steht ihr **Passwort**: Das ist das Passwort des Servers.

## 2. Verbinden

1. Den [Connector](connector.md) installieren und öffnen.
2. Unter **🛰️ Server**: die Adresse (z. B. `100.78.107.162:4600`, die Adresse des Umbrel im Tailnet oder
   NetBird-Netz, oder `umbrel.local:4600` im Heimnetz), das Passwort des Servers und deinen Namen, dann
   **Verbinden**. Beim ersten Mal legt der Server dein Konto an.
3. **🖥️ Deine Arbeiter auf diesem Computer**: dein Claude-Abo und GitHub auf diesem Computer anmelden
   (je ein Klick, es öffnet sich ein Konsolenfenster bzw. Terminal).
4. **🔐 Deine Anmeldungen auf dem Server**: GitHub anmelden (oder vom Computer übernehmen). Damit handelt
   das Office auf GitHub als du und klont die Repos, die du als Etage hinzufügst.
5. **🌐 Office im Browser öffnen**: Dein Browser öffnet das Office, angemeldet als du. Sprache und
   Bildschirm teilen gehen dort auch über `http://`, weil der Connector den Server über
   `http://127.0.0.1:48600` weiterreicht; lass ihn dafür laufen (in der Menüleiste reicht).

Beim nächsten Start verbindet sich der Connector von selbst. Schließt du sein Fenster, läuft er in der
Menüleiste (Mac) bzw. im Infobereich (Windows) weiter; von dort öffnest du das Office, die Einstellungen
oder trennst die Verbindung.

## 3. Mitspieler

1. Mitspieler brauchen einen Weg zu eurem Server: im selben Netz, oder über [Tailscale](https://tailscale.com)
   oder NetBird (teile den Umbrel mit ihnen). Über das offene Internet stellt ihr ihn nur hinter HTTPS.
2. Gib ihnen die **Adresse** und das **Passwort** des Servers. Jeder trägt im Connector seinen eigenen
   Namen ein und hat damit sein eigenes Konto.
3. Wer das Passwort hat, ist Admin. Gebt es nur Leuten, denen ihr das zutraut. Unter **☰ → 🔑 Accounts**
   seht ihr, wer ein Konto hat.

## 4. Wessen KI?

Jeder Arbeiter läuft mit dem Claude-Abo von dem, der ihn eingestellt hat:

- **Normalfall, auf deinem PC:** Deine Arbeiter laufen in deinem Arbeitsplatz, mit dem Claude Code und
  der GitHub CLI dieses Computers (die Karten unter **🖥️ Deine Arbeiter auf diesem Computer**). Ist dein PC
  aus, machen sie im Office Pause.
- **Auf dem Server:** Mit **Lieber auf dem Server laufen lassen** laufen sie dort, mit deinen Anmeldungen
  auf dem Server (**🔐 Deine Anmeldungen auf dem Server**: Claude und GitHub, oder ein Token). Unter ☰ →
  **🔑 Accounts** → **✳️ The office’s own Claude** kann ein Admin die Claude-Anmeldung des Servers für alle
  freigeben; dann gibt es **Claude des Servers nutzen**.

GitHub bleibt pro Person: Kommentare, Commits und Pull Requests erscheinen unter dem eigenen Namen.

## 5. Projekte als Etage

Jeder kann sein eigenes Projekt als Etage ins Haus holen, auch ein privates:

1. Im Connector unter **🔐 Deine Anmeldungen auf dem Server** mit **deinem** GitHub anmelden. Der Server
   klont mit dieser Anmeldung.
2. Im Aufzug (**E**) → *Projekt hinzufügen* siehst du deine Repositorys und fügst eins hinzu. Es ist für
   alle eine Etage.
3. Stellt jemand dort einen Arbeiter ein, klont sein Arbeitsplatz das Repo auf seinen PC. Damit seine
   Arbeiter dort **pushen und Pull Requests öffnen**, braucht sein GitHub-Konto Zugriff: Lade ihn auf
   GitHub als Collaborator ein (Repository → *Settings → Collaborators → Add people*).

## Gut zu wissen

- Die Grafik stellst du für dich unter ⚙️ Settings → **You → Graphics** ein: **⚖️ Balanced** (Standard)
  sieht aus wie High, spart aber viel Arbeit; **🍃 Eco** für Laptops im Akkubetrieb.
- Alles, was der Server sich merkt (Konten, Etagen, Projekte, Board, Chat, die Anmeldungen auf dem Server),
  liegt im Daten-Ordner der Umbrel-App und übersteht Updates und Neustarts.
- Siehe auch die Sicherheitshinweise in [How it works](how-it-works.md).
