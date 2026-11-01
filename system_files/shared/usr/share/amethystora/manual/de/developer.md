# Entwicklermodus

`amethystora-dx` ist derselbe Desktop mit einer Entwicklerwerkstatt obendrauf: Container-Engines,
virtuelle Maschinen, VSCodium und die Profiling-Werkzeuge, um herauszufinden, warum etwas langsam ist.

Die Idee dahinter: Ihre Entwicklungsumgebung sollte nicht Ihr Betriebssystem sein. Toolchains,
Sprachlaufzeiten und Datenbanken leben in Containern, die im eigenen Repository des Projekts beschrieben
sind, sodass das Projekt auf Ihrem Rechner, auf dem Mac eines Kollegen und in der CI gleich gebaut wird,
und das System darunter bleibt sauber genug, um es ohne Bedenken zu aktualisieren.

## Einschalten

Wechseln Sie zum dx-Abbild und starten Sie neu:

```bash
sudo bootc switch --enforce-container-sigpolicy ghcr.io/arsslensoft/amethystora-dx:stable
```

Wenn Sie [überlagerte Pakete](software.md#layering-the-last-resort) haben, verwenden Sie stattdessen
`sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/amethystora-dx:stable`, was sie
behält. Der Wechsel zurück zu `amethystora` funktioniert auf dieselbe Weise.

Beim ersten Start fügt das dx-Abbild jedes Administratorkonto den Gruppen `docker`, `incus-admin`,
`libvirt` und `wireshark` hinzu. Wenn `docker ps` „permission denied“ meldet, melden Sie sich einmal ab
und wieder an.

## Was es hinzufügt

| Werkzeug | Für |
| --- | --- |
| VSCodium | Den Editor: VS Code, aus seinem quelloffenen Code gebaut, ohne Microsofts Telemetrie oder Lizenz. Erweiterungen kommen von [Open VSX](https://open-vsx.org); Open Remote SSH und Container Tools sind installiert. Seine Farben folgen dem [Thema](themes.md). |
| Docker Engine | Mit buildx und compose. Die Vorgabe für Dev-Container. |
| Podman | Mit `podman-compose` und `podman machine`. Immer vorhanden, ohne root-Rechte, und der Podman-Socket ist aktiv. |
| Incus | System-Container und virtuelle Maschinen, verwaltet wie Cloud-Instanzen |
| libvirt und virt-manager | Virtuelle Maschinen mit QEMU und KVM |
| Sysprof, perf, bcc, bpftrace, bpftop, bpftool, trace-cmd | Profiling und Tracing, vom ganzen System bis hinunter zu einer einzelnen Funktion |
| gdb, strace, ltrace, Valgrind | Fehlersuche: ein Programm schrittweise durchgehen, die Aufrufe beobachten, die es an den Kernel und an seine Bibliotheken richtet, Speicherfehler finden. gdb an ein Programm anzuhängen, das es nicht selbst gestartet hat, erfordert `sudo` ([warum](security.md#on-from-the-start)). |
| Wireshark | Netzwerkverkehr mitschneiden und lesen, ohne `sudo` |
| android-tools | `adb` und `fastboot` |
| ROCm | GPU-Berechnungen auf AMD-Grafik |
| flatpak-builder | Flatpaks bauen |

## Dev-Container

Ein Dev-Container ist eine `.devcontainer/devcontainer.json` im Projekt: das Abbild, die Werkzeuge und
die Editor-Erweiterungen, die das Projekt braucht. Alles, was das Projekt installiert, bleibt darin.

Microsofts Erweiterung Dev Containers funktioniert nur in Microsofts eigenem VS Code, deshalb laufen
Dev-Container unter Amethystora über die Befehlszeile `devcontainer`, die Referenzimplementierung
[der Spezifikation](https://containers.dev), der auch die JetBrains-IDEs folgen:

```bash
brew install devcontainer
devcontainer up --workspace-folder .
devcontainer exec --workspace-folder . bash
```

Um Podman statt Docker zu verwenden, fügen Sie `--docker-path podman --docker-compose-path podman-compose`
zu `devcontainer up` hinzu.

Wenn ein Container einen eingehängten Ordner wegen SELinux nicht lesen kann, versehen Sie den Ordner neu
mit Labels, statt SELinux auszuschalten: `restorecon -R -v ~/code/myproject`.

## Andere Editoren

- **JetBrains:** `ame apps jetbrains-toolbox` installiert die JetBrains Toolbox in Ihren persönlichen
  Ordner, die dann die IDEs installiert und aktualisiert. Die Flatpaks der IDEs werden nicht empfohlen.
- **Neovim, Helix und Co.:** `brew install neovim`, und `brew install devcontainer` für die
  Dev-Container-Befehlszeile.

## Kubernetes und die Cloud

Installieren Sie die Befehlszeilenwerkzeuge mit Homebrew, damit sie aktuell bleiben, ohne das Abbild
anzurühren:

```bash
brew install kubectl helm k9s kind
```

`kind` führt einen ganzen Kubernetes-Cluster in Docker-Containern aus, was der schnellste Weg ist, etwas
gegen einen echten Cluster auszuprobieren.
