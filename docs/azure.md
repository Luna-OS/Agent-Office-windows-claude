# Azure-Referenz

Die ganze Geschichte hinter `deploy/azure.sh`. Die Kurzfassung steht in der [README](../README.md#auf-azure-bereitstellen).

Es ist das Azure-Gegenstück zu [`deploy/aws.sh`](aws.md): dieselben Befehle, dasselbe [`deploy/provision.sh`](../deploy/provision.sh) auf der Maschine und dieselbe Regel, dass das Office nur über einen SSH-Tunnel erreichbar ist. Hast du die Azure CLI angemeldet (`az login`), gibt dir ein Befehl dein eigenes Office auf einer Azure-VM:

```bash
git clone https://github.com/Luna-OS/OpenAgenticOffice && cd OpenAgenticOffice
deploy/azure.sh up
```

Danach besteht der ganze Lebenszyklus aus vier weiteren Befehlen:

```bash
deploy/azure.sh open      # Tunnel zum Office und im Browser öffnen
deploy/azure.sh pause     # die VM freigeben, um Geld zu sparen (fragt nach); nur Platte und IP kosten
deploy/azure.sh resume    # wieder starten: gleiche Adresse, gleiche Dateien, dann öffnen
deploy/azure.sh destroy   # die Ressourcengruppe des Office samt Inhalt löschen (fragt nach)
```

Was `up` in ein paar Minuten tut:

1. Prüft, ob die VM-Größe in der Region für dein Abonnement angeboten wird, bevor etwas angelegt wird.
2. Legt eine eigene Ressourcengruppe `open-agentic-office` an, mit dem Tag `open-agentic-office=open-agentic-office`. Alles andere kommt hinein, also ist `destroy` ein einziges `az group delete`. Das Skript nutzt oder löscht keine Gruppe, die es nicht selbst angelegt hat (eine ohne diesen Tag).
3. Erzeugt ein SSH-Schlüsselpaar in `~/.config/open-agentic-office/azure/<name>/`. Es ist RSA, weil Azure ED25519-Schlüssel für neue VMs nur in der Vorschau annimmt.
4. Legt eine Netzwerksicherheitsgruppe an, deren einzige eingehende Regel **nur SSH (Port 22) und nur für deine aktuelle IP** öffnet. Das Office selbst hängt nie im Internet.
5. Gibt der Maschine eine statische öffentliche IP, damit die Adresse Pausen und Größenänderungen überlebt.
6. Startet eine **Standard_D4as_v5**-VM (4 vCPU, 16 GiB) mit Ubuntu 24.04 LTS, einer 64-GiB-Premium-SSD und Trusted Launch (Secure Boot und virtuelles TPM). Du meldest dich als `azureuser` an.
7. Führt dasselbe [`deploy/provision.sh`](../deploy/provision.sh) aus wie für [jeden Server](self-hosting.md): git, die GitHub CLI, **Claude Code** und das Office (Rust-Binärdatei aus dem neuesten Release, sonst per cargo und trunk gebaut) unter systemd, auf `127.0.0.1:4600` der VM. Das Office hält seine Daten in `~/open-agentic-office` auf der VM und klont Projekte nach `~/workspace/<owner>/<repo>`.
8. Öffnet einen SSH-Tunnel und deinen Browser auf `http://localhost:4600`. **Die erste Seite zeigt das Office-Passwort einmalig. Schreib es auf.** Das Office öffnet sich am Aufzug: Wähle ein Repository, und es wird die erste Etage.

Lass das Terminal offen, solange du das Office nutzt; Strg+C schließt den Tunnel. Beim nächsten Mal: `deploy/azure.sh open`.

## Die Maschine

**Warum Standard_D4as_v5.** Es ist dieselbe Größe wie die t3.xlarge von `deploy/aws.sh`, 4 vCPUs und 16 GiB, zu etwa demselben Preis: rund 0,17 $ pro Stunde auf Abruf in East US, gegen 0,166 $ für die t3.xlarge. Azures nächster Zwilling der t3 ist die burstfähige `Standard_B4s_v2` (0,166 $), aber beide bursten verschieden. Eine t3 läuft standardmäßig *unlimited*: Sind die CPU-Guthaben leer, läuft sie weiter und berechnet den Mehrverbrauch. Eine B-Serien-VM ohne Guthaben bremst auf 40 % ihrer CPUs, und eine neue hat weniger als eine Stunde Volllast gespart. Arbeiter, die bauen und testen, kommen schnell dahin, darum ist die Vorgabe eine Größe, die nie bremst.

| Größe | vCPU | GiB | ca. $/Stunde | Wofür |
| --- | --- | --- | --- | --- |
| `Standard_D4as_v5` (Vorgabe) | 4 | 16 | 0,17 | Ein paar Arbeiter gleichzeitig, mit voller Geschwindigkeit |
| `Standard_B4s_v2` | 4 | 16 | 0,17 | Dasselbe, burstfähig: bremst auf 40 %, wenn das Guthaben leer ist |
| `Standard_B4as_v2` | 4 | 16 | 0,15 | Ebenfalls burstfähig, auf AMD |
| `Standard_D8as_v5` | 8 | 32 | 0,34 | Ein Team oder viele Arbeiter auf einmal |

(East US, auf Abruf, aus Azures Preisliste; andere Regionen weichen ab.)

Wähle eine mit `up --size <größe>` oder ändere sie später mit `deploy/azure.sh resize <größe>`. Das Ändern gibt die VM frei, wechselt die Größe und startet sie wieder, was ein paar Minuten dauert. Adresse, Platte und alles darauf bleiben. Ein pausiertes Office bleibt pausiert, in der neuen Größe. Azure lehnt manche Änderungen ab, und `resize` prüft sie, bevor es etwas anhält: zwischen Intel/AMD- und Arm-Größen, zwischen einer Größe mit lokaler temporärer Platte und einer ohne (`Standard_B4ms` und `Standard_B4s_v2`, etwa), auf eine Größe ohne Trusted Launch oder ohne Premium SSD (kein `s` nach der Zahl, wie `Standard_D8_v5`) oder auf eine, die nur NVMe-Platten nimmt (die meisten v6-Größen). Für diese: `destroy` und noch einmal `up`. Kann Azure die VM nicht in der neuen Größe starten (gerade kein Platz in der Region), setzt `resize` sie auf die alte zurück.

**Arm.** Die Arm-Größen haben ein `p` im Namen. `up` nimmt die Cobalt-Größen wie `Standard_D4ps_v6` und gibt ihnen die Arm-Fassung von Ubuntu. Die älteren Ampere-B-Serien (`Standard_B4ps_v2`) lehnt es ab, weil sie Trusted Launch nicht können.

**Die Platte.** Premium SSD wird nach Stufe abgerechnet, und jede Größe von 33 bis 64 GiB kostet gleich viel (P6), darum ist die Vorgabe 64. `up --disk 128` gibt der Platte eines neuen Office die nächsthöhere Stufe.

**Pausieren.** `pause` *gibt die VM frei* (deallocate). Azure berechnet für eine freigegebene VM nichts, wohl aber für eine nur angehaltene (von innen heruntergefahren oder `az vm stop`), darum gibt das Skript immer frei. Solange pausiert ist, zahlst du nur die Platte (rund 10 $ im Monat für P6) und die statische IP. `resume` startet sie unter derselben Adresse neu, und die Arbeiter kommen schlafend zurück: Drücke R an ihrem Schreibtisch.

## Dein Team

Alles in der [AWS-Referenz](aws.md) über Mitspieler gilt genauso, mit `deploy/azure.sh` statt `deploy/aws.sh`. **👥 Team** im **☰**-Menü installiert ihre SSH-Schlüssel von GitHub, und ihre Schlüssel melden sich als eingeschränkter Benutzer `office` an, der nur zum Office-Port weiterleiten darf. In deinem Terminal:

```bash
deploy/azure.sh invite octocat        # nimmt die SSH-Schlüssel von github.com/octocat
deploy/azure.sh allow 203.0.113.7     # ihre IP (SSH antwortet nur erlaubten IPs)
deploy/azure.sh allow anywhere        # oder alle IPs: SSH nimmt trotzdem nur deinen und eingeladene Schlüssel
deploy/azure.sh uninvite octocat      # ihre Schlüssel entfernen und offene Tunnel kappen
deploy/azure.sh team                  # wer eingeladen ist
deploy/azure.sh revoke 203.0.113.7    # eine IP wieder von der Liste nehmen
```

Das Office weiß, dass es mit `deploy/azure.sh` bereitgestellt wurde, darum sind die Befehle, die es unter **👥 Team** und auf der **🌐 Dienste**-Tafel vorschlägt, die für Azure.

Konten funktionieren wie in der [README](../README.md#nutzer-hinzufügen). Einen Einladungslink erzeugst du in deinem Terminal so:

```bash
deploy/azure.sh ssh 'sudo -u azureuser /opt/open-agentic-office/open-agentic-office konten einladen ada --daten "$(cat /etc/open-agentic-office/home)"'
```

## Alles andere

```bash
deploy/azure.sh open                     # Tunnel + Office im Browser öffnen
deploy/azure.sh service 5173             # den Webserver eines Arbeiters von der 🌐 Dienste-Tafel öffnen
deploy/azure.sh status                   # VM, Adresse, Office oben?, Team, erlaubte IPs
deploy/azure.sh resize Standard_D8as_v5  # größere oder kleinere VM; gleiche Adresse, ein paar Minuten Ausfall
deploy/azure.sh update                   # neueste Fassung von open-agentic-office installieren und neu starten
deploy/azure.sh reset-password           # neues Passwort, einmalig angezeigt; meldet alle ab
deploy/azure.sh ssh | logs               # auf die VM / den Logs des Office folgen
```

**⬆️ Office aktualisieren** im **☰**-Menü geht ebenfalls.

Nützliche Optionen für `up`:

- `--location westeurope` wählt die Region für ein neues Office (`--region` geht auch). Ohne nimmt `up` den Standardort deiner Azure CLI (`az config set defaults.location=westeurope`), sonst `eastus`. Ein Office bleibt in der Region, in der es entstanden ist.
- `--subscription <id oder name>` nutzt ein anderes Abonnement als das aktuelle der Azure CLI. Die anderen Befehle des Office merken es sich.
- `--size` und `--disk` setzen VM- und Plattengröße.
- `--project owner/repo` klont dieses Repo zusätzlich als erste Etage des Office.
- `--allow <ip>` lässt gleich zu Beginn weitere IPs an SSH.
- `--name <name>` betreibt mehrere Offices nebeneinander, jedes in einer eigenen Ressourcengruppe `open-agentic-office-<name>`. Jeder andere Befehl nimmt dann dasselbe `--name`, vor oder nach dem Befehl, und die Befehle, die das Office selbst vorschlägt, enthalten es.
- `--claude-token "$(claude setup-token)"`, `--anthropic-api-key`, `--github-token` und `--no-github-token` funktionieren wie auf [AWS](aws.md).

**Von einem zweiten Computer.** Der SSH-Schlüssel liegt auf dem Computer, der `up` ausgeführt hat. Führe auf einem anderen, im selben Abonnement angemeldet (oder mit `--subscription`), `deploy/azure.sh connect` aus: Es erzeugt dem Computer einen Schlüssel, fügt ihn mit `az vm user update` der VM hinzu und lässt seine IP durch die Firewall. Mehr ändert es nicht, während `up` die VM auch mit dem GitHub-Token und git-Namen dieses Computers neu einrichten würde. Auch das Kopieren von `~/.config/open-agentic-office/azure/<name>/` geht.

**`up` noch einmal ausführen** richtet die VM neu ein, so wirken `--size`, `--project` oder ein neues `--claude-token`. Ohne `--claude-token` oder `--anthropic-api-key` behält es die Claude-Anmeldung von vorher.

**Einstellungen.** Wie auf AWS stehen Office-Einstellungen in `/etc/open-agentic-office/env` auf der VM (`deploy/azure.sh ssh`, dann `sudo nano /etc/open-agentic-office/env` und `sudo systemctl restart open-agentic-office`). Die Uhr der VM geht nach UTC, setze dort also `OFFICE_STADT="Portland, Oregon"`, damit Wetter und Feiertagskalender des Office deine sind.

## Wenn Azure Nein sagt

- **Kontingent.** *„Operation could not be completed as it results in exceeding approved … Cores quota“*: Dein Abonnement darf in der Region nicht so viele vCPUs dieser Familie betreiben. Bitte im Azure-Portal unter **Kontingente → Compute** um mehr (für die Vorgabe „Standard DASv5 Family vCPUs“) oder führe `up` mit einer anderen `--size` noch einmal aus. Für eine andere Region zuerst `destroy`, denn die Ressourcengruppe des Office liegt schon in der ersten. Testversionen und Studentenabonnements bekommen 4 vCPUs pro Region und können nicht mehr beantragen: Die Vorgabe passt, alles Größere braucht ein Pay-as-you-go-Abonnement.
- **Größe nicht verfügbar.** `up` prüft zuerst und hält an, bevor etwas angelegt wird. Die nutzbaren Größen einer Region listest du mit: `az vm list-skus -l eastus --resource-type virtualMachines --size Standard_B -o table`.
- **Ein halb gebautes Office.** Hält `up` mittendrin an (Kontingent, verlorene Verbindung), führe es noch einmal aus: Es nutzt weiter, was es schon angelegt hat. `destroy` entfernt alles. (Azure legt für die Region unter Umständen auch eine eigene Ressourcengruppe `NetworkWatcherRG` an. Sie ist kostenlos und gehört nicht dem Office.)
- **SSH verbindet nicht.** SSH antwortet nur den erlaubten IPs. In einem neuen Netz sagt `open` das; `deploy/azure.sh allow me` lässt deine neue IP herein.
