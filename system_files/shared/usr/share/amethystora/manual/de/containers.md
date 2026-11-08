# Container

Für alles, was eine klassische Distribution erwartet (eine Compiler-Toolchain, ein `.deb`, das ein
Hersteller ausliefert, ein Paket, das es nur im Arch User Repository gibt, ein Werkzeug nur für Ubuntu),
erstellen Sie einen Container. Er teilt Ihren persönlichen Ordner und Ihre Anzeige, sodass Apps darin
Fenster öffnen wie jede andere, und er lässt sich spurlos wegwerfen. Nichts, was in einem installiert ist,
berührt das System.

> **Ein Container ist keine Sandbox.** Was Sie in einem installieren, läuft als Sie, mit Ihrem
> persönlichen Ordner, Ihrer Anzeige, Ihrem Ton und Ihrem Sitzungsbus. Wenn eine App auf Flathub ist,
> holen Sie sie von dort: Flatpak-Apps laufen in einer Sandbox, sind signiert und werden mit dem System
> aktualisiert.

## Pakete aus anderen Distributionen

`amepkg`, kurz für `amethystora-pkg`, erstellt Container aus Vorlagen (Debian, Ubuntu, Fedora, Arch,
Alpine, openSUSE Tumbleweed und openSUSE Leap) und installiert mit deren eigenen Paketverwaltungen hinein:

```bash
amepkg containers new --template debian
amepkg debian install gimp
amepkg debian enter
```

Die erste Zeile lädt das Abbild herunter, das die Vorlage nennt, erstellt den Container und bringt ihn
auf den neuesten Stand. `install` installiert mit `apt` aus Debians eigenen signierten Paketquellen und
legt jede App, die das Paket mitbringt, in die Anwendungsübersicht, markiert mit „(on debian)“; `remove`
nimmt sie wieder heraus. Bevor `amepkg` eine App installiert, sucht es dieselbe App auf Flathub und bietet
diese zuerst an.

| Befehl | Funktion |
| --- | --- |
| `amepkg containers list` | Die Container, ihre Vorlagen, wann jeder zuletzt aktualisiert wurde, ihre Apps |
| `amepkg containers new` | Ein neuer Container: fragt nach der Vorlage und dem Namen |
| `amepkg containers rm <name>` | Löscht den Container und alles, was darin installiert ist |
| `amepkg containers reset <name>` | Löscht ihn und erstellt ihn aus seiner Vorlage neu; sein persönlicher Ordner bleibt |
| `amepkg <name> install`, `remove` oder `purge <package>` | Installiert oder entfernt Pakete |
| `amepkg <name> search <words>`, `show <package>` oder `list` | Durchsucht seine Paketquellen oder das Installierte |
| `amepkg <name> update`, `upgrade`, `autoremove` oder `clean` | Frischt seine Paketlisten auf, aktualisiert ihn, räumt auf |
| `amepkg <name> enter` oder `run <command>` | Eine Shell darin oder ein einzelner Befehl |
| `amepkg <name> start` oder `stop` | Startet oder stoppt ihn |
| `amepkg <name> export --app <app>` oder `--bin <path>` | Legt eine App in die Anwendungsübersicht oder einen Befehl in `~/.local/bin`; `unexport` nimmt das zurück |

Es listet und berührt nur die Container, die es selbst erstellt hat: Ihre eigenen Distrobox-Container
bleiben unberührt. Unter Arch frischt `update` nur die Paketlisten auf, und Arch unterstützt keine
Installation aus Listen, die neuer sind als die bereits vorhandenen Pakete: `upgrade`, das sie zuerst
auffrischt, ist der richtige Befehl.

## Eine Paketdatei

Doppelklicken Sie in Dateien auf ein `.deb` oder `.rpm`, oder in einem Terminal:

```bash
amepkg install ~/Downloads/some-app.deb
```

Ein `.deb` kommt in einen Debian-Container, ein `.rpm` in einen Fedora-Container und ein `.pkg.tar.zst` in
einen Arch-Container, der aus seiner Vorlage erstellt wird, falls es noch keinen gibt. Zuerst wird die
Datei auf Viren geprüft, und nichts wird installiert, wenn sie bekannter Schadsoftware entspricht. Dann
wird angezeigt, was die Datei über sich selbst sagt: ihren Namen und ihre Version, wer sie erstellt hat,
ob sie signiert ist und ob sie bei der Installation eigene Skripte ausführt. Ein `.rpm` kann eine Signatur
tragen; ein `.deb` normalerweise nicht, weil Debian seine Paketquellen signiert statt jeder einzelnen
Datei, sodass ein `.deb` von einer Website keinen Nachweis trägt, wer es erstellt hat. Wenn dieselbe App
auf Flathub ist oder das Paket in den eigenen Paketquellen des Containers liegt, werden diese zuerst
angeboten: Beide sind signiert und werden aktuell gehalten, eine Datei von einer Website ist weder das eine
noch das andere.

Ein aus einer Datei installiertes Paket wird mit seinem Container nur dann aktualisiert, wenn die
Paketquelle seines Herstellers mitkommt, wie sie manche Herstellerpakete bei der Installation einrichten.

## Aktuell halten

