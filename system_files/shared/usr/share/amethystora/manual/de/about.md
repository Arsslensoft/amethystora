# Über Amethystora

Amethystora wird von Arsslen Idadi ([@arsslensoft](https://github.com/arsslensoft)) erstellt und gepflegt. Es
ist freie Software unter der Apache License 2.0 und wird offen entwickelt auf
[github.com/arsslensoft/amethystora](https://github.com/arsslensoft/amethystora). Fehlermeldungen, Ideen und
Pull-Requests sind dort willkommen.

Amethystora basiert auf [Universal Blue](https://universal-blue.org), Copyright der Mitwirkenden von Universal Blue, lizenziert unter der Apache License 2.0.

## Dieses Handbuch

Die Seiten sind Markdown-Dateien in `/usr/share/amethystora/manual`, eine pro Seite, und die Seite mit den
Tastenkürzeln ist `/usr/share/amethystora/keybindings.md`, dieselbe Datei, die `ame desktop keybindings`
anzeigt. Sie kommen mit dem Abbild, sodass das Handbuch immer das System beschreibt, das Sie verwenden,
und es funktioniert ohne Netzwerkverbindung.

```bash
amethystora-manual                 # open it where you left off
amethystora-manual updates         # open a page by its file name
amethystora-manual --list          # the pages there are
```

Im Handbuch: `Ctrl+K` oder `/` sucht, `Alt+Left` und `Alt+Right` gehen zurück und vorwärts, `Ctrl+=` und
`Ctrl+-` ändern die Textgröße, und `Ctrl+0` setzt sie zurück.

## Auf den Schultern von Riesen

Amethystora basiert auf [Fedora](https://fedoraproject.org) und [GNOME](https://www.gnome.org) und auf der
Arbeit vieler anderer Projekte: dem Linux-Kernel und systemd, bootc und rpm-ostree, Flatpak und Flathub,
Homebrew, Distrobox und Podman sowie jeder GNOME-Erweiterung, die unter
[Sich zurechtfinden](desktop.md#extensions) aufgeführt ist. Die Symbole sind
[candy-icons](https://github.com/EliverLara/candy-icons) von Eliver Lara, und das GTK-Thema von
Amethystora ist aus seinem Sweet umgefärbt. Die Hintergrundbilder der Themen wurden für Amethystora
gezeichnet.

Fedora und das Fedora-Logo sind Marken von Red Hat, Inc. Amethystora wird nicht vom Fedora-Projekt oder
von Red Hat bereitgestellt, unterstützt oder befürwortet. Fedora selbst finden Sie unter
[fedoraproject.org](https://fedoraproject.org).

Die Lizenzen von allem im Abbild liegen in `/usr/share/licenses`. Einige Teile sind keine freie Software
und unterliegen den eigenen Bedingungen ihrer Hersteller: Gerätefirmware, der DisplayLink-Treiber für
Docks und der NVIDIA-Treiber auf den NVIDIA-Abbildern. `/usr/share/licenses/amethystora/NOTICE` listet sie
auf.

## Quellcode

Jedes Paket im Abbild ist in `/usr/share/licenses/amethystora/SOURCES` aufgeführt, mit seiner Lizenz, dem
Quellpaket, aus dem es gebaut wurde, und dem Ort dieser Quelle. Die Quelle eines Fedora-Pakets liegt im
Build-System von Fedora unter der dort angegebenen Adresse, oder einen Befehl entfernt:

```bash
dnf download --srpm <package>    # the source of the version Fedora has now
```

Einige Pakete werden von anderen gebaut (RPM Fusion, negativo17, Fedora Copr), die jeweils nur ihren
aktuellen Build veröffentlichen. Wo die Lizenz eines solchen Pakets die Herausgabe der Quelle verlangt,
liegt das Quellpaket, aus dem dieses Abbild gebaut wurde, im Abbild selbst, in `/usr/src/amethystora`.

Alles andere wird aus [dem Repository von Amethystora](https://github.com/arsslensoft/amethystora) gebaut, bei
dem Commit, der als `BUILD_ID` in `/usr/lib/os-release` genannt ist. Mindestens drei Jahre nach der letzten
Veröffentlichung eines Abbilds wird der vollständige Quellcode von allem darin, dessen Lizenz die
Bereitstellung des Quellcodes verlangt (die GNU GPL und LGPL, die MPL, die CDDL, die Lizenz des
Fraunhofer-AAC-Codecs), außerdem jedem zur Verfügung gestellt, der über den
[Issue-Tracker](https://github.com/arsslensoft/amethystora/issues) danach fragt; das schriftliche Angebot
steht in `/usr/share/licenses/amethystora/NOTICE`.
