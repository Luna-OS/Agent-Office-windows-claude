# Dokploy-Referenz

Die ganze Geschichte hinter `deploy/dokploy.sh`. Die Kurzfassung steht in der [README](../README.md#auf-dokploy-bereitstellen).

Du brauchst einen **[Dokploy](https://dokploy.com)-Server** und einen **API-Schlüssel** dafür, dazu `ssh`, `curl`, `git`, `jq` (wertet die JSON-Antworten der Dokploy-API aus) und einen Klon dieses Repos. Erzeuge den Schlüssel in Dokploy unter **Settings → Profile → API/CLI Keys** und lass sein Rate-Limiting aus: Das Skript fragt den Build alle paar Sekunden ab.

```bash
git clone https://github.com/Luna-OS/OpenAgenticOffice && cd OpenAgenticOffice
export DOKPLOY_API_KEY=<dein Schlüssel>
deploy/dokploy.sh up --url https://dokploy.example.com --claude-token "$(claude setup-token)"
```

Danach:

```bash
deploy/dokploy.sh open      # Tunnel zum Office und im Browser öffnen
deploy/dokploy.sh update    # diesen Checkout neu hochladen, bauen und neu ausrollen
deploy/dokploy.sh destroy   # das Office, sein Volume und alles darauf löschen (fragt nach)
```

Was `up` tut, beim ersten Mal in etwa fünf Minuten:

1. Erzeugt ein SSH-Schlüsselpaar. Es liegt in `~/.config/open-agentic-office/dokploy/<name>/`, zusammen mit der Adresse deines Dokploy und dem API-Schlüssel (nur für dich lesbar), damit spätere Befehle beides nicht brauchen.
2. Legt ein Dokploy-Projekt an, nach dem Office benannt, mit einer Anwendung `office`. Sie läuft auf Dokploys eigenem Server oder mit `--server <name>` auf einem seiner Remote-Server (auf Dokploy Cloud brauchst du einen).
3. Hängt ein **Docker-Volume auf `/data`** ein. Alles, was das Office behält, liegt darauf (siehe unten).
4. Veröffentlicht das SSH des Containers auf **Port 2222 des Servers** (einen anderen wählst du mit `--ssh-port`). Das ist der einzige Weg hinein: Die Anwendung hat keine Domain, Traefik leitet also nie zu ihr, und das Office selbst hängt nie im Internet.
5. Zippt diesen Checkout und lädt ihn hoch, und Dokploy baut ihn mit [`deploy/container/Dockerfile`](../deploy/container/Dockerfile): ein Rust-Multi-Stage-Build (Server mit cargo, Client mit trunk) und eine schlanke Laufzeit mit git, GitHub CLI, Build-Werkzeugen und sshd. Das Zip enthält, was `git add -A` sähe: deine uncommitteten Änderungen und neuen Dateien inklusive, alles, was `.gitignore` auslässt, ausgelassen, und keine `.env`-Dateien oder Schlüssel. **Claude Code** wird beim ersten Start des Office auf das Volume installiert, es aktualisiert sich also selbst. Docker Swarm startet den Container neu, wenn er abstürzt.
6. Meldet die GitHub CLI mit deinem lokalen `gh auth token` an (liegt auf dem Volume) und gibt git deinen Namen und deine E-Mail.
7. Öffnet einen SSH-Tunnel und deinen Browser auf `http://localhost:4600`. **Die erste Seite zeigt das Office-Passwort einmalig. Schreib es auf.** Das Office behält danach nur einen Hash.

Das Office lauscht in seinem Container auf `127.0.0.1:4600`, und alle erreichen es durch den Tunnel. `localhost` gilt als sicherer Ursprung, also funktionieren Sprachchat und Bildschirm teilen, ohne Zertifikate. Dein Schlüssel meldet sich als `agentoffice` an, der Benutzer, unter dem das Office läuft (`deploy/dokploy.sh ssh` gibt dir seine Shell). Das Office startet ohne Etagen: Fahre mit dem Aufzug und wähle eines der Repositories, die dein GitHub-Token sieht.

**Die SSH-Adresse.** Du und deine Mitspieler verbindet euch per SSH zur IP des Servers, wie Dokploy sie kennt: die IP des Remote-Servers oder, für Dokploys eigenen Server, die unter **Settings → Web Server**. Ist das nicht die richtige Adresse (ein DNS-Name oder ein Server hinter NAT), gib `--ssh-host <host>` an `up` mit. Docker öffnet veröffentlichte Ports an `ufw` vorbei, aber eine Firewall vor dem Server (die von Hetzner, eine AWS-Sicherheitsgruppe, die von DigitalOcean) muss TCP auf dem SSH-Port erlauben. Wähle nicht Port 22: Das ist das eigene SSH des Servers.

**Was einen Neustart überlebt.** Dokploy ersetzt den Container bei jedem Neustart und jedem Deployment; das Volume bleibt. Es ist nach der Anwendung benannt, etwa `open-agentic-office-x1y2z3-data`. Darauf liegt:

| Auf dem Volume | Was darin steckt |
| --- | --- |
| `/data/home/open-agentic-office` | Die eigenen Daten des Office: Passwort-Hash, Konten und Einladungen, Etagen, Chat, Einstellungen, Arcade-Rekorde, die eigenen Claude- und GitHub-Anmeldungen jedes Kontos |
| `/data/home/workspace` | Die Projekte, geklont als `<owner>/<repo>`, mit den Worktrees der Arbeiter |
| `/data/home/.local`, `.claude`, `.claude.json` | Claude Code selbst, seine Anmeldung und die Sitzungen, die Arbeiter fortsetzen |
| `/data/home/.config/gh`, `.gitconfig` | Die Anmeldung der GitHub CLI sowie Name und E-Mail für git |
| `/data/ssh` | Der SSH-Host-Schlüssel, damit ssh dem Office nach einem neuen Ausrollen weiter vertraut |
| `/data/team` | SSH-Schlüssel der Mitspieler, aus **👥 Team** |

`/data/home` ist das Home-Verzeichnis von `agentoffice`, also liegt alles andere, was ein Arbeiter in `~` ablegt, ebenfalls auf dem Volume. Der Rest des Containers (der Office-Code in `/opt/open-agentic-office`, alles systemweit Installierte) kommt jedes Mal frisch aus dem Image. Ein Neustart oder Deployment hält laufende Arbeiter an; sie kommen an ihren Schreibtischen zurück, wenn das Office es tut. Dokploy startet normalerweise den neuen Container, bevor es den alten stoppt; `up` stellt das Office auf „erst stoppen“, damit sich nie zwei Offices das Volume teilen.

**Dein Team einladen.** Mitspieler brauchen weder ein Dokploy-Konto noch dieses Repo, nur `ssh`. Klicke im Office im **☰**-Menü auf **👥 Team** und tippe ihren GitHub-Benutzernamen, oder führe `deploy/dokploy.sh invite octocat` aus. In beiden Fällen bekommen sie einen Befehl wie:

```
ssh -L 4600:localhost:4600 ssh://office@203.0.113.7:2222
```

Sie lassen ihn laufen, öffnen http://localhost:4600 und melden sich an. Ihre Schlüssel melden sich als Benutzer `office` an, der **nur** zum Office-Port weiterleiten darf: keine Shell, keine anderen Ports, kein `-R`. Der SSH-Port antwortet jeder IP, aber sshd nimmt nur deinen und eingeladene Schlüssel an, nie ein Passwort. Tunnel der **🌐 Dienste** zu den Webservern der Arbeiter funktionieren genauso (`deploy/dokploy.sh service 5173` für dich).

```bash
deploy/dokploy.sh open                 # Tunnel + Office im Browser öffnen
deploy/dokploy.sh service 5173         # den Webserver eines Arbeiters von der 🌐 Dienste-Tafel öffnen
deploy/dokploy.sh invite <gh-user>     # einen Mitspieler hineintunneln lassen (oder: invite <name> <key.pub>)
deploy/dokploy.sh uninvite <name>      # seine Schlüssel entfernen und offene Tunnel kappen
deploy/dokploy.sh team                 # wer eingeladen ist
deploy/dokploy.sh status               # seine Seite in Dokploy, letztes Deployment, SSH-Adresse, Office oben?, Team
deploy/dokploy.sh update               # diesen Checkout neu hochladen, bauen und neu ausrollen
deploy/dokploy.sh restart              # den Container ohne Neubau neu starten
deploy/dokploy.sh reset-password       # neues Passwort, einmalig angezeigt; meldet alle ab
deploy/dokploy.sh ssh | logs           # eine Shell im Container / den Logs des Office folgen
```

**Aktualisieren.** Hole die neueste Fassung von OpenAgenticOffice in deinen Klon und führe `deploy/dokploy.sh update` aus. **⬆️ Office aktualisieren** im **☰**-Menü ist für Server, die mit `deploy/provision.sh` eingerichtet wurden: Der Code eines Containers kommt aus seinem Image. Schlägt ein Build fehl, steht sein Log im Reiter **Deployments** der Anwendung in Dokploy (`status` gibt den Link aus).

**Claude-Anmeldung.** Gib `--claude-token "$(claude setup-token)"` an (dein Claude-Abo) oder `--anthropic-api-key <key>`; beides wird eine Umgebungsvariable der Anwendung. Oder gib keins an und führe `/login` im Terminal des ersten Arbeiters aus: Diese Anmeldung bleibt auf dem Volume.

**GitHub.** Standardmäßig meldet dein lokales `gh auth token` die GitHub CLI im Office an. Jeder, der das Office nutzen kann, kann dieses Token nutzen, gib also `--github-token <feingranulares Token>` oder `--no-github-token` an, wenn das zu viel ist. Codex und OpenCode sind nicht im Image: Trage sie in `deploy/container/Dockerfile` ein, wenn du sie nutzt.

**Weitere Einstellungen.** Das Office liest dieselben Umgebungsvariablen wie überall ([Konfiguration](configuration.md)). Trage sie im Reiter **Environment** der Anwendung in Dokploy ein, etwa `OFFICE_STADT=Portland, Oregon`, und führe dann `deploy/dokploy.sh restart` aus. `up` setzt dort nur seine eigenen Variablen und behält den Rest.

**Mehrere Offices.** `--name <name>` gibt jedem ein eigenes Projekt, eine eigene Anwendung, ein eigenes Volume und ein eigenes Zustandsverzeichnis; gib jedem auch einen eigenen `--ssh-port`.

**Kosten.** Das Office läuft auf deinem Dokploy-Server, neben allem anderen dort. Gib ihm Platz für die Arbeiter: 4 vCPUs und 8 bis 16 GB Speicher sind bequem. `deploy/dokploy.sh destroy` löscht die Anwendung, dann ihr Volume, dann das Projekt, wenn nichts anderes darin ist. Das Image, das Dokploy gebaut hat, bleibt in Docker bis zur nächsten Bereinigung. Ältere Dokploy-Versionen können ein Volume nicht über ihre API löschen; das Skript gibt dann das `docker volume rm` aus, das du auf dem Server ausführst.
