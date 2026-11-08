# Spiele und Android-Apps

Keines von beiden ist von Anfang an eingerichtet, weil jedes etwas braucht, das das Abbild Apps
vorenthält, die es nicht brauchen. Ein Befehl richtet jeweils alles ein, sagt vorher, was er ändert, und
nimmt alles auch wieder zurück.

## Spiele

```bash
ame apps gaming
```

Das installiert **Steam** von Flathub, mit **ProtonPlus**, das weitere Versionen von Proton hinzufügt,
und zwei Werkzeugen, die die Spiele von Steam nutzen können: dem Overlay **MangoHud** und dem Compositor
**gamescope** von Valve. Außerdem bietet es **Heroic** für die Stores von Epic, GOG und Amazon an und
**Lutris** für Spiele von überall sonst.

Jedem Flatpak wird auf diesem Desktop X11 verweigert, weil eine App unter X11 mitlesen kann, was Sie in
andere X11-Apps eingeben ([was Flatpak-Apps nicht dürfen](software.md#what-flatpak-apps-may-not-do)).
Steam und die Windows-Spiele, die es über Proton ausführt, zeichnen nur über X11, deshalb wird X11 für
Steam und die Starter, die Sie hinzufügen, zurückgewährt, allein für Ihr Konto. Die Seite **Apps** der
Sicherheits-App führt das zusammen mit jeder anderen Berechtigung auf
([App-Berechtigungen](app-permissions.md)).

In den **Eigenschaften** eines Spiels in Steam, unter **Startoptionen**:

| Startoption | Funktion |
| --- | --- |
| `mangohud %command%` | Das Overlay: Bildrate, Frametimes, Temperaturen |
| `gamemoderun %command%` | Der Leistungsmodus des Prozessors, während das Spiel läuft, über GameMode |
| `gamescope -f -- %command%` | Das Spiel in seinem eigenen Compositor, im Vollbild, was bei Spielen hilft, die Wayland nicht mögen |

Gamecontroller funktionieren ohne Einrichtung: Controller von Xbox, PlayStation, Nintendo und Steam,
8BitDo und die übrigen, die Steam kennt. **Add another launcher** fügt Heroic oder Lutris später hinzu,
und **Take it all away** deinstalliert alles, samt Spielen, und nimmt zurück, was gewährt wurde:

```bash
ame apps gaming launchers
ame apps gaming off
```

## Ein schnellerer CPU-Scheduler für Spiele

Der Kernel entscheidet, welches Programm auf welchem Prozessorkern wie lange läuft. Sein eigener
Scheduler passt für fast alles. Ein sched-ext-Scheduler ist ein kleines Programm, dem der Kernel diese
Aufgabe übergibt, und einer, der für eine bestimmte Art von Arbeit gemacht ist, kann sie besser erledigen:

```bash
ame system scheduler
```

| Scheduler | Für |
| --- | --- |
| `kernel` | Alles: der eigene des Kernels und die Vorgabe |
| `lavd` | Spiele, flüssig, während andere Arbeit läuft: gemacht für Gaming-Handhelds und Laptops |
| `bpfland` | Einen Desktop, der reaktionsschnell bleibt, während etwas Schweres gebaut oder gerendert wird |
| `flash` | Gleichmäßiges Timing, für Audio- und Musikarbeit |

Die Wahl bleibt nach einem Neustart erhalten, und `ame system scheduler kernel` stellt den eigenen
Scheduler des Kernels wieder her. Wenn ein sched-ext-Scheduler anhält, übernimmt der Kernel von selbst
wieder, sodass einer im schlimmsten Fall die Dinge langsamer macht. Ein Kernel, der vor dwarves 1.32 von
Fedora gebaut wurde, kann die aktuellen Scheduler nicht laden; der Befehl sagt das und lässt den eigenen
des Kernels an seinem Platz.

## Android-Apps

```bash
ame apps android
```

**Waydroid** führt Android, LineageOS 20, in einem Container neben Ihrem Desktop aus, und die
Android-Apps, die Sie installieren, erscheinen wie jede andere in der Anwendungsübersicht. Bei der
Einrichtung wird Android von den Servern von Waydroid heruntergeladen, etwa ein Gigabyte. Es kommt ohne
die Apps von Google und ohne den Play Store, deren Apps Googles eigenen Bedingungen unterliegen: Holen Sie
Apps von F-Droid oder installieren Sie eine `.apk`, die Sie heruntergeladen haben:

```bash
waydroid app install ~/Downloads/some-app.apk
```

Öffnen Sie **Waydroid** in der Anwendungsübersicht; der erste Start dauert eine Minute. Sein
Containerdienst läuft als root, deshalb ist er erst eingeschaltet, wenn Sie ihn einrichten, und Sie können
ihn beenden, wenn Sie ihn nicht nutzen:

| Befehl | Funktion |
| --- | --- |
| `ame apps android start` | Android starten und öffnen |
| `ame apps android stop` | Es beenden, bis Sie es wieder starten |
| `ame apps android off` | Android entfernen, mit jeder App und allem, was darin gespeichert ist |

Waydroid kann mit dem Treiber von NVIDIA nicht zeichnen, deshalb zeichnet Android auf den NVIDIA-Abbildern
in Software: gut genug für die meisten Apps, langsam für Spiele.
