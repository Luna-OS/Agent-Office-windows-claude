# Online-Lobby: das Office auf deinem Umbrel (oder deinem PC)

Zurück zur [README](../README.md) · [Connector](connector.md) · [Windows-App](windows-app.md) · [Mac-App](mac-app.md) · [Umbrel-App im Detail](umbrel.md)

Ein OpenAgenticOffice mit vielen Arbeitern braucht Rechenleistung und Speicher: Jeder Arbeiter ist ein
Claude Code (oder Codex, OpenCode …) mit eigenem Terminal. In einer **Online-Lobby** läuft das alles auf
einem Server, zum Beispiel deinem [Umbrel](https://umbrel.com). Dein Computer zeigt nur noch das
3D-Office, im Browser, und deine Mitspieler kommen in dieselbe Lobby: mit dem [Connector](connector.md) für
Windows oder macOS, oder direkt im Browser.

Das ist **nicht** Claudes eigene Cloud (claude.ai/code): Die Lobby und ihre Arbeiter laufen auf deinem
eigenen Server, mit deinem Claude-Abo oder dem deiner Mitspieler.

## 1. Die Lobby auf dem Umbrel installieren

Die Umbrel-App „OpenAgenticOffice“ kommt aus einem Community App Store, zum Beispiel deinem eigenen:

1. Einmal die App in den Store legen (an einem Computer, der den Store-Checkout erreicht, mit einem Klon
   dieses Repositorys):

   ```bash
   git clone <adresse-deines-app-stores>
   deploy/umbrel/publish.sh <app-store-checkout> --pr
   ```

   Das legt den App-Ordner (Store-ID + `-open-agentic-office`) mit der neuesten Version des Images auf
   einem eigenen Branch an, pusht ihn und zeigt den Link, der den Pull Request im Store öffnet. Den
   mergen, fertig (`--push` statt `--pr` committet direkt auf den aktuellen Branch). Mehr dazu, auch zum
   Aktualisieren: [docs/umbrel.md](umbrel.md). Das Image baut der Workflow
   [`umbrel-image.yml`](../.github/workflows/umbrel-image.yml) aus [`deploy/container/Dockerfile`](../deploy/container/Dockerfile).
2. Auf dem Umbrel: **App Store → ⋯ (oben rechts) → Community App Stores**, die Adresse des Stores
   eintragen und hinzufügen.
3. Im Store **OpenAgenticOffice** installieren. Umbrel zeigt dir auf der Seite der App das **Passwort**
   des Office: Das ist das gemeinsame Office-Passwort zum ersten Anmelden.
4. Öffnen: Die Lobby ist unter `http://umbrel.local:4600` erreichbar (oder der IP deines Umbrel, z. B. im
   Tailnet `http://100.x.y.z:4600`).

Beim ersten Start installiert der Container Claude Code in seinen Daten-Ordner. Danach meldet sich jeder
Spieler im Office mit seinem eigenen Claude und GitHub an (siehe unten); die Anmeldungen liegen auf dem
Umbrel, im Ordner des jeweiligen Kontos.

## 2. Mit dem Connector beitreten

Die App für Windows und macOS ist ein [Connector](connector.md): Sie verbindet dich mit der Lobby und
öffnet das Office in deinem Browser.

1. **Lobby beitreten**: die Adresse eintragen (z. B. `100.78.107.162:4600`, die Adresse deines Umbrel im
   Tailnet oder NetBird-Netz, oder `umbrel.local:4600` im Heimnetz) und **Verbinden**. Der Connector
   prüft, ob die Lobby antwortet, und merkt sie sich.
2. **Du als Host, beim ersten Mal**: Im Office mit dem Office-Passwort von der Seite der Umbrel-App
   anmelden (Name leer lassen). Dann unter **☰ → 🔑 Konten** Name und Passwort wählen: Das ist dein
   Admin-Konto, ab jetzt meldest du dich damit an.
3. **🔐 Deine Anmeldungen** (**☰**-Menü): **Mit Claude anmelden** (Claudes Seite geht im Browser auf, den
   Code dort kopieren und im Office einfügen) und **Mit GitHub anmelden** (Einmal-Code kopieren, auf GitHub
   einfügen). Ab jetzt laufen deine Arbeiter auf dem Umbrel mit deinem Claude-Abo, und auf GitHub handelt
   das Office als du.
4. **Im Browser öffnen**: Sprache und Bildschirm teilen gehen auch über `http://`, weil der Connector die
   Lobby über `http://127.0.0.1:48600` weiterreicht; lass ihn dafür laufen.

Beim nächsten Start kennt der Connector die Lobby schon; ein Klick auf sie öffnet das Office.

## 3. Mitspieler einladen

1. Mitspieler brauchen einen Weg zu deinem Umbrel: im selben Netz, oder über [Tailscale](https://tailscale.com)
   oder NetBird (teile den Umbrel mit ihnen). Über das offene Internet solltest du die Lobby nur hinter
   HTTPS stellen.
2. Im Office unter **☰ → 🔑 Konten** einen **Einladungslink** (`…/beitreten#…`) erstellen und kopieren. Der
   Link funktioniert einmal und gilt 7 Tage.
3. Wer eingeladen ist, installiert den Connector, fügt den Link unter **Lobby beitreten** ein, wählt Name und
   Passwort und meldet sich mit dem **eigenen** Claude und GitHub an. Ohne Connector geht der Link auch im
   Browser.
4. Wenn alle ein Konto haben: das gemeinsame Passwort im Office unter ☰ → 🔑 Konten ausschalten.

## Ohne Umbrel: die Lobby auf deinem PC hosten

Im Connector unter **Ohne Server → Auf diesem Computer starten** läuft das Office auf deinem PC, zuerst nur
für ihn selbst (`127.0.0.1`): Niemand sonst kommt hinein, auch nicht im selben WLAN oder über Tailscale.
Um Mitspieler hereinzulassen:

1. Im Connector **👥 Lobby für andere öffnen**. Das Office startet neu, jetzt für alle Netzwerke deines
   Computers (`--host 0.0.0.0`), mit einem eigenen Lobby-Passwort (`OFFICE_PASSWORT`).
2. **Windows fragt**, ob „open-agentic-office“ im Netzwerk erreichbar sein darf: für **private Netzwerke
   zulassen**. Steht dein WLAN in Windows auf „Öffentlich“, stell es auf „Privat“ (Einstellungen → Netzwerk
   und Internet → WLAN → dein Netz → Netzwerkprofil). Auf dem Mac: eingehende Verbindungen erlauben.
3. Der Connector zeigt die **Einladung**: deine Adresse (z. B. `192.168.178.20:4600` im WLAN, oder die über
   Tailscale oder NetBird) und das Passwort. Verschick beides.
4. Besser mit eigenem Konto (für das eigene GitHub, die eigenen Projekte und die eigene KI): im Office unter
   **☰ → 🔑 Konten** einen Einladungslink erstellen und weitergeben. Wer eingeladen ist, fügt ihn im eigenen
   Connector ein.

Wer mit dem Passwort hereinkommt, ist Admin und kann Arbeiter einstellen, die **auf deinem Computer**
Befehle ausführen. Lade nur Leute ein, denen du das zutraust, und gib ihnen am besten eigene Konten. Alle
Arbeiter laufen dann auf deinem PC; soll er geschont werden, nimm die Umbrel-App. **Lobby wieder schließen**
im Connector macht das Office wieder nur für dich erreichbar.

Auf der Kommandozeile geht dasselbe ohne Connector:
`open-agentic-office --host 0.0.0.0 --passwort 'ein gutes Passwort'` (siehe [Konfiguration](configuration.md)).

## 4. Wessen KI? Die des Hosts oder die eigene

Jeder Arbeiter läuft mit dem Claude-Abo von dem, der ihn eingestellt hat:

- **Eigene KI:** Mit einem Konto meldet sich jeder Spieler im Office unter **☰ → 🔐 Deine Anmeldungen** mit
  seinem eigenen Claude-Abo an (*Mit Claude anmelden*, oder ein Token aus `claude setup-token`). Seine
  Arbeiter laufen dann auf seinem Abo und seinen Limits, auch wenn sie auf deinem Umbrel laufen.
- **KI des Hosts:** Unter **☰ → 🔑 Konten → ✳️ Claude des Office → Für alle freigeben** gibt ein Admin die
  Claude-Anmeldung des Office (also deine) für alle frei. Jeder Spieler wählt dann **Claude des Hosts
  nutzen** oder bleibt bei seinem eigenen und kann jederzeit wechseln. Laufende Arbeiter behalten das Abo,
  mit dem sie gestartet sind. Stoppt der Admin das Teilen, laufen neue Arbeiter der Mitspieler wieder auf
  ihrem eigenen Abo.
- Admins dürfen die Anmeldung des Office immer nutzen. Ohne Konten (nur mit dem gemeinsamen Passwort) laufen
  alle Arbeiter auf der Anmeldung des Office.

GitHub bleibt pro Person: Kommentare, Commits und Pull Requests erscheinen unter dem eigenen Namen.

## 5. Projekte der Mitspieler als Etage

Jeder Spieler kann sein eigenes Projekt als Etage ins Haus holen, auch ein privates:

1. Der Mitspieler meldet sich im Office unter **🔐 Deine Anmeldungen** mit **seinem** GitHub an (*Mit GitHub
   anmelden*, Code bei github.com/login/device eingeben). Sein GitHub auf seinem eigenen PC zählt nicht: Die
   Lobby klont auf dem Rechner, auf dem sie läuft.
2. Im Aufzug (**E**) → *Projekt hinzufügen* sieht er dann **seine** Repositorys und fügt eins hinzu. Es wird
   mit seiner Anmeldung geklont und ist für alle in der Lobby eine Etage.
3. **Du arbeitest mit deinem Claude daran:** Jeder Arbeiter läuft mit dem Claude dessen, der ihn einstellt,
   auf jeder Etage. Setz auf seiner Etage einen Arbeiter an einen Tisch, und er arbeitet mit deinem Abo an
   seinem Projekt.
4. Damit deine Arbeiter dort auch **pushen und Pull Requests öffnen** und die Issue- und PR-Tafeln das
   Repository lesen können, braucht dein GitHub-Konto Zugriff darauf: Der Mitspieler lädt dich auf GitHub als
   Collaborator ein (Repository → *Settings → Collaborators → Add people*). Ohne das arbeiten deine Arbeiter
   trotzdem am Code, nur Push und PRs gehen dann über seine Arbeiter.

Ohne Konto (nur mit dem gemeinsamen Passwort) läuft alles über das GitHub des Hosts: Dann gehen nur
Repositorys, die der Host sehen kann.

## Gut zu wissen

- Alles, was die Lobby sich merkt (Passwort, Konten, Etagen, Projekte, die Anmeldungen bei Claude und
  GitHub), liegt im Daten-Ordner der Umbrel-App (`OFFICE_HOME`) und übersteht Updates und Neustarts.
- Jeder Spieler mit Konto kann Arbeiter einstellen, die auf dem Umbrel Befehle ausführen. Gib Konten nur
  Leuten, denen du das zutraust; siehe die Sicherheitshinweise in [So funktioniert es](how-it-works.md).
