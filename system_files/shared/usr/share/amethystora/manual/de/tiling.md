# Fenster kacheln

PaperWM ordnet die Fenster für Sie an. Jede Arbeitsfläche ist ein Streifen aus Fenstern, die in voller
Höhe nebeneinanderstehen, und der Streifen kann breiter sein als der Bildschirm. Ein neues Fenster öffnet
sich rechts von dem, in dem Sie sich befinden, Fenster überlappen sich nie, und der Streifen scrollt so,
dass das fokussierte Fenster sichtbar bleibt:

```
  ┌────────┬──────────────┬──────────────────────┬────────┐
  │  Mail  │   Terminal   │       Browser        │  Chat  │
  └────────┴──────────────┴──────────────────────┴────────┘
           ╰──────────── the screen ─────────────╯
```

Mail und Chat sind weiterhin offen, nur jenseits des Bildschirmrands. `Super+Left` scrollt zurück zu Mail.
Schieben Sie den Mauszeiger gegen den linken oder rechten Bildschirmrand, um zu sehen, was dahinter
liegt, und klicken Sie dort auf ein Fenster, um zu ihm zu wechseln. Auf einem Touchpad wischen Sie mit
drei Fingern, um den Streifen zu verschieben.

Die Tasten folgen einem Muster. `Super` und eine Pfeiltaste wechselt zu einem anderen Fenster. Nehmen Sie
`Ctrl` hinzu, und stattdessen bewegt sich das Fenster. Nehmen Sie `Shift` hinzu, und Sie wechseln zu
einem anderen Monitor.

## Sich bewegen

| Taste | Funktion |
| --- | --- |
| `Super+Left` / `Super+Right` | Das Fenster links / rechts |
| `Super+Up` / `Super+Down` | Das Fenster darüber / darunter, wenn sich Fenster eine Spalte teilen |
| `Super+Home` / `Super+End` | Das erste / letzte Fenster auf der Arbeitsfläche |
| `Super+[` / `Super+]` | Den Streifen verschieben, ohne das fokussierte Fenster zu wechseln |
| `Super+Shift` und eine Pfeiltaste | Der Monitor in dieser Richtung |

## Anordnen

| Taste | Funktion |
| --- | --- |
| `Super+Ctrl+Left` / `Super+Ctrl+Right` | Das Fenster entlang des Streifens nach links / rechts verschieben |
| `Super+Ctrl+Up` / `Super+Ctrl+Down` | Es in seiner Spalte nach oben / unten verschieben |
| `Super+I` | Das Fenster rechts in diese Spalte holen, unter dieses |
| `Super+O` | Das unterste Fenster dieser Spalte in eine eigene Spalte hinausschieben |
| `Super+Shift+O` | Dieses Fenster in eine eigene Spalte hinausschieben |
| `Super+Ctrl+Page Up` / `Page Down` | Es auf die vorherige / nächste Arbeitsfläche verschieben |
| `Super+Shift+Ctrl` und eine Pfeiltaste | Es auf den Monitor in dieser Richtung verschieben |
| `Super+T`, `Super` gedrückt halten | Das Fenster beim Bewegen mitnehmen und dort ablegen, wo Sie loslassen |

## Größe

| Taste | Funktion |
| --- | --- |
| `Super+R` | Die Breite schrittweise auf 38 %, 50 % und 62 % des Bildschirms setzen (`Super+Alt+R` geht zurück) |
| `Super+Shift+R` | Dasselbe für die Höhe eines Fensters in einer Spalte |
| `Super+F` | Die Bildschirmbreite ausfüllen, und zurück |
| `Super+C` | Das Fenster auf dem Bildschirm zentrieren |
| `Super+Shift+C` | Wohin der Streifen ein fokussiertes Fenster scrollt: gerade in Sicht, in die Mitte oder an den Rand |
| `Super+Shift+W` | Wo sich neue Fenster öffnen: rechts, links oder unter diesem |

## Schwebende Fenster

Dialoge schweben von selbst über dem Streifen. `Super+Ctrl+Escape` hebt das fokussierte Fenster aus dem
Streifen, sodass es ebenfalls schwebt, und setzt es wieder zurück. `Super+Alt+Escape` blendet alle auf
diese Weise schwebenden Fenster auf einmal aus und holt sie wieder zurück.

## Und der Rest

| Taste | Funktion |
| --- | --- |
| `Super+N` | Ein neues Fenster der App, in der Sie sich befinden |
| `Super+Ctrl+B` | Die obere Leiste auf dieser Arbeitsfläche aus- oder einblenden |

Die Einstellungen von PaperWM finden Sie unter **PaperWM** in der **Erweiterungsverwaltung**. Dazu gehören
die Abstände, die Breiten, durch die `Super+R` schaltet, und jede Taste, auch einige, die diese Seite
auslässt.

## Klassische Fenster

Wenn Sie lieber Fenster hätten, die frei schweben und sich überlappen, wie unter Windows und macOS,
wechseln Sie zum klassischen Layout:

```bash
ame desktop layout classic
```

Das schaltet PaperWM aus, und Aktualisierungen lassen es ausgeschaltet. Fenster öffnen sich dann dort, wo
GNOME sie platziert, und Sie ziehen sie an der Titelleiste. `Super+Up` maximiert und `Super+Down` stellt
wieder her, `Super+Left` / `Super+Right` legen ein Fenster auf diese Bildschirmhälfte, und
`Super+Shift+Left` / `Super+Shift+Right` verschieben es auf den nächsten Monitor.
`ame desktop layout tiling` bringt den Streifen zurück. Bei Ihrer ersten Anmeldung bietet eine
Benachrichtigung dieselbe Wahl an.
