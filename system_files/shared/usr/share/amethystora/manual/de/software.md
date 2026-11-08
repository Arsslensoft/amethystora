# Software installieren

Hier gibt es kein `dnf install`, und Sie werden es nicht vermissen. Jede Art von Software hat ihren eigenen
Platz, keiner davon verändert das Systemabbild, und deshalb kann keine App je eine Aktualisierung
beschädigen oder von einer beschädigt werden.

| Sie möchten | Verwenden Sie | Wie |
| --- | --- | --- |
| Eine App mit Fenster | Flatpak, von Flathub | Bazaar, das Software-Center |
| Ein Befehlszeilenwerkzeug | Homebrew | `brew install <name>` |
| Eine komplette Linux-Umgebung: `apt`, `dnf`, `pacman`, eine Toolchain | Einen Container | `amepkg` ([Container](containers.md)) |
| Ein `.deb` oder `.rpm` von einer Website | Einen eigens dafür erstellten Container | In Dateien öffnen ([Eine Paketdatei](containers.md#a-package-file)) |
| Eine Website als App | Eine Web-App | `amethystora-webapp install` ([Web-Apps](web-apps.md)) |
| Etwas, das auf dem Host selbst sein muss | Überlagern | `rpm-ostree install`, als letztes Mittel |

## Apps: Bazaar und Flathub

Öffnen Sie **Bazaar** und installieren Sie von [Flathub](https://flathub.org), wo fast jede
Linux-Desktop-App veröffentlicht wird. Flatpak-Apps aktualisieren sich zusammen mit dem System, und wenn
Sie eine entfernen, bleibt nichts zurück. In einem Terminal:

```bash
flatpak install flathub org.gimp.GIMP
flatpak list --app
flatpak uninstall --delete-data org.gimp.GIMP
```

Für Flatpaks gibt es zwei weitere Werkzeuge:

- **Flatseal** zeigt und ändert, worauf jede App zugreifen darf: Ordner, Geräte, das Netzwerk.
- **Warehouse** verwaltet installierte Apps und räumt auf, was sie zurücklassen.

### Was Flatpak-Apps nicht dürfen

Jedem Flatpak werden X11, `/dev/input` und der Flatpak-Dienst selbst verweigert, egal was sein eigenes
Manifest verlangt. Zusammen sind das die Wege, über die eine App mitlesen könnte, was Sie in eine andere
eingeben. Apps, die für Wayland geschrieben sind, merken davon nichts. Eine App, die nur X11 spricht
(Wine, manche Spiele, manche ältere Electron-Builds), braucht es zurückgewährt, eine App nach der
anderen, in Flatseal oder mit:

```bash
flatpak override --user --socket=x11 <app-id>
```

Worauf jede App über ihre Sandbox hinaus zugreifen kann, was sie verlangt hat und was Sie ihr gewährt
haben, steht auf der Seite **Apps** der Sicherheits-App und in einem Terminal mit `ame security apps`
([App-Berechtigungen](app-permissions.md)).

## Befehlszeilenwerkzeuge: Homebrew

```bash
brew install ripgrep fd bat
brew search <name>
```

Homebrew installiert in sein eigenes Präfix, ohne `sudo`, und wird zusammen mit dem System aktualisiert.
Die Seite über das [Terminal](terminal.md#command-line-tools) erklärt mehr.

## Überlagern: das letzte Mittel

`rpm-ostree install` fügt dem Abbild auf diesem Rechner ein Paket hinzu. Verwenden Sie es nur für das, was
nirgendwo anders leben kann, etwa einen Treiber oder einen Systemdienst:

```bash
sudo rpm-ostree install <package>
```

Starten Sie neu, um es zu verwenden. Jedes überlagerte Paket macht jede Aktualisierung langsamer, und das
Paket bleibt nur erhalten, solange Sie über `rpm-ostree` aktualisieren. Zwei Dinge sollten Sie wissen:

- **Abbilder oder Kanäle wechseln:** `bootc switch` baut das neue System allein aus dem Abbild und lässt
  überlagerte Pakete kommentarlos fallen. Auf einem Rechner mit überlagerten Paketen wechseln Sie
  stattdessen mit
  `sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/<image>:<stream>`.
  Automatische Aktualisierungen erledigen das für Sie.
- **Rückgängig machen:** `sudo rpm-ostree uninstall <package>` entfernt eines, und
  `sudo rpm-ostree reset` kehrt zum reinen Abbild zurück.

Eine `dnf`-Paketquelle hinzuzufügen oder `dnf install` auf dem Host auszuführen, funktioniert nicht: Das
System hat keine beschreibbare Paketdatenbank.

## VPN

- **Tailscale** ist installiert, und Sie können es ohne `sudo` ausführen: `tailscale up` tritt Ihrem
  Tailnet bei. Die Firewall vertraut dem Tailnet, in dem die eigenen Zugriffsregeln Ihres Tailnets gelten.
- **WireGuard**-Konfigurationen jedes Anbieters lassen sich unter **Einstellungen → Netzwerk → VPN**
  importieren.
