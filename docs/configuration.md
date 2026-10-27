# Konfiguration

Zurück zur [README](../README.md).

## Wo das Office seine Sachen ablegt

Das Office legt seine Daten im Datenordner `~/open-agentic-office` ab (verschieben mit `--daten` oder `OFFICE_HOME`) und klont Projekte als `<projekte>/<owner>/<repo>`. Um sie woanders zu klonen, etwa nach `~/Workspace`, startest du mit `--projekte` oder `OFFICE_PROJEKTE`. Schon vorhandene Etagen bleiben, wo sie sind; ein Checkout desselben Repos, der schon im neuen Ordner liegt, wird so übernommen.

Im Datenordner:

| Datei / Ordner | Inhalt |
| --- | --- |
| `einstellungen.json` | Server-Einstellungen, auch der Hash des gemeinsamen Passworts |
| `sitzung.key` | Schlüssel, mit dem die Sitzungs-Cookies signiert werden |
| `konten.json` | Konten und offene Einladungen |
| `etagen.json` | Die Liste der Etagen (Checkout, Farben) |
| `etagen/<id>/` | Zustand der Etage: Arbeiter (`workers.json`), Schwarzes Brett, Spielsachen, Jukebox (`jukebox.json`) |
| `homes/<konto>/` | Die eigenen Claude-, GitHub- und Git-Anmeldungen jedes Kontos (das Konto zu entziehen löscht sie) |
| `hook-port` | Port des Hook-Servers vom letzten Start (nur Loopback) |

Das Office startest du in einem Projektordner mit `open-agentic-office ~/code/mein-projekt`: Dieses Projekt wird zur Etage. Worktrees der Arbeiter liegen in `<projekt>/.office/worktrees/<name>-<id>` auf dem Branch `office/<name>-<id>`.

## Kommandozeile

```
open-agentic-office [PROJEKTORDNER] [Optionen]

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
| `OFFICE_HOOK_PORT` | Port des Loopback-Hook-Servers festlegen |
| `OFFICE_SELBSTUPDATE=1` | Das Office aktualisiert sich selbst (kommt später) |

Die Health-Prüfung für Skripte ist `GET /api/gesundheit` (oder `/api/health`), öffentlich, Antwort `{"ok":true}`.

> Stand: Die Flags werden vom Server-Crate (`crates/server`) festgelegt; `open-agentic-office --hilfe` zeigt immer die tatsächlich gebauten.

## Client bauen

```bash
cd crates/client
trunk build --release     # Ausgabe nach dist/ im Repo-Wurzelordner
trunk serve               # für die Entwicklung, mit Neuladen
```

Der Server liefert `dist/` aus (`--client` / `OFFICE_CLIENT` überschreibt den Ort).
