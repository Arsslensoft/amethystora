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
die auf den Neustart wartet, und die, die für ein [Rollback](#rolling-back) aufbewahrt wird, und schaltet
automatische Aktualisierungen aus oder wieder ein.

Wenn Sie [Container](software.md#keeping-them-up-to-date) von `amepkg` haben, aktualisiert
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

## Rollback

Das vorherige System wird immer aufbewahrt. Wenn eine Aktualisierung ein Problem mitbringt:

- **Im Bootmenü** wählen Sie den zweiten Eintrag, das System vor der Aktualisierung. Nichts wird
  geändert, und der nächste Neustart kehrt zum neuesten zurück. Eine Benachrichtigung bei der Anmeldung
  sagt, dass Sie die vorherige Version verwenden, und ein Klick darauf öffnet Systemaktualisierungen, das
  Ihnen **Diese Version behalten** anbietet, was sie zu der Version macht, die der Rechner startet, und
  nach Ihrem Passwort fragt, oder **Mit der neuesten neu starten**.
- **Um beim vorherigen System zu bleiben**, machen Sie es in einem Terminal zur Vorgabe und starten neu:

```bash
sudo bootc rollback
systemctl reboot
```

Ihre Dateien und Einstellungen sind in beiden dieselben. Die nächste Aktualisierung bringt die neueste
Version zurück, ob automatisch oder nicht; um also zu bleiben, wo Sie sind, bis eine Korrektur erscheint,
schalten Sie in Systemaktualisierungen die automatischen Aktualisierungen aus oder
[bleiben Sie bei dem Build](#holding-on-to-one-build), auf dem Sie sind.
[Melden Sie dann das Problem](troubleshooting.md#reporting-a-problem), und folgen Sie wieder dem Kanal,
sobald ein korrigiertes Abbild erscheint.

## Kanäle

| Tag | Was Sie bekommen |
| --- | --- |
| `stable` | Jede Woche gebaut, mit dem Kernel auf dem Stand von Fedora CoreOS, der dem von Fedora etwas hinterherhinkt, damit Kernel-Regressionen vorher auffallen. Für alle. |
| `stable-daily` | Dasselbe, aber neu gebaut, sobald sich Amethystora ändert, statt wöchentlich |
| `latest` | Das neueste Fedora und der neueste Kernel, neu gebaut, sobald sich Amethystora ändert |
| `beta` | Was als Nächstes kommt. Rechnen Sie mit Ecken und Kanten. |

## Kanäle und Abbilder wechseln

Um zu einem anderen Kanal oder Abbild zu wechseln, wechseln Sie dorthin und starten neu:

```bash
sudo bootc switch --enforce-container-sigpolicy ghcr.io/arsslensoft/amethystora:latest
```

Ersetzen Sie `amethystora` durch `amethystora-dx` für den [Entwicklermodus](developer.md) und `latest`
durch den gewünschten Kanal.

Wenn Sie [überlagerte Pakete](software.md#layering-the-last-resort) haben, wechseln Sie stattdessen mit
`rpm-ostree`, weil `bootc switch` das neue System allein aus dem Abbild baut und sie fallen lassen würde:

```bash
sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/amethystora:latest
```

## Bei einem Build bleiben

Jeder Build hat auch ein eigenes Tag, etwa `stable-44.20260922`, nach der Version in den
Versionshinweisen. Wenn Sie zu ihm wechseln, bleibt der Rechner auf diesem Build, was nützlich ist,
während Sie auf eine Korrektur warten. Ein Rechner auf einem einzelnen Build erhält keine Aktualisierungen;
`ame security status` erinnert Sie daran und gibt den Befehl aus, mit dem Sie wieder dem Kanal folgen.

## Signierte Aktualisierungen

Eine Aktualisierung wird nur angenommen, wenn sie die Signatur von Amethystora trägt, geprüft gegen den
Schlüssel in `/etc/pki/containers/amethystora.pub`. Ein manipuliertes Abbild oder eines von jemand anderem
wird abgelehnt statt installiert. Ein Dienst prüft bei jedem Start, ob die Signaturprüfung noch
eingeschaltet ist, und schaltet sie wieder ein, falls ein Wechsel sie ausgeschaltet hat.
