# Willkommen bei Amethystora

Amethystora ist ein Desktop, den Sie einmal einrichten und dann einfach benutzen. Es basiert auf Fedora
und GNOME, aktualisiert sich im Hintergrund selbst, und eine Aktualisierung, die schiefgeht, ist mit
einem Neustart wieder rückgängig gemacht.

Es ist kein Haufen Pakete, den Sie in Ordnung halten müssen. Das gesamte Betriebssystem ist ein
einziges Abbild, das gebaut, getestet und signiert wird, bevor es Ihren Rechner je erreicht, und das als
Ganzes ersetzt wird, wenn ein neues erscheint. Ihre Dateien, Ihre Einstellungen und Ihre Apps liegen
unberührt daneben. Deshalb gibt es nichts aufzuräumen, keine halb fertige Systemaktualisierung, die Sie
retten müssten, und keine Neuinstallation alle paar Jahre.

Auf diesem Fundament steht ein Desktop, der für die Tastatur gemacht ist, so schön wie schnell:
Fenster, die sich in ein Raster fügen, sechs Arbeitsflächen, die immer dort sind, wo Sie sie verlassen
haben, ein Themenwechsel, der alles auf einmal neu einfärbt, und ein KI-Agent, der weiß, wie dieses
System funktioniert.

Zwei Entscheidungen lohnen sich am ersten Tag. Wenn Sie lieber Fenster hätten, die frei schweben und
sich überlappen, wie unter Windows und macOS, bietet eine Benachrichtigung sie kurz nach Ihrer ersten
Anmeldung an, und `ame desktop layout classic` wechselt jederzeit dorthin
([Klassische Fenster](tiling.md#classic-windows)). Wenn der Rechner für ein Kind ist, beschränkt der
[Jugendschutz](logins.md#parental-controls) dessen Konto auf die Apps und Zeiten, die Sie wählen.

## So funktioniert dieses Handbuch

Lesen Sie es beim ersten Mal von vorne bis hinten; jede Seite endet mit einem Link zur nächsten. Danach
suchen Sie einfach: `Ctrl+K` oder `/` springt ins Suchfeld, und die Ergebnisse führen direkt zu dem
Abschnitt, den Sie brauchen.

- **Einstieg** macht einen neuen Rechner startklar: die ersten Schritte und die Installation.
- **Der Desktop** ist alles, was Sie jeden Tag berühren: sich zurechtfinden, die Tastenkürzel, Themen,
  das Terminal, Web-Apps und der Agent.
- **Software** zeigt, wie Sie alles installieren, von Apps bis zu Entwicklerwerkzeugen, ohne das System
  zu beschädigen.
- **Das System** behandelt Aktualisierungen, Sicherheit und Hardware.
- **Hilfe** ist die Anlaufstelle, wenn etwas nicht stimmt.

Diese Seite öffnet sich von selbst, wenn Sie sich zum ersten Mal anmelden, und nur dann. `Super+F1`
öffnet dieses Handbuch von überall aus, ebenso „Manual“ im Amethystora-Menü (`Super+Alt+Space`). In
einem Terminal öffnet `amethystora-manual` es, und `amethystora-manual keybindings` springt direkt zu
einer Seite.

## Was Ihnen gehört und was dem Abbild

Ein einziger Gedanke erklärt das meiste davon, wie Amethystora sich verhält:

| Wo | Wem | Was damit geschieht |
| --- | --- | --- |
| `/usr` | Dem Abbild | Schreibgeschützt. Wird mit jeder Aktualisierung vollständig ersetzt. |
| `/etc` | Beiden | Die Vorgaben des Abbilds, bei jeder Aktualisierung mit Ihren Änderungen zusammengeführt. |
| `/var` und `/home` | Ihnen | Wird von keiner Aktualisierung angerührt. |
| `~/.config`, `~/.local` | Ihnen | Ihre Einstellungen, pro Konto. |

Apps kommen von Flathub, Befehlszeilenwerkzeuge von Homebrew, und Entwicklungsumgebungen leben in
Containern. Keines davon verändert das Abbild, weshalb eine Aktualisierung sie nie beschädigen kann und
sie nie eine Aktualisierung beschädigen können. [Software installieren](software.md) erklärt jedes davon.

> **Kurz gesagt:** Halten Sie das Betriebssystem unberührt, Ihre Arbeit daneben, und starten Sie neu,
> wenn Sie für heute fertig sind. Das ist die ganze Wartung.
