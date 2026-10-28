# Konfiguration

Zurück zur [README](../README.md).

## Wo das Office seine Sachen ablegt

Das Office legt seine Daten im Datenordner `~/open-agentic-office` ab (verschieben mit `--daten` oder `OFFICE_HOME`) und klont Projekte als `<projekte>/<owner>/<repo>`. Um sie woanders zu klonen, etwa nach `~/Workspace`, startest du mit `--projekte` oder `OFFICE_PROJEKTE`. Schon vorhandene Etagen bleiben, wo sie sind; ein Checkout desselben Repos, der schon im neuen Ordner liegt, wird so übernommen.

Im Datenordner:

| Datei / Ordner | Inhalt |
| --- | --- |
| `zugang.json` | Hash des gemeinsamen Passworts |
| `einstellungen.json` | Server-Einstellungen |
| `bestenliste.json` | Bestenliste des Spielautomaten (fürs ganze Gebäude) |
| `medien/` | Zustand der Medien (Bildschirm teilen, Dach) |
| `karten/*.json` | Eigene Karten |
| `sitzung.key` | Schlüssel, mit dem die Sitzungs-Cookies signiert werden |
| `konten.json` | Konten und offene Einladungen |
| `etagen.json` | Die Liste der Etagen (Checkout, Farben) |
| `etagen/<id>/` | Zustand der Etage: Arbeiter (`workers.json`), Schwarzes Brett, Spielsachen, Jukebox (`jukebox.json`) |
| `homes/<konto>/` | `claude/`, `gh/`, `gitconfig` und `anmeldungen.json`: die eigenen Anmeldungen jedes Kontos (das Konto zu entziehen löscht sie) |
| `hook-port` | Port des Hook-Servers vom letzten Start (nur Loopback) |

Das Office startest du in einem Projektordner mit `open-agentic-office ~/code/mein-projekt`: Dieses Projekt wird zur Etage. Worktrees der Arbeiter liegen in `<projekt>/.office/worktrees/<name>-<id>` auf dem Branch `office/<name>-<id>`.

## Kommandozeile

```
open-agentic-office [Optionen] [PROJEKTORDNER]
open-agentic-office einrichten [Optionen]   Erst-Einrichtung (Projekte-Ordner, GitHub, Etagen) noch einmal

      --daten <dir>       Datenordner (Standard ~/open-agentic-office, Env OFFICE_HOME)
      --projekte <dir>    Wohin neue Etagen geklont werden, als <dir>/<owner>/<repo> (Env OFFICE_PROJEKTE)
      --projekt <o/r>     Dieses Repo beim Start klonen, wenn es noch keine Etage gibt
      --client <dir>      Ordner des gebauten Browser-Clients (Env OFFICE_CLIENT;
                          Standard: ./dist neben der Binärdatei bzw. dist/ im Repo)
  -p, --port <n>          Port (Standard 4600, Env PORT)
      --host <adresse>    Bind-Adresse (Standard 127.0.0.1; 0.0.0.0 lässt dein Netzwerk herein, Env OFFICE_HOST)
      --passwort <pw>     Gemeinsames Office-Passwort (Env OFFICE_PASSWORT); ohne: nur lokal offen
      --passwort-zuruecksetzen   Ein neues gemeinsames Passwort setzen und ausgeben
      --no-open           Den Browser nicht öffnen, den Anmelde-Link ausgeben (Env OFFICE_NO_OPEN=1)
      --max-arbeiter <n>  Höchstens n Arbeiter gleichzeitig
      --stadt <name>      Stadt für Sonne und Wetter (Env OFFICE_STADT)
  -V, --version           Version
  -h, --hilfe             Hilfe
      --proxy-vertrauen   X-Forwarded-* vertrauen (hinter Caddy/nginx)
      --beanspruch-token  Einmal-Token, mit dem der erste Admin die Lobby übernimmt (Env OFFICE_BEANSPRUCH_TOKEN)

open-agentic-office konten [liste | einladen [name] [--admin] | entziehen <name>
                            | rolle <name> admin|mitglied | passwort an|aus] [--daten <dir>]

  Konten einladen, auflisten und entziehen, das gemeinsame Passwort aus- oder
  anschalten. Funktioniert, während das Office läuft.
```

Weitere Umgebungsvariablen:

| Variable | Wirkung |
| --- | --- |
| `OFFICE_TURN` | TURN-Server für Sprache, kommagetrennt, z. B. `turn:nutzer:pass@host:3478` (Standard: STUN `stun.l.google.com:19302`) |
| `OFFICE_PROXY_VERTRAUEN` | Wie `--proxy-vertrauen` |
| `OFFICE_AGENT_<ID>` | Programm eines Anbieters überschreiben, z. B. `OFFICE_AGENT_CLAUDE=/pfad/zu/claude`; `OFFICE_AGENT_EIGEN` ist der eigene Befehl |
| `OFFICE_DSH_PROFIL` | Profil von DeepSeek Harness über ACP (Standard `acp`) |
| `OFFICE_PTY_WIRT=aus` | Terminals ohne den eigenen Wirt-Prozess `office-pty-wirt` betreiben (dann überleben sie keinen Neustart des Servers). Der Wirt beendet sich selbst, wenn 30 Minuten lang kein Server zurückkommt oder keine Terminals mehr laufen |
| `OFFICE_TEAM_HELFER`, `OFFICE_SERVE_HELFER` | Pfade der Hilfsprogramme für SSH-Team und Tailscale Serve (Standard `/usr/local/bin/open-agentic-office-team` bzw. `-serve`) |
| `OFFICE_HOOK_PORT` | Port des Loopback-Hook-Servers festlegen |
| `OFFICE_LOG` | Log-Stufe, z. B. `debug` |
| `OFFICE_SELBSTUPDATE=1` | Schaltet **☰ → Office aktualisieren** frei |
| `OFFICE_DEPLOY_SKRIPT` | Pfad des Update-Skripts; der Server ruft `<skript> pruefen` (JSON mit `aktuell`, `neueste`, `aenderungen`, `hinter`) und `<skript> ausfuehren` (installiert und startet den Dienst neu). `deploy/provision.sh` setzt es auf `/usr/local/bin/open-agentic-office-update` |
| `OFFICE_DEPLOY_BEFEHL` | Name des Skripts, mit dem du das Office bereitgestellt hast (z. B. `deploy/aws.sh`), damit das Office in vorgeschlagenen Befehlen darauf verweist |
| `OFFICE_OEFFENTLICHER_HOST` | Adresse, unter der Teammitglieder das Office erreichen (für Tunnel-Befehl und Links) |
| `OFFICE_TAILSCALE_HOST` | Name des Office im Tailnet |

Die Health-Prüfung für Skripte ist `GET /api/gesundheit` (oder `/api/health`), öffentlich, Antwort `{"ok":true}`.

> Stand: Die Flags werden vom Server-Crate (`crates/server`) festgelegt; `open-agentic-office --hilfe` zeigt immer die tatsächlich gebauten.

## Client bauen

```bash
cd crates/client
trunk build --release     # Ausgabe nach dist/ im Repo-Wurzelordner
trunk serve               # für die Entwicklung, mit Neuladen
```

Der Server liefert `dist/` aus (`--client` / `OFFICE_CLIENT` überschreibt den Ort).
