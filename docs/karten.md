# Karten

Zurück zur [README](../README.md) · [Funktionen](features.md).

Unter **⚙️ Einstellungen** → *Karte* verwandelt jeder das ganze Gebäude für alle Etagen in eine andere Halle und zurück: das **Büro**, die **Burg** (Steinhalle mit Thron, siehe [Funktionen](features.md)) oder eine **eigene Karte**.

## Eigene Karten aus JSON

Eine Datei `<datenordner>/karten/<id>.json` erscheint in der Liste (sie wird beim Öffnen und Wählen neu gelesen). Eine Karte baut auf der Burg auf und nennt nur ihre **Änderungen**:

- Objekte werden Schlüssel für Schlüssel zusammengeführt,
- Listen ersetzen die der Burg,
- `null` nimmt Optionales weg (`"thron": null` heißt: keine Schlange und keine Hand des Königs).

| Feld | Inhalt |
| --- | --- |
| `name` | Anzeigename in der Liste |
| `breite`, `laenge`, `hoehe` | Maße der Halle in Metern |
| `tafeln` | Liste von `{x, z, laenge, plaetze}`: die langen Tische mit Arbeitsplätzen |
| `pulte` | `{issues, queue, pulls}`, je `{x, z, blick}`: die Pulte der Tafel-Agenten |
| `rat` | `{x, z}`: der Ratstisch (Besprechungen) |
| `wandtafeln` | `{issues, queue, pulls, dienste}`, je `{x, y, z, blick, breite, hoehe}` |
| `thron` | `{x, z}` |
| `ankunft` | `{x, z, blick}`: wo man ankommt |
| `kerker` | `true` oder `false` |
| `farben` | `{stein, boden, teppich, holz, gold}` |

Regeln, damit die Karte lädt: Die Halle misst 12–200 × 20–300 × 5–40 m; höchstens 40 Tafeln mit höchstens 12 Plätzen je Seite und zusammen mindestens 32 Plätze; alles liegt innerhalb der Wände; mit Kerker mindestens 24 × 40 m. Fehler erscheinen auf Deutsch.

Ein kommentiertes Beispiel steht in `crates/welt/src/grundriss/halle.rs`; getestet wird es in `crates/ende-zu-ende/tests/welt.rs::eigene_karte_aus_json`.
