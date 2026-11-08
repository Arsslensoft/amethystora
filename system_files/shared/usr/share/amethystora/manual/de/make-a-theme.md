# Ein Thema ändern oder ein eigenes erstellen

Themen werden schreibgeschützt in `/usr/share/amethystora/themes` ausgeliefert. Ein gleichnamiger Ordner
in `~/.config/amethystora/themes` wird darübergelegt, sodass Sie nur die Dateien schreiben, die Sie ändern
möchten. Ein Ordner mit eigenem Namen ist ein neues Thema. Jede Datei ist klein und optional, außer der
Palette:

| Datei | Enthält |
| --- | --- |
| `colors.toml` | Die Palette: `accent`, `foreground`, `background`, `cursor`, `selection_foreground`, `selection_background` und `color0` bis `color15` |
| `light.mode` | Vorhanden und leer, wenn das Thema hell ist |
| `pair.theme` | Das Thema, zu dem `Super+Ctrl+D` wechselt |
| `accent.theme` | GNOMEs Akzentfarbe: blue, teal, green, yellow, orange, red, pink, purple oder slate. Ohne sie wird die nächstliegende gewählt |
| `gtk.theme` | Der Name eines GTK-Themas, statt GNOMEs eigenem Aussehen |
| `icons.theme` | Ein Symbolthema, statt candy-icons |
| `cursor.theme` | Ein Zeigerthema |
| `vscode.theme` | Das Farbthema von VSCodium, zu dem gewechselt wird |
| `tophat.theme` | Die Farbe der Anzeigen in der oberen Leiste, wenn die Akzentfarbe als dünne Linie schlecht lesbar ist |
| `backgrounds/` | Hintergrundbilder |
| `backgrounds.list` | Hintergrundbilder, die anderswo im System liegen, ein Pfad pro Zeile. Das erste ist das, mit dem das Thema beginnt |

Ein Thema ohne eigene Bilder bekommt vier, die in seinen Farben gezeichnet werden: ein sanftes Leuchten,
Hügel im Dunst, eine Höhenlinienkarte und fließende Bänder.

Ein eigenes Thema braucht drei Befehle:

```bash
mkdir -p ~/.config/amethystora/themes/sunset
cp /usr/share/amethystora/themes/amethystora/colors.toml ~/.config/amethystora/themes/sunset/
amethystora-theme set sunset
```

Bearbeiten Sie die Farben, führen Sie `amethystora-theme set sunset` erneut aus und sehen Sie sich das
Ergebnis an.

## Weiterführendes

- **Wie eine Konfiguration geschrieben wird.** Die Terminalpalette, der Prompt, die Farben von btop und
  die gezeichneten Hintergrundbilder sind Vorlagen in `/usr/share/amethystora/themed`. Kopieren Sie eine
  nach `~/.config/amethystora/themed/` und bearbeiten Sie die Kopie, um zu ändern, wie jedes Thema sie
  darstellt.
- **Bei jedem Wechsel etwas ausführen.** Legen Sie eine ausführbare Datei in
  `~/.config/amethystora/hooks/theme-set.d/`. Sie erhält den Namen des Themas in `AMETHYSTORA_THEME` und
  seinen Ordner in `AMETHYSTORA_THEME_DIR`.
- **Ihr eigener Prompt bleibt Ihrer.** Wenn Sie `~/.config/starship.toml` selbst geschrieben haben, lässt
  das Thema ihn in Ruhe.
- **Bearbeiten Sie nie `~/.config/amethystora/current/`.** Es wird bei jedem Wechsel neu geschrieben.

## Das Bootmenü

Auch das Bootmenü kann die Amethystora-Grafik tragen. Standardmäßig ist das ausgeschaltet, weil das
Bootmenü außerhalb des Abbilds liegt:

```bash
ame desktop boot-menu
```

Führen Sie den Befehl erneut aus, um die Grafik wieder zu entfernen. Sobald sie eingeschaltet ist, halten
Aktualisierungen des Abbilds sie auf dem neuesten Stand.