Jeder Container wird einmal täglich von einem Timer Ihres eigenen Kontos aktualisiert
(`amethystora-pkg-upgrade.timer`). Wie die automatischen Aktualisierungen des Systems wartet er, solange
der Rechner mit schwachem Akku läuft, ausgelastet ist oder wenig Speicher hat. **Jetzt aktualisieren** in
[Systemaktualisierungen](updates.md#updating-now) aktualisiert sie ebenfalls, direkt nach dem System, und
zeigt unter **Container**, wie es lief: Ein Container, der sich nicht aktualisieren ließ, macht die
Aktualisierung des Systems nie zu einer fehlgeschlagenen. In einem Terminal:

```bash
amepkg upgrade-all
```

Der [Sicherheitsbericht](security.md#see-where-you-stand) meldet es, wenn ein Container zwei Wochen ohne
Aktualisierung geblieben ist, und die Seite **Container** der Sicherheits-App listet auf, was in jedem
installiert ist.

## Vorlagen und Paketverwaltungen

Eine Vorlage nennt das Abbild, mit dem ein Container beginnt, die Paketverwaltung, mit der er bedient
wird, und die Pakete, mit denen jeder neue Container startet. Eine Paketverwaltung besteht aus den zehn
Befehlen, die `amepkg` für sie ausführt, von `install` bis `clean`. Erstellen Sie Ihre eigenen:

```bash
amepkg templates new --name devbox --base quay.io/toolbx/ubuntu-toolbox:24.04 \
    --pkg-manager apt --packages "build-essential git"
amepkg templates list
amepkg managers list
```

Eine Vorlage kann ihren Containern einen eigenen persönlichen Ordner geben (`--own-home`), was deren
Einstellungen von Ihren getrennt hält, und die `--unshare`-Optionen von distrobox
(`--unshare netns,ipc`), die zu Befehlszeilenwerkzeugen passen; eine App mit Fenster braucht meist, was
diese wegnehmen. Ein eigener persönlicher Ordner versteckt Ihre Dateien nicht: Sie bleiben über ihren
vollständigen Pfad erreichbar. Ihre eigenen Vorlagen liegen in `~/.local/share/amethystora/pkg`, und die
eingebauten lassen sich weder ändern noch entfernen. `export` schreibt eine in eine Datei, und `import`
liest sie zurück, hier oder auf einem anderen Rechner.
`import` liest auch die YAML-Dateien, in denen Apx seine Stacks und
Paketverwaltungen beschreibt.

Die Abbilder der eingebauten Vorlagen sind auf einen Digest festgelegt, der im signierten Systemabbild
ausgeliefert wird, sodass ein Container genau mit dem Abbild beginnt, mit dem Amethystora gebaut und
getestet wurde, und die Abbilder von Debian, Alpine, openSUSE Tumbleweed und openSUSE Leap werden
zusätzlich gegen die Signaturen ihrer Herausgeber geprüft: das von Leap gegen openSUSEs eigene Schlüssel,
die auch jedes Leap-Abbild prüfen, das Sie selbst herunterladen. Das Abbild einer eigenen Vorlage wird so
übernommen, wie seine Registry es ausliefert: `templates list` markiert es als ungeprüft, und das Erstellen
eines Containers daraus fragt zuerst nach, es sei denn, es ist mit `@sha256:…` festgelegt oder stammt aus
einer Registry, deren Signaturen dieser Rechner prüft.

## Das AUR

Das Arch User Repository ist ausgeschaltet. Schalten Sie es ein mit:

```bash
ame apps aur
amepkg containers new --template arch-aur
```

Das fügt die Vorlage `arch-aur` hinzu, deren Container einen eigenen persönlichen Ordner bekommen, und die
Paketverwaltung `paru`, die beim ersten Mal im Container aus dem AUR gebaut wird. Jeder kann ins AUR
hochladen, und niemand prüft, was dort liegt, deshalb zeigt `amepkg`, bevor irgendetwas gebaut wird, was
das AUR darüber sagt: wer es betreut, wie viele dafür stimmen, wann es eingereicht und zuletzt
aktualisiert wurde und ob es als veraltet markiert ist, mit einer Warnung bei einem Paket, das jünger als
30 Tage ist oder keine Stimmen hat. paru zeigt Ihnen dann das PKGBUILD, das Skript, das es baut: Lesen Sie
es. Was die eigenen Paketquellen von Arch haben, kommt zuerst von dort. Die tägliche Aktualisierung lässt
in Ruhe, was aus dem AUR kam, weil jede neue Version ein neues Skript zum Lesen ist:
`amepkg <name> upgrade` in einem Terminal aktualisiert auch diese.

## In Skripten

Die Listen geben mit `--json` JSON aus, dessen Felder stabil bleiben:

- `amepkg containers list --json`: `name`, `container` (sein Name in podman), `template`, `manager`,
  `image`, `status`, `home`, `init`, `unshare`, `exported_apps`, `exported_bins`, `packages`,
  `files`, `aur_packages`, `aur_pending`, `created`, `last_upgrade` (beide in Unix-Sekunden) und
  `last_upgrade_result`
- `amepkg templates list --json`: `name`, `description`, `base`, `manager`, `packages`, `own_home`,
  `unshare`, `built_in`, `verified` und `pinned`
- `amepkg managers list --json`: `name`, `need_sudo`, `noconfirm`, die zehn Befehle und `built_in`

`containers new`, `templates new` und `update` sowie `managers new` und `update` fragen in einem Terminal
nach allem, was ihnen nicht übergeben wurde. Mit `--no-prompt` nehmen sie nur Optionen entgegen und
schlagen bei Fehlendem fehl. Skripte, Starter und Dienste rufen es als `amethystora-pkg` auf, mit seinem
vollständigen Namen.

## Container von Hand

Distrobox und Toolbox funktionieren wie überall:

```bash
distrobox create --name ubuntu --image quay.io/toolbx/ubuntu-toolbox:24.04
distrobox enter ubuntu
```

**DistroShelf** zeigt jeden Container, Ihre und die von `amepkg`, in einem Fenster, und das Terminal
öffnet über das Menü neben seinem Knopf für einen neuen Reiter einen Reiter in jedem davon. Eine darin
installierte App lässt sich von innen mit `distrobox-export --app <name>` in die Anwendungsübersicht
legen.
