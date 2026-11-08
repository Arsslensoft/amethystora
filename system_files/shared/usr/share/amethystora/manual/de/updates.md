# Aktualisierungen

Aktualisierungen laufen automatisch und leise. Das System, Ihre Flatpak-Apps und Ihre Homebrew-Werkzeuge
werden im Hintergrund geprüft und heruntergeladen, während Sie arbeiten. Das neue System wird neben dem
laufenden bereitgestellt und übernimmt beim nächsten Neustart, alles auf einmal. Nichts wird je halb
aktualisiert, sodass eine Aktualisierung den Rechner nicht in einem Zustand zurücklassen kann, in dem er
nicht mehr startet.

> **Die eine Gewohnheit, die Sie beibehalten sollten:** Fahren Sie herunter oder starten Sie neu, wenn Sie
> für heute fertig sind. Dann wird eine bereitgestellte Aktualisierung angewendet.

## Jetzt aktualisieren

Öffnen Sie **Systemaktualisierungen** in der Anwendungsübersicht (oder **Check for updates** unter
**System** im Amethystora-Menü) und drücken Sie **Jetzt aktualisieren**. Das aktualisiert das System, Ihre
Flatpak-Apps und Ihre Homebrew-Werkzeuge in einem Durchgang und zeigt jeden Schritt, während er geschieht,
ohne nach einem Passwort zu fragen. Es ist dieselbe Aktualisierung, die auch die automatischen ausführen;
läuft also bereits eine, verfolgt das Fenster diese, statt eine weitere zu starten. Sie können das Fenster
jederzeit schließen; die Aktualisierung läuft weiter.

Ein neues System übernimmt trotzdem erst beim nächsten Neustart. Sobald eines bereitsteht, sagt
Systemaktualisierungen das und bietet den Neustart an. Es zeigt auch die Version, die Sie verwenden, die,
die auf den Neustart wartet, und die, die für ein [Rollback](rolling-back.md) aufbewahrt wird, und schaltet
automatische Aktualisierungen aus oder wieder ein.

Wenn Sie [Container](containers.md#keeping-them-up-to-date) von `amepkg` haben, aktualisiert
**Jetzt aktualisieren** sie direkt nach dem Rest, unter **Container**. Einer, der sich nicht aktualisieren
lässt, meldet das dort, und die Aktualisierung des Systems gilt trotzdem als erledigt.

In einem Terminal dasselbe in einem Durchgang:

```bash
ame update
```

| Befehl | Funktion |
| --- | --- |
| `ame update` | Jetzt alles aktualisieren |
| `amepkg upgrade-all` | Die Container anderer Distributionen jetzt aktualisieren |
| `ame system auto-updates` | Automatische Aktualisierungen ausschalten oder wieder einschalten |
| `ame changelog` | Was sich in den Paketen seit dem Abbild, das Sie verwenden, geändert hat |
| `rpm-ostree status` | Das laufende System, die bereitgestellte Aktualisierung und das System für ein Rollback |
| `fwupdmgr get-updates` | Firmware-Aktualisierungen für diesen Rechner, von seinem Hersteller |

Die Versionshinweise zu jedem stabilen Build stehen auf
[GitHub](https://github.com/arsslensoft/amethystora/releases).

## Firmware

Die Firmware des Rechners und seiner Geräte (die UEFI-Firmware, Docks, SSDs, Tastaturen, der
Fingerabdruckleser) wird getrennt vom System aktualisiert, weil eine Firmware-Aktualisierung einen
Rechner am Netzteil und einen Neustart erfordern kann. Jeden Tag sucht fwupd im LVFS, wo die Hersteller
sie veröffentlichen, nach neuer Firmware, und der Abschnitt **Firmware** in **Aktualisierungen** listet
jedes Gerät auf, für das es welche gibt, von welcher Version auf welche. **Firmware aktualisieren …**
installiert sie in einem Terminal, wo fwupd sagt, was es tun wird, und nachfragt, bevor es den Rechner neu
startet. **Jetzt prüfen** fragt sofort erneut beim LVFS nach. In einem Terminal:

```bash
fwupdmgr get-updates      # what there is
fwupdmgr update           # install it
fwupdmgr security         # how well the firmware protects this machine
```

Firmware für die UEFI-Firmware selbst wird beim nächsten Neustart geschrieben, von der Firmware: Lassen
Sie den Rechner am Netzteil, bis er wieder gestartet ist. Der Sicherheitsbericht zeigt die Bewertung von
fwupd für die Schutzmaßnahmen der Firmware, von HSI:0 bis HSI:5; wie hoch ein Rechner kommen kann, liegt
überwiegend bei seinem Hersteller.
