# Installation

Laden Sie Amethystora herunter, schreiben Sie es auf einen USB-Stick, starten Sie den Rechner vom Stick
und probieren Sie es aus: Auf dem Rechner ändert sich nichts, bis Sie sich für die Installation
entscheiden. Die Installation dauert etwa zehn Minuten und braucht kein Netzwerk.

## Herunterladen

| Download | Was es ist |
| --- | --- |
| [amethystora.iso](https://download.amethystora.org/amethystora.iso) | Amethystora auf einem Stick: ausprobieren und dann von dort installieren. Beginnen Sie mit diesem. |
| [amethystora-non-uefi.iso](https://download.amethystora.org/amethystora-non-uefi.iso) | Nur das Installationsprogramm, das auch Rechner startet, die zu alt für UEFI sind |

Jedes ist mehrere Gigabyte groß. Neben jedem ISO liegt seine Prüfsumme, `.sha256`, signiert mit dem
Schlüssel, gegen den jede Aktualisierung geprüft wird. So prüfen Sie, ob das Heruntergeladene dem
entspricht, was gebaut wurde:

```bash
curl -LO https://raw.githubusercontent.com/arsslensoft/amethystora/main/cosign.pub
curl -LO https://download.amethystora.org/amethystora.iso.sha256
curl -LO https://download.amethystora.org/amethystora.iso.sha256.bundle
cosign verify-blob --key cosign.pub --bundle amethystora.iso.sha256.bundle amethystora.iso.sha256
sha256sum -c amethystora.iso.sha256
```

Unter Windows gibt `certutil -hashfile amethystora.iso SHA256` die Prüfsumme aus, die Sie mit der in
`amethystora.iso.sha256` vergleichen.

Auf einem Rechner mit einer NVIDIA-Grafikkarte installieren Sie von einem der beiden: Eine
Benachrichtigung nach dem ersten Start bietet das Abbild mit NVIDIAs Treiber an
([Hardware](hardware.md#graphics)).

## Auf einen USB-Stick schreiben

Verwenden Sie einen Stick mit 16 GB oder mehr: Alles darauf wird gelöscht.

- **Unter Windows oder macOS:** Fedora Media Writer (wählen Sie **.iso-Datei auswählen**) oder
  balenaEtcher. Unter Windows funktioniert auch Rufus, im DD-Image-Modus.
- **Unter Linux:** Impression oder `dd`:

  ```bash
  sudo dd if=amethystora.iso of=/dev/sdX bs=4M status=progress oflag=sync
  ```

  wobei `/dev/sdX` der Stick ist, so wie `lsblk` ihn auflistet. Wenn Sie sich hier vertun, löscht `dd`
  eine andere Festplatte.

Ein mit Ventoy erstellter Stick startet das ISO nur im GRUB2-Modus von Ventoy.

## Ausprobieren

Stecken Sie den Stick ein und starten Sie den Rechner davon: Drücken Sie beim Start seine Bootmenü-Taste
(F12, F11, F10, F8 oder Esc bei den meisten PCs; auf einem Intel-Mac halten Sie Option gedrückt) und
wählen Sie den Stick. Secure Boot kann eingeschaltet bleiben. Wählen Sie **Try or install Amethystora**
oder den **basic graphics mode**, falls der Bildschirm schwarz bleibt.

Amethystora startet so, wie es installiert sein wird, bereits angemeldet. WLAN, Ton, Bildschirm und die
übrige Hardware zeigen, ob dieser Rechner dazu passt. Nichts wird auf die Festplatte des Rechners
geschrieben, und was Sie tun, ist verschwunden, sobald er herunterfährt. Die Apps von Flathub sind nicht
auf dem Stick: Sie werden beim ersten Start des installierten Systems installiert.

Das Live-ISO startet nur Rechner mit UEFI-Firmware, die fast jeder Rechner seit 2012 hat. Für einen
älteren verwenden Sie das Installationsprogramm allein.

## Installieren

Wählen Sie **Install to Hard Drive** im Dash. Das Installationsprogramm fragt nach Ihrer Sprache und der
Festplatte und macht sich dann an die Arbeit.

- **Wählen Sie die Festplattenverschlüsselung.** Sie lässt sich nachträglich nicht einschalten, und sie
  schützt Ihre Dateien, wenn der Rechner verloren geht oder gestohlen wird. Die Passphrase wird bei
  jedem Start abgefragt, bis [das TPM die Festplatte entsperrt](security.md#unlock-the-disk-with-the-tpm),
  sofern Sie das möchten.
- **Um Windows zu behalten,** lesen Sie zuerst [Neben Windows](#next-to-windows).

Wenn es fertig ist, starten Sie neu und ziehen den Stick ab. Der erste Start fragt nach Ihrer Sprache,
Tastatur, Ihrem Konto und Passwort, und die Apps installieren sich von selbst, sobald der Rechner online
ist. Gehen Sie dann die [Ersten Schritte](getting-started.md) durch.

## Neben Windows

Amethystora installiert sich neben Windows, und ein Menü bei jedem Start lässt Sie eines von beiden
wählen. Erledigen Sie in Windows vor der Installation Folgendes:

1. **Bewahren Sie Ihren BitLocker-Wiederherstellungsschlüssel auf.** Die meisten Laptops mit Windows 11
   verschlüsseln ihre Festplatte mit BitLocker, auch Geräteverschlüsselung genannt. Sie finden den
   Schlüssel unter **Einstellungen › Datenschutz und Sicherheit › Geräteverschlüsselung** oder unter
   [aka.ms/myrecoverykey](https://aka.ms/myrecoverykey); bewahren Sie ihn an einem anderen Ort als auf
   diesem Rechner auf: Die Installation fügt dem Bootmenü des Rechners einen Eintrag hinzu, und Windows
   fragt danach möglicherweise einmal nach dem Schlüssel.
2. **Schaffen Sie Platz.** Klicken Sie mit der rechten Maustaste auf **Start**, öffnen Sie die
   **Datenträgerverwaltung**, klicken Sie mit der rechten Maustaste auf die Windows-Partition (C:) und
   wählen Sie **Volume verkleinern**. Lassen Sie mindestens 64 GB für Amethystora frei, mehr, wenn Sie
   Ihre Dateien dort aufbewahren werden.
3. **Schalten Sie den Schnellstart aus:** **Systemsteuerung › Energieoptionen › Auswählen, was beim
   Drücken von Netzschaltern geschehen soll**, und deaktivieren Sie **Schnellstart aktivieren**. Ist er
   eingeschaltet, fährt Windows nur halb herunter und hält seine Festplatte gesperrt.

Installieren Sie dann im Installationsprogramm in den freien Platz, den Sie geschaffen haben, nie auf die
ganze Festplatte. Lassen Sie Secure Boot eingeschaltet: Windows 11 braucht es, und Amethystora startet
damit. Wenn die Uhr von Windows ein paar Stunden falsch geht, nachdem Sie Amethystora benutzt haben,
schalten Sie in den Datums- und Uhrzeiteinstellungen von Windows **Uhrzeit automatisch festlegen** ein.

## Von einem anderen Fedora-Atomic-Desktop

Ein Rechner, auf dem bereits Fedora Silverblue oder ein anderer als Abbild auf Fedora gebauter Desktop
läuft, wechselt ohne Neuinstallation zu Amethystora. Der Wechsel ersetzt das Abbild des
Betriebssystems und behält alles in `/home`, `/etc` und `/var`.

- **Sichern Sie** alles, was Sie nicht verlieren dürfen. Der Wechsel ist sicher, aber eine Sicherung ist
  es immer.
- Pakete, die Sie mit `rpm-ostree install` über das alte System gelegt haben, kommen nicht mit.
  Flatpaks, Homebrew und Container schon.

```bash
sudo bootc switch ghcr.io/arsslensoft/amethystora:stable
```

Starten Sie dann neu. Beim ersten Start bereitet ein Dienst von selbst den Wechsel zu signaturgeprüften
Aktualisierungen vor, und ab dem Neustart danach wird eine Aktualisierung nur angenommen, wenn sie die
Signatur von Amethystora trägt.

## Ein Abbild wählen

| Abbild | Für |
| --- | --- |
| `ghcr.io/arsslensoft/amethystora` | Den Standard-Desktop |
| `ghcr.io/arsslensoft/amethystora-dx` | Entwickler: bringt zusätzlich Docker, Incus, libvirt, VSCodium und mehr ([Entwicklermodus](developer.md)) |
| `ghcr.io/arsslensoft/amethystora-nvidia-open` | Den Standard-Desktop mit NVIDIAs offenem Kernel-Treiber |
| `ghcr.io/arsslensoft/amethystora-dx-nvidia-open` | Den Entwicklermodus mit NVIDIAs offenem Kernel-Treiber |

Die ISOs installieren die Abbilder ohne NVIDIAs Treiber. Auf einem Rechner mit einer NVIDIA-Karte bietet
der erste Start an, zum passenden NVIDIA-Abbild zu wechseln ([Hardware](hardware.md#graphics)).

## Einen Kanal wählen

Jedes Abbild gibt es in drei Kanälen, festgelegt durch das Tag nach dem Doppelpunkt:

| Tag | Für |
| --- | --- |
| `stable` | Alle. Wöchentlich gebaut, mit dem Kernel auf dem Stand von Fedora CoreOS, der dem von Fedora etwas hinterherhinkt, damit Kernel-Regressionen vorher auffallen. |
| `latest` | Das Neueste, was Fedora hat, neu gebaut, sobald sich Amethystora ändert. |
| `beta` | Zum Testen dessen, was als Nächstes kommt. Rechnen Sie mit Ecken und Kanten. |

Die ISOs installieren `stable`. Sie können jederzeit zwischen Abbildern und Kanälen wechseln;
[Aktualisierungen](updates.md#switching-streams-and-images) zeigt, wie.

## Nach dem ersten Start

Gehen Sie die [Ersten Schritte](getting-started.md) durch. Der eine Schritt, den Sie nicht überspringen
sollten, ist die Registrierung des Secure-Boot-Schlüssels, falls Secure Boot eingeschaltet ist.
